use super::*;
use crate::recipes::{Recipe, Rule, Step};
use std::{fs, path::Path};

fn run_at(root: &Path, name: &str, rule: &Rule, text: &str, undo: bool) -> String {
    let input = root.join(format!("{name}-in"));
    let output = root.join(format!("{name}-out"));
    fs::write(&input, text).unwrap();
    let stats =
        transform::transform(rule, &input, &output, "lemon", undo, &Control::default()).unwrap();
    let result = fs::read_to_string(output).unwrap();
    assert_eq!(stats.bytes, u64::try_from(result.len()).unwrap());
    assert_eq!(
        stats.scalars,
        u64::try_from(result.chars().count()).unwrap()
    );
    result
}

#[test]
fn unicode_substitutions_keep_nonletters_and_literal_key_phase() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        run_at(
            root.path(),
            "caesar",
            &Rule::Caesar { shift: 3 },
            "Café, 世界!\r\nA\0",
            false
        ),
        "Fdié, 世界!\r\nD\0"
    );
    assert_eq!(
        run_at(
            root.path(),
            "vigenere",
            &Rule::Vigenere,
            "ATTACK é🌍 AT DAWN!",
            false
        ),
        "LXFOPV é🌍 EF RNHR!"
    );
    assert_eq!(
        run_at(
            root.path(),
            "rail",
            &Rule::RailFence { rails: 3 },
            "Aé🙂B中C",
            false
        ),
        "A中éBC🙂"
    );
}

#[test]
fn every_reversible_rule_restores_exact_unicode_bytes() {
    let root = tempfile::tempdir().unwrap();
    let rules = [
        Rule::WordMirror,
        Rule::PairSwap,
        Rule::RotateVowels { steps: 3 },
        Rule::PositionShift {
            base: 7,
            keyed: true,
        },
        Rule::Caesar { shift: 13 },
        Rule::Atbash,
        Rule::Vigenere,
        Rule::Beaufort,
        Rule::Affine { a: 7, b: 12 },
    ];
    let text = "# Café e\u{301} 🌍👩‍👧\r\nGrüße Ελληνικά 中文\tA\0z\n";
    for (i, rule) in rules.into_iter().enumerate() {
        let encoded = run_at(root.path(), &format!("enc-{i}"), &rule, text, false);
        assert_eq!(
            run_at(root.path(), &format!("dec-{i}"), &rule, &encoded, true),
            text,
            "{rule:?}"
        );
    }
    for rails in 2..=16 {
        let rule = Rule::RailFence { rails };
        for (i, text) in ["", "🙂", text].into_iter().enumerate() {
            let encoded = run_at(root.path(), &format!("re-{rails}-{i}"), &rule, text, false);
            assert_eq!(
                run_at(
                    root.path(),
                    &format!("rd-{rails}-{i}"),
                    &rule,
                    &encoded,
                    true
                ),
                text
            );
        }
    }
}

#[test]
fn state_survives_buffers_and_word_mirror_spills_long_words() {
    let root = tempfile::tempdir().unwrap();
    let text = format!("{}é\r\n{}🙂", "AbCd".repeat(50000), "aZ ".repeat(25000));
    for (i, rule) in [
        Rule::WordMirror,
        Rule::Vigenere,
        Rule::PositionShift {
            base: 5,
            keyed: true,
        },
        Rule::RailFence { rails: 13 },
    ]
    .into_iter()
    .enumerate()
    {
        let encoded = run_at(root.path(), &format!("large-e{i}"), &rule, &text, false);
        assert_eq!(
            run_at(root.path(), &format!("large-d{i}"), &rule, &encoded, true),
            text
        );
    }
    assert_eq!(
        run_at(
            root.path(),
            "collapse",
            &Rule::CollapseDoubles,
            &format!("{}ébb", "b".repeat(100000)),
            false
        ),
        "béb"
    );
}

