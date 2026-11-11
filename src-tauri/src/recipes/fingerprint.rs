//! A canonical configuration fingerprint and passphrase-derived rule parameters.
//!
//! These hashes are deterministic identifiers and puzzle inputs, not signatures,
//! password verifiers, key stretching or authenticated encryption.
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{Recipe, Rule, Step};

const FINGERPRINT_DOMAIN: &[u8] = b"cryptoplay:recipe-configuration:v1\0";
const DERIVATION_DOMAIN: &[u8] = b"cryptoplay:rule-parameter:v1\0";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Configuration<'a> {
    schema_version: u32,
    steps: &'a [Step],
}

/// Full fingerprint for exact normalized rule configuration. Metadata such as
/// recipe ID, title and description is deliberately *not* part of its identity.
/// The normalized serde representation makes object field order irrelevant.
fn digest(recipe: &Recipe) -> Result<[u8; 32], String> {
    let canonical = serde_json::to_vec(&Configuration {
        schema_version: recipe.schema_version,
        steps: &recipe.steps,
    })
    .map_err(|error| format!("Cannot canonicalize recipe: {error}"))?;
    let mut hash = Sha256::new();
    hash.update(if recipe.schema_version == 2 {
        b"cryptoplay:recipe-configuration:v2\0".as_slice()
    } else {
        FINGERPRINT_DOMAIN
    });
    hash.update(canonical);
    Ok(hash.finalize().into())
}

fn lowercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFingerprint {
    pub algorithm: &'static str,
    pub version: u8,
    /// A full SHA-256 digest; labels do not convey authenticity.
    pub value: String,
    /// Short human-readable comparison code, not collision-proof.
    pub display: String,
}

pub fn fingerprint(recipe: &Recipe) -> Result<RecipeFingerprint, String> {
    recipe.validate()?;
    let value = lowercase_hex(&digest(recipe)?);
    Ok(RecipeFingerprint {
        algorithm: "SHA-256",
        version: recipe.schema_version as u8,
        display: format!("CRYPT-R{}-{}", recipe.schema_version, &value[..16]),
        value,
    })
}

fn parameter(
    recipe: &Recipe,
    index: usize,
    passphrase: &str,
    type_id: &str,
    modulo: u32,
    minimum: u32,
) -> Result<u8, String> {
    // The recipe, step identity and passphrase jointly determine the result.
    // This is intentionally fast for interactive puzzles, not a password KDF.
    if passphrase.is_empty() || !passphrase.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Err("A passphrase of English letters is required for derived parameters.".into());
    }
    let digest = digest(recipe)?;
    let mut hash = Sha256::new();
    hash.update(if recipe.schema_version == 2 {
        b"cryptoplay:rule-parameter:v2\0".as_slice()
    } else {
        DERIVATION_DOMAIN
    });
    hash.update(digest);
    hash.update(
        u32::try_from(index)
            .map_err(|_| "Rule index overflow.")?
            .to_le_bytes(),
    );
    hash.update(type_id.as_bytes());
    hash.update([0]);
    hash.update(passphrase.as_bytes());
    let result = hash.finalize();
    let number = u32::from_le_bytes(result[..4].try_into().expect("SHA-256 always has 32 bytes"));
    Ok((minimum + number % modulo) as u8)
}

