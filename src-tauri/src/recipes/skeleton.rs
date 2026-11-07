//! Dictionary-ranked guesses for a consonant skeleton. This ranking is
//! heuristic and cannot restore vowels or original repeated consonants.
use super::{collapse_doubles, remove_vowels};
use crate::engine::recovery::{Reconstruction, WordChoice, WordSuggestion};
use std::collections::{HashMap, HashSet};

fn skeleton(text: &str) -> String {
    collapse_doubles(&remove_vowels(text)).to_ascii_lowercase()
}
pub fn reconstruct(intermediate: &str, choices: &[WordChoice]) -> Result<Reconstruction, String> {
    if intermediate.len() > 16_384 {
        return Err("Skeleton is too large.".into());
    }
    let words: Vec<_> = intermediate
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|s| !s.is_empty())
        .collect();
    if words.len() > 40 || words.iter().any(|w| w.len() > 80) {
        return Ok(Reconstruction {
            suggested: None,
            words: Vec::new(),
            note: "This message exceeds the bounded candidate search.".into(),
        });
    }
    let mut pins = HashMap::new();
    if choices.len() > words.len() {
        return Err("Too many selected words.".into());
    }
    for choice in choices {
        if choice.index >= words.len() {
            return Err("Candidate position is outside the message.".into());
        }
        if pins
            .insert(choice.index, choice.candidate.as_str())
            .is_some()
        {
            return Err("Repeated word selection.".into());
        }
    }
    let mut suggestions = Vec::new();
    let mut ambiguous = 0;
    // Frequency-ordered 20k vocabulary; no runtime network or AI dependency.
    let vocab = include_str!("../../data/english-frequency.txt");
    let mut cache: HashMap<String, Vec<&str>> = HashMap::new();
    let mut seen = HashSet::new();
    for candidate in vocab
        .lines()
        .filter(|w| w.bytes().all(|b| b.is_ascii_lowercase()))
    {
        let signature = skeleton(candidate);
        cache.entry(signature).or_default().push(candidate);
    }
    for (i, original) in words.iter().enumerate() {
        let signature = original.to_ascii_lowercase();
        let choices_for_word = cache.get(&signature).cloned().unwrap_or_default();
        if choices_for_word.is_empty() {
            return Ok(Reconstruction {
                suggested: None,
                words: Vec::new(),
                note: format!(
                    "No ranked English candidate matches skeleton word {} ('{}').",
                    i + 1,
                    original
                ),
            });
        }
        let selected = if let Some(locked) = pins.get(&i) {
            if !choices_for_word.contains(locked) {
                return Err(format!(
                    "'{locked}' does not match skeleton word {}.",
                    i + 1
                ));
            }
            locked.to_string()
        } else {
            choices_for_word[0].to_owned()
        };
        let mut alternatives = Vec::new();
        for word in &choices_for_word {
            if *word != selected && seen.insert((i, *word)) && alternatives.len() < 4 {
                alternatives.push((*word).to_owned());
            }
        }
        let count = choices_for_word.len();
        if count > 1 {
            ambiguous += 1;
        }
        suggestions.push(WordSuggestion {
            transformed: original.to_string(),
            suggested: selected,
            alternatives,
            compatible_count: count,
        });
    }
    let sentence = suggestions
        .iter()
        .map(|w| w.suggested.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Reconstruction {
        suggested: Some(sentence),
        words: suggestions,
        note: format!(
            "Speculative skeleton expansion: {ambiguous} of {} words admit multiple vowel/double-consonant readings. These are frequency-ranked guesses.",
            words.len()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skeleton_recovers_candidate_and_respects_pin() {
        let out = reconstruct("th qck brwn fx jmps vr th lzy dg", &[]).unwrap();
        assert!(out.suggested.is_some(), "{out:?}");
        let pinned = reconstruct(
            "th qck brwn fx jmps vr th lzy dg",
            &[WordChoice {
                index: 3,
                candidate: "fox".into(),
            }],
        )
        .unwrap();
        assert_eq!(pinned.words[3].suggested, "fox");
        assert!(
            reconstruct(
                "th qck brwn fx jmps vr th lzy dg",
                &[WordChoice {
                    index: 3,
                    candidate: "cat".into()
                }]
            )
            .is_err()
        );
    }
}
