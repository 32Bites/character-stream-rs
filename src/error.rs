use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
/// Type to represent stream errors.
pub enum CharacterError {
    #[error("Failed to read bytes from the stream.")]
    NoBytesRead,
    #[error("An IO error occurred on bytes {:?}: {}", .bytes, .error)]
    IoError { bytes: Vec<u8>, error: io::Error },

    #[error("An error occurred on bytes {:?}: {}", .bytes, .error)]
    Other {
        bytes: Vec<u8>,
        error: anyhow::Error,
    },
}
