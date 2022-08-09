mod character_iter;
mod character_stream;
mod error;

pub use crate::character_stream::*;
pub use character_iter::*;
pub use error::*;

pub struct Peek;
pub struct MultiPeek;
