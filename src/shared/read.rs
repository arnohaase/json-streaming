use core::error::Error;
use core::fmt::{Display, Formatter};
use core::str::{FromStr, Utf8Error};
use core::marker::PhantomData;

#[derive(Debug, PartialEq, Eq)]
pub enum JsonReadToken<'a> {
    StartObject,
    EndObject,
    StartArray,
    EndArray,

    Key(&'a str),
    StringLiteral(&'a str),
    NumberLiteral(JsonNumber<'a>),
    BooleanLiteral(bool),
    NullLiteral,

    EndOfStream,
}
impl <'a> JsonReadToken<'a> {
    pub fn kind(&self) -> &'static str {
        match self {
            JsonReadToken::StartObject => "{",
            JsonReadToken::EndObject => "}",
            JsonReadToken::StartArray => "[",
            JsonReadToken::EndArray => "]",
            JsonReadToken::Key(_) => "key",
            JsonReadToken::StringLiteral(_) => "string",
            JsonReadToken::NumberLiteral(_) => "number",
            JsonReadToken::BooleanLiteral(_) => "boolean",
            JsonReadToken::NullLiteral => "null",
            JsonReadToken::EndOfStream => "<EOF>",
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub struct JsonNumber<'a>(pub &'a str);
impl <'a> JsonNumber<'a> {
    pub fn parse<F: FromStr>(&self) -> Result<F, F::Err> {
        self.0.parse()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Location {
    /// in bytes, not characters - aligned with how Rust counts offsets in strings
    pub offset: usize,
    pub line: usize,
    /// in bytes, not characters - aligned with how Rust counts offsets in strings
    pub column: usize,
}
impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {}, column {} (offset {})", self.line, self.column, self.offset)
    }
}
impl Location {
    pub fn start() -> Location {
        Location {
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn after_byte(&mut self, byte: u8) {
        self.offset += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        }
        else {
            self.column += 1;
        }
    }
}


#[derive(Debug)]
pub enum JsonParseError<E: Error> {
    Io(E),
    Utf8(Utf8Error),
    Parse(&'static str, Location),
    UnexpectedToken(&'static str, Location), //TODO kind of token
    BufferOverflow(Location),
}
impl <E: Error> Display for JsonParseError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            JsonParseError::Io(err) => write!(f, "I/O error: {}", err),
            JsonParseError::Utf8(err) => write!(f, "Invalid UTF8: {}", err),
            JsonParseError::Parse(msg, location) => write!(f, "parse error: {} @ {}", msg, location),
            JsonParseError::UnexpectedToken(kind, location) => write!(f, "unexpected token '{}' @ {}", kind, location),
            JsonParseError::BufferOverflow(location) => write!(f, "buffer overflow @ {}", location),
        }
    }
}

impl <E: Error> Error for JsonParseError<E> {
}
impl <E: Error> From<E> for JsonParseError<E> {
    fn from(value: E) -> Self {
        JsonParseError::Io(value)
    }
}


pub type JsonParseResult<T, E> = Result<T, JsonParseError<E>>;


/// Simple state tracking to handle those parts of the grammar that require only local context. That
///  is essentially everything except the distinction between objects and arrays.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ReaderState {
    /// Immediately after a nested object or array starts. This needs separate handling from
    ///  'BeforeEntry' to reject trailing commas in objects and arrays
    Initial,
    /// Ready to accept the current container's next entry, i.e. a value (for arrays) or a key/value
    ///  pair (for objects)
    BeforeEntry,
    /// After a key, i.e. values are the only valid follow-up
    AfterKey,
    /// After a value, i.e. a comma or the closing bracket of the current container is expected
    AfterValue,
}

pub(crate) struct ReaderInner<B: AsMut<[u8]>, E: Error> {
    pub buf: B,
    pub ind_end_buf: usize,
    pub lenient_comma_handling: bool,
    pub state: ReaderState,
    pub parked_next: Option<u8>,
    pub cur_location: Location,
    pd: PhantomData<E>,
}
impl <B: AsMut<[u8]>, E: Error> ReaderInner<B, E> {
    pub fn new(buf: B, lenient_comma_handling: bool) -> Self {
        Self {
            buf,
            ind_end_buf: 0,
            lenient_comma_handling,
            state: ReaderState::Initial,
            parked_next: None,
            cur_location: Location::start(),
            pd: PhantomData,
        }
    }

    pub fn append_to_buf(&mut self, ch: u8) -> JsonParseResult<(), E> {
        if self.ind_end_buf >= self.buf.as_mut().len() {
            return self.buf_overflow();
        }
        self.buf.as_mut()[self.ind_end_buf] = ch;
        self.ind_end_buf += 1;
        Ok(())
    }

    /// see https://de.wikipedia.org/wiki/UTF-8
    pub fn append_code_point(&mut self, cp: u16) -> JsonParseResult<(), E> {
        match cp {
            0x0000..=0x007F => {
                self.append_to_buf(cp as u8)
            }
            0x0080..=0x07FF => {
                self.append_to_buf(0xC0 | ((cp >> 6) as u8 & 0x1F))?;
                self.append_to_buf(0x80 | ( cp       as u8 & 0x3F))
            }
            _ => { // 0x00800..0xffff
                self.append_to_buf(0xE0 | ((cp >> 12) as u8 & 0x0F))?;
                self.append_to_buf(0x80 | ((cp >>  6) as u8 & 0x3F))?;
                self.append_to_buf(0x80 | ( cp        as u8 & 0x3F))
            }
        }
    }

    pub fn buf_as_str(&mut self) -> JsonParseResult<&str, E> {
        // the reference is used only immutably, but all callers have a mutable refrence anyway
        //  and calling as_mut() avoids the need for another type bound
        core::str::from_utf8(
            &self.buf.as_mut()[..self.ind_end_buf])
            .map_err(|e| JsonParseError::Utf8(e))
    }

    pub fn ensure_accept_value(&mut self) -> JsonParseResult<(), E> {
        match self.state {
            ReaderState::Initial |
            ReaderState::BeforeEntry |
            ReaderState::AfterKey => {
                Ok(())
            }
            ReaderState::AfterValue => {
                if self.lenient_comma_handling {
                    Ok(())
                }
                else {
                    self.parse_err("missing comma")
                }
            }
        }
    }

    pub fn ensure_accept_end_nested(&mut self) -> JsonParseResult<(), E> {
        match self.state {
            ReaderState::Initial |
            ReaderState::AfterValue => {
                Ok(())
            }
            ReaderState::BeforeEntry => {
                self.parse_err("trailing comma")
            }
            ReaderState::AfterKey => {
                self.parse_err("key without a value")
            }
        }
    }

    pub fn state_change_for_value(&mut self) -> JsonParseResult<(), E> {
        match self.state {
            ReaderState::Initial |
            ReaderState::BeforeEntry |
            ReaderState::AfterKey => {
                self.state = ReaderState::AfterValue;
                Ok(())
            }
            ReaderState::AfterValue => {
                self.parse_err("missing comma")
            }
        }
    }

    pub fn on_comma(&mut self) -> JsonParseResult<(), E> {
        match self.state {
            ReaderState::AfterValue => {
                self.state = ReaderState::BeforeEntry;
                Ok(())
            }
            ReaderState::Initial |
            ReaderState::BeforeEntry |
            ReaderState::AfterKey => {
                self.parse_err("unexpected comma")
            }
        }
    }

    pub fn parse_err<T>(&self, msg: &'static str) -> JsonParseResult<T, E> {
        Err(JsonParseError::Parse(msg, self.cur_location))
    }

    pub fn buf_overflow<T>(&self) -> JsonParseResult<T, E> {
        Err(JsonParseError::BufferOverflow(self.cur_location))
    }
}
