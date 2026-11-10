//! Document-wide transforms. A buffer boundary never resets recipe state.
use super::{
    Control,
    stream::{ArtifactStats, ArtifactWriter, BUFFER, ScalarReader, add, io_error},
};
use crate::{
    engine,
    recipes::{Rule, classical},
};
use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};
use tempfile::NamedTempFile;

pub fn transform(
    rule: &Rule,
    source: &Path,
    destination: &Path,
    key: &str,
    undo: bool,
    control: &Control,
) -> Result<ArtifactStats, String> {
    transform_known(rule, source, destination, key, undo, control, None)
}

pub fn transform_known(
    rule: &Rule,
    source: &Path,
    destination: &Path,
    key: &str,
    undo: bool,
    control: &Control,
    scalars: Option<u64>,
) -> Result<ArtifactStats, String> {
    control.check()?;
    control.begin(
        "Transforming text",
        Some(
            std::fs::metadata(source)
                .map_err(|e| io_error("Cannot inspect transformation input", e))?
                .len(),
        ),
    )?;
    let mut input = ScalarReader::open(source)?;
    let mut output = ArtifactWriter::create(destination)?;
    let parent = destination.parent().ok_or("Missing artifact directory.")?;
    match rule {
        Rule::RailFence { rails } if (2..=16).contains(rails) => rail(
            &mut input,
            source,
            &mut output,
            RailOptions {
                rails: *rails,
                undo,
                known: scalars,
                parent,
            },
            control,
        )?,
        Rule::WordMirror => mirror(&mut input, &mut output, parent, control)?,
        Rule::Caesar { .. }
        | Rule::Atbash
        | Rule::Vigenere
        | Rule::Beaufort
        | Rule::Affine { .. } => substitute(rule, &mut input, &mut output, key, undo, control)?,
        Rule::PhoneticRotate if !undo => phonetic(&mut input, &mut output, key, control)?,
        Rule::PairSwap
        | Rule::RotateVowels { .. }
        | Rule::PositionShift { .. }
        | Rule::RemoveVowels
        | Rule::CollapseDoubles => letters(rule, &mut input, &mut output, key, undo, control)?,
        _ => return Err("Unsupported document transformation or unresolved derived rule.".into()),
    }
    control.check()?;
    output.finish()
}

fn check(input: &ScalarReader<File>, control: &Control, counter: &mut u16) -> Result<(), String> {
    *counter += 1;
    if *counter == 4096 {
        *counter = 0;
        control.observe(input.bytes)?;
    }
    Ok(())
}

fn substitute(
    rule: &Rule,
    input: &mut ScalarReader<File>,
    output: &mut ArtifactWriter,
    key: &str,
    undo: bool,
    control: &Control,
) -> Result<(), String> {
    let keyed = matches!(rule, Rule::Vigenere | Rule::Beaufort);
    if keyed && (key.is_empty() || key.len() > 64 || !key.bytes().all(|c| c.is_ascii_lowercase())) {
        return Err("This recipe needs a passphrase of 1-64 English letters.".into());
    }
    let mut phase = 0;
    loop {
        control.observe(input.bytes)?;
        let mut block = String::with_capacity(BUFFER + 4);
        while block.len() < BUFFER {
            let Some(scalar) = input.next()? else {
                break;
            };
            block.push(scalar);
        }
        if block.is_empty() {
            break;
        }
        let letters: String = block
            .chars()
            .filter(char::is_ascii_alphabetic)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        let rotated = if keyed {
            format!("{}{}", &key[phase..], &key[..phase])
        } else {
            String::new()
        };
        let mapped = classical::transform(rule, &letters, &rotated, undo)?;
        let mut mapped = mapped.chars();
        for scalar in block.chars() {
            if scalar.is_ascii_alphabetic() {
                let next = mapped.next().ok_or("Incomplete cipher substitution.")?;
                output.push(if scalar.is_ascii_lowercase() {
                    next.to_ascii_lowercase()
                } else {
                    next
                })?;
            } else {
                output.push(scalar)?;
            }
        }
        if keyed {
            phase = (phase + letters.len() % key.len()) % key.len();
        }
    }
    Ok(())
}

fn vowel(scalar: char) -> bool {
    scalar.is_ascii() && b"aeiou".contains(&(scalar as u8).to_ascii_lowercase())
}
fn case(scalar: char, lower: u8) -> char {
    char::from(if scalar.is_ascii_uppercase() {
        lower.to_ascii_uppercase()
    } else {
        lower
    })
}