/// Produce a regular classical rule with a derived value and an inspection label.
/// Existing Caesar / Rail Fence rules are not modified by this operation.
pub(crate) fn resolve(
    recipe: &Recipe,
    index: usize,
    rule: &Rule,
    passphrase: &str,
) -> Result<Option<(Rule, String)>, String> {
    match rule {
        Rule::DerivedCaesar => {
            let shift = parameter(recipe, index, passphrase, "derived_caesar", 26, 0)?;
            Ok(Some((
                Rule::Caesar { shift },
                format!("Passphrase-derived Caesar (shift {shift})"),
            )))
        }
        Rule::DerivedRailFence => {
            let rails = parameter(recipe, index, passphrase, "derived_rail_fence", 15, 2)?;
            Ok(Some((
                Rule::RailFence { rails },
                format!("Passphrase-derived Rail Fence ({rails} rails)"),
            )))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{Step, decode, encode};

    #[test]
    fn document_recipe_has_a_separate_fingerprint_domain() {
        let mut recipe = sample();
        recipe.schema_version = 2;
        recipe.steps = vec![Step {
            enabled: true,
            rule: Rule::Caesar { shift: 3 },
        }];
        let value = fingerprint(&recipe).expect("document schema is supported");
        assert_eq!(value.version, 2);
        assert_eq!(value.display, "CRYPT-R2-0a5edbfce2cdc5e3");
        assert_eq!(
            value.value,
            "0a5edbfce2cdc5e32091b6f8eaa7b59670f6cccab82d5b5e3e88630a7a0c0611"
        );
        assert_ne!(value.value, fingerprint(&sample()).unwrap().value);
    }

    fn sample() -> Recipe {
        Recipe {
            schema_version: 1,
            id: "user-example".into(),
            name: "Example".into(),
            description: "Original title".into(),
            steps: vec![
                Step {
                    enabled: true,
                    rule: Rule::DerivedCaesar,
                },
                Step {
                    enabled: true,
                    rule: Rule::DerivedRailFence,
                },
                Step {
                    enabled: true,
                    rule: Rule::Atbash,
                },
            ],
        }
    }

    #[test]
    fn fingerprints_ignore_metadata_but_bind_to_configuration() {
        let initial = sample();
        let id = fingerprint(&initial).unwrap();
        assert_eq!(id.algorithm, "SHA-256");
        assert_eq!(id.version, 1);
        assert_eq!(id.value.len(), 64);
        assert_eq!(id.display.len(), 25);
        // A cross-platform golden digest fixes canonical serialization and
        // domain separation; changing it requires an explicit new version.
        assert_eq!(
            id.value,
            "4dc70d683de336cfb7654cbbc6a1a72930d9febbb2299444cbc4b41f79feef23"
        );

        let mut renamed = initial.clone();
        renamed.id = "user-new-id".into();
        renamed.name = "Another title".into();
        renamed.description = "Different description".into();
        assert_eq!(fingerprint(&renamed).unwrap().value, id.value);

        renamed.steps.swap(0, 1);
        assert_ne!(fingerprint(&renamed).unwrap().value, id.value);
        renamed.steps.swap(0, 1);
        renamed.steps[0].enabled = false;
        assert_ne!(fingerprint(&renamed).unwrap().value, id.value);
    }

    #[test]
    fn fingerprint_is_independent_of_json_property_order() {
        let original = sample();
        let serialized = serde_json::to_string(&original).unwrap();
        let reconstructed: Recipe = serde_json::from_str(&serialized).unwrap();
        let differently_ordered = format!(
            r#"{{"steps":{},"description":"Original title","name":"Example","id":"user-example","schemaVersion":1}}"#,
            serde_json::to_string(&original.steps).unwrap()
        );
        let parsed: Recipe = serde_json::from_str(&differently_ordered).unwrap();
        assert_eq!(
            fingerprint(&reconstructed).unwrap().value,
            fingerprint(&parsed).unwrap().value
        );
    }

    #[test]
    fn derived_parameters_roundtrip_across_layered_rules() {
        let recipe = sample();
        let input = "The Quick Brown Fox, 123!\nAn example.";
        let encoded = encode(&recipe, input, "spectacular").unwrap();
        let decoded = decode(&recipe, &encoded.output, "spectacular", &[]).unwrap();
        assert!(encoded.exact);
        assert_eq!(decoded.output, input);
        assert!(encoded.stages[0].label.contains("shift "));
        assert!(encoded.stages[1].label.contains("rails)"));
        assert!(
            decoded
                .stages
                .iter()
                .any(|stage| stage.label.contains("derived"))
        );
        assert_ne!(
            encode(&recipe, input, "mystery").unwrap().output,
            encoded.output
        );
        assert_ne!(
            decode(&recipe, &encoded.output, "mystery", &[])
                .unwrap()
                .output,
            input
        );
    }

    #[test]
    fn same_configuration_and_key_stay_deterministic_across_metadata_changes() {
        let recipe = sample();
        let mut copied = recipe.clone();
        copied.id = "user-another".into();
        copied.name = "Renamed".into();
        assert_eq!(
            encode(&recipe, "Puzzle text!", "mystery").unwrap().output,
            encode(&copied, "Puzzle text!", "mystery").unwrap().output
        );
    }

    #[test]
    fn missing_key_is_rejected() {
        let recipe = sample();
        assert!(encode(&recipe, "Hello", "").is_err());
        assert!(decode(&recipe, "Abc", "", &[]).is_err());
    }
}
