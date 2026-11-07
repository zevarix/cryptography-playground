//! Speculative reconstruction of English from Recipe 001's lossy phonetic stream.
//!
//! A candidate must reproduce the transformed word *exactly* when its dictionary
//! pronunciation is replayed with the supplied key and preceding rotation state.
//! Ranking those valid candidates by English frequency does not establish that
//! any one candidate was the original word.
use super::{VOWEL_OUTPUTS, choices, dictionary, initial, is_vowel, phone};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

const FAMILY_PHONES: [&str; 24] = [
    "B", "CH", "D", "DH", "F", "G", "HH", "JH", "K", "L", "M", "N", "NG", "P", "R", "S", "SH", "T",
    "TH", "V", "W", "Y", "Z", "ZH",
];
const BEAM_WIDTH: usize = 18;
const MAX_RECONSTRUCTION_WORDS: usize = 40;
const MAX_TRANSFORMED_WORD_LENGTH: usize = 80;
const MAX_ALTERNATIVES: usize = 5;
static WORD_RANKS: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordSuggestion {
    pub transformed: String,
    pub suggested: String,
    pub alternatives: Vec<String>,
    pub compatible_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconstruction {
    pub suggested: Option<String>,
    pub words: Vec<WordSuggestion>,
    pub note: String,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct RotationState {
    family_counts: [usize; FAMILY_PHONES.len()],
    vowel_count: usize,
}

impl Default for RotationState {
    fn default() -> Self {
        Self {
            family_counts: [0; FAMILY_PHONES.len()],
            vowel_count: 0,
        }
    }
}

#[derive(Clone)]
struct Path {
    state: RotationState,
    score: f64,
    words: Vec<WordSuggestion>,
}

#[derive(Clone)]
struct Match<'a> {
    word: &'a str,
    state: RotationState,
    score: f64,
}

fn frequency_ranks() -> &'static HashMap<&'static str, usize> {
    WORD_RANKS.get_or_init(|| {
        include_str!("../../data/english-frequency.txt")
            .lines()
            .enumerate()
            .map(|(index, word)| (word, index + 1))
            .collect()
    })
}

fn family_index(name: &str) -> Option<usize> {
    FAMILY_PHONES
        .iter()
        .position(|candidate| *candidate == name)
}

/// Check whether a phoneme sequence might produce this string under *some*
/// rotation phase. This gives us a small set of candidate dictionary entries
/// before testing the actual global rotation counters.
fn potentially_compatible(pronunciation: &[&str], transformed: &str) -> bool {
    let target = transformed.as_bytes();
    let mut positions = vec![0];
    let mut previous_was_vowel = false;

    for raw in pronunciation {
        let sound = phone(raw);
        if is_vowel(sound) {
            if previous_was_vowel {
                continue;
            }
            previous_was_vowel = true;
            positions = positions
                .into_iter()
                .filter(|&pos| target.get(pos).is_some_and(u8::is_ascii_lowercase))
                .map(|pos| pos + 1)
                .filter(|&pos| VOWEL_OUTPUTS.contains(&target[pos - 1]))
                .collect();
        } else {
            previous_was_vowel = false;
            let Some(variants) = choices(sound) else {
                return false;
            };
            positions = positions
                .into_iter()
                .flat_map(|pos| {
                    variants.iter().filter_map(move |variant| {
                        target
                            .get(pos..)
                            .is_some_and(|remaining| remaining.starts_with(variant.as_bytes()))
                            .then_some(pos + variant.len())
                    })
                })
                .collect();
        }
        if positions.is_empty() {
            return false;
        }
        positions.sort_unstable();
        positions.dedup();
    }
    positions.contains(&target.len())
}

/// Replay every candidate phoneme with the *actual* prior global counters.
/// Returned states are valid only if the whole transformed word matches.
fn exact_match(
    pronunciation: &[&str],
    transformed: &str,
    start: RotationState,
    key: &str,
) -> Option<RotationState> {
    let mut state = start;
    let mut at = 0;
    let target = transformed.as_bytes();
    let start_vowel = usize::from(key.as_bytes()[0] - b'a') % VOWEL_OUTPUTS.len();
    let mut previous_was_vowel = false;

    for raw in pronunciation {
        let sound = phone(raw);
        if is_vowel(sound) {
            if previous_was_vowel {
                continue;
            }
            previous_was_vowel = true;
            let wanted = VOWEL_OUTPUTS[(start_vowel + state.vowel_count) % VOWEL_OUTPUTS.len()];
            if target.get(at) != Some(&wanted) {
                return None;
            }
            state.vowel_count += 1;
            at += 1;
        } else {
            previous_was_vowel = false;
            let variants = choices(sound)?;
            let index = family_index(sound)?;
            let choice =
                (initial(key, sound, variants.len()) + state.family_counts[index]) % variants.len();
            let spelling = variants[choice].as_bytes();
            if !target.get(at..)?.starts_with(spelling) {
                return None;
            }
            state.family_counts[index] += 1;
            at += spelling.len();
        }
    }
    (at == target.len()).then_some(state)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.bytes().enumerate() {
        let mut next = vec![i + 1];
        for (j, cb) in b.bytes().enumerate() {
            next.push(
                (prev[j + 1] + 1)
                    .min(next[j] + 1)
                    .min(prev[j] + usize::from(ca != cb)),
            );
        }
        prev = next;
    }
    prev[b.len()]
}

fn word_score(candidate: &str, transformed: &str) -> f64 {
    // The ranking is a heuristic prior, *not* a statement of recovery confidence.
    // A small spelling-similarity tie-breaker prefers familiar original spellings.
    let freq_rank = frequency_ranks().get(candidate).copied().unwrap_or(250_000);
    (20_001.0 / freq_rank as f64).ln() - 0.16 * edit_distance(candidate, transformed) as f64
}

fn ordered_matches<'a>(
    candidates: &[(&'a str, &'a [&'static str])],
    transformed: &str,
    state: RotationState,
    key: &str,
) -> Vec<Match<'a>> {
    let mut matches: Vec<_> = candidates
        .iter()
        .filter_map(|(word, phones)| {
            exact_match(phones, transformed, state, key).map(|next| Match {
                word,
                state: next,
                score: word_score(word, transformed),
            })
        })
        .collect();
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.word.cmp(b.word))
    });
    matches
}

