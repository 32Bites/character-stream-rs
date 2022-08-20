use std::{
    collections::VecDeque,
    error::Error,
    fs::File,
    io::{self, BufReader, Cursor, Read},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use anyhow::anyhow;

use crate::{CharacterError, CharacterIterator, MultiPeek, Peek, INTERRUPTED_MAX};

pub trait Peekable<T> {
    fn peek(&mut self) -> Option<&T>;
}

pub trait MultiPeekable<T> {
    fn peek(&mut self) -> Option<&T>;
    fn reset_peek(&mut self);
}

pub trait CharStream {
    fn read_char(&mut self) -> CharacterStreamResult;
    fn is_lossy(&self) -> bool;
}

/// A result that contains a parsed character or a [CharacterStreamError].
pub type CharacterStreamResult = Result<char, CharacterError>;
/// Wrapper struct for any stream that implements [BufRead](std::io::BufRead) and [Seek](std::io::Seek).
///
/// It allows you to read in bytes from a stream, and attempt to parse them into characters.
///
/// These bytes however, must be valid UTF-8 code points.
///
/// This wrapper does NOT parse graphemes.
pub struct CharacterStream<Reader: Read> {
    /// The stream from which the incoming bytes are from.
    pub stream: Reader,
    /// Whether or not we should care whether invalid bytes are detected.
    ///
    /// If `true`, then invalid byte sequences will be replaced with a U+FFFD.
    ///
    /// If `false`, then an error will be returned.
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

impl<Reader: Read> CharacterStream<Reader> {
    /// Create a [CharacterStream] from a stream.
    ///
    /// Set `is_lossy` to `true` if you don't want to handle invalid byte sequences.
    pub fn new(stream: Reader, is_lossy: bool) -> Self {
        Self { stream, is_lossy }
    }

    /// Kinda builder pattern.
    pub fn lossy(mut self, is_lossy: bool) -> Self {
        self.is_lossy = is_lossy;
        self
    }

    /// Wrap `self` into a single-peek [PeekableCharacterStream].
    pub fn peeky(self) -> PeekableCharacterStream<Reader, Peek> {
        self.into()
    }

    /// Wrap `self` into a multi-peek [PeekableCharacterStream].
    pub fn peeky_multi(self) -> PeekableCharacterStream<Reader, MultiPeek> {
        self.into()
    }

    /// Reads a set amount of bytes from the stream.
    ///
    /// Set `amount` to the amount of bytes you would like to read.
    ///
    /// Upon success, a [`Vec<u8>`] is returned, holding the read bytes.
    ///
    /// Upon failure, an [error](CharacterError) is returned.
    pub fn read_bytes(&mut self, amount: usize) -> Result<Vec<u8>, CharacterError> {
        let handle = (&mut self.stream).take(amount as u64);
        let result: Vec<Result<u8, io::Error>> = handle.bytes().collect();
        let bytes: Vec<u8> = result
            .iter()
            .filter_map(|r| match r {
                Ok(b) => Some(*b),
                _ => None,
            })
            .collect();
        let error = result.into_iter().find_map(|r| match r {
            Err(error) => Some(error),
            _ => None,
        });

        match error {
            Some(error) => Err(CharacterError::IoError { bytes, error }),
            None => {
                let len = bytes.len();
                if len == 0 {
                    Err(CharacterError::NoBytesRead)
                } else if len != amount {
                    Err(CharacterError::Other {
                        bytes,
                        error: anyhow!("Failed to read the specified amount of bytes."),
                    })
                } else {
                    Ok(bytes)
                }
            }
        }
    }

    /// Reads a singluar byte from the stream.
    pub fn read_byte(&mut self) -> Result<u8, CharacterError> {
        Ok(self.read_bytes(1)?[0])
    }
}

impl<Reader: Read> CharStream for CharacterStream<Reader> {
    /// Attempts to read a character from the stream.
    ///
    /// If `is_lossy` is set to `true`, then invalid byte sequences will be a U+FFFD.
    ///
    /// If `is_lossy` is set to `false`, then invalid byte sequences will be returned in addition to a parse error.
    fn read_char(&mut self) -> CharacterStreamResult {
        match self.read_byte() {
            Ok(read_byte) => match remaining_byte_count(read_byte) {
                Some(remaining_count) => {
                    let mut bytes = vec![read_byte];
                    if remaining_count > 0 {
                        bytes.extend(self.read_bytes(remaining_count)?);
                    }

                    let chars: Vec<char> = if self.is_lossy {
                        String::from_utf8_lossy(&bytes).to_string()
                    } else {
                        match String::from_utf8(bytes.clone()) {
                            Ok(string) => string,
                            Err(error) => {
                                return Err(CharacterError::Other {
                                    bytes,
                                    error: anyhow!(error),
                                })
                            }
                        }
                    }
                    .chars()
                    .collect();

                    let len = chars.len();

                    if len == 1 {
                        Ok(chars[0])
                    } else {
                        Err(CharacterError::Other {
                            bytes,
                            error: anyhow!(format!("Expected 1 character, not {}", len)),
                        })
                    }
                }
                None => {
                    if self.is_lossy {
                        Ok('\u{FFFD}')
                    } else {
                        Err(CharacterError::Other {
                            bytes: vec![read_byte],
                            error: anyhow!("Invalid starting byte"),
                        })
                    }
                }
            },
            Err(error) => return Err(error),
        }
    }

    fn is_lossy(&self) -> bool {
        self.is_lossy
    }
}

impl<Reader: std::fmt::Debug + Read> std::fmt::Debug for CharacterStream<Reader> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<Reader: Read> Deref for CharacterStream<Reader> {
    type Target = Reader;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<Reader: Read> DerefMut for CharacterStream<Reader> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<Reader: Read> AsRef<Reader> for CharacterStream<Reader> {
    fn as_ref(&self) -> &Reader {
        &*self
    }
}

impl<Reader: Read> AsMut<Reader> for CharacterStream<Reader> {
    fn as_mut(&mut self) -> &mut Reader {
        &mut *self
    }
}

impl<Reader: Read> From<Reader> for CharacterStream<Reader> {
    fn from(reader: Reader) -> Self {
        Self::new(reader, false)
    }
}

pub struct PeekableCharacterStream<Reader: Read, PI> {
    pub stream: CharacterStream<Reader>,
    pub buffer: VecDeque<CharacterStreamResult>,
    pub position: usize,
    _phantom: PhantomData<PI>,
}

impl<Reader: Read, PI> PeekableCharacterStream<Reader, PI> {
    pub fn new(stream: Reader, is_lossy: bool) -> Self {
        Self {
            stream: CharacterStream::new(stream, is_lossy),
            buffer: VecDeque::new(),
            position: 0,
            _phantom: PhantomData,
        }
    }

    pub fn from_stream(stream: CharacterStream<Reader>) -> Self {
        Self {
            stream,
            buffer: VecDeque::new(),
            position: 0,
            _phantom: PhantomData,
        }
    }
    
    #[inline]
    fn _read_char(&mut self) -> CharacterStreamResult {
        self.buffer
            .pop_front()
            .unwrap_or_else(|| self.stream.read_char())
    }
}

impl<Reader: Read, PI> From<CharacterStream<Reader>> for PeekableCharacterStream<Reader, PI> {
    fn from(stream: CharacterStream<Reader>) -> Self {
        Self::from_stream(stream)
    }
}

impl<Reader: Read> Peekable<CharacterStreamResult> for PeekableCharacterStream<Reader, Peek> {
    fn peek(&mut self) -> Option<&CharacterStreamResult> {
        if self.buffer.len() == 1 {
            return self.buffer.front();
        }

        let character_result = self.read_char();
        self.buffer.push_back(character_result);

        self.buffer.front()
    }
}

impl<Reader: Read> MultiPeekable<CharacterStreamResult>
    for PeekableCharacterStream<Reader, MultiPeek>
{
    fn peek(&mut self) -> Option<&CharacterStreamResult> {
        let ret = if self.position < self.buffer.len() {
            Some(&self.buffer[self.position])
        } else {
            match self.stream.read_char() {
                Err(CharacterError::NoBytesRead) => None,
                o => {
                    self.buffer.push_back(o);
                    Some(&self.buffer[self.position])
                }
            }
        };

        self.position += 1;
        ret
    }

    fn reset_peek(&mut self) {
        self.position = 0;
    }
}

impl<Reader: Read> CharStream for PeekableCharacterStream<Reader, Peek> {
    fn read_char(&mut self) -> CharacterStreamResult {
        self._read_char()
    }

    fn is_lossy(&self) -> bool {
        self.stream.is_lossy
    }
}

impl<Reader: Read> CharStream for PeekableCharacterStream<Reader, MultiPeek> {
    fn read_char(&mut self) -> CharacterStreamResult {
        self.reset_peek();
        self._read_char()
    }

    fn is_lossy(&self) -> bool {
        self.stream.is_lossy
    }
}

/// Helper trait for converting values into a [CharacterStream].
pub trait ToCharacterStream<Reader: Read> {
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
pub trait TryToCharacterStream<Reader: Read> {
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

impl<Reader: Read> IntoIterator for CharacterStream<Reader> {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = CharacterIterator<Self>;

    fn into_iter(self) -> Self::IntoIter {
        CharacterIterator::new(self, INTERRUPTED_MAX)
    }
}

impl<Reader: Read> IntoIterator for PeekableCharacterStream<Reader, Peek> {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = CharacterIterator<Self>;

    fn into_iter(self) -> Self::IntoIter {
        CharacterIterator::new(self, INTERRUPTED_MAX)
    }
}

impl<Reader: Read> IntoIterator for PeekableCharacterStream<Reader, MultiPeek> {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = CharacterIterator<Self>;

    fn into_iter(self) -> Self::IntoIter {
        CharacterIterator::new(self, INTERRUPTED_MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossy_test() {
        let mut character_stream =
            b"These are valid characters \xF0\x9F\x92\xBB \xF0\x9F\x92\xBB \xF0\x9F\x92\xBB! The following bytes are not valid:\x80\xFF"
                .to_character_stream_lossy().peeky_multi();

        loop {
            match character_stream.read_char() {
                Ok(c) => {
                    println!("{:X?}; Next: {:?}", c, character_stream.peek());
                }
                Err(error) => match &error {
                    CharacterError::IoError {
                        bytes: _,
                        error: err,
                    } => {
                        let kind = err.kind();
                        if kind == std::io::ErrorKind::UnexpectedEof {
                            break;
                        } else {
                            panic!("{}", error)
                        }
                    }
                    CharacterError::NoBytesRead => break,
                    error => panic!("{}", error),
                },
            }
        }

        println!();
    }
}
