use std::{
    error::Error,
    io::{Read, Seek}, iter::FusedIterator,
};

use crate::{CharacterStream, CharacterStreamResult, ToCharacterStream, TryToCharacterStream};
/// The maximum amount of [Interrupted](std::io::ErrorKind::Interrupted) errors before the iterator gives up.
///
/// I know this will not show up in rustdoc, however I do feel as if it should be documented, even if it's within the source code.
pub const INTERRUPTED_MAXIMUM: usize = 5;

/// Iterator over a [CharacterStream](crate::CharacterStream)
pub struct CharacterIterator<Reader: Read> {
    /// The stream to iterate over.
    pub(crate) stream: CharacterStream<Reader>,
    /// A measure of the amount of [Interrupted](std::io::ErrorKind::Interrupted) errors.
    pub(crate) interrupted_count: usize,
}

impl<Reader: Read> CharacterIterator<Reader> {
    /// Create a iterator from a [CharacterStream](crate::CharacterStream)
    pub fn new(stream: CharacterStream<Reader>) -> Self {
        Self {
            stream,
            interrupted_count: 0,
        }
    }

    /// Return a reference to the underlying stream.
    pub fn stream(&self) -> &Reader {
        &self.stream
    }

    /// Return a mutable reference to the underlying stream.
    pub fn stream_mut(&mut self) -> &mut Reader {
        &mut self.stream
    }

    /// Peek a character from the stream.
    pub fn peek(&mut self) -> Option<CharacterStreamResult>
    where
        Reader: Seek,
    {
        self.stream.peek_char().ok()
    }

    /// Is the character parser lossy?
    pub fn is_lossy(&self) -> bool {
        self.stream.is_lossy
    }
}

impl<Reader: Read + std::fmt::Debug> std::fmt::Debug for CharacterIterator<Reader> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CharacterIterator").field("stream", &self.stream).field("interrupted_count", &self.interrupted_count).finish()
    }
}

impl<Reader: Read> Iterator for CharacterIterator<Reader> {
    type Item = CharacterStreamResult;

    fn next(&mut self) -> Option<Self::Item> {
        match self.stream.read_char() {
            Ok(character) => {
                if self.interrupted_count > 0 {
                    self.interrupted_count = 0;
                }

                Some(character)
            }
            Err(error) => match error.kind() {
                std::io::ErrorKind::Interrupted => {
                    if self.interrupted_count <= INTERRUPTED_MAXIMUM {
                        self.interrupted_count += 1;
                        self.next()
                    } else {
                        None
                    }
                }
                std::io::ErrorKind::UnexpectedEof => None,
                _ => {
                    println!("An unknown error has occurred: {}", error);
                    None
                }
            },
        }
    }
}

impl<Reader: Read> FusedIterator for CharacterIterator<Reader> {}

/// Trait for easy conversion of a type into a [CharacterIterator].
pub trait ToCharacterIterator<Reader: Read> {
    /// Convert into a [CharacterIterator].
    fn to_character_iterator(&self) -> CharacterIterator<Reader>;

    /// Convert into a lossy [CharacterIterator].
    fn to_character_iterator_lossy(&self) -> CharacterIterator<Reader>;
}

impl<Reader: Read, T: ToCharacterStream<Reader>> ToCharacterIterator<Reader> for T {
    fn to_character_iterator(&self) -> CharacterIterator<Reader> {
        self.to_character_stream().into_iter()
    }

    fn to_character_iterator_lossy(&self) -> CharacterIterator<Reader> {
        self.to_character_stream_lossy().into_iter()
    }
}

/// Trait for easy conversion of a type into a [CharacterIterator] with a potential for failure.
pub trait TryToCharacterIterator<Reader: Read> {
    /// Attempt to convert into a [CharacterIterator].
    fn try_to_character_iterator(&self) -> Result<CharacterIterator<Reader>, Box<dyn Error>>;

    /// Attempt to convert into a lossy [CharacterIterator].
    fn try_to_character_iterator_lossy(&self) -> Result<CharacterIterator<Reader>, Box<dyn Error>>;
}

impl<Reader: Read, T: TryToCharacterStream<Reader>> TryToCharacterIterator<Reader> for T {
    fn try_to_character_iterator(&self) -> Result<CharacterIterator<Reader>, Box<dyn Error>> {
        Ok(self.try_to_character_stream()?.into_iter())
    }

    fn try_to_character_iterator_lossy(&self) -> Result<CharacterIterator<Reader>, Box<dyn Error>> {
        Ok(self.try_to_character_stream_lossy()?.into_iter())
    }
}
