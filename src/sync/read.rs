use core::error::Error;
use core::fmt::{Display, Formatter};
use core::str::{FromStr, Utf8Error};
use std::io;
use std::io::Read;

#[derive(Debug, PartialEq, Eq)]
pub enum JsonReadEvent<'a> {
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

#[derive(Debug, PartialEq, Eq)]
pub struct JsonNumber<'a>(&'a str);
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
pub enum JsonParseError {
    Io(io::Error),
    Utf8(Utf8Error),
    Parse(&'static str, Location),
    UnexpectedEvent(Location), //TODO event kind
    BufferOverflow(Location),
}
impl Display for JsonParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonParseError::Io(err) => write!(f, "I/O error: {}", err),
            JsonParseError::Utf8(err) => write!(f, "Invalid UTF8: {}", err),
            JsonParseError::Parse(msg, location) => write!(f, "parse error: {} @ {}", msg, location),
            JsonParseError::UnexpectedEvent(location) => write!(f, "unexpected event @ {}", location),
            JsonParseError::BufferOverflow(location) => write!(f, "buffer overflow @ {}", location),
        }
    }
}

impl Error for JsonParseError {
}
impl From<io::Error> for JsonParseError {
    fn from(value: io::Error) -> Self {
        JsonParseError::Io(value)
    }
}
impl From<Utf8Error> for JsonParseError {
    fn from(value: Utf8Error) -> Self {
        JsonParseError::Utf8(value)
    }
}

type ParseResult<T> = Result<T, JsonParseError>;


/// Simple state tracking to handle those parts of the grammar that require only local context. That
///  is essentially everything except the distinction between objects and arrays.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReaderState {
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

//TODO StackBufferJsonReader
//TODO HeapBufferJsonReader

//TODO flag for 'require comma after array / object' -> top-level, newline-separated sequence of objects

//TODO documentation: tokenizer, no grammar check --> grammar checking wrapper?
pub struct ProvidedBufferJsonReader<'a, R: Read, const N:usize = 8192> {
    buf: &'a mut [u8;N],
    ind_end_buf: usize,
    reader: R,
    state: ReaderState,
    parked_next: Option<u8>,
    cur_location: Location,
}
impl<'a, R: Read, const N:usize> ProvidedBufferJsonReader<'a, R, N> {
    pub fn new(buf: &'a mut [u8;N], reader: R) -> Self {
        Self {
            buf,
            ind_end_buf: 0,
            reader,
            state: ReaderState::Initial,
            parked_next: None,
            cur_location: Location::start(),
        }
    }

    fn ensure_accept_value(&mut self) -> ParseResult<()> {
        match self.state {
            ReaderState::Initial |
            ReaderState::BeforeEntry |
            ReaderState::AfterKey => {
                Ok(())
            }
            ReaderState::AfterValue => {
                self.parse_err("missing comma")
            }
        }
    }