fn letters(
    rule: &Rule,
    input: &mut ScalarReader<File>,
    output: &mut ArtifactWriter,
    key: &str,
    undo: bool,
    control: &Control,
) -> Result<(), String> {
    if undo && matches!(rule, Rule::RemoveVowels | Rule::CollapseDoubles) {
        return Err("Lossy rules have no exact inverse.".into());
    }
    let mut word = 0_u8;
    let mut letter = 0_u8;
    let mut in_word = false;
    let mut phase = 0;
    let mut previous = '\0';
    let mut counter = 0;
    if matches!(rule, Rule::PositionShift { keyed: true, .. })
        && (key.is_empty() || key.len() > 64 || !key.bytes().all(|c| c.is_ascii_lowercase()))
    {
        return Err("This recipe needs a passphrase of 1-64 English letters.".into());
    }
    while let Some(scalar) = input.next()? {
        check(input, control, &mut counter)?;
        let lower = scalar.to_ascii_lowercase();
        let mapped = match rule {
            Rule::RemoveVowels if vowel(scalar) => continue,
            Rule::CollapseDoubles => {
                if scalar.is_ascii_alphabetic() && !vowel(scalar) && lower == previous {
                    continue;
                }
                previous = if scalar.is_ascii_alphabetic() {
                    lower
                } else {
                    '\0'
                };
                scalar
            }
            Rule::PairSwap if scalar.is_ascii_alphabetic() => case(
                scalar,
                match lower {
                    'b' => b'p',
                    'p' => b'b',
                    'd' => b't',
                    't' => b'd',
                    'g' => b'k',
                    'k' => b'g',
                    'v' => b'f',
                    'f' => b'v',
                    'z' => b's',
                    's' => b'z',
                    _ => lower as u8,
                },
            ),
            Rule::RotateVowels { steps } if *steps <= 4 && vowel(scalar) => {
                let at = b"aeiou"
                    .iter()
                    .position(|b| *b == lower as u8)
                    .ok_or("Invalid vowel.")?;
                let n = usize::from(*steps);
                case(
                    scalar,
                    b"aeiou"[if undo { (at + 5 - n) % 5 } else { (at + n) % 5 }],
                )
            }
            Rule::PositionShift { base, keyed } if *base <= 25 => {
                if !scalar.is_ascii_alphabetic() {
                    in_word = false;
                    letter = 0;
                    scalar
                } else {
                    if !in_word {
                        word = (word + 1) % 26;
                        in_word = true;
                    }
                    letter = (letter + 1) % 26;
                    let k = if *keyed {
                        key.as_bytes()[phase] - b'a'
                    } else {
                        0
                    };
                    let distance =
                        (u16::from(*base) + u16::from(word) + u16::from(letter) + u16::from(k))
                            % 26;
                    let value = u16::from(lower as u8 - b'a');
                    let next = if undo {
                        (value + 26 - distance) % 26
                    } else {
                        (value + distance) % 26
                    };
                    if *keyed {
                        phase = (phase + 1) % key.len();
                    }
                    case(
                        scalar,
                        b'a' + u8::try_from(next).map_err(|_| "Shift overflow.")?,
                    )
                }
            }
            Rule::RotateVowels { steps } if *steps > 4 => {
                return Err("Invalid vowel rotation.".into());
            }
            Rule::PositionShift { .. } => return Err("Invalid position shift.".into()),
            _ => scalar,
        };
        output.push(mapped)?;
    }
    Ok(())
}

fn mirror(
    input: &mut ScalarReader<File>,
    output: &mut ArtifactWriter,
    parent: &Path,
    control: &Control,
) -> Result<(), String> {
    let mut word = Vec::with_capacity(BUFFER);
    let mut spool: Option<NamedTempFile> = None;
    let mut counter = 0;
    loop {
        let scalar = input.next()?;
        check(input, control, &mut counter)?;
        if scalar.is_some_and(|c| c.is_ascii_alphabetic()) {
            word.push(scalar.ok_or("Missing word letter.")? as u8);
            if word.len() == BUFFER {
                if spool.is_none() {
                    spool = Some(
                        NamedTempFile::new_in(parent)
                            .map_err(|e| io_error("Cannot spool word", e))?,
                    );
                }
                spool
                    .as_mut()
                    .ok_or("Missing word spool.")?
                    .write_all(&word)
                    .map_err(|e| io_error("Cannot spool word", e))?;
                word.clear();
            }
            continue;
        }
        let was_spooled = spool.is_some();
        if let Some(mut file) = spool.take() {
            file.write_all(&word)
                .map_err(|e| io_error("Cannot finish word spool", e))?;
            let mut remaining = file
                .as_file()
                .metadata()
                .map_err(|e| io_error("Cannot inspect word spool", e))?
                .len();
            control.begin("Reversing long word", None)?;
            while remaining > 0 {
                control.check()?;
                let length = usize::try_from(
                    remaining.min(u64::try_from(BUFFER).map_err(|_| "Buffer size overflow.")?),
                )
                .map_err(|_| "Word chunk overflow.")?;
                remaining -= u64::try_from(length).map_err(|_| "Word length overflow.")?;
                file.seek(SeekFrom::Start(remaining))
                    .map_err(|e| io_error("Cannot seek word spool", e))?;
                word.resize(length, 0);
                file.read_exact(&mut word)
                    .map_err(|e| io_error("Cannot read word spool", e))?;
                for byte in word.iter().rev() {
                    output.push(char::from(*byte))?;
                }
            }
        } else {
            for byte in word.iter().rev() {
                output.push(char::from(*byte))?;
            }
        }
        if was_spooled {
            control.begin("Scanning words", None)?;
        }
        word.clear();
        if let Some(scalar) = scalar {
            output.push(scalar)?;
        } else {
            break;
        }
    }
    Ok(())
}

