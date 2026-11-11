//! Strict UTF-8 scalar I/O with fixed working buffers and checked counters.
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Write},
    path::Path,
};

pub const BUFFER: usize = 64 * 1024;

pub fn io_error(operation: &str, error: std::io::Error) -> String {
    format!("{operation}: {error}")
}
pub fn add(value: u64, increment: u64) -> Result<u64, String> {
    value
        .checked_add(increment)
        .ok_or_else(|| "Document size or offset overflow.".into())
}
pub fn digest_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

pub struct ScalarReader<R: Read> {
    input: BufReader<R>,
    pub bytes: u64,
}
impl<R: Read> ScalarReader<R> {
    pub fn into_inner(self) -> R {
        self.input.into_inner()
    }
    pub fn new(input: R) -> Self {
        Self {
            input: BufReader::with_capacity(BUFFER, input),
            bytes: 0,
        }
    }
    pub fn next(&mut self) -> Result<Option<char>, String> {
        let mut bytes = [0_u8; 4];
        if self
            .input
            .read(&mut bytes[..1])
            .map_err(|e| io_error("Cannot read document", e))?
            == 0
        {
            return Ok(None);
        }
        let width = match bytes[0] {
            0..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Err(format!("Invalid UTF-8 at byte {}.", self.bytes)),
        };
        self.input
            .read_exact(&mut bytes[1..width])
            .map_err(|_| format!("Incomplete UTF-8 at byte {}.", self.bytes))?;
        let scalar = std::str::from_utf8(&bytes[..width])
            .map_err(|_| format!("Invalid UTF-8 at byte {}.", self.bytes))?
            .chars()
            .next()
            .ok_or("Missing UTF-8 scalar.")?;
        self.bytes = add(
            self.bytes,
            u64::try_from(width).map_err(|_| "UTF-8 width overflow.")?,
        )?;
        Ok(Some(scalar))
    }
}
impl ScalarReader<File> {
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self::new(File::open(path).map_err(|e| {
            io_error("Cannot open document artifact", e)
        })?))
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ArtifactStats {
    pub bytes: u64,
    pub scalars: u64,
    pub digest: String,
}

pub struct ArtifactWriter {
    file: File,
    buffer: Vec<u8>,
    hash: Sha256,
    bytes: u64,
    scalars: u64,
}
impl ArtifactWriter {
    pub fn create(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| io_error("Cannot create document artifact", e))?;
        Ok(Self {
            file,
            buffer: Vec::with_capacity(BUFFER + 4),
            hash: Sha256::new(),
            bytes: 0,
            scalars: 0,
        })
    }
    pub fn push(&mut self, scalar: char) -> Result<(), String> {
        let mut encoded = [0; 4];
        let text = scalar.encode_utf8(&mut encoded);
        self.bytes = add(
            self.bytes,
            u64::try_from(text.len()).map_err(|_| "UTF-8 length overflow.")?,
        )?;
        self.scalars = add(self.scalars, 1)?;
        self.buffer.extend_from_slice(text.as_bytes());
        if self.buffer.len() >= BUFFER {
            self.flush()?;
        }
        Ok(())
    }
    pub fn text(&mut self, text: &str) -> Result<(), String> {
        for scalar in text.chars() {
            self.push(scalar)?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        self.file
            .write_all(&self.buffer)
            .map_err(|e| io_error("Cannot write document artifact", e))?;
        self.hash.update(&self.buffer);
        self.buffer.clear();
        Ok(())
    }
    pub fn finish(mut self) -> Result<ArtifactStats, String> {
        self.flush()?;
        Ok(ArtifactStats {
            bytes: self.bytes,
            scalars: self.scalars,
            digest: digest_hex(&self.hash.finalize()),
        })
    }
}

/// Capture actual UTF-8 artifact contents at the completed-result boundary.
pub(super) fn capture_stats(
    path: &Path,
    control: &super::Control,
) -> Result<ArtifactStats, String> {
    let mut reader = ScalarReader::open(path)?;
    let mut hash = Sha256::new();
    let mut scalars = 0_u64;
    let mut buffer = [0_u8; 4];
    while let Some(scalar) = reader.next()? {
        hash.update(scalar.encode_utf8(&mut buffer).as_bytes());
        scalars = scalars.checked_add(1).ok_or("Artifact length overflow.")?;
        if scalars.is_multiple_of(4096) {
            control.check()?;
        }
    }
    control.check()?;
    Ok(ArtifactStats {
        bytes: reader.bytes,
        scalars,
        digest: digest_hex(&hash.finalize()),
    })
}
