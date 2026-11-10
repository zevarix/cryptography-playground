//! Adapters to the ciphers crate with the recipe engine's ASCII contract.
//! Substitutions preserve case and nonletters around the upstream letters-only
//! API. Rail Fence transposes every byte, including spaces and punctuation.
//!
//! Validate parameters before calling upstream constructors that may panic.
use ciphers::{Affine, Atbash, Beaufort, Caesar, Cipher, RailFence, Vigenere};

use super::Rule;

pub(super) fn transform(
    rule: &Rule,
    text: &str,
    passphrase: &str,
    undo: bool,
) -> Result<String, String> {
    if !text.is_ascii() {
        return Err("Classical-cipher input must be ASCII.".into());
    }
    match rule {
        Rule::Caesar { shift } if *shift <= 25 => substitute(&Caesar::new(*shift), text, undo),
        Rule::Atbash => substitute(&Atbash::new(), text, undo),
        Rule::Vigenere | Rule::Beaufort
            if !passphrase.is_empty()
                && passphrase.len() <= 64
                && passphrase.bytes().all(|b| b.is_ascii_lowercase()) =>
        {
            if matches!(rule, Rule::Vigenere) {
                substitute(&Vigenere::new(passphrase), text, undo)
            } else {
                substitute(&Beaufort::new(passphrase), text, undo)
            }
        }
        Rule::Affine { a, b } if *a >= 1 && *a <= 25 && a % 2 == 1 && a % 13 != 0 && *b <= 25 => {
            substitute(&Affine::new(i32::from(*a), i32::from(*b)), text, undo)
        }
        Rule::RailFence { rails } if (2..=16).contains(rails) => {
            run_cipher(&RailFence::new(usize::from(*rails)), text, undo)
        }
        _ => Err("Unsupported or invalid classical-cipher rule parameters.".into()),
    }
}

fn run_cipher(cipher: &impl Cipher, text: &str, undo: bool) -> Result<String, String> {
    if undo {
        cipher.decipher(text)
    } else {
        cipher.encipher(text)
    }
    .map_err(|error| format!("Classical cipher rejected its input: {error:?}"))
}

fn substitute(cipher: &impl Cipher, text: &str, undo: bool) -> Result<String, String> {
    // Only letters advance the repeating key; punctuation keeps its position.
    let letters: String = text
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .map(|byte| char::from(byte.to_ascii_uppercase()))
        .collect();
    if letters.is_empty() {
        return Ok(text.to_owned());
    }
    let mapped = run_cipher(cipher, &letters, undo)?;
    if mapped.len() != letters.len() || !mapped.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err("Classical cipher returned an invalid substitution.".into());
    }
    let mut mapped = mapped.bytes();
    let mut output = String::with_capacity(text.len());
    for byte in text.bytes() {
        let letter = if byte.is_ascii_alphabetic() {
            let next = mapped.next().expect("substitution length was checked");
            if byte.is_ascii_lowercase() {
                next.to_ascii_lowercase()
            } else {
                next
            }
        } else {
            byte
        };
        output.push(char::from(letter));
    }
    Ok(output)
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

    #[test]
    fn substitutions_preserve_ascii_formatting_and_key_position() {
        let ascii = String::from_utf8((0..=127).collect()).unwrap();
        let shifted = transform(&Rule::Caesar { shift: 1 }, &ascii, "", false).unwrap();
        assert_eq!(
            transform(&Rule::Vigenere, &ascii, "b", false).unwrap(),
            shifted
        );
        for (input, output) in ascii.bytes().zip(shifted.bytes()) {
            if !input.is_ascii_alphabetic() {
                assert_eq!(output, input);
            }
        }
        for rule in [Rule::Atbash, Rule::Beaufort, Rule::Affine { a: 5, b: 8 }] {
            let ciphertext = transform(&rule, &ascii, "key", false).unwrap();
            assert_eq!(transform(&rule, &ciphertext, "key", true).unwrap(), ascii);
            assert_eq!(
                transform(&rule, "123!\t\n", "key", false).unwrap(),
                "123!\t\n"
            );
        }
    }

    #[test]
    fn rail_fence_preserves_short_inputs_and_every_ascii_byte() {
        let ascii = String::from_utf8((0..=127).collect()).unwrap();
        for rails in 2..=16 {
            for sample in ["", "a", "A!", "A1 !b\n", ascii.as_str()] {
                let rule = Rule::RailFence { rails };
                let ciphertext = transform(&rule, sample, "", false).unwrap();
                assert_eq!(
                    transform(&rule, &ciphertext, "", true).unwrap(),
                    sample,
                    "rails={rails}, bytes={}",
                    sample.len()
                );
            }
        }
        assert_eq!(
            transform(&Rule::RailFence { rails: 3 }, "A1 !b\n", "", false).unwrap(),
            "Ab1!\n "
        );
    }
}
