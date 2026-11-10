//! Global phonetic search with bounded beams and disk-backed parent history.
use super::*;
use crate::documents::{
    Control,
    stream::{ScalarReader, add, io_error},
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path as FsPath,
};
use tempfile::NamedTempFile;

type Possible = (&'static str, &'static [&'static str]);
#[derive(Serialize, Deserialize)]
struct Node {
    parent: Option<u64>,
    index: u64,
    word: WordSuggestion,
}
struct Beam {
    state: RotationState,
    score: f64,
    node: Option<u64>,
}
struct Next {
    state: RotationState,
    score: f64,
    node: Node,
}

fn normalized(mut state: RotationState) -> RotationState {
    for (index, family) in FAMILY_PHONES.iter().enumerate() {
        if let Some(variants) = choices(family) {
            state.family_counts[index] %= variants.len();
        }
    }
    state.vowel_count %= VOWEL_OUTPUTS.len();
    state
}
fn token_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        dictionary()
            .values()
            .map(|phones| {
                phones
                    .iter()
                    .map(|raw| {
                        let sound = phone(raw);
                        if is_vowel(sound) {
                            1
                        } else {
                            choices(sound).map_or(0, |variants| {
                                variants.iter().map(|s| s.len()).max().unwrap_or(0)
                            })
                        }
                    })
                    .sum()
            })
            .max()
            .unwrap_or(0)
    })
}
fn append(file: &mut File, node: &Node) -> Result<u64, String> {
    let bytes = serde_json::to_vec(node).map_err(|_| "Cannot serialize reconstruction history.")?;
    let length = u32::try_from(bytes.len()).map_err(|_| "Reconstruction record is too large.")?;
    if length > 65536 {
        return Err("Reconstruction record exceeds dictionary metadata bounds.".into());
    }
    let at = file
        .stream_position()
        .map_err(|e| io_error("Cannot locate reconstruction history", e))?;
    file.write_all(&length.to_le_bytes())
        .and_then(|_| file.write_all(&bytes))
        .and_then(|_| file.write_all(&length.to_le_bytes()))
        .map_err(|e| io_error("Cannot write reconstruction history", e))?;
    Ok(at)
}
fn read_node(file: &mut File, at: u64) -> Result<Node, String> {
    file.seek(SeekFrom::Start(at))
        .map_err(|e| io_error("Cannot seek reconstruction history", e))?;
    let mut length = [0; 4];
    file.read_exact(&mut length)
        .map_err(|e| io_error("Cannot read reconstruction record", e))?;
    let length = u32::from_le_bytes(length);
    if length > 65536 {
        return Err("Invalid reconstruction history record length.".into());
    }
    let mut bytes =
        vec![0; usize::try_from(length).map_err(|_| "Reconstruction record size overflow.")?];
    file.read_exact(&mut bytes)
        .map_err(|e| io_error("Incomplete reconstruction history", e))?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid reconstruction history record.".into())
}
fn prefer(a: &Next, b: &Next) -> bool {
    a.score > b.score
        || (a.score == b.score
            && (&a.node.word.suggested, a.node.parent) < (&b.node.word.suggested, b.node.parent))
}

