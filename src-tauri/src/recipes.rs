//! User-authored recipes are versioned JSON data. Only rule implementations
//! are Rust code. This is an experimental cipher workbench, not encryption.
use serde::{Deserialize, Serialize};

use crate::engine;
use crate::engine::recovery::{Reconstruction, WordChoice};

pub const MAX_INPUT_BYTES: usize = 16_384;
pub const MAX_STEPS: usize = 16;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub rule: Rule,
}
fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    PhoneticRotate,
    RemoveVowels,
    CollapseDoubles,
    WordMirror,
    PairSwap,
    RotateVowels { steps: u8 },
    PositionShift { base: u8, keyed: bool },
    // Public classical cipher adapters; type IDs are stable JSON recipe content.
    Caesar { shift: u8 },
    Atbash,
    Vigenere,
    Beaufort,
    Affine { a: u8, b: u8 },
    RailFence { rails: u8 },
}

impl Rule {
    fn name(&self) -> &'static str {
        match self {
            Self::PhoneticRotate => "Phonetic rotation",
            Self::RemoveVowels => "Remove vowels",
            Self::CollapseDoubles => "Collapse doubled consonants",
            Self::WordMirror => "Mirror each word",
            Self::PairSwap => "Voiced/unvoiced letter pairs",
            Self::RotateVowels { .. } => "Rotate vowels",
            Self::PositionShift { .. } => "Position shift",
            Self::Caesar { .. } => "Caesar / ROT13",
            Self::Atbash => "Atbash",
            Self::Vigenere => "Vigenere",
            Self::Beaufort => "Beaufort",
            Self::Affine { .. } => "Affine",
            Self::RailFence { .. } => "Rail Fence",
        }
    }
    fn lossy(&self) -> bool {
        matches!(
            self,
            Self::PhoneticRotate | Self::RemoveVowels | Self::CollapseDoubles
        )
    }
    fn needs_key(&self) -> bool {
        matches!(
            self,
            Self::PhoneticRotate
                | Self::PositionShift { keyed: true, .. }
                | Self::Vigenere
                | Self::Beaufort
        )
    }
}
impl Recipe {
    fn enabled(&self) -> impl Iterator<Item = &Rule> {
        self.steps
            .iter()
            .filter(|step| step.enabled)
            .map(|step| &step.rule)
    }
    pub fn requires_key(&self) -> bool {
        self.enabled().any(Rule::needs_key)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported recipe schema version (expected 1).".into());
        }
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("Recipe ID must be 1-64 lowercase letters, digits or hyphens.".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 100 || self.description.len() > 1024 {
            return Err(
                "Recipe name is required (max 100 characters), description max 1024.".into(),
            );
        }
        if self.steps.is_empty()
            || self.steps.len() > MAX_STEPS
            || !self.steps.iter().any(|s| s.enabled)
        {
            return Err("Recipe needs 1-16 steps and at least one enabled step.".into());
        }
        for rule in self.enabled() {
            match rule {
                Rule::PositionShift { base, .. } if *base > 25 => {
                    return Err("Shift offset must be between 0 and 25.".into());
                }
                Rule::RotateVowels { steps } if *steps > 4 => {
                    return Err("Vowel rotation must be between 0 and 4.".into());
                }
                Rule::Caesar { shift } if *shift > 25 => {
                    return Err("Caesar shift must be between 0 and 25.".into());
                }
                Rule::Affine { a, b }
                    if !(1..=25).contains(a) || a % 2 == 0 || a % 13 == 0 || *b > 25 =>
                {
                    return Err(
                        "Affine requires a coprime multiplier 1..25 and offset 0..25.".into(),
                    );
                }
                Rule::RailFence { rails } if !(2..=16).contains(rails) => {
                    return Err("Rail Fence must use between 2 and 16 rails.".into());
                }
                _ => {}
            }
        }
        // Phonetic recognition requires real English words; it cannot soundly
        // parse arbitrary transformed strings from an earlier step.
        let enabled: Vec<&Rule> = self.enabled().collect();
        if enabled
            .iter()
            .enumerate()
            .any(|(i, rule)| matches!(rule, Rule::PhoneticRotate) && i != 0)
        {
            return Err("Phonetic rotation must be the first enabled rule.".into());
        }
        if enabled
            .iter()
            .filter(|rule| matches!(rule, Rule::PhoneticRotate))
            .count()
            > 1
        {
            return Err("Phonetic rotation may appear only once.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageOutput {
    pub label: String,
    pub text: String,
    pub lossy: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeResult {
    pub output: String,
    pub intermediate: String,
    pub exact: bool,
    pub reconstruction: Option<Reconstruction>,
    pub note: String,
    pub stages: Vec<StageOutput>,
}

pub fn builtins() -> Result<Vec<Recipe>, String> {
    const DOCUMENTS: [&str; 11] = [
        include_str!("../../recipes/001.json"),
        include_str!("../../recipes/002.json"),
        include_str!("../../recipes/003.json"),
        include_str!("../../recipes/004.json"),
        include_str!("../../recipes/005.json"),
        include_str!("../../recipes/006.json"),
        include_str!("../../recipes/007.json"),
        include_str!("../../recipes/008.json"),
        include_str!("../../recipes/009.json"),
        include_str!("../../recipes/010.json"),
        include_str!("../../recipes/011.json"),
    ];
    DOCUMENTS
        .iter()
        .map(|text| {
            let recipe: Recipe = serde_json::from_str(text)
                .map_err(|err| format!("Invalid bundled recipe: {err}"))?;
            recipe.validate()?;
            Ok(recipe)
        })
        .collect()
}

fn validate_inputs(recipe: &Recipe, text: &str, key: &str) -> Result<String, String> {
    recipe.validate()?;
    if text.is_empty() || text.len() > MAX_INPUT_BYTES || !text.is_ascii() {
        return Err("Message must be nonempty ASCII text up to 16 KiB.".into());
    }
    let key = key.trim().to_ascii_lowercase();
    if recipe.requires_key()
        && (key.is_empty() || key.len() > 64 || !key.bytes().all(|b| b.is_ascii_lowercase()))
    {
        return Err("This recipe needs a passphrase of 1-64 English letters.".into());
    }
    Ok(key)
}

fn map_case(value: u8, mapped_lowercase: u8) -> u8 {
    if value.is_ascii_uppercase() {
        mapped_lowercase.to_ascii_uppercase()
    } else {
        mapped_lowercase
    }
}

fn mirror_words(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let mut start = 0;
    while start < bytes.len() {
        if !bytes[start].is_ascii_alphabetic() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        bytes[start..end].reverse();
        start = end;
    }
    String::from_utf8(bytes).expect("ASCII input")
}

fn pair_swap(text: &str) -> String {
    text.bytes()
        .map(|b| {
            let mapped = match b.to_ascii_lowercase() {
                b'b' => b'p',
                b'p' => b'b',
                b'd' => b't',
                b't' => b'd',
                b'g' => b'k',
                b'k' => b'g',
                b'v' => b'f',
                b'f' => b'v',
                b'z' => b's',
                b's' => b'z',
                other => other,
            };
            char::from(map_case(b, mapped))
        })
        .collect()
}
fn is_vowel(b: u8) -> bool {
    b"aeiou".contains(&b.to_ascii_lowercase())
}

fn vowel_rotation(text: &str, steps: u8, undo: bool) -> String {
    text.bytes()
        .map(|byte| {
            let lower = byte.to_ascii_lowercase();
            let Some(index) = b"aeiou".iter().position(|x| *x == lower) else {
                return char::from(byte);
            };
            let n = usize::from(steps);
            let next = if undo {
                (index + 5 - n) % 5
            } else {
                (index + n) % 5
            };
            char::from(map_case(byte, b"aeiou"[next]))
        })
        .collect()
}
fn remove_vowels(text: &str) -> String {
    text.bytes()
        .filter(|b| !is_vowel(*b))
        .map(char::from)
        .collect()
}
fn collapse_doubles(text: &str) -> String {
    let mut prev = 0_u8;
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphabetic() && !is_vowel(byte) && byte.to_ascii_lowercase() == prev {
            continue;
        }
        out.push(char::from(byte));
        prev = if byte.is_ascii_alphabetic() {
            byte.to_ascii_lowercase()
        } else {
            0
        };
    }
    out
}

/// Word and character position are one-based; key index carries over letters
/// but does not advance over spaces/punctuation.
fn shift(text: &str, key: &str, base: u8, keyed: bool, undo: bool) -> String {
    let mut word_index = 0_usize;
    let mut letter_index = 0_usize;
    let mut global_letter = 0_usize;
    text.bytes()
        .map(|byte| {
            if !byte.is_ascii_alphabetic() {
                letter_index = 0;
                return char::from(byte);
            }
            if letter_index == 0 {
                word_index += 1;
            }
            letter_index += 1;
            let k = if keyed {
                usize::from(key.as_bytes()[global_letter % key.len()] - b'a')
            } else {
                0
            };
            let distance = (usize::from(base) + word_index + letter_index + k) % 26;
            let lower = usize::from(byte.to_ascii_lowercase() - b'a');
            let transformed = if undo {
                (lower + 26 - distance) % 26
            } else {
                (lower + distance) % 26
            };
            global_letter += 1;
            char::from(map_case(byte, b'a' + transformed as u8))
        })
        .collect()
}

fn apply(rule: &Rule, text: &str, key: &str, undo: bool) -> Result<String, String> {
    match rule {
        Rule::PhoneticRotate if !undo => engine::phonetic_transform(text, key),
        Rule::PhoneticRotate => Err("Phonetic collapse cannot be reversed exactly.".into()),
        Rule::RemoveVowels if !undo => Ok(remove_vowels(text)),
        Rule::RemoveVowels => Err("Removed vowels cannot be recovered exactly.".into()),
        Rule::CollapseDoubles if !undo => Ok(collapse_doubles(text)),
        Rule::CollapseDoubles => Err("Collapsed consonants cannot be recovered exactly.".into()),
        Rule::WordMirror => Ok(mirror_words(text)),
        Rule::PairSwap => Ok(pair_swap(text)),
        Rule::RotateVowels { steps } => Ok(vowel_rotation(text, *steps, undo)),
        Rule::PositionShift { base, keyed } => Ok(shift(text, key, *base, *keyed, undo)),
        Rule::Caesar { .. }
        | Rule::Atbash
        | Rule::Vigenere
        | Rule::Beaufort
        | Rule::Affine { .. }
        | Rule::RailFence { .. } => classical::transform(rule, text, key, undo),
    }
}

pub fn encode(recipe: &Recipe, text: &str, key: &str) -> Result<RecipeResult, String> {
    let key = validate_inputs(recipe, text, key)?;
    let mut current = text.to_owned();
    let mut stages = Vec::new();
    let mut exact = true;
    for rule in recipe.enabled() {
        current = apply(rule, &current, &key, false)?;
        exact &= !rule.lossy();
        stages.push(StageOutput {
            label: rule.name().into(),
            text: current.clone(),
            lossy: rule.lossy(),
        });
    }
    Ok(RecipeResult {
        output: current.clone(),
        intermediate: current,
        exact,
        reconstruction: None,
        note: if exact {
            "Reversible pipeline: decoding restores the input exactly.".into()
        } else {
            "Lossy pipeline: some original information is permanently discarded.".into()
        },
        stages,
    })
}

pub fn decode(
    recipe: &Recipe,
    text: &str,
    key: &str,
    choices: &[WordChoice],
) -> Result<RecipeResult, String> {
    let key = validate_inputs(recipe, text, key)?;
    let mut current = text.to_owned();
    let mut stages = Vec::new();
    let mut lost = false;
    let enabled: Vec<&Rule> = recipe.enabled().collect();
    for rule in enabled.iter().rev() {
        if rule.lossy() {
            lost = true;
            continue;
        }
        if !lost {
            current = apply(rule, &current, &key, true)?;
            stages.push(StageOutput {
                label: format!("Undo {}", rule.name()),
                text: current.clone(),
                lossy: false,
            });
        }
    }
    if !lost {
        if !choices.is_empty() {
            return Err("Reversible recipes have no ambiguous word selections.".into());
        }
        return Ok(RecipeResult {
            output: current.clone(),
            intermediate: current,
            exact: true,
            reconstruction: None,
            note: "All enabled transformations reversed exactly.".into(),
            stages,
        });
    }

    // Only supported reconstruction pipelines produce ranked word candidates.
    // Other valid lossy combinations still return the exact surviving layer.
    let reconstruction = match enabled.as_slice() {
        // The loss-carrying prefix is recognized by rule types, not recipe IDs.
        // Every reversible suffix may be added, removed or reordered; decoding
        // already reversed it before invoking a reconstruction strategy.
        [Rule::PhoneticRotate, suffix @ ..] if suffix.iter().all(|r| !r.lossy()) => {
            if choices.is_empty() {
                Some(engine::recovery::reconstruct(&current, &key))
            } else {
                Some(engine::recovery::reconstruct_with_choices(
                    &current, &key, choices,
                )?)
            }
        }
        [Rule::RemoveVowels, Rule::CollapseDoubles, suffix @ ..]
            if suffix.iter().all(|r| !r.lossy()) =>
        {
            Some(skeleton::reconstruct(&current, choices)?)
        }
        _ => {
            if !choices.is_empty() {
                return Err("This lossy rule combination has no compatible candidate reconstruction strategy.".into());
            }
            None
        }
    };
    let output = reconstruction
        .as_ref()
        .and_then(|r| r.suggested.clone())
        .unwrap_or_default();
    let note = reconstruction.as_ref().map_or_else(
        || "Exact surviving transformation recovered; this rule combination has no English reconstruction strategy.".into(),
        |r|r.note.clone()
    );
    Ok(RecipeResult {
        output,
        intermediate: current,
        exact: false,
        reconstruction,
        note,
        stages,
    })
}

mod classical;
pub mod skeleton;
pub mod store;

#[cfg(test)]
mod tests {
    use super::*;
    fn get(id: &str) -> Recipe {
        builtins()
            .unwrap()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
    }
    const SENTENCE: &str = "The quick brown fox jumps over the lazy dog.";

    #[test]
    fn all_bundled_recipes_validate_and_are_data() {
        let recipes = builtins().unwrap();
        assert_eq!(recipes.len(), 11);
        for recipe in recipes {
            recipe.validate().unwrap();
        }
    }
    #[test]
    fn recipe_001_matches_prior_cipher_byte_for_byte() {
        let recipe = get("builtin-001");
        let old = engine::encode(SENTENCE, "spectacular").unwrap();
        let new = encode(&recipe, SENTENCE, "spectacular").unwrap();
        assert_eq!(old.intermediate, new.stages[0].text);
        assert_eq!(old.ciphertext, new.output);
        let decoded = decode(&recipe, &new.output, "spectacular", &[]).unwrap();
        assert_eq!(decoded.intermediate, old.intermediate);
    }
    #[test]
    fn word_mirror_exact_round_trip_preserves_punctuation() {
        let recipe = get("builtin-003");
        let text = "Hello, Rust! ABC 123 test.";
        let encrypted = encode(&recipe, text, "").unwrap();
        let decrypted = decode(&recipe, &encrypted.output, "", &[]).unwrap();
        assert_eq!(text, decrypted.output);
        assert!(decrypted.exact);
    }
    #[test]
    fn reversible_pairs_and_vowels_round_trip() {
        let recipe = get("builtin-004");
        let encrypted = encode(&recipe, SENTENCE, "").unwrap();
        let recovered = decode(&recipe, &encrypted.output, "", &[]).unwrap();
        assert_eq!(SENTENCE, recovered.output);
        assert!(recovered.exact);
    }
    #[test]
    fn skeleton_lossy_intermediate_and_reconstruction() {
        let recipe = get("builtin-002");
        let encrypted = encode(&recipe, SENTENCE, "").unwrap();
        assert_eq!(
            encrypted.stages[0].text,
            "Th qck brwn fx jmps vr th lzy dg."
        );
        let decoded = decode(&recipe, &encrypted.output, "", &[]).unwrap();
        assert_eq!(decoded.intermediate, "Th qck brwn fx jmps vr th lzy dg.");
        assert!(!decoded.exact);
    }
    #[test]
    fn phonetic_reconstruction_accepts_any_reversible_suffix() {
        let mut recipe = get("builtin-001");
        recipe.steps.insert(
            1,
            Step {
                enabled: true,
                rule: Rule::WordMirror,
            },
        );
        recipe.validate().unwrap();
        let encoded = encode(&recipe, SENTENCE, "spectacular").unwrap();
        let recovered = decode(&recipe, &encoded.output, "spectacular", &[]).unwrap();
        assert!(
            recovered.reconstruction.is_some(),
            "reversible suffix must be composable"
        );
        assert_eq!(
            recovered.intermediate,
            engine::encode(SENTENCE, "spectacular")
                .unwrap()
                .intermediate
        );
        assert!(!recovered.exact);
    }
    #[test]
    fn consonant_skeleton_reconstruction_accepts_reversible_suffix() {
        let mut recipe = get("builtin-002");
        recipe.steps.insert(
            2,
            Step {
                enabled: true,
                rule: Rule::WordMirror,
            },
        );
        recipe.validate().unwrap();
        let encoded = encode(&recipe, SENTENCE, "").unwrap();
        let recovered = decode(&recipe, &encoded.output, "", &[]).unwrap();
        assert_eq!(recovered.intermediate, "Th qck brwn fx jmps vr th lzy dg.");
        assert!(recovered.reconstruction.is_some());
    }
    #[test]
    fn disabled_steps_are_skipped_in_both_directions() {
        let mut recipe = get("builtin-003");
        recipe.steps[0].enabled = false;
        let encoded = encode(&recipe, SENTENCE, "").unwrap();
        let recovered = decode(&recipe, &encoded.output, "", &[]).unwrap();
        assert_eq!(recovered.output, SENTENCE);
        assert!(recovered.exact);
        assert_eq!(encoded.stages.len(), 1);
    }

    fn reversible_strategy() -> impl proptest::strategy::Strategy<Value = Rule> {
        use proptest::prelude::*;
        prop_oneof![
            Just(Rule::WordMirror),
            Just(Rule::PairSwap),
            (0u8..=4).prop_map(|steps| Rule::RotateVowels { steps }),
            (0u8..=25, any::<bool>()).prop_map(|(base, keyed)| Rule::PositionShift { base, keyed }),
            (0u8..=25).prop_map(|shift| Rule::Caesar { shift }),
            Just(Rule::Atbash),
            Just(Rule::Vigenere),
            Just(Rule::Beaufort),
            proptest::sample::select(vec![1u8, 3, 5, 7, 9, 11, 15, 17, 19, 21, 23, 25])
                .prop_flat_map(|a| (Just(a), 0u8..=25))
                .prop_map(|(a, b)| Rule::Affine { a, b }),
            (2u8..=16).prop_map(|rails| Rule::RailFence { rails }),
        ]
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig {
            cases: 100,
            ..proptest::prelude::ProptestConfig::default()
        })]

        #[test]
        fn arbitrary_reversible_rule_compositions_roundtrip(
            input in proptest::collection::vec(0u8..=127, 1..220),
            rules in proptest::collection::vec(reversible_strategy(), 1..7),
        ) {
            let text: String = input.into_iter().map(char::from).collect();
            let recipe = Recipe {
                schema_version: 1,
                id: "user-generated-reversible".into(),
                name: "Generated reversible recipe".into(),
                description: String::new(),
                steps: rules.into_iter().map(|rule| Step { enabled: true, rule }).collect(),
            };
            recipe.validate().unwrap();
            let cipher = encode(&recipe, &text, "secret").unwrap();
            let decoded = decode(&recipe, &cipher.output, "secret", &[]).unwrap();
            proptest::prop_assert!(decoded.exact);
            proptest::prop_assert_eq!(decoded.output, text);
        }
    }

    #[test]
    fn classical_rules_still_use_json_recipe_content() {
        let examples = builtins().unwrap();
        assert!(
            examples.len() >= 10,
            "classical examples should be bundled as JSON"
        );
        for recipe in examples {
            recipe.validate().unwrap();
        }
        let vigenere = get("builtin-006");
        let result = encode(&vigenere, "ATTACK AT DAWN!", "lemon").unwrap();
        assert_eq!(result.output, "LXFOPV EF RNHR!");
        assert_eq!(
            decode(&vigenere, &result.output, "lemon", &[])
                .unwrap()
                .output,
            "ATTACK AT DAWN!"
        );
        let rail = get("builtin-009");
        let text = "WEAREDISCOVEREDFLEEATONCE";
        let output = encode(&rail, text, "").unwrap();
        assert_eq!(output.output, "WECRLTEERDSOEEFEAOCAIVDEN");
        assert_eq!(decode(&rail, &output.output, "", &[]).unwrap().output, text);
    }
    #[test]
    fn mixed_classical_recipe_roundtrips_exactly() {
        let recipe = get("builtin-011");
        let plaintext = "The Quick Brown fox, 123!\nTest.";
        let encoded = encode(&recipe, plaintext, "lemon").unwrap();
        assert_ne!(encoded.output, plaintext);
        assert!(encoded.exact);
        let decoded = decode(&recipe, &encoded.output, "lemon", &[]).unwrap();
        assert_eq!(decoded.output, plaintext);
        assert!(decoded.exact);
        assert_eq!(decoded.stages.len(), 3);
    }

    #[test]
    fn phonetic_lossy_output_survives_classical_transposition_suffix() {
        let mut recipe = get("builtin-001");
        recipe.steps.insert(
            1,
            Step {
                enabled: true,
                rule: Rule::RailFence { rails: 3 },
            },
        );
        let encoded = encode(&recipe, SENTENCE, "spectacular").unwrap();
        let decoded = decode(&recipe, &encoded.output, "spectacular", &[]).unwrap();
        assert!(!decoded.exact);
        assert_eq!(
            decoded.intermediate,
            engine::encode(SENTENCE, "spectacular")
                .unwrap()
                .intermediate
        );
        assert!(decoded.reconstruction.is_some());
    }

    #[test]
    fn rejects_unicode_before_rail_fence_byte_indexing() {
        let recipe = get("builtin-009");
        assert!(encode(&recipe, "emoji 😀", "").is_err());
        assert!(decode(&recipe, "emoji 😀", "", &[]).is_err());
    }

    #[test]
    fn classical_rule_validation_catches_malformed_inputs() {
        let mut recipe = get("builtin-008");
        recipe.steps[0].rule = Rule::Affine { a: 2, b: 5 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::Affine { a: 13, b: 5 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::Affine { a: 5, b: 26 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::RailFence { rails: 0 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::RailFence { rails: 17 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::Caesar { shift: 26 };
        assert!(recipe.validate().is_err());
        recipe.steps[0].rule = Rule::Vigenere;
        assert!(recipe.validate().is_ok());
        assert!(encode(&recipe, "abc", "").is_err());
        assert!(serde_json::from_str::<Recipe>(
            r#"{"schemaVersion":1,"id":"user-x","name":"x","description":"","steps":[{"enabled":true,"rule":{"type":"affine","a":5,"b":8,"unexpected":1}}]}"#
        ).is_err());
    }

    #[test]
    fn rejects_unknown_rules_and_bad_params() {
        assert!(serde_json::from_str::<Recipe>(r#"{"schemaVersion":1,"id":"x","name":"x","description":"","steps":[{"rule":{"type":"code_execution"}}]}"#).is_err());
        let mut recipe = get("builtin-003");
        recipe.steps[1].rule = Rule::PositionShift {
            base: 30,
            keyed: false,
        };
        assert!(recipe.validate().is_err());
        recipe.steps[1].rule = Rule::PositionShift {
            base: 5,
            keyed: true,
        };
        assert!(encode(&recipe, "abc", "").is_err());
    }
}