#[test]
fn fixed_rules_agree_with_legacy_ascii_results() {
    let root = tempfile::tempdir().unwrap();
    let text = "The Quick BROWN fox, 123!\r\nA test.";
    for (i, rule) in [
        Rule::PhoneticRotate,
        Rule::RemoveVowels,
        Rule::CollapseDoubles,
        Rule::WordMirror,
        Rule::PairSwap,
        Rule::RotateVowels { steps: 2 },
        Rule::PositionShift {
            base: 5,
            keyed: true,
        },
        Rule::Caesar { shift: 3 },
        Rule::Atbash,
        Rule::Vigenere,
        Rule::Beaufort,
        Rule::Affine { a: 5, b: 8 },
        Rule::RailFence { rails: 3 },
    ]
    .into_iter()
    .enumerate()
    {
        let recipe = Recipe {
            schema_version: 1,
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            steps: vec![Step {
                enabled: true,
                rule: rule.clone(),
            }],
        };
        let expected = crate::recipes::encode(&recipe, text, "lemon")
            .unwrap()
            .output;
        assert_eq!(
            run_at(root.path(), &format!("ascii-{i}"), &rule, text, false),
            expected,
            "{rule:?}"
        );
    }
}

#[test]
fn key_phase_and_split_utf8_have_independent_boundary_vectors() {
    let root = tempfile::tempdir().unwrap();
    let text = format!("{}éAAAAAAA", "A".repeat(65534));
    assert!(run_at(root.path(), "phase", &Rule::Vigenere, &text, false).ends_with("éNLEMONL"));
    let text = format!("{}🙂A", "A".repeat(65535));
    assert!(
        run_at(
            root.path(),
            "split",
            &Rule::Caesar { shift: 1 },
            &text,
            false
        )
        .ends_with("🙂B")
    );
}

#[test]
fn phonetic_rotation_keeps_global_vowel_state_beyond_interactive_size() {
    let root = tempfile::tempdir().unwrap();
    let text = "a ".repeat(20000);
    let output = run_at(
        root.path(),
        "phonetic-large",
        &Rule::PhoneticRotate,
        &text,
        false,
    );
    assert_eq!(output, "e i o u a ".repeat(4000).trim_end());
}

#[test]
fn rail_documents_match_legacy_for_all_supported_counts() {
    let root = tempfile::tempdir().unwrap();
    let text = String::from_utf8((0..=127).collect()).unwrap();
    for rails in 2..=16 {
        let rule = Rule::RailFence { rails };
        let expected = crate::recipes::classical::transform(&rule, &text, "", false).unwrap();
        assert_eq!(
            run_at(
                root.path(),
                &format!("rail-ascii-{rails}"),
                &rule,
                &text,
                false
            ),
            expected
        );
    }
}

#[test]
fn manual_document_recipes_roundtrip_unicode_and_keep_bom_as_text() {
    let mut recipe = crate::recipes::builtins().unwrap().remove(2);
    recipe.schema_version = 2;
    recipe.steps = vec![
        Step {
            enabled: true,
            rule: Rule::Caesar { shift: 3 },
        },
        Step {
            enabled: true,
            rule: Rule::RailFence { rails: 3 },
        },
    ];
    let text = "\u{feff}Café e\u{301} 世界 🙂\r\n";
    let encoded = crate::recipes::encode(&recipe, text, "").unwrap();
    assert_eq!(
        crate::recipes::decode(&recipe, &encoded.output, "", &[])
            .unwrap()
            .output,
        text
    );
    assert!(
        crate::recipes::encode(&recipe, "", "")
            .unwrap()
            .output
            .is_empty()
    );
    recipe.schema_version = 1;
    assert!(crate::recipes::encode(&recipe, text, "").is_err());
}

#[test]
fn invalid_utf8_and_cancel_are_failures_without_clobber() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("invalid");
    fs::write(&source, [0xf0, 0x9f, 0x80]).unwrap();
    assert!(
        transform::transform(
            &Rule::Atbash,
            &source,
            &root.path().join("invalid-out"),
            "",
            false,
            &Control::default()
        )
        .is_err()
    );
    fs::write(&source, "abc").unwrap();
    let output = root.path().join("existing");
    fs::write(&output, "keep").unwrap();
    assert!(
        transform::transform(
            &Rule::Atbash,
            &source,
            &output,
            "",
            false,
            &Control::default()
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), "keep");
    let control = Control::default();
    control.cancel();
    assert!(
        transform::transform(
            &Rule::Atbash,
            &source,
            &root.path().join("cancelled"),
            "",
            false,
            &control
        )
        .is_err()
    );
}
