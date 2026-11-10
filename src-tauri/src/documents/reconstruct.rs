//! Optional English readings. Exact decoded artifacts are never replaced.
use super::{
    Control,
    jobs::{Artifact, Direction, DocumentStore, Job},
    storage::{self, Preview},
    stream::{ArtifactWriter, ScalarReader, add, io_error},
    transform,
};
use crate::{
    engine::recovery::{WordSuggestion, document},
    recipes::{Rule, fingerprint},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use tempfile::TempDir;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocChoice {
    pub word_id: String,
    pub candidate: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingWord {
    pub word_id: String,
    pub transformed: String,
    pub suggested: String,
    pub alternatives: Vec<String>,
    pub compatible_count: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingInfo {
    pub id: String,
    pub job_id: String,
    pub words: String,
    pub note: String,
}
pub(super) struct Reading {
    pub _root: TempDir,
    pub info: ReadingInfo,
    pub artifact: Artifact,
    pub candidates: PathBuf,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePage {
    pub words: Vec<ReadingWord>,
    pub next_offset: String,
    pub eof: bool,
}

struct Admission(Arc<Job>);
impl Drop for Admission {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.reconstructing = false;
        }
    }
}
struct Builder {
    text: ArtifactWriter,
    candidates: BufWriter<File>,
    words: u64,
    job_id: String,
}
impl Builder {
    fn word(&mut self, suggestion: WordSuggestion, rendered: &str) -> Result<(), String> {
        self.text.text(rendered)?;
        let word = ReadingWord {
            word_id: format!("{}:{}", self.job_id, self.words),
            transformed: suggestion.transformed,
            suggested: suggestion.suggested,
            alternatives: suggestion.alternatives,
            compatible_count: suggestion.compatible_count,
        };
        serde_json::to_writer(&mut self.candidates, &word)
            .map_err(|_| "Cannot write reconstruction candidates.")?;
        self.candidates
            .write_all(b"\n")
            .map_err(|e| io_error("Cannot finish candidate record", e))?;
        self.words = add(self.words, 1)?;
        Ok(())
    }
}

fn pins(
    job_id: &str,
    choices: &[DocChoice],
    scalar_count: u64,
) -> Result<HashMap<u64, String>, String> {
    let prefix = format!("{job_id}:");
    let mut pins = HashMap::new();
    for choice in choices {
        let index: u64 = choice
            .word_id
            .strip_prefix(&prefix)
            .ok_or("A word selection belongs to a different document job.")?
            .parse()
            .map_err(|_| "Invalid document word identifier.")?;
        if index >= scalar_count
            || choice.candidate.is_empty()
            || choice.candidate.len() > crate::engine::document_word_limit()
            || !choice.candidate.bytes().all(|c| c.is_ascii_lowercase())
        {
            return Err(
                "A selected candidate must be a dictionary word at a valid document position."
                    .into(),
            );
        }
        if pins.insert(index, choice.candidate.clone()).is_some() {
            return Err("A word position may have only one selected candidate.".into());
        }
    }
    Ok(pins)
}

pub fn run(
    store: &DocumentStore,
    job_id: &str,
    choices: &[DocChoice],
    control: &Control,
) -> Result<ReadingInfo, String> {
    control.check()?;
    let job = store.job(job_id)?;
    let source = job.result()?;
    let info = store.status(job_id)?;
    if !matches!(info.direction, Direction::Decode) || info.exact {
        return Err("English reconstruction requires a completed lossy decode.".into());
    }
    let enabled: Vec<_> = info
        .recipe
        .steps
        .iter()
        .filter(|s| s.enabled)
        .map(|s| &s.rule)
        .collect();
    let phonetic = match enabled.as_slice() {
        [Rule::PhoneticRotate, suffix @ ..] if suffix.iter().all(|r| !r.lossy()) => true,
        [Rule::RemoveVowels, Rule::CollapseDoubles, suffix @ ..] if suffix.iter().all(|r| !r.lossy()) => false,
        _ => return Err("This lossy rule combination has no compatible English reconstruction strategy. The exact surviving text remains available.".into()),
    };
    let pins = pins(job_id, choices, source.stats.scalars)?;
    {
        let mut state = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?;
        if state.reconstructing {
            return Err("English reconstruction is already running for this job.".into());
        }
        state.reconstructing = true;
    }
    let _admission = Admission(job.clone());
    let root = tempfile::Builder::new()
        .prefix("reading-")
        .tempdir_in(job.root.path())
        .map_err(|e| io_error("Cannot create private reading artifacts", e))?;
    let id = root
        .path()
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Cannot identify English reading.")?
        .to_owned();
    let output = root.path().join("english");
    let candidates = root.path().join("candidates.jsonl");
    let file =
        File::create(&candidates).map_err(|e| io_error("Cannot create candidate artifact", e))?;
    let mut builder = Builder {
        text: ArtifactWriter::create(&output)?,
        candidates: BufWriter::new(file),
        words: 0,
        job_id: job_id.into(),
    };
    control.begin("Searching for an English reading", Some(source.stats.bytes))?;
    if phonetic {
        document::search(
            &source.path,
            root.path(),
            &job.key,
            &pins,
            control,
            |_, word| {
                if builder.words > 0 {
                    builder.text.push(' ')?;
                }
                let rendered = word.suggested.clone();
                builder.word(word, &rendered)
            },
        )?;
    } else {
        skeleton(&source.path, &pins, &mut builder, control)?;
    }
    if pins.keys().any(|index| *index >= builder.words) {
        return Err("A selected word is outside the document.".into());
    }
    if builder.words == 0 {
        return Err("English reconstruction requires at least one word.".into());
    }
    builder
        .candidates
        .flush()
        .map_err(|e| io_error("Cannot finish candidate artifact", e))?;
    let words = builder.words;
    let stats = builder.text.finish()?;
    let artifact = Artifact {
        path: output,
        stats,
        label: "Speculative English reading".into(),
        lossy: true,
    };
    control.begin("Re-encoding complete reading", None)?;
    verify(&job, &info.recipe, &artifact, root.path(), control)?;
    control.check()?;
    let info = ReadingInfo { id, job_id: job_id.into(), words: words.to_string(), note: "Speculative dictionary reading, verified by re-encoding the complete document. This does not establish original wording, spelling or formatting.".into() };
    let reading = Arc::new(Reading {
        _root: root,
        info: info.clone(),
        artifact,
        candidates,
    });
    job.state
        .lock()
        .map_err(|_| "Document job state is unavailable.")?
        .reading = Some(reading);
    Ok(info)
}

fn signature(text: &str) -> String {
    let mut out = String::new();
    let mut previous = '\0';
    for scalar in text.chars().map(|c| c.to_ascii_lowercase()) {
        if "aeiou".contains(scalar) {
            continue;
        }
        if scalar != previous {
            out.push(scalar);
        }
        previous = scalar;
    }
    out
}
fn vocabulary() -> &'static HashMap<String, Vec<&'static str>> {
    static VOCABULARY: OnceLock<HashMap<String, Vec<&'static str>>> = OnceLock::new();
    VOCABULARY.get_or_init(|| {
        let mut words: HashMap<String, Vec<&str>> = HashMap::new();
        for word in include_str!("../../data/english-frequency.txt")
            .lines()
            .filter(|w| w.bytes().all(|c| c.is_ascii_lowercase()))
        {
            words.entry(signature(word)).or_default().push(word);
        }
        words
    })
}
fn skeleton_case(candidate: &str, original: &str) -> Result<String, String> {
    let mut original = original.chars();
    let mut last = '\0';
    let mut mapped = '\0';
    let mut output = String::new();
    for scalar in candidate.chars() {
        if "aeiou".contains(scalar) {
            output.push(scalar);
            continue;
        }
        if scalar != last {
            let source = original
                .next()
                .ok_or("Candidate skeleton is longer than its source.")?;
            if source.to_ascii_lowercase() != scalar {
                return Err("Candidate does not match the surviving consonants.".into());
            }
            mapped = if source.is_ascii_uppercase() {
                scalar.to_ascii_uppercase()
            } else {
                scalar
            };
            last = scalar;
        }
        output.push(mapped);
    }
    if original.next().is_some() {
        return Err("Candidate skeleton is shorter than its source.".into());
    }
    Ok(output)
}
fn skeleton(
    source: &Path,
    pins: &HashMap<u64, String>,
    builder: &mut Builder,
    control: &Control,
) -> Result<(), String> {
    let vocabulary = vocabulary();
    let limit = vocabulary.keys().map(String::len).max().unwrap_or(0);
    let mut input = ScalarReader::open(source)?;
    let mut token = String::new();
    loop {
        let scalar = input.next()?;
        control.observe(input.bytes)?;
        if let Some(c) = scalar.filter(char::is_ascii_alphabetic) {
            if token.len() >= limit {
                return Err("A skeleton word is outside the ranked dictionary vocabulary.".into());
            }
            token.push(c);
            continue;
        }
        if scalar.is_some_and(|c| !c.is_ascii() && c.is_alphabetic()) {
            return Err("English reconstruction does not support non-English alphabetic input. The exact Unicode intermediate remains available.".into());
        }
        if !token.is_empty() {
            let candidates = vocabulary.get(&token.to_ascii_lowercase()).ok_or_else(|| format!("No ranked English candidate matches word {}. The exact intermediate remains available.", builder.words + 1))?;
            let selected = pins
                .get(&builder.words)
                .map(String::as_str)
                .unwrap_or(candidates[0]);
            if !candidates.contains(&selected) {
                return Err("A selected word does not match the surviving consonants.".into());
            }
            let rendered = skeleton_case(selected, &token)?;
            builder.word(
                WordSuggestion {
                    transformed: token.clone(),
                    suggested: selected.into(),
                    alternatives: candidates
                        .iter()
                        .filter(|w| **w != selected)
                        .take(5)
                        .map(|w| (*w).to_owned())
                        .collect(),
                    compatible_count: candidates.len(),
                },
                &rendered,
            )?;
            token.clear();
        }
        if let Some(scalar) = scalar {
            builder.text.push(scalar)?;
        } else {
            break;
        }
    }
    Ok(())
}
fn verify(
    job: &Job,
    recipe: &crate::recipes::Recipe,
    reading: &Artifact,
    root: &Path,
    control: &Control,
) -> Result<(), String> {
    let scratch = tempfile::Builder::new()
        .prefix("verification-")
        .tempdir_in(root)
        .map_err(|e| io_error("Cannot create reading verification artifacts", e))?;
    let mut current = reading.clone();
    for (index, step) in recipe.steps.iter().enumerate().filter(|(_, s)| s.enabled) {
        control.check()?;
        let resolved = fingerprint::resolve(recipe, index, &step.rule, &job.key)?;
        let actual = resolved.as_ref().map_or(&step.rule, |(rule, _)| rule);
        let path = scratch.path().join(format!("stage-{index}"));
        let stats = transform::transform_known(
            actual,
            &current.path,
            &path,
            &job.key,
            false,
            control,
            Some(current.stats.scalars),
        )?;
        current = Artifact {
            path,
            stats,
            label: String::new(),
            lossy: false,
        };
    }
    if current.stats.bytes != job.source.stats.bytes
        || current.stats.digest != job.source.stats.digest
    {
        return Err("The suggested reading does not re-encode to the complete captured ciphertext. The exact intermediate remains available.".into());
    }
    Ok(())
}

pub(super) fn current(
    store: &DocumentStore,
    job_id: &str,
    reading_id: &str,
) -> Result<Arc<Reading>, String> {
    let job = store.job(job_id)?;
    let state = job
        .state
        .lock()
        .map_err(|_| "Document job state is unavailable.")?;
    state
        .reading
        .as_ref()
        .filter(|r| r.info.id == reading_id)
        .cloned()
        .ok_or_else(|| "This English reading is no longer active for the document job.".into())
}
pub fn preview(
    store: &DocumentStore,
    job_id: &str,
    reading_id: &str,
    offset: u64,
) -> Result<Preview, String> {
    let reading = current(store, job_id, reading_id)?;
    storage::preview(&reading.artifact.path, offset)
}
pub fn page(
    store: &DocumentStore,
    job_id: &str,
    reading_id: &str,
    offset: u64,
) -> Result<CandidatePage, String> {
    let reading = current(store, job_id, reading_id)?;
    let mut file = File::open(&reading.candidates)
        .map_err(|e| io_error("Cannot open reading candidates", e))?;
    let length = file
        .metadata()
        .map_err(|e| io_error("Cannot inspect candidate artifact", e))?
        .len();
    if offset > length {
        return Err("Candidate page offset is outside the artifact.".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| io_error("Cannot seek candidates", e))?;
    let mut input = BufReader::new(file);
    let mut words = Vec::new();
    let mut next = offset;
    for _ in 0..32 {
        let mut bytes = Vec::new();
        let read = (&mut input)
            .take(65537)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| io_error("Cannot read candidate page", e))?;
        if read == 0 {
            break;
        }
        if read > 65536 {
            return Err("Candidate record exceeds dictionary metadata bounds.".into());
        }
        words.push(serde_json::from_slice(&bytes).map_err(|_| "Invalid candidate record.")?);
        next = add(
            next,
            u64::try_from(read).map_err(|_| "Candidate offset overflow.")?,
        )?;
    }
    Ok(CandidatePage {
        words,
        next_offset: next.to_string(),
        eof: next == length,
    })
}
