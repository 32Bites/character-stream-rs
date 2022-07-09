use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{BufRead, BufReader, Cursor, Error as IoError, Seek, SeekFrom},
    ops::{Deref, DerefMut},
};

use crate::CharacterIterator;

/// The output type of a [CharacterStream]'s [read_char](CharacterStream::read_char) method.

/// An error that represents a UTF-8 parse failure.
/// Holds the bytes that it failed to parse, and the corresponding error;
#[derive(Debug)]
pub struct CharacterStreamError(pub Vec<u8>, pub Box<dyn Error + Send + Sync>);

impl Display for CharacterStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed when parsing bytes {:?}: {:?}", self.0, self.1)
    }
}

impl Error for CharacterStreamError {}

/// A result that contains a parsed character or a [CharacterStreamError].
pub type CharacterStreamResult = Result<char, CharacterStreamError>;
/// Wrapper struct for any stream that implements [BufRead](std::io::BufRead) and [Seek](std::io::Seek).
///
/// It allows you to read in bytes from a stream, and attempt to parse them into characters.
///
/// These bytes however, must be valid UTF-8 code points.
///
/// This wrapper does NOT parse graphemes.
pub struct CharacterStream<Reader: BufRead + Seek> {
    /// The stream from which the incoming bytes are from.
    pub stream: Reader,
    /// Whether or not we should care whether invalid bytes are detected.
    ///
    /// If `true`, then invalid byte sequences will be replaced with a U+FFFD.
    ///
    /// If `false`, then [Failure](CharacterStreamResult::Failue) will be the returned result.
    pub is_lossy: bool,
}

fn remaining_byte_count(byte: u8) -> Option<usize> {
    let count = if (byte >> 7) == 0 {
        // Single byte character
        0
    } else if (byte >> 5) == 6 {
        // Two byte character
        1
    } else if (byte >> 4) == 14 {
        // Three byte character
        2
    } else if (byte >> 3) == 30 {
        // Four byte character
        3
    } else {
        return None;
    };

    Some(count)
}

impl<Reader: BufRead + Seek> CharacterStream<Reader> {
    /// Create a [CharacterStream] from a stream.
    ///
    /// The created [CharacterStream] will not be lossy.
    pub fn from(stream: Reader) -> Self {
        Self::new(stream, false)
    }

    /// Create a [CharacterStream] from a stream.
    ///
    /// Set `is_lossy` to `true` if you don't want to handle invalid byte sequences.
    pub fn new(stream: Reader, is_lossy: bool) -> Self {
        Self { stream, is_lossy }
    }

    /// Reads a set amount of bytes from the stream.
    ///
    /// Set `up_to` to the amount of bytes you would like to read.
    ///
    /// Upon success, a [`Vec<u8>`] is returned, holding the read bytes.
    ///
    /// Upon failure, an [error](std::io::Error) is returned.
    pub fn read_bytes(&mut self, up_to: usize) -> Result<Vec<u8>, IoError> {
        let mut buffer = vec![0u8; up_to];
        self.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    /// Does exactly what [read_bytes](Self::read_bytes) performs,
    /// the difference being it seeks back to the position before the read,
    /// serving as a lookahead function.
    pub fn peek_bytes(&mut self, up_to: usize) -> Result<Vec<u8>, IoError> {
        let current_position = self.stream_position()?;
        let bytes = self.read_bytes(up_to)?;
        self.seek(SeekFrom::Start(current_position))?;
        Ok(bytes)
    }

    /// Reads a singluar byte from the stream.
    pub fn read_byte(&mut self) -> Result<u8, IoError> {
        Ok(self.read_bytes(1)?[0])
    }

    /// Does exactly what [read_byte](Self::read_byte) performs,
    /// the difference being it seeks back to the position before the read,
    /// serving as a lookahead function.
    pub fn peek_byte(&mut self) -> Result<u8, IoError> {
        Ok(self.peek_bytes(1)?[0])
    }

    /// Attempts to read a character from the stream.
    ///
    /// If `is_lossy` is set to `true`, then invalid byte sequences will be a U+FFFD.
    ///
    /// If `is_lossy` is set to `false`, then invalid byte sequences will be returned in addition to a parse error.
    pub fn read_char(&mut self) -> Result<CharacterStreamResult, IoError> {
        match self.read_byte() {
            Ok(read_byte) => match remaining_byte_count(read_byte) {
                Some(remaining_count) => {
                    let mut bytes = vec![read_byte];
                    bytes.extend(self.read_bytes(remaining_count)?);

                    let chars: Vec<char> = if self.is_lossy {
                        String::from_utf8_lossy(&bytes).to_string()
                    } else {
                        match String::from_utf8(bytes.clone()) {
                            Ok(string) => string,
                            Err(error) => {
                                return Ok(Err(CharacterStreamError(bytes, error.into())))
                            }
                        }
                    }
                    .chars()
                    .collect();

                    let len = chars.len();

                    if len == 1 {
                        Ok(Ok(chars[0]))
                    } else {
                        Ok(Err(CharacterStreamError(
                            bytes,
                            format!("Expected 1 character, not {}", len).into(),
                        )))
                    }
                }
                None => {
                    if self.is_lossy {
                        Ok(Ok('\u{FFFD}'))
                    } else {
                        Ok(Err(CharacterStreamError(
                            vec![read_byte],
                            "Invalid starting byte".into(),
                        )))
                    }
                }
            },
            Err(error) => return Err(error),
        }
    }

    /// Performs the same action as [read_char](Self::read_char), the difference being,
    /// it seeks back to the position prior to the read.
    pub fn peek_char(&mut self) -> Result<CharacterStreamResult, IoError> {
        let current_position = self.stream_position()?;
        let result = self.read_char();
        self.seek(SeekFrom::Start(current_position))?;

        result
    }
}

impl<Reader: BufRead + Seek> Deref for CharacterStream<Reader> {
    type Target = Reader;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<Reader: BufRead + Seek> DerefMut for CharacterStream<Reader> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<Reader: BufRead + Seek> AsRef<Reader> for CharacterStream<Reader> {
    fn as_ref(&self) -> &Reader {
        &self.stream
    }
}

impl<Reader: BufRead + Seek> AsMut<Reader> for CharacterStream<Reader> {
    fn as_mut(&mut self) -> &mut Reader {
        &mut self.stream
    }
}

impl<Reader: BufRead + Seek> IntoIterator for CharacterStream<Reader> {
    type Item = CharacterStreamResult;
    type IntoIter = CharacterIterator<Reader>;