/// The exact intermediate is always produced by the cipher decoder first.
/// This supplementary search only works for common, bounded dictionary input.
pub fn reconstruct(intermediate: &str, key: &str) -> Reconstruction {
    let tokens: Vec<&str> = intermediate.split_whitespace().collect();
    if tokens.len() > MAX_RECONSTRUCTION_WORDS
        || tokens
            .iter()
            .any(|word| word.len() > MAX_TRANSFORMED_WORD_LENGTH)
    {
        return Reconstruction {
            suggested: None, words: Vec::new(),
            note: "Message is too large for bounded English reconstruction; exact intermediate is still available.".into(),
        };
    }

    let mut beams = vec![Path {
        state: RotationState::default(),
        score: 0.0,
        words: Vec::new(),
    }];

    for (position, transformed) in tokens.iter().enumerate() {
        let possible: Vec<_> = dictionary()
            .iter()
            .filter(|(word, pronunciation)| {
                word.bytes().all(|byte| byte.is_ascii_lowercase())
                    && potentially_compatible(pronunciation, transformed)
            })
            .map(|(word, phones)| (word.as_str(), phones.as_slice()))
            .collect();

        let mut next_paths: HashMap<RotationState, Path> = HashMap::new();
        for path in &beams {
            let matches = ordered_matches(&possible, transformed, path.state, key);
            let alternatives: Vec<String> = matches
                .iter()
                .take(MAX_ALTERNATIVES)
                .map(|entry| entry.word.to_owned())
                .collect();

            for candidate in matches.iter().take(BEAM_WIDTH) {
                let mut words = path.words.clone();
                words.push(WordSuggestion {
                    transformed: (*transformed).to_owned(),
                    suggested: candidate.word.to_owned(),
                    alternatives: alternatives
                        .iter()
                        .filter(|alternative| alternative.as_str() != candidate.word)
                        .cloned()
                        .collect(),
                    compatible_count: matches.len(),
                });
                let candidate_path = Path {
                    state: candidate.state,
                    score: path.score + candidate.score,
                    words,
                };
                match next_paths.entry(candidate.state) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        if candidate_path.score > entry.get().score {
                            entry.insert(candidate_path);
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(candidate_path);
                    }
                }
            }
        }
        if next_paths.is_empty() {
            return Reconstruction {
                suggested: None,
                words: Vec::new(),
                note: format!(
                    "No complete dictionary expansion matched word {} ('{}'). Check the key and recipe; the exact pre-shift text is shown separately.",
                    position + 1,
                    transformed,
                ),
            };
        }

        beams = next_paths.into_values().collect();
        beams.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        beams.truncate(BEAM_WIDTH);
    }

    let Some(best) = beams.into_iter().next() else {
        return Reconstruction {
            suggested: None,
            words: Vec::new(),
            note: "No reconstruction available.".into(),
        };
    };
    let ambiguities = best.words.iter().filter(|w| w.compatible_count > 1).count();
    Reconstruction {
        suggested: Some(
            best.words
                .iter()
                .map(|w| w.suggested.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        note: format!(
            "Speculative English expansion: {ambiguities} of {} words have multiple valid dictionary candidates. Original spelling, capitalization and punctuation are not recoverable.",
            best.words.len(),
        ),
        words: best.words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;

    #[test]
    fn exact_forward_roundtrip_is_required_for_suggestions() {
        let input = "The quick brown fox jumps over the lazy dog.";
        let encoded = engine::encode(input, "gravel").unwrap();
        let recovered = reconstruct(&encoded.intermediate, "gravel");
        assert!(recovered.suggested.is_some(), "{}", recovered.note);
        assert_eq!(recovered.words.len(), 9);
        assert!(
            recovered
                .words
                .iter()
                .zip(encoded.intermediate.split_whitespace())
                .all(|(word, transformed)| word.transformed == transformed)
        );
        assert!(recovered.words.iter().any(|word| word.compatible_count > 1));

        // The speculative candidate must reproduce exactly the same intermediate
        // representation, even though it need not match the original phrase.
        let candidate = recovered.suggested.unwrap();
        let replay = engine::encode(&candidate, "gravel").unwrap();
        assert_eq!(replay.intermediate, encoded.intermediate);

        // User's native screenshot example is a regression test.
        let screenshot =
            engine::decode("eel rhaxl iubbe edhr dqoyxkc uxu jxm dynk udbn", "gravel").unwrap();
        assert_eq!(screenshot.intermediate, encoded.intermediate);
        assert!(screenshot.reconstruction.suggested.is_some());
        let word = screenshot
            .reconstruction
            .words
            .iter()
            .find(|word| word.transformed == "fucc")
            .unwrap();
        assert!(
            word.suggested == "fox" || word.alternatives.iter().any(|other| other == "fox"),
            "The intended original 'fox' must remain a supported candidate"
        );
    }

    #[test]
    fn fox_and_fix_are_an_actual_lossy_collision() {
        let fox = engine::encode("The quick brown fox jumps over the lazy dog", "gravel").unwrap();
        let fix = engine::encode("The quick brown fix jumps over the lazy dog", "gravel").unwrap();
        assert_eq!(fox.intermediate, fix.intermediate);
        assert_eq!(fox.ciphertext, fix.ciphertext);

        let decoded = engine::decode(&fox.ciphertext, "gravel").unwrap();
        let word = decoded
            .reconstruction
            .words
            .iter()
            .find(|word| word.transformed == "fucc")
            .unwrap();
        let options = std::iter::once(word.suggested.as_str())
            .chain(word.alternatives.iter().map(String::as_str))
            .collect::<Vec<_>>();
        assert!(options.contains(&"fix"));
        assert!(options.contains(&"fox"));
    }

    #[test]
    fn incorrect_text_is_not_misrepresented_as_plaintext() {
        let result = reconstruct("zzzznotapronunciation", "gravel");
        assert!(result.suggested.is_none());
        assert!(result.words.is_empty());
    }

    #[test]
    fn too_many_words_are_not_exhaustively_searched() {
        let text = std::iter::repeat_n("abc", MAX_RECONSTRUCTION_WORDS + 1)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(reconstruct(&text, "gravel").suggested.is_none());
    }
}
