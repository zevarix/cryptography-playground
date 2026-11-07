//! Full-inventory US-English phoneme experiment (not secure encryption).
mod recovery;
use serde::Serialize;
use std::{collections::HashMap, sync::OnceLock};

const MAX_BYTES: usize = 16_384;
const VOWEL_OUTPUTS: &[u8; 5] = b"aeiou";
static DICT: OnceLock<HashMap<String, Vec<&'static str>>> = OnceLock::new();

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encoded {
    pub ciphertext: String,
    pub intermediate: String,
    pub words: usize,
    pub phonemes: usize,
}
#[derive(Debug, Serialize)]
pub struct Decoded {
    pub intermediate: String,
    pub reconstruction: recovery::Reconstruction,
}

fn key(s: &str) -> Result<String, String> {
    let s = s.trim().to_ascii_lowercase();
    if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err("Key must be 1-64 letters A-Z without spaces.".into());
    }
    Ok(s)
}
fn tokens(s: &str) -> Result<Vec<String>, String> {
    if s.len() > MAX_BYTES {
        return Err("Maximum input size is 16 KiB.".into());
    }
    if s.chars().any(|c| !c.is_ascii() && c.is_alphabetic()) {
        return Err("Only ASCII English words are supported.".into());
    }
    let words: Vec<_> = s
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|p| !p.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if words.is_empty() {
        return Err("Enter at least one word.".into());
    }
    Ok(words)
}
fn dictionary() -> &'static HashMap<String, Vec<&'static str>> {
    DICT.get_or_init(|| {
        let mut out = HashMap::new();
        for line in include_str!("../data/cmudict.dict").lines() {
            let mut parts = line.split_whitespace();
            let Some(word) = parts.next() else { continue };
            if word.starts_with(';') || word.contains('(') {
                continue;
            }
            let sounds: Vec<_> = parts.take_while(|p| !p.starts_with('#')).collect();
            if !sounds.is_empty() {
                out.entry(word.to_ascii_lowercase()).or_insert(sounds);
            }
        }
        out
    })
}
fn phone(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_ascii_digit())
}
fn is_vowel(s: &str) -> bool {
    matches!(
        s,
        "AA" | "AE"
            | "AH"
            | "AO"
            | "AW"
            | "AY"
            | "EH"
            | "ER"
            | "EY"
            | "IH"
            | "IY"
            | "OW"
            | "OY"
            | "UH"
            | "UW"
    )
}
fn choices(s: &str) -> Option<&'static [&'static str]> {
    Some(match s {
        "B" => &["b", "bb"],
        "CH" => &["ch", "tch"],
        "D" => &["d", "dd"],
        "DH" => &["th", "dh"],
        "F" => &["f", "ph", "gh"],
        "G" => &["g", "gg"],
        "HH" => &["h", "wh"],
        "JH" => &["j", "g", "dge"],
        "K" => &["k", "c", "ck", "q"],
        "L" => &["l", "ll"],
        "M" => &["m", "mm"],
        "N" => &["n", "kn", "gn"],
        "NG" => &["ng", "n"],
        "P" => &["p", "pp"],
        "R" => &["r", "wr"],
        "S" => &["s", "c", "sc"],
        "SH" => &["sh", "ch", "ti"],
        "T" => &["t", "tt"],
        "TH" => &["th", "t"],
        "V" => &["v", "ve"],
        "W" => &["w", "wh"],
        "Y" => &["y", "i"],
        "Z" => &["z", "s"],
        "ZH" => &["s", "si", "zh"],
        _ => return None,
    })
}
fn initial(key: &str, family: &str, size: usize) -> usize {
    let hash = key
        .bytes()
        .chain(family.bytes())
        .fold(2_166_136_261_u64, |h, b| {
            h.wrapping_mul(16_777_619).wrapping_add(u64::from(b))
        });
    (hash % size as u64) as usize
}
fn shift(s: &str, key: &str, undo: bool) -> Result<String, String> {
    let mut index = 0_usize;
    let mut result = Vec::new();
    for (wi, w) in tokens(s)?.iter().enumerate() {
        let mut output = String::new();
        for (li, b) in w.bytes().enumerate() {
            let k = usize::from(key.as_bytes()[index % key.len()] - b'a');
            let n = (3 + wi + 1 + li + 1 + k) % 26;
            let v = usize::from(b - b'a');
            let v = if undo {
                (v + 26 - n) % 26
            } else {
                (v + n) % 26
            };
            output.push(char::from(b'a' + v as u8));
            index += 1;
        }
        result.push(output);
    }
    Ok(result.join(" "))
}
pub fn encode(plaintext: &str, passphrase: &str) -> Result<Encoded, String> {
    let key = key(passphrase)?;
    let words = tokens(plaintext)?;
    let mut counters: HashMap<&str, usize> = HashMap::new();
    let mut vowel_count = 0_usize;
    let mut phonemes = 0_usize;
    let start = usize::from(key.as_bytes()[0] - b'a') % VOWEL_OUTPUTS.len();
    let mut output_words = Vec::new();
    for word in &words {
        let pronunciation = dictionary().get(word.as_str()).ok_or_else(|| {
            format!("Missing pronunciation for '{word}' in bundled US-English dictionary.")
        })?;
        let mut result = String::new();
        let mut previous_vowel = false;
        for raw in pronunciation {
            phonemes += 1;
            let p = phone(raw);
            if is_vowel(p) {
                if !previous_vowel {
                    result.push(char::from(VOWEL_OUTPUTS[(start + vowel_count) % 5]));
                    vowel_count += 1;
                }
                previous_vowel = true;
            } else {
                let spellings = choices(p).ok_or_else(|| format!("Unmapped phoneme {p}"))?;
                let count = counters.entry(p).or_default();
                let offset = (initial(&key, p, spellings.len()) + *count) % spellings.len();
                result.push_str(spellings[offset]);
                *count += 1;
                previous_vowel = false;
            }
        }
        output_words.push(result);
    }
    let intermediate = output_words.join(" ");
    let ciphertext = shift(&intermediate, &key, false)?;
    Ok(Encoded {
        ciphertext,
        intermediate,
        words: words.len(),
        phonemes,
    })
}
pub fn decode(ciphertext: &str, passphrase: &str) -> Result<Decoded, String> {
    let key = key(passphrase)?;
    if ciphertext
        .chars()
        .any(|c| !c.is_ascii_alphabetic() && !c.is_ascii_whitespace())
    {
        return Err("Ciphertext must contain only letters and whitespace.".into());
    }
    let intermediate = shift(ciphertext, &key, true)?;
    let reconstruction = recovery::reconstruct(&intermediate, &key);
    Ok(Decoded {
        intermediate,
        reconstruction,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_phonemes_in_bundled_dictionary_are_supported() {
        for pronunciation in dictionary().values() {
            for p in pronunciation {
                let p = phone(p);
                assert!(is_vowel(p) || choices(p).is_some(), "Unknown phoneme: {p}");
            }
        }
    }
    #[test]
    fn round_trip_recovers_intermediate_not_plaintext() {
        let e = encode(
            "The quick brown fox jumps over the lazy dog.",
            "spectacular",
        )
        .unwrap();
        let d = decode(&e.ciphertext, "spectacular").unwrap();
        assert_eq!(e.intermediate, d.intermediate);
        assert_eq!(e.words, 9);
    }
    #[test]
    fn errors_are_explicit() {
        assert!(encode("zzzznoneword", "secret").is_err());
        assert!(encode("Hello", "").is_err());
        assert!(decode("!", "secret").is_err());
    }
}
