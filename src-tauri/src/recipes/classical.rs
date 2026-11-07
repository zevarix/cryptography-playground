//! Small, bounded adapters to the published cifers classical-cipher crate.
//! The recipe engine validates the schema, parameters, ASCII input and
//! passphrase before calling APIs that may panic on invalid affine values.
//!
//! All supported adapters are reversible on validated ASCII input. Rail Fence
//! intentionally transposes every character, including spaces and punctuation,
//! rather than dropping them as some historical demonstrations do.
use cifers::{Affine, Beaufort, Caeser, Cipher, Railfence, Vigenere};

use super::Rule;

pub(super) fn transform(
    rule: &Rule,
    text: &str,
    passphrase: &str,
    undo: bool,
) -> Result<String, String> {
    let transformed = match rule {
        Rule::Caesar { shift } if *shift <= 25 => {
            let cipher = Caeser::new().set_shift(i32::from(*shift));
            if undo {
                cipher.decipher(text)
            } else {
                cipher.encipher(text)
            }
        }
        Rule::Atbash => {
            let cipher = Affine::atbash();
            if undo {
                cipher.decipher(text)
            } else {
                cipher.encipher(text)
            }
        }
        Rule::Vigenere | Rule::Beaufort
            if !passphrase.is_empty()
                && passphrase.len() <= 64
                && passphrase.bytes().all(|b| b.is_ascii_lowercase()) =>
        {
            if matches!(rule, Rule::Vigenere) {
                let cipher = Vigenere::new().set_code(passphrase.to_owned());
                if undo {
                    cipher.decipher(text)
                } else {
                    cipher.encipher(text)
                }
            } else {
                let cipher = Beaufort::new().set_code(passphrase.to_owned());
                if undo {
                    cipher.decipher(text)
                } else {
                    cipher.encipher(text)
                }
            }
        }
        Rule::Affine { a, b } if *a >= 1 && *a <= 25 && a % 2 == 1 && a % 13 != 0 && *b <= 25 => {
            // The upstream setters assert on invalid values; guards above
            // match Recipe::validate's allowed affine parameter domain.
            let cipher = Affine::new().set_a(i32::from(*a)).set_b(i32::from(*b));
            if undo {
                cipher.decipher(text)
            } else {
                cipher.encipher(text)
            }
        }
        Rule::RailFence { rails } if (2..=16).contains(rails) => {
            let cipher = Railfence::new().set_key(*rails);
            if undo {
                cipher.decipher(text)
            } else {
                cipher.encipher(text)
            }
        }
        _ => return Err("Unsupported or invalid classical-cipher rule parameters.".into()),
    };
    Ok(transformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_test_vectors() {
        assert_eq!(
            transform(&Rule::Caesar { shift: 3 }, "Hello, World!", "", false).unwrap(),
            "Khoor, Zruog!"
        );
        assert_eq!(
            transform(&Rule::Atbash, "ABC xyz!", "", false).unwrap(),
            "ZYX cba!"
        );
        assert_eq!(
            transform(&Rule::Vigenere, "ATTACK AT DAWN!", "lemon", false).unwrap(),
            "LXFOPV EF RNHR!"
        );
        assert_eq!(
            transform(&Rule::Beaufort, "exampletext", "key", false).unwrap(),
            "ghyypnglunl"
        );
        assert_eq!(
            transform(&Rule::Affine { a: 5, b: 8 }, "AFFINE CIPHER", "", false).unwrap(),
            "IHHWVC SWFRCP"
        );
        assert_eq!(
            transform(
                &Rule::RailFence { rails: 3 },
                "WEAREDISCOVEREDFLEEATONCE",
                "",
                false
            )
            .unwrap(),
            "WECRLTEERDSOEEFEAOCAIVDEN"
        );
    }

    #[test]
    fn punctuation_and_case_roundtrip() {
        let sample = "The quick BROWN fox, 123! Next line.\n";
        for rule in [
            Rule::Caesar { shift: 13 },
            Rule::Atbash,
            Rule::Vigenere,
            Rule::Beaufort,
            Rule::Affine { a: 7, b: 12 },
            Rule::RailFence { rails: 3 },
        ] {
            let ciphertext = transform(&rule, sample, "lemon", false).unwrap();
            let recovered = transform(&rule, &ciphertext, "lemon", true).unwrap();
            assert_eq!(recovered, sample, "did not roundtrip: {rule:?}");
        }
    }

    #[test]
    fn invalid_upstream_parameters_are_rejected_before_panic() {
        assert!(transform(&Rule::Affine { a: 2, b: 8 }, "abc", "", false).is_err());
        assert!(transform(&Rule::Affine { a: 13, b: 8 }, "abc", "", false).is_err());
        assert!(transform(&Rule::Affine { a: 5, b: 26 }, "abc", "", false).is_err());
        assert!(transform(&Rule::RailFence { rails: 0 }, "abc", "", false).is_err());
        assert!(transform(&Rule::RailFence { rails: 17 }, "abc", "", false).is_err());
        assert!(transform(&Rule::Vigenere, "abc", "", false).is_err());
        assert!(transform(&Rule::Beaufort, "abc", "with spaces", false).is_err());
    }
}