/// Emit a single consistent global path in ascending word order. The history
/// grows on disk; neither an in-memory sentence nor a per-word path vector does.
pub(crate) fn search(
    source: &FsPath,
    root: &FsPath,
    key: &str,
    pins: &HashMap<u64, String>,
    control: &Control,
    mut sink: impl FnMut(u64, WordSuggestion) -> Result<(), String>,
) -> Result<u64, String> {
    control.check()?;
    let mut reader = ScalarReader::open(source)?;
    let mut history = NamedTempFile::new_in(root)
        .map_err(|e| io_error("Cannot create reconstruction history", e))?;
    let mut beams = vec![Beam {
        state: RotationState::default(),
        score: 0.0,
        node: None,
    }];
    // FIFO bounded cache: eviction changes work, never accepted input or results.
    let mut cache: HashMap<String, Vec<Possible>> = HashMap::new();
    let mut order = std::collections::VecDeque::new();
    let mut count = 0;
    loop {
        control.observe(reader.bytes)?;
        let mut token = String::new();
        loop {
            let next = reader.next()?;
            match next {
                Some(c) if c.is_ascii_lowercase() => {
                    if token.len() >= token_limit() { return Err("A transformed word is outside the pronunciation dictionary's output vocabulary.".into()); }
                    token.push(c);
                }
                Some(c) if c.is_ascii_whitespace() => { if !token.is_empty() { break; } }
                None => break,
                _ => return Err("The phonetic intermediate must contain lowercase English letters and whitespace.".into()),
            }
        }
        if token.is_empty() {
            break;
        }
        if !cache.contains_key(&token) {
            let mut possible = Vec::new();
            for (index, (word, phones)) in dictionary().iter().enumerate() {
                if index % 1024 == 0 {
                    control.check()?;
                }
                if word.bytes().all(|c| c.is_ascii_lowercase())
                    && potentially_compatible(phones, &token)
                {
                    possible.push((word.as_str(), phones.as_slice()));
                }
            }
            if order.len() == 128
                && let Some(old) = order.pop_front()
            {
                cache.remove(&old);
            }
            order.push_back(token.clone());
            cache.insert(token.clone(), possible);
        }
        let possible = cache.get(&token).ok_or("Missing dictionary candidates.")?;
        let mut next: HashMap<RotationState, Next> = HashMap::new();
        for beam in &beams {
            control.check()?;
            let matches = ordered_matches(possible, &token, beam.state, key);
            for candidate in matches
                .iter()
                .filter(|m| pins.get(&count).is_none_or(|p| p == m.word))
                .take(BEAM_WIDTH)
            {
                let state = normalized(candidate.state);
                let value = Next {
                    state,
                    score: beam.score + candidate.score,
                    node: Node {
                        parent: beam.node,
                        index: count,
                        word: WordSuggestion {
                            transformed: token.clone(),
                            suggested: candidate.word.to_owned(),
                            alternatives: matches
                                .iter()
                                .take(MAX_ALTERNATIVES)
                                .filter(|m| m.word != candidate.word)
                                .map(|m| m.word.to_owned())
                                .collect(),
                            compatible_count: matches.len(),
                        },
                    },
                };
                match next.entry(state) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(value);
                    }
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        if prefer(&value, entry.get()) {
                            entry.insert(value);
                        }
                    }
                }
            }
        }
        if next.is_empty() {
            return Err(format!(
                "The bounded dictionary search found no complete reading at word {} under the selected constraints. The exact intermediate remains available.",
                add(count, 1)?
            ));
        }
        let mut ranked: Vec<_> = next.into_values().collect();
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.state.family_counts.cmp(&b.state.family_counts))
                .then_with(|| a.state.vowel_count.cmp(&b.state.vowel_count))
        });
        ranked.truncate(BEAM_WIDTH);
        beams.clear();
        for value in ranked {
            let node = append(history.as_file_mut(), &value.node)?;
            beams.push(Beam {
                state: value.state,
                score: value.score,
                node: Some(node),
            });
        }
        count = add(count, 1)?;
    }
    if count == 0 {
        return Err("English reconstruction requires at least one word.".into());
    }
    if pins.keys().any(|index| *index >= count) {
        return Err("A selected word is outside the document.".into());
    }
    control.begin("Tracing complete English reading", None)?;
    let mut reversed = NamedTempFile::new_in(root)
        .map_err(|e| io_error("Cannot create reconstruction traceback", e))?;
    let mut at = beams.first().and_then(|b| b.node);
    let mut remaining = count;
    while let Some(position) = at {
        control.check()?;
        let node = read_node(history.as_file_mut(), position)?;
        remaining = remaining
            .checked_sub(1)
            .ok_or("Reconstruction history contains too many words.")?;
        if node.index != remaining || node.parent.is_some_and(|parent| parent >= position) {
            return Err("Reconstruction history ordering is invalid.".into());
        }
        at = node.parent;
        append(reversed.as_file_mut(), &node)?;
    }
    if remaining != 0 {
        return Err("Reconstruction history is incomplete.".into());
    }
    let mut end = reversed
        .as_file_mut()
        .stream_position()
        .map_err(|e| io_error("Cannot locate reconstruction traceback", e))?;
    while end > 0 {
        control.check()?;
        let footer = end.checked_sub(4).ok_or("Invalid traceback footer.")?;
        reversed
            .seek(SeekFrom::Start(footer))
            .map_err(|e| io_error("Cannot seek traceback footer", e))?;
        let mut length = [0; 4];
        reversed
            .read_exact(&mut length)
            .map_err(|e| io_error("Cannot read traceback footer", e))?;
        let start = footer
            .checked_sub(u64::from(u32::from_le_bytes(length)))
            .and_then(|p| p.checked_sub(4))
            .ok_or("Invalid traceback length.")?;
        let node = read_node(reversed.as_file_mut(), start)?;
        sink(node.index, node.word)?;
        end = start;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_during_traceback_never_returns_a_partial_success() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("intermediate");
        let text = "e i o u a ".repeat(20);
        std::fs::write(&source, text.trim_end()).unwrap();
        let control = Control::default();
        let mut emitted = 0;
        let result = search(
            &source,
            root.path(),
            "lemon",
            &HashMap::new(),
            &control,
            |_, _| {
                emitted += 1;
                control.cancel();
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(emitted, 1);
        assert_eq!(std::fs::read_to_string(source).unwrap(), text.trim_end());
    }
}