    fn ensure_accept_end_nested(&mut self) -> ParseResult<()> {
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

    fn state_change_for_value(&mut self) -> ParseResult<()> {
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

    fn on_comma(&mut self) -> ParseResult<()> {
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

    pub fn next(&mut self) -> ParseResult<JsonReadEvent> {
        self.consume_whitespace()?;

        match self.read_next_byte()? {
            None => {
                Ok(JsonReadEvent::EndOfStream)
            },
            Some(b',') => {
                self.on_comma()?;
                self.next()
            }
            Some(b'{') => {
                self.ensure_accept_value()?;
                self.state = ReaderState::Initial;
                Ok(JsonReadEvent::StartObject)
            },
            Some(b'}') => {
                self.ensure_accept_end_nested()?;
                self.state = ReaderState::AfterValue;
                Ok(JsonReadEvent::EndObject)
            },
            Some(b'[') => {
                self.ensure_accept_value()?;
                self.state = ReaderState::Initial;
                Ok(JsonReadEvent::StartArray)
            },
            Some(b']') => {
                self.ensure_accept_end_nested()?;
                self.state = ReaderState::AfterValue;
                Ok(JsonReadEvent::EndArray)
            },

            Some(b'n') => {
                self.state_change_for_value()?;
                self.consume_null_literal()
            },
            Some(b't') => {
                self.state_change_for_value()?;
                self.consume_true_literal()
            },
            Some(b'f') => {
                self.state_change_for_value()?;
                self.consume_false_literal()
            },

            Some(b'"') => self.parse_after_quote(), // key or string value based on following ':'
            Some(b) => {
                self.state_change_for_value()?;
                match b {
                    b'-' | b'0'..=b'9' => self.parse_number_literal(b),
                    _ => self.parse_err("invalid JSON literal")
                }
            },
        }
    }

    pub fn expect_next_key(&mut self) -> ParseResult<Option<&str>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::Key(key) => Ok(Some(key)),
            JsonReadEvent::EndObject => Ok(None),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_number<T: FromStr>(&mut self) -> ParseResult<T> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NumberLiteral(n) => {
                match n.parse::<T>() {
                    Ok(n) => Ok(n),
                    Err(_) => self.parse_err("invalid number"),
                }
            },
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_number<T: FromStr>(&mut self) -> ParseResult<Option<T>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::NumberLiteral(n) => {
                match n.parse::<T>() {
                    Ok(n) => Ok(Some(n)),
                    Err(_) => self.parse_err("invalid number"),
                }
            },
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_string(&mut self) -> ParseResult<&str> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StringLiteral(s) => Ok(s),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_string(&mut self) -> ParseResult<Option<&str>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StringLiteral(s) => Ok(Some(s)),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_bool(&mut self) -> ParseResult<bool> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::BooleanLiteral(b) => Ok(b),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_bool(&mut self) -> ParseResult<Option<bool>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::BooleanLiteral(b) => Ok(Some(b)),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_start_object(&mut self) -> ParseResult<()> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StartObject => Ok(()),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_start_object(&mut self) -> ParseResult<Option<()>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StartObject => Ok(Some(())),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_start_array(&mut self) -> ParseResult<()> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StartArray => Ok(()),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_start_array(&mut self) -> ParseResult<Option<()>> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StartArray => Ok(Some(())),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    fn consume_whitespace(&mut self) -> ParseResult<()> {
        while let Some(next) = self.read_next_byte()? {
            match next {
                b' ' | b'\t' | b'\n' | b'\r' => {
                }
                next => {
                    self.parked_next = Some(next);
                    break;
                }
            }
        }
        Ok(())
    }

    fn read_next_byte(&mut self) -> ParseResult<Option<u8>> {
        // Parsing JSON requires a lookahead of a single byte, which is stored in 'parked_next'
        if let Some(parked) = self.parked_next.take() {
            return Ok(Some(parked));
        }

        //TODO BufRead? no_std trait? optimize & clean up!
        let mut read_buf: [u8;1] = [0];
        let num_read = self.reader.read(&mut read_buf)?;
        if num_read == 0 {
            Ok(None)
        }
        else {
            self.cur_location.after_byte(read_buf[0]);
            Ok(Some(read_buf[0]))
        }
    }

    fn consume_null_literal(&mut self) -> ParseResult<JsonReadEvent> {
        if self.read_next_byte()? != Some(b'u') {
            return self.parse_err("incomplete null literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.parse_err("incomplete null literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.parse_err("incomplete null literal");
        }
        Ok(JsonReadEvent::NullLiteral)
    }

    fn consume_true_literal(&mut self) -> ParseResult<JsonReadEvent> {
        if self.read_next_byte()? != Some(b'r') {
            return self.parse_err("incomplete true literal");
        }
        if self.read_next_byte()? != Some(b'u') {
            return self.parse_err("incomplete true literal");
        }
        if self.read_next_byte()? != Some(b'e') {
            return self.parse_err("incomplete true literal");
        }
        Ok(JsonReadEvent::BooleanLiteral(true))
    }

    fn consume_false_literal(&mut self) -> ParseResult<JsonReadEvent> {
        if self.read_next_byte()? != Some(b'a') {
            return self.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b's') {
            return self.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b'e') {
            return self.parse_err("incomplete false literal");
        }
        Ok(JsonReadEvent::BooleanLiteral(false))
    }

    fn append_to_buf(&mut self, ch: u8) -> ParseResult<()> {
        if self.ind_end_buf >= N {
            return self.buf_overflow();
        }
        self.buf[self.ind_end_buf] = ch;
        self.ind_end_buf += 1;
        Ok(())
    }

    fn parse_after_quote(&mut self) -> ParseResult<JsonReadEvent> {
        self.ind_end_buf = 0;

        loop {
            if let Some(next) = self.read_next_byte()? {
                match next {
                    b'"' => break,
                    b'\\' => {
                        match self.read_next_byte()? {
                            Some(b'"') => self.append_to_buf(b'"')?,
                            Some(b'\\') => self.append_to_buf(b'\\')?,
                            Some(b'/') => self.append_to_buf(b'/')?,
                            Some(b'b') => self.append_to_buf(0x08)?,
                            Some(b'f') => self.append_to_buf(0x0c)?,
                            Some(b'n') => self.append_to_buf(b'\n')?,
                            Some(b'r') => self.append_to_buf(b'\r')?,
                            Some(b't') => self.append_to_buf(b'\t')?,
                            Some(b'u') => {
                                let cp = self.parse_unicode_codepoint()?;
                                self.append_code_point(cp)?;
                            },
                            _ => return self.parse_err("invalid escape in string literal"),
                        }
                    },
                    ch => {
                        self.append_to_buf(ch)?;
                    }
                }
            }
            else {
                return self.parse_err("unterminated string literal");
            }
        }

        // the buffer contains the string's contents - the next character determines whether this
        //  is key or a string value. Recall that we don't check for valid JSON.

        self.consume_whitespace()?;
        match self.read_next_byte()? {
            Some(b':') => {
                match self.state {
                    ReaderState::Initial |
                    ReaderState::BeforeEntry => {
                        self.state = ReaderState::AfterKey;
                    }
                    ReaderState::AfterKey => {
                        return self.parse_err("two keys without value");
                    }
                    ReaderState::AfterValue => {
                        return self.parse_err("missing comma");
                    }
                }
                Ok(JsonReadEvent::Key(core::str::from_utf8(&self.buf[..self.ind_end_buf])?))
            },
            other => {
                self.state_change_for_value()?;
                self.parked_next = other;
                Ok(JsonReadEvent::StringLiteral(core::str::from_utf8(&self.buf[..self.ind_end_buf])?))
            }
        }
    }

    fn parse_unicode_codepoint(&mut self) -> ParseResult<u16> {
        // exactly four hex digits specifying a code point
        let mut cp: u16 = 0;
        for _ in 0..4 {
            if let Some(b) = self.read_next_byte()? {
                cp = cp << 4;
                match b {
                    b'0'..=b'9' => cp += (b - b'0') as u16,
                    b'a'..=b'f' => cp += (b - b'a' + 10) as u16,
                    b'A'..=b'Z' => cp += (b - b'A' + 10) as u16,
                    _ => {
                        return self.parse_err("not a four-digit hex number after \\u");
                    }
                }
            }
            else {
                return self.parse_err("incomplete UTF codepoint in string literal");
            }
        }
        Ok(cp)
    }

    /// see https://de.wikipedia.org/wiki/UTF-8
    fn append_code_point(&mut self, cp: u16) -> ParseResult<()> {
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

    fn parse_number_literal(&mut self, b: u8) -> ParseResult<JsonReadEvent> {
        self.buf[0] = b;
        self.ind_end_buf = 1;

        while let Some(next) = self.read_next_byte()? {
            match next {
                b'0'..=b'9' |
                b'+' | b'-' | b'e' | b'E' |
                b'.' => {
                    self.append_to_buf(next)?;
                }
                other => {
                    self.parked_next = Some(other);
                    break;
                }
            }
        }
        Ok(JsonReadEvent::NumberLiteral(JsonNumber(core::str::from_utf8(&self.buf[..self.ind_end_buf])?)))
    }

    fn parse_err<T>(&self, msg: &'static str) -> ParseResult<T> {
        Err(JsonParseError::Parse(msg, self.cur_location))
    }

    fn buf_overflow<T>(&self) -> ParseResult<T> {
        Err(JsonParseError::BufferOverflow(self.cur_location))
    }

    #[inline]
    pub fn location(&self) -> Location {
        self.cur_location
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::*;
    use rstest::*;

    fn assert_is_similar_error(actual: &JsonParseError, expected: &JsonParseError) {
        match actual {
            JsonParseError::Io(self_e) => {
                if let JsonParseError::Io(other_e) = expected {
                    assert_eq!(self_e.kind(), other_e.kind());
                    return;
                }
            }
            JsonParseError::Utf8(_) => {
                if let JsonParseError::Utf8(_) = expected {
                    return;
                }
            }
            JsonParseError::Parse(msg, _) => {
                if let JsonParseError::Parse(other_msg, _) = expected {
                    assert_eq!(msg, other_msg);
                    return;
                }
            }
            JsonParseError::UnexpectedEvent(_) => {
                if let JsonParseError::UnexpectedEvent(_) = expected {
                    return;
                }
            }
            JsonParseError::BufferOverflow(_) => {
                if let JsonParseError::BufferOverflow(_) = expected {
                    return;
                }
            }
        }

        panic!("{:?} != {:?}", actual, expected);
    }

    #[rstest]
    #[case::empty("", vec![], None)]
    #[case::empty_repeated_end_of_stream("", vec![JsonReadEvent::EndOfStream, JsonReadEvent::EndOfStream, JsonReadEvent::EndOfStream, ], None)]

    #[case::null_literal("null", vec![JsonReadEvent::NullLiteral], None)]
    #[case::true_literal("true", vec![JsonReadEvent::BooleanLiteral(true)], None)]
    #[case::false_literal("false", vec![JsonReadEvent::BooleanLiteral(false)], None)]
    #[case::start_object("{", vec![JsonReadEvent::StartObject], None)]
    #[case::end_object("{}", vec![JsonReadEvent::StartObject, JsonReadEvent::EndObject], None)]
    #[case::start_array("[", vec![JsonReadEvent::StartArray], None)]
    #[case::end_array("[]", vec![JsonReadEvent::StartArray, JsonReadEvent::EndArray], None)]

    #[case::key("\"xyz\":", vec![JsonReadEvent::Key("xyz")], None)]
    #[case::key_with_escapes("\"x\\ry\\nz\":", vec![JsonReadEvent::Key("x\ry\nz")], None)]
    #[case::key_ws("\"xyz\" \n:", vec![JsonReadEvent::Key("xyz")], None)]
    #[case::key_value("\"xyz\" \n:\r\tfalse", vec![JsonReadEvent::Key("xyz"), JsonReadEvent::BooleanLiteral(false)], None)]

    #[case::string_literal(r#""abc""#, vec![JsonReadEvent::StringLiteral("abc")], None)]
    #[case::string_literal_empty(r#""""#, vec![JsonReadEvent::StringLiteral("")], None)]
    #[case::string_literal_quot(r#""\"""#, vec![JsonReadEvent::StringLiteral("\"")], None)]
    #[case::string_literal_backslash(r#""\\""#, vec![JsonReadEvent::StringLiteral("\\")], None)]
    #[case::string_literal_slash(r#""\/""#, vec![JsonReadEvent::StringLiteral("/")], None)]
    #[case::string_literal_backslash(r#""\b""#, vec![JsonReadEvent::StringLiteral("\x08")], None)]
    #[case::string_literal_formfeed(r#""\f""#, vec![JsonReadEvent::StringLiteral("\x0c")], None)]
    #[case::string_literal_linefeed(r#""\n""#, vec![JsonReadEvent::StringLiteral("\n")], None)]
    #[case::string_literal_carriage_return(r#""\r""#, vec![JsonReadEvent::StringLiteral("\r")], None)]
    #[case::string_literal_tab(r#""\t""#, vec![JsonReadEvent::StringLiteral("\t")], None)]
    #[case::string_literal_unicode_y(r#""\u0079""#, vec![JsonReadEvent::StringLiteral("y")], None)]
    #[case::string_literal_unicode_umlaut_two_bytes(r#""\u00e4""#, vec![JsonReadEvent::StringLiteral("ä")], None)]
    #[case::string_literal_unicode_omega_two_bytes(r#""\u03a9""#, vec![JsonReadEvent::StringLiteral("Ω")], None)]
    #[case::string_literal_unicode_euro_three_bytes(r#""\u20ac""#, vec![JsonReadEvent::StringLiteral("€")], None)]
    #[case::string_literal_combined(r#""a\n b\t \u00e4öü \u03a9 12.2\u20ac""#, vec![JsonReadEvent::StringLiteral("a\n b\t äöü Ω 12.2€")], None)]

    #[case::number_literal("123", vec![JsonReadEvent::NumberLiteral(JsonNumber("123"))], None)]
    #[case::number_literal_negative("-456", vec![JsonReadEvent::NumberLiteral(JsonNumber("-456"))], None)]
    #[case::number_literal_zero("0", vec![JsonReadEvent::NumberLiteral(JsonNumber("0"))], None)]
    #[case::number_literal_fraction("0.92", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92"))], None)]
    #[case::number_literal_fraction_small("0.0000000000000092", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.0000000000000092"))], None)]
    #[case::number_literal_fraction_neg("-0.0000000000000092", vec![JsonReadEvent::NumberLiteral(JsonNumber("-0.0000000000000092"))], None)]
    #[case::number_literal_exp_lower("0.92e4", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92e4"))], None)]
    #[case::number_literal_exp_upper("0.92E6", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92E6"))], None)]
    #[case::number_literal_pos_exp_lower("0.92e+4", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92e+4"))], None)]
    #[case::number_literal_pos_exp_upper("0.92E+6", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92E+6"))], None)]
    #[case::number_literal_neg_exp_lower("0.92e-4", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92e-4"))], None)]
    #[case::number_literal_neg_exp_upper("0.92E-6", vec![JsonReadEvent::NumberLiteral(JsonNumber("0.92E-6"))], None)]

    #[case::number_literal_no_leading_zero(".1", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]
    #[case::no_matching_literal("x", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]
    #[case::invalid_number_continuation("1x", vec![JsonReadEvent::NumberLiteral(JsonNumber("1"))], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::invalid_number_continuation_quote("x\"", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]

    #[case::string_literal_unterminated_short(r#""abc "#, vec![], Some(JsonParseError::Parse("unterminated string literal", Location::start())))]
    #[case::string_literal_unterminated_long(r#""abc                                                                         "#, vec![], Some(JsonParseError::BufferOverflow(Location::start())))]
    #[case::string_literal_invalid_escape(r#""\q""#, vec![], Some(JsonParseError::Parse("invalid escape in string literal", Location::start())))]
    #[case::string_literal_unicode_string_ends(r#""\u004""#, vec![], Some(JsonParseError::Parse("not a four-digit hex number after \\u", Location::start())))]
    #[case::string_literal_unicode_invalid_character_1(r#""\ux041""#, vec![], Some(JsonParseError::Parse("not a four-digit hex number after \\u", Location::start())))]
    #[case::string_literal_unicode_invalid_character_2(r#""\u0x41""#, vec![], Some(JsonParseError::Parse("not a four-digit hex number after \\u", Location::start())))]
    #[case::string_literal_unicode_invalid_character_3(r#""\u00x1""#, vec![], Some(JsonParseError::Parse("not a four-digit hex number after \\u", Location::start())))]
    #[case::string_literal_unicode_invalid_character_4(r#""\u004x""#, vec![], Some(JsonParseError::Parse("not a four-digit hex number after \\u", Location::start())))]
    #[case::string_literal_unicode_uppercase_u(r#""\U0041""#, vec![], Some(JsonParseError::Parse("invalid escape in string literal", Location::start())))]
    #[case::string_literal_unicode_uppercase(r#""\uABCD""#, vec![JsonReadEvent::StringLiteral("\u{abcd}")], None)]
    #[case::string_literal_unicode_mixed_case_1(r#""\uaBcD""#, vec![JsonReadEvent::StringLiteral("\u{abcd}")], None)]
    #[case::string_literal_unicode_mixed_case_2(r#""\uAbCd""#, vec![JsonReadEvent::StringLiteral("\u{abcd}")], None)]

    #[case::null_wrong_continuation_1("nul", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_2("nxll", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_3("nUll", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_4("nuxl", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_5("nuLl", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_6("nulx", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_wrong_continuation_7("nulL", vec![], Some(JsonParseError::Parse("incomplete null literal", Location::start())))]
    #[case::null_uppercase("Null", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]
    #[case::null_uppercase_2("NULL", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]

    #[case::true_wrong_continuation_1("tru", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_2("txue", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_3("tRue", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_4("trxe", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_5("trUe", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_6("trux", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_wrong_continuation_7("truE", vec![], Some(JsonParseError::Parse("incomplete true literal", Location::start())))]
    #[case::true_uppercase_1("True", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]
    #[case::true_uppercase_2("TRUE", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]

    #[case::false_wrong_continuation_1("fals", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_2("fxlse", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_3("fAlse", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_4("faxse", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_5("faLse", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_6("falxe", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_7("falSe", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_8("falsx", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_wrong_continuation_9("falsE", vec![], Some(JsonParseError::Parse("incomplete false literal", Location::start())))]
    #[case::false_uppercase_1("False", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]
    #[case::false_uppercase_2("FALSE", vec![], Some(JsonParseError::Parse("invalid JSON literal", Location::start())))]

    #[case::object_end_just_comma(r#"{, }"#, vec![JsonReadEvent::StartObject], Some(JsonParseError::Parse("unexpected comma", Location::start())))]
    #[case::object_end_trailing_comma(r#"{"a": null, }"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a"), JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("trailing comma", Location::start())))]
    #[case::object_end_after_key(r#"{"a": }"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a")], Some(JsonParseError::Parse("key without a value", Location::start())))]
    #[case::array_end_just_comma(r#"[, ]"#, vec![JsonReadEvent::StartArray], Some(JsonParseError::Parse("unexpected comma", Location::start())))]
    #[case::array_end_trailing_comma(r#"[null, ]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("trailing comma", Location::start())))]
    #[case::array_end_after_key(r#"["a": ]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::Key("a")], Some(JsonParseError::Parse("key without a value", Location::start())))]

    #[case::missing_comma_null(r#"[null null]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_true(r#"[null true]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_false(r#"[null false]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_number(r#"[null 123]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_string(r#"[null "abc"]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_object(r#"[null {}]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_array(r#"[null []]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::missing_comma_key(r#"{"a": null "b": 1}"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a"), JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("missing comma", Location::start())))]
    #[case::key_after_key(r#"{"a": "b": 1}"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a")], Some(JsonParseError::Parse("two keys without value", Location::start())))]
    #[case::comma_after_key(r#"{"a": , "b": 1}"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a")], Some(JsonParseError::Parse("unexpected comma", Location::start())))]

    #[case::object_comma_after_comma(r#"{"a": null, ,}"#, vec![JsonReadEvent::StartObject, JsonReadEvent::Key("a"), JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("unexpected comma", Location::start())))]
    #[case::array_comma_after_comma(r#"[ null, ,]"#, vec![JsonReadEvent::StartArray, JsonReadEvent::NullLiteral], Some(JsonParseError::Parse("unexpected comma", Location::start())))]

    #[case::object(r#"{ "a": 1, "b": true, "c": "xyz" }"#, vec![
        JsonReadEvent::StartObject,
        JsonReadEvent::Key("a"),
        JsonReadEvent::NumberLiteral(JsonNumber("1")),
        JsonReadEvent::Key("b"),
        JsonReadEvent::BooleanLiteral(true),
        JsonReadEvent::Key("c"),
        JsonReadEvent::StringLiteral("xyz"),
        JsonReadEvent::EndObject,
    ], None)]
    #[case::array(r#"[ 6, "xy", true, null ]"#, vec![
        JsonReadEvent::StartArray,
        JsonReadEvent::NumberLiteral(JsonNumber("6")),
        JsonReadEvent::StringLiteral("xy"),
        JsonReadEvent::BooleanLiteral(true),
        JsonReadEvent::NullLiteral,
        JsonReadEvent::EndArray,
    ], None)]
    #[case::complex(r#"{"abc":"yo","xyz":"yo","aaaa":["111","11",{},[],null,true,false,-23987,23987,23.235,null,null,23.235e-1,null,null],"ooo":{"lll":"whatever","ar":[]}}"#, vec![
        JsonReadEvent::StartObject,
        JsonReadEvent::Key("abc"),
        JsonReadEvent::StringLiteral("yo"),
        JsonReadEvent::Key("xyz"),
        JsonReadEvent::StringLiteral("yo"),
        JsonReadEvent::Key("aaaa"),
        JsonReadEvent::StartArray,
        JsonReadEvent::StringLiteral("111"),
        JsonReadEvent::StringLiteral("11"),
        JsonReadEvent::StartObject,
        JsonReadEvent::EndObject,
        JsonReadEvent::StartArray,
        JsonReadEvent::EndArray,
        JsonReadEvent::NullLiteral,
        JsonReadEvent::BooleanLiteral(true),
        JsonReadEvent::BooleanLiteral(false),
        JsonReadEvent::NumberLiteral(JsonNumber("-23987")),
        JsonReadEvent::NumberLiteral(JsonNumber("23987")),
        JsonReadEvent::NumberLiteral(JsonNumber("23.235")),
        JsonReadEvent::NullLiteral,
        JsonReadEvent::NullLiteral,
        JsonReadEvent::NumberLiteral(JsonNumber("23.235e-1")),
        JsonReadEvent::NullLiteral,
        JsonReadEvent::NullLiteral,
        JsonReadEvent::EndArray,
        JsonReadEvent::Key("ooo"),
        JsonReadEvent::StartObject,
        JsonReadEvent::Key("lll"),
        JsonReadEvent::StringLiteral("whatever"),
        JsonReadEvent::Key("ar"),
        JsonReadEvent::StartArray,
        JsonReadEvent::EndArray,
        JsonReadEvent::EndObject,
        JsonReadEvent::EndObject,
    ], None)]

    fn test_parse(#[case] input: &str, #[case] expected: Vec<JsonReadEvent>, #[case] expected_error: Option<JsonParseError>) {
        let mut buf = [0u8;64];
        let input_with_whitespace = format!(" \r\n\t{} \r\n\t", input);

        {
            let r = Cursor::new(input.as_bytes().to_vec());
            let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
            for evt in &expected {
                let parsed_evt = parser.next();
                assert_eq!(&parsed_evt.unwrap(), evt);
            }
            if let Some(expected_error) = &expected_error {
                match parser.next() {
                    Ok(_) => panic!("expected error but was ok: {}", expected_error),
                    Err(e) => assert_is_similar_error(&e, expected_error),
                }
            }
            else {
                assert_eq!(parser.next().unwrap(), JsonReadEvent::EndOfStream);
            }
        }
        {
            let r = Cursor::new(input_with_whitespace.as_bytes().to_vec());
            let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
            for evt in &expected {
                assert_eq!(&parser.next().unwrap(), evt);
            }
            if let Some(expected_error) = &expected_error {
                match parser.next() {
                    Ok(_) => panic!("expected error but was ok: {}", expected_error),
                    Err(e) => assert_is_similar_error(&e, expected_error),
                }
            }
            else {
                assert_eq!(parser.next().unwrap(), JsonReadEvent::EndOfStream);
            }
        }
    }

    #[rstest]
    #[case::simple("1", Some(1), Some(1), 1.0, 1.0)]
    #[case::big("1345678345", Some(1345678345), Some(1345678345), 1345678345.0, 1345678345.0)]
    #[case::bigger("3345678345", Some(3345678345), None, 3345678345.0, 3345678345.0)]
    #[case::too_big("13456783459", None, None, 13456783459.0, 13456783459.0)]
    #[case::negative("-1", None, Some(-1), -1.0, -1.0)]
    #[case::fract("1.0", None, None, 1.0, 1.0)]
    #[case::exp("1e3", None, None, 1e3, 1e3)]
    #[case::neg_exp("1e-3", None, None, 1e-3, 1e-3)]
    #[case::pos_exp("1e+3", None, None, 1e3, 1e3)]
    #[case::fract_exp("1.23e3", None, None, 1230.0, 1230.0)]
    #[case::fract_neg_exp("1.23e-3", None, None, 1.23e-3, 1.23e-3)]
    #[case::fract_pos_exp("1.23e+3", None, None, 1.23e3, 1.23e3)]
    fn test_json_number_parse(#[case] s: &str, #[case] expected_u32: Option<u32>, #[case] expected_i32: Option<i32>, #[case] expected_f64: f64, #[case] expected_f32: f32) {
        let n = JsonNumber(s);

        {
            let parsed = n.parse::<u32>();
            match expected_u32 {
                Some(e) => assert_eq!(e, parsed.unwrap()),
                None => assert!(parsed.is_err()),
            }
        }
        {
            let parsed = n.parse::<i32>();
            match expected_i32 {
                Some(e) => assert_eq!(e, parsed.unwrap()),
                None => assert!(parsed.is_err()),
            }
        }
        {
            let parsed = n.parse::<f64>();
            assert_eq!(expected_f64, parsed.unwrap());
        }
        {
            let parsed = n.parse::<f32>();
            assert_eq!(expected_f32, parsed.unwrap());
        }
    }


    #[rstest]
    #[case::simple(Location::start(), vec![b'a'], Location { offset: 1, line: 1, column: 2,})]
    #[case::cr(Location::start(), vec![b'\r'], Location { offset: 1, line: 1, column: 2,})]
    #[case::tab(Location::start(), vec![b'\t'], Location { offset: 1, line: 1, column: 2,})]
    #[case::nl(Location::start(), vec![b'\n'], Location { offset: 1, line: 2, column: 1,})]
    #[case::in_line(Location::start(), vec![b'\r', b'\n', b'\n', b'x', b'y'], Location { offset: 5, line: 3, column: 3,})]
    #[case::sequence(Location::start(), vec![b'a', b'b', b'\n', b'x', b'\n'], Location { offset: 5, line: 3, column: 1,})]
    fn test_location_after_byte(#[case] mut initial: Location, #[case] bytes: Vec<u8>, #[case] expected: Location) {
        for byte in bytes {
            initial.after_byte(byte);
        }
        assert_eq!(initial, expected);
    }


    #[rstest]
    #[case::key(r#""abc": null"#, Some(Some("abc")))]
    #[case::other_key(r#""xyz": null"#, Some(Some("xyz")))]
    #[case::end_object(r#"}"#, Some(None))]
    #[case::null(r#"null"#, None)]
    #[case::bool(r#"true"#, None)]
    #[case::number(r#"1"#, None)]
    #[case::string(r#""a""#, None)]
    #[case::start_object(r#"{"#, None)]
    #[case::start_array(r#"["#, None)]
    #[case::end_array(r#"]"#, None)]
    fn test_expect_next_key(#[case] json: &str, #[case] expected: Option<Option<&str>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_key() {
            Ok(actual) => assert_eq!(actual, expected.unwrap()),
            Err(JsonParseError::UnexpectedEvent(_)) => assert!(expected.is_none()),
            Err(e) => panic!("unexpected error: {}", e)
        }
    }

    #[rstest]
    #[case::simple("1", Ok(1))]
    #[case::other_number("500", Err(JsonParseError::Parse("invalid number", Location::start())))]
    #[case::null("null", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::string("\"abc\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_number(#[case] json: &str, #[case] expected_num: ParseResult<u8>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_number::<u8>() {
            Ok(n) => assert_eq!(n, expected_num.unwrap()),
            Err(act_e) => match expected_num {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::simple("1", Ok(Some(1)))]
    #[case::other_number("500", Err(JsonParseError::Parse("invalid number", Location::start())))]
    #[case::null("null", Ok(None))]
    #[case::string("\"abc\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_opt_number(#[case] json: &str, #[case] expected_num: ParseResult<Option<u8>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_opt_number::<u8>() {
            Ok(n) => assert_eq!(n, expected_num.unwrap()),
            Err(act_e) => match expected_num {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::simple("\"qrs\"", Ok("qrs"))]
    #[case::null("null", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_string(#[case] json: &str, #[case] expected: ParseResult<&str>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_string() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::simple("\"rst\"", Ok(Some("rst")))]
    #[case::null("null", Ok(None))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_opt_string(#[case] json: &str, #[case] expected: ParseResult<Option<&str>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_opt_string() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }
    #[rstest]
    #[case::bool_true("true", Ok(true))]
    #[case::bool_false("false", Ok(false))]
    #[case::null("null", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::string("\"a\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_bool(#[case] json: &str, #[case] expected: ParseResult<bool>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_bool() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::bool_true("true", Ok(Some(true)))]
    #[case::bool_false("false", Ok(Some(false)))]
    #[case::null("null", Ok(None))]
    #[case::string("\"x\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_opt_bool(#[case] json: &str, #[case] expected: ParseResult<Option<bool>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_opt_bool() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::null("null", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::string("\"a\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Ok(()))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_start_object(#[case] json: &str, #[case] expected: ParseResult<()>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_start_object() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::bool("false", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::null("null", Ok(None))]
    #[case::string("\"x\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Ok(Some(())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_opt_start_object(#[case] json: &str, #[case] expected: ParseResult<Option<()>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_opt_start_object() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::null("null", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::bool("true", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::string("\"a\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Ok(()))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_start_array(#[case] json: &str, #[case] expected: ParseResult<()>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_start_array() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }

    #[rstest]
    #[case::bool("false", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::null("null", Ok(None))]
    #[case::string("\"x\"", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::number("12", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::key("\"abc\": ", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_object("{", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::end_object("}", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    #[case::start_array("[", Ok(Some(())))]
    #[case::end_array("]", Err(JsonParseError::UnexpectedEvent(Location::start())))]
    fn test_expect_next_opt_start_array(#[case] json: &str, #[case] expected: ParseResult<Option<()>>) {
        let mut buf = [0u8;64];
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = ProvidedBufferJsonReader::new(&mut buf, r);
        match parser.expect_next_opt_start_array() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }
}
