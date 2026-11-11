//! Owned snapshots and byte-addressed UTF-8 previews. Native paths stay here.
use super::{
    Control,
    stream::{ArtifactStats, ArtifactWriter, ScalarReader, add, digest_hex, io_error},
};
use same_file::Handle;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: String,
    pub name: String,
    pub encoding: &'static str,
    pub bytes: String,
    pub digest: String,
    pub bom: bool,
    pub legacy_compatible: bool,
}

pub(super) struct Snapshot {
    pub _root: TempDir,
    pub source: PathBuf,
    pub canonical_source: PathBuf,
    pub payload: PathBuf,
    pub stats: ArtifactStats,
    pub info: DocumentInfo,
}

struct HashingReader<R> {
    input: R,
    hash: Sha256,
}
impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let read = self.input.read(bytes)?;
        self.hash.update(&bytes[..read]);
        Ok(read)
    }
}

fn verify_source(
    source: &Path,
    handle: &Handle,
    before: &Metadata,
    after: &Metadata,
) -> Result<(), String> {
    let current =
        Handle::from_path(source).map_err(|e| io_error("Cannot recheck source identity", e))?;
    if *handle != current
        || before.len() != after.len()
        || before
            .modified()
            .map_err(|e| io_error("Cannot inspect source modification time", e))?
            != after
                .modified()
                .map_err(|e| io_error("Cannot recheck source modification time", e))?
    {
        return Err(
            "The source changed while it was being opened. Open it again after editing finishes."
                .into(),
        );
    }
    Ok(())
}

pub(super) fn capture(
    source: &Path,
    session: &Path,
    control: &Control,
) -> Result<Snapshot, String> {
    control.check()?;
    let file = File::open(source).map_err(|e| io_error("Cannot open source file", e))?;
    let before = file
        .metadata()
        .map_err(|e| io_error("Cannot inspect source file", e))?;
    if !before.is_file() {
        return Err("Choose a regular text file.".into());
    }
    control.begin("Reading UTF-8 source", Some(before.len()))?;
    let identity = Handle::from_file(
        file.try_clone()
            .map_err(|e| io_error("Cannot retain source handle", e))?,
    )
    .map_err(|e| io_error("Cannot inspect source identity", e))?;
    let canonical_source = source
        .canonicalize()
        .map_err(|e| io_error("Cannot resolve source location", e))?;
    let root = tempfile::Builder::new()
        .prefix("document-")
        .tempdir_in(session)
        .map_err(|e| io_error("Cannot create snapshot directory", e))?;
    let id = root
        .path()
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Cannot identify snapshot.")?
        .to_owned();
    let payload = root.path().join("payload");
    let mut input = ScalarReader::new(HashingReader {
        input: file,
        hash: Sha256::new(),
    });
    let mut output = ArtifactWriter::create(&payload)?;
    let mut bom = false;
    let mut ascii = true;
    let mut first = true;
    let mut counter = 0_u16;
    while let Some(scalar) = input.next()? {
        if first && scalar == '\u{feff}' {
            bom = true;
        } else {
            ascii &= scalar.is_ascii();
            output.push(scalar)?;
        }
        first = false;
        counter += 1;
        if counter == 4096 {
            counter = 0;
            control.observe(input.bytes)?;
        }
    }
    control.observe(input.bytes)?;
    let bytes = input.bytes;
    let hashing = input.into_inner();
    let after = hashing
        .input
        .metadata()
        .map_err(|e| io_error("Cannot recheck captured source", e))?;
    verify_source(source, &identity, &before, &after)?;
    let digest = digest_hex(&hashing.hash.finalize());
    let stats = output.finish()?;
    let info = DocumentInfo {
        id,
        name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        encoding: "UTF-8",
        bytes: bytes.to_string(),
        digest,
        bom,
        legacy_compatible: ascii
            && stats.bytes > 0
            && stats.bytes
                <= u64::try_from(crate::recipes::MAX_INPUT_BYTES)
                    .map_err(|_| "Legacy size overflow.")?,
    };
    Ok(Snapshot {
        _root: root,
        source: source.to_owned(),
        canonical_source,
        payload,
        stats,
        info,
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub text: String,
    pub offset: String,
    pub next_offset: String,
    pub bytes: String,
    pub eof: bool,
}

pub(super) fn preview(path: &Path, offset: u64) -> Result<Preview, String> {
    let mut file = File::open(path).map_err(|e| io_error("Cannot open preview artifact", e))?;
    let bytes = file
        .metadata()
        .map_err(|e| io_error("Cannot inspect preview artifact", e))?
        .len();
    if offset > bytes {
        return Err("Preview offset is outside the document.".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| io_error("Cannot seek preview artifact", e))?;
    let mut reader = ScalarReader::new(file);
    let mut text = String::new();
    while text.len() < 16384 {
        let Some(scalar) = reader.next()? else {
            break;
        };
        text.push(scalar);
    }
    let next_offset = add(offset, reader.bytes)?;
    Ok(Preview {
        text,
        offset: offset.to_string(),
        next_offset: next_offset.to_string(),
        bytes: bytes.to_string(),
        eof: next_offset == bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_verification_detects_replacement_and_edits() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::write(&source, "before").unwrap();
        let file = File::open(&source).unwrap();
        let before = file.metadata().unwrap();
        let handle = Handle::from_file(file.try_clone().unwrap()).unwrap();
        std::fs::write(&source, "after with different length").unwrap();
        assert!(verify_source(&source, &handle, &before, &file.metadata().unwrap()).is_err());
        std::fs::rename(&source, root.path().join("old")).unwrap();
        std::fs::write(&source, "before").unwrap();
        assert!(verify_source(&source, &handle, &before, &before).is_err());
    }
}
