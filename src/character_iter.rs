use std::{error::Error, io::Read};

use crate::{
    CharStream, CharacterStream, CharacterStreamResult, MultiPeek, MultiPeekable, Peek, Peekable,
    PeekableCharacterStream, ToCharacterStream, TryToCharacterStream,
};

pub(crate) const INTERRUPTED_MAX: usize = 5;

/// Iterator over a [CharacterStream](crate::CharacterStream)
pub struct CharacterIterator<Stream: CharStream> {
    /// The stream to iterate over.
    pub(crate) stream: Stream,
    /// Maximum amount of [Interrupted](std::io::ErrorKind::Interrupted) errors.
    pub(crate) interrupted_max: usize,
    pub interrupted_count: usize,
}

impl<Stream: CharStream> CharacterIterator<Stream> {
    /// Create a iterator from a [CharacterStream](crate::CharacterStream)
    pub fn new(stream: Stream, interrupted_max: usize) -> Self {
        Self {
            stream,
            interrupted_max,
            interrupted_count: 0,
        }
    }

    /// Return a reference to the underlying stream.
    pub fn stream(&self) -> &Stream {
        &self.stream
    }

    /// Return a mutable reference to the underlying stream.
    pub fn stream_mut(&mut self) -> &mut Stream {
        &mut self.stream
    }

    /// Is the character parser lossy?
    pub fn is_lossy(&self) -> bool {
        self.stream.is_lossy()
    }
}

impl<Reader: Read> CharacterIterator<CharacterStream<Reader>> {
    /// Make the underlying stream peekable.
    pub fn peek(self) -> CharacterIterator<PeekableCharacterStream<Reader, Peek>> {
        CharacterIterator::new(self.stream.peeky(), self.interrupted_max)
    }

    /// Make the underlying stream multi-peekable
    pub fn peek_multi(self) -> CharacterIterator<PeekableCharacterStream<Reader, MultiPeek>> {
        CharacterIterator::new(self.stream.peeky_multi(), self.interrupted_max)
    }
}

impl<Reader: Read> CharacterIterator<PeekableCharacterStream<Reader, Peek>> {
    /// Peek the next character in the stream.
    pub fn peek(&mut self) -> Option<&<Self as Iterator>::Item> {
        self.stream.peek()
    }
}

impl<Reader: Read> CharacterIterator<PeekableCharacterStream<Reader, MultiPeek>> {
    /// Peek the next character in the stream. (multi-peek)
    pub fn peek(&mut self) -> Option<&<Self as Iterator>::Item> {
        self.stream.peek()
    }

    pub fn reset_peek(&mut self) {
        self.stream.reset_peek()
    }
}

impl<Stream: CharStream + std::fmt::Debug> std::fmt::Debug for CharacterIterator<Stream> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<Stream: CharStream> Iterator for CharacterIterator<Stream> {
    type Item = CharacterStreamResult;

    fn next(&mut self) -> Option<Self::Item> {
        match self.stream.read_char() {
            Ok(character) => {
                if self.interrupted_count > 0 {
                    self.interrupted_count = 0;
                }

                Some(Ok(character))
            }
            Err(error) => match error {
                crate::CharacterError::NoBytesRead => None,
                crate::CharacterError::IoError {
                    bytes: _,
                    error: ref err,
                } => match err.kind() {
                    std::io::ErrorKind::Interrupted => {
                        if self.interrupted_count <= self.interrupted_max {
                            self.interrupted_count += 1;
                            self.next()
                        } else {
                            None
                        }
                    }
                    std::io::ErrorKind::UnexpectedEof => None,
                    _ => Some(Err(error)),
                },
                other => Some(Err(other)),
            },
        }
    }
}

// impl<Stream: CharStream> FusedIterator for CharacterIterator<Stream> {}

/// Trait for easy conversion of a type into a [CharacterIterator].
pub trait ToCharacterIterator<Reader: Read> {
    /// Convert into a [CharacterIterator].
    fn to_character_iterator(&self) -> CharacterIterator<CharacterStream<Reader>>;

    /// Convert into a lossy [CharacterIterator].
    fn to_character_iterator_lossy(&self) -> CharacterIterator<CharacterStream<Reader>>;
}

impl<Reader: Read, T: ToCharacterStream<Reader>> ToCharacterIterator<Reader> for T {
    fn to_character_iterator(&self) -> CharacterIterator<CharacterStream<Reader>> {
        self.to_character_stream().into_iter()
    }

    fn to_character_iterator_lossy(&self) -> CharacterIterator<CharacterStream<Reader>> {
        self.to_character_stream_lossy().into_iter()
    }
}

/// Trait for easy conversion of a type into a [CharacterIterator] with a potential for failure.
pub trait TryToCharacterIterator<Reader: Read> {
    /// Attempt to convert into a [CharacterIterator].
    fn try_to_character_iterator(
        &self,
    ) -> Result<CharacterIterator<CharacterStream<Reader>>, Box<dyn Error>>;

    /// Attempt to convert into a lossy [CharacterIterator].
    fn try_to_character_iterator_lossy(
        &self,
    ) -> Result<CharacterIterator<CharacterStream<Reader>>, Box<dyn Error>>;
}

impl<Reader: Read, T: TryToCharacterStream<Reader>> TryToCharacterIterator<Reader> for T {
    fn try_to_character_iterator(
        &self,
    ) -> Result<CharacterIterator<CharacterStream<Reader>>, Box<dyn Error>> {
        Ok(self.try_to_character_stream()?.into_iter())
    }

    fn try_to_character_iterator_lossy(
        &self,
    ) -> Result<CharacterIterator<CharacterStream<Reader>>, Box<dyn Error>> {
        Ok(self.try_to_character_stream_lossy()?.into_iter())
    }
}