    fn into_iter(self) -> Self::IntoIter {
        CharacterIterator::new(self)
    }
}

/// Helper trait for converting values into a [CharacterStream].
pub trait ToCharacterStream<Reader: BufRead + Seek> {
    /// Convert into a [CharacterStream].
    fn to_character_stream(&self) -> CharacterStream<Reader>;

    /// Convert into a lossy [CharacterStream].
    fn to_character_stream_lossy(&self) -> CharacterStream<Reader>;
}

impl<T: AsRef<[u8]>> ToCharacterStream<Cursor<Vec<u8>>> for T {
    fn to_character_stream(&self) -> CharacterStream<Cursor<Vec<u8>>> {
        CharacterStream::from(Cursor::new(self.as_ref().to_vec()))
    }

    fn to_character_stream_lossy(&self) -> CharacterStream<Cursor<Vec<u8>>> {
        CharacterStream::new(Cursor::new(self.as_ref().to_vec()), true)
    }
}

/// Helper trait for converting values into a [CharacterStream], with a potential for failure.
pub trait TryToCharacterStream<Reader: BufRead + Seek> {
    /// Attempt to convert into a [CharacterStream].
    fn try_to_character_stream(&self) -> Result<CharacterStream<Reader>, Box<dyn Error>>;

    /// Attempt to convert into a lossy [CharacterStream].
    fn try_to_character_stream_lossy(&self) -> Result<CharacterStream<Reader>, Box<dyn Error>>;
}

impl TryToCharacterStream<BufReader<File>> for File {
    fn try_to_character_stream(&self) -> Result<CharacterStream<BufReader<File>>, Box<dyn Error>> {
        let file = self.try_clone()?;
        Ok(CharacterStream::from(BufReader::new(file)))
    }

    fn try_to_character_stream_lossy(
        &self,
    ) -> Result<CharacterStream<BufReader<File>>, Box<dyn Error>> {
        let file = self.try_clone()?;
        Ok(CharacterStream::new(BufReader::new(file), true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossy_test() {
        let mut character_stream =
            b"These are valid characters \xF0\x9F\x92\xBB \xF0\x9F\x92\xBB \xF0\x9F\x92\xBB! The following bytes are not valid:\x80\xFF"
                .to_character_stream_lossy();

        loop {
            match character_stream.read_char() {
                Ok(result) => match result {
                    Ok(c) => print!("{}", c),
                    Err(CharacterStreamError(_, _)) => unreachable!(),
                },
                Err(error) => {
                    let kind = error.kind();
                    if kind == std::io::ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        panic!("{}", error)
                    }
                }
            }
        }

        println!();
    }
}
