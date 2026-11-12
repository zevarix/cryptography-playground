//! One native, same-filesystem, no-replace directory publication primitive.
//! A preflight existence check plus std::fs::rename would clobber an empty
//! destination directory on Unix, so publication must use the native flag.
use std::{io, path::Path};

#[cfg(unix)]
fn native_path(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "A native path contains a NUL byte.",
        )
    })
}

#[cfg(target_vendor = "apple")]
pub(super) fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint};
    unsafe extern "C" {
        fn renamex_np(source: *const c_char, destination: *const c_char, flags: c_uint) -> c_int;
    }
    let source = native_path(source)?;
    let destination = native_path(destination)?;
    // SAFETY: both arguments are owned, NUL-terminated native path strings
    // alive for the call. The ABI and RENAME_EXCL=4 match Darwin sys/stdio.h.
    let result = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), 4) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
pub(super) fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint};
    unsafe extern "C" {
        fn renameat2(
            old_directory: c_int,
            source: *const c_char,
            new_directory: c_int,
            destination: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }
    let source = native_path(source)?;
    let destination = native_path(destination)?;
    // SAFETY: owned NUL-terminated paths remain alive for the call. Linux's
    // AT_FDCWD=-100 and RENAME_NOREPLACE=1 request one exclusive rename.
    let result = unsafe { renameat2(-100, source.as_ptr(), -100, destination.as_ptr(), 1) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub(super) fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    }
    let encode = |path: &Path| -> io::Result<Vec<u16>> {
        let mut path: Vec<_> = path.as_os_str().encode_wide().collect();
        if path.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A native path contains a NUL byte.",
            ));
        }
        path.push(0);
        Ok(path)
    };
    let source = encode(source)?;
    let destination = encode(destination)?;
    // SAFETY: both UTF-16 path buffers are NUL-terminated and alive for the
    // call. Flags=0 forbids replacement and cross-volume copy fallback.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) };
    if result != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux", windows)))]
pub(super) fn rename_exclusive(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Exclusive experiment publication is unavailable on this platform.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_publication_preserves_existing_empty_and_nonempty_targets() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("captured"), "original").unwrap();
        std::fs::create_dir(&target).unwrap();
        assert!(rename_exclusive(&source, &target).is_err());
        assert!(source.join("captured").exists());
        assert!(target.is_dir());
        assert!(!target.join("captured").exists());
        std::fs::write(target.join("human"), "keep").unwrap();
        assert!(rename_exclusive(&source, &target).is_err());
        assert_eq!(
            std::fs::read_to_string(target.join("human")).unwrap(),
            "keep"
        );
        std::fs::remove_file(target.join("human")).unwrap();
        std::fs::remove_dir(&target).unwrap();
        rename_exclusive(&source, &target).unwrap();
        assert_eq!(
            std::fs::read_to_string(target.join("captured")).unwrap(),
            "original"
        );
        assert!(!source.exists());
    }
}