fn rail_index(at: u64, rails: u8) -> usize {
    let last = u64::from(rails - 1);
    let position = at % (last * 2);
    usize::try_from(if position <= last {
        position
    } else {
        last * 2 - position
    })
    .expect("rail index is at most 15")
}

struct RailOptions<'a> {
    rails: u8,
    undo: bool,
    known: Option<u64>,
    parent: &'a Path,
}
fn rail(
    input: &mut ScalarReader<File>,
    source: &Path,
    output: &mut ArtifactWriter,
    options: RailOptions<'_>,
    control: &Control,
) -> Result<(), String> {
    let RailOptions {
        rails,
        undo,
        known,
        parent,
    } = options;
    let mut files: Vec<NamedTempFile> = (0..rails)
        .map(|_| NamedTempFile::new_in(parent).map_err(|e| io_error("Cannot create rail spool", e)))
        .collect::<Result<_, _>>()?;
    let mut writers: Vec<_> = files
        .iter_mut()
        .map(|f| BufWriter::with_capacity(BUFFER, f.as_file_mut()))
        .collect();
    let mut total = 0_u64;
    let mut counter = 0;
    if undo {
        total = if let Some(total) = known {
            total
        } else {
            control.begin("Counting rail characters", None)?;
            let mut total = 0;
            while input.next()?.is_some() {
                check(input, control, &mut counter)?;
                total = add(total, 1)?;
            }
            total
        };
        control.begin("Distributing rail characters", None)?;
        *input = ScalarReader::open(source)?;
        let cycle = u64::from(rails - 1) * 2;
        let mut counts = vec![total / cycle; usize::from(rails)];
        for count in counts.iter_mut().take(usize::from(rails - 1)).skip(1) {
            *count = count.checked_mul(2).ok_or("Rail count overflow.")?;
        }
        for at in 0..total % cycle {
            let index = rail_index(at, rails);
            counts[index] = add(counts[index], 1)?;
        }
        for (index, count) in counts.into_iter().enumerate() {
            for _ in 0..count {
                check(input, control, &mut counter)?;
                let scalar = input.next()?.ok_or("Rail input ended early.")?;
                let mut bytes = [0; 4];
                writers[index]
                    .write_all(scalar.encode_utf8(&mut bytes).as_bytes())
                    .map_err(|e| io_error("Cannot write rail spool", e))?;
            }
        }
        if input.next()?.is_some() {
            return Err("Rail input has unexpected extra data.".into());
        }
    } else {
        while let Some(scalar) = input.next()? {
            check(input, control, &mut counter)?;
            let mut bytes = [0; 4];
            writers[rail_index(total, rails)]
                .write_all(scalar.encode_utf8(&mut bytes).as_bytes())
                .map_err(|e| io_error("Cannot write rail spool", e))?;
            total = add(total, 1)?;
        }
    }
    control.begin("Flushing rail buffers", None)?;
    for writer in &mut writers {
        writer
            .flush()
            .map_err(|e| io_error("Cannot finish rail spool", e))?;
    }
    drop(writers);
    let mut readers = files
        .iter_mut()
        .map(|file| {
            file.seek(SeekFrom::Start(0))
                .map_err(|e| io_error("Cannot rewind rail spool", e))?;
            Ok(ScalarReader::new(file.as_file_mut()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    control.begin("Assembling rail result", None)?;
    if undo {
        for at in 0..total {
            if at % 4096 == 0 {
                control.check()?;
            }
            output.push(
                readers[rail_index(at, rails)]
                    .next()?
                    .ok_or("Rail spool ended early.")?,
            )?;
        }
    } else {
        for reader in &mut readers {
            while let Some(scalar) = reader.next()? {
                check(input, control, &mut counter)?;
                output.push(scalar)?;
            }
        }
    }
    Ok(())
}

fn phonetic(
    input: &mut ScalarReader<File>,
    output: &mut ArtifactWriter,
    key: &str,
    control: &Control,
) -> Result<(), String> {
    let mut state = engine::DocumentPhonetic::new(key)?;
    let mut word = String::new();
    let mut has_words = false;
    let mut counter = 0;
    loop {
        let scalar = input.next()?;
        check(input, control, &mut counter)?;
        if scalar.is_some_and(|c| !c.is_ascii() && c.is_alphabetic()) {
            return Err("Phonetic rotation supports English ASCII words only.".into());
        }
        if let Some(letter) = scalar.filter(char::is_ascii_alphabetic) {
            if word.len() >= engine::document_word_limit() {
                return Err(
                    "Word is outside the bundled pronunciation dictionary vocabulary.".into(),
                );
            }
            word.push(letter.to_ascii_lowercase());
            continue;
        }
        if !word.is_empty() {
            let mapped = state.word(&word)?;
            if has_words {
                output.push(' ')?;
            }
            output.text(&mapped)?;
            word.clear();
            has_words = true;
        }
        if scalar.is_none() {
            break;
        }
    }
    if !has_words {
        return Err("Phonetic rotation requires at least one English word.".into());
    }
    Ok(())
}
