//! Rust-owned UTF-8 document processing; IPC carries only bounded previews.
pub(crate) mod activities;
pub(crate) mod commands;
pub(crate) mod export;
pub(crate) mod jobs;
pub(crate) mod storage;
pub(crate) mod stream;
pub(crate) mod transform;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct Control {
    cancelled: AtomicBool,
    pub bytes: AtomicU64,
}
impl Control {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("Operation cancelled.".into())
        } else {
            Ok(())
        }
    }
    pub fn observe(&self, bytes: u64) -> Result<(), String> {
        self.bytes.store(bytes, Ordering::Relaxed);
        self.check()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lifecycle_tests;

#[cfg(test)]
mod export_tests;

#[cfg(test)]
mod activity_tests;
