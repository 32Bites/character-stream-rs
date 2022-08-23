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

impl CharacterError {
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            CharacterError::NoBytesRead => None,
            CharacterError::Other { bytes, error: _ }
            | CharacterError::IoError { bytes, error: _ } => Some(&bytes),
        }
    }
}
