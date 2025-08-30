use crate::blocking::io::BlockingRead;
use crate::shared::read::*;
use core::str::FromStr;


//TODO documentation: tokenizer, no grammar check --> grammar checking wrapper?
pub struct JsonReader<B: AsMut<[u8]>, R: BlockingRead> {
    inner: ReaderInner<B, R::Error>,
    reader: R,
}

#[cfg(not(feature = "no-std"))]
impl<R: BlockingRead> JsonReader<Vec<u8>, R> {
    pub fn new(buf_size: usize, reader: R) -> Self {
        let buf = vec![0u8; buf_size];
        Self::new_with_provided_buffer(buf, reader, false)
    }

    //TODO unit test
    pub fn new_with_lenient_comma_handling(buf_size: usize, reader: R) -> Self {
        let buf = vec![0u8; buf_size];
        Self::new_with_provided_buffer(buf, reader, true)
    }
}

impl<B: AsMut<[u8]>, R: BlockingRead> JsonReader<B, R> {
    pub fn new_with_provided_buffer(buf: B, reader: R, lenient_comma_handling: bool) -> Self {
        Self {
            inner: ReaderInner::new(buf, lenient_comma_handling),
            reader,
        }
    }

    pub fn next(&mut self) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        self.consume_whitespace()?;

        match self.read_next_byte()? {
            None => {
                Ok(JsonReadEvent::EndOfStream)
            },
            Some(b',') => {
                self.inner.on_comma()?;
                self.next()
            }
            Some(b'{') => {
                self.inner.ensure_accept_value()?;
                self.inner.state = ReaderState::Initial;
                Ok(JsonReadEvent::StartObject)
            },
            Some(b'}') => {
                self.inner.ensure_accept_end_nested()?;
                self.inner.state = ReaderState::AfterValue;
                Ok(JsonReadEvent::EndObject)
            },
            Some(b'[') => {
                self.inner.ensure_accept_value()?;
                self.inner.state = ReaderState::Initial;
                Ok(JsonReadEvent::StartArray)
            },
            Some(b']') => {
                self.inner.ensure_accept_end_nested()?;
                self.inner.state = ReaderState::AfterValue;
                Ok(JsonReadEvent::EndArray)
            },

            Some(b'n') => {
                self.inner.state_change_for_value()?;
                self.consume_null_literal()
            },
            Some(b't') => {
                self.inner.state_change_for_value()?;
                self.consume_true_literal()
            },
            Some(b'f') => {
                self.inner.state_change_for_value()?;
                self.consume_false_literal()
            },

            Some(b'"') => self.parse_after_quote(), // key or string value based on following ':'
            Some(b) => {
                self.inner.state_change_for_value()?;
                match b {
                    b'-' | b'0'..=b'9' => self.parse_number_literal(b),
                    _ => self.inner.parse_err("invalid JSON literal")
                }
            },
        }
    }

    pub fn expect_next_key(&mut self) -> JsonParseResult<Option<&str>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::Key(key) => Ok(Some(key)),
            JsonReadEvent::EndObject => Ok(None),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_number<T: FromStr>(&mut self) -> JsonParseResult<T, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NumberLiteral(n) => {
                match n.parse::<T>() {
                    Ok(n) => Ok(n),
                    Err(_) => self.inner.parse_err("invalid number"),
                }
            },
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_number<T: FromStr>(&mut self) -> JsonParseResult<Option<T>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::NumberLiteral(n) => {
                match n.parse::<T>() {
                    Ok(n) => Ok(Some(n)),
                    Err(_) => self.inner.parse_err("invalid number"),
                }
            },
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_string(&mut self) -> JsonParseResult<&str, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StringLiteral(s) => Ok(s),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_string(&mut self) -> JsonParseResult<Option<&str>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StringLiteral(s) => Ok(Some(s)),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_bool(&mut self) -> JsonParseResult<bool, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::BooleanLiteral(b) => Ok(b),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_bool(&mut self) -> JsonParseResult<Option<bool>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::BooleanLiteral(b) => Ok(Some(b)),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_start_object(&mut self) -> JsonParseResult<(), R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StartObject => Ok(()),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_start_object(&mut self) -> JsonParseResult<Option<()>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StartObject => Ok(Some(())),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_start_array(&mut self) -> JsonParseResult<(), R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::StartArray => Ok(()),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    pub fn expect_next_opt_start_array(&mut self) -> JsonParseResult<Option<()>, R::Error> {
        let location = self.location();
        let next = self.next()?;
        match next {
            JsonReadEvent::NullLiteral => Ok(None),
            JsonReadEvent::StartArray => Ok(Some(())),
            _ => Err(JsonParseError::UnexpectedEvent(location)),
        }
    }

    fn consume_whitespace(&mut self) -> JsonParseResult<(), R::Error> {
        while let Some(next) = self.read_next_byte()? {
            match next {
                b' ' | b'\t' | b'\n' | b'\r' => {
                }
                next => {
                    self.inner.parked_next = Some(next);
                    break;
                }
            }
        }
        Ok(())
    }

    fn read_next_byte(&mut self) -> JsonParseResult<Option<u8>, R::Error> {
        // Parsing JSON requires a lookahead of a single byte, which is stored in 'parked_next'
        if let Some(parked) = self.inner.parked_next.take() {
            return Ok(Some(parked));
        }

        //TODO BufRead? no_std trait? optimize & clean up!
        if let Some(byte) =self.reader.read()? {
            self.inner.cur_location.after_byte(byte);
            Ok(Some(byte))
        }
        else {
            Ok(None)
        }
    }

    fn consume_null_literal(&mut self) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        if self.read_next_byte()? != Some(b'u') {
            return self.inner.parse_err("incomplete null literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.inner.parse_err("incomplete null literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.inner.parse_err("incomplete null literal");
        }
        Ok(JsonReadEvent::NullLiteral)
    }

    fn consume_true_literal(&mut self) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        if self.read_next_byte()? != Some(b'r') {
            return self.inner.parse_err("incomplete true literal");
        }
        if self.read_next_byte()? != Some(b'u') {
            return self.inner.parse_err("incomplete true literal");
        }
        if self.read_next_byte()? != Some(b'e') {
            return self.inner.parse_err("incomplete true literal");
        }
        Ok(JsonReadEvent::BooleanLiteral(true))
    }

    fn consume_false_literal(&mut self) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        if self.read_next_byte()? != Some(b'a') {
            return self.inner.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b'l') {
            return self.inner.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b's') {
            return self.inner.parse_err("incomplete false literal");
        }
        if self.read_next_byte()? != Some(b'e') {
            return self.inner.parse_err("incomplete false literal");
        }
        Ok(JsonReadEvent::BooleanLiteral(false))
    }

    fn parse_after_quote(&mut self) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        self.inner.ind_end_buf = 0;

        loop {
            if let Some(next) = self.read_next_byte()? {
                match next {
                    b'"' => break,
                    b'\\' => {
                        match self.read_next_byte()? {
                            Some(b'"') => self.inner.append_to_buf(b'"')?,
                            Some(b'\\') => self.inner.append_to_buf(b'\\')?,
                            Some(b'/') => self.inner.append_to_buf(b'/')?,
                            Some(b'b') => self.inner.append_to_buf(0x08)?,
                            Some(b'f') => self.inner.append_to_buf(0x0c)?,
                            Some(b'n') => self.inner.append_to_buf(b'\n')?,
                            Some(b'r') => self.inner.append_to_buf(b'\r')?,
                            Some(b't') => self.inner.append_to_buf(b'\t')?,
                            Some(b'u') => {
                                let cp = self.parse_unicode_codepoint()?;
                                self.inner.append_code_point(cp)?;
                            },
                            _ => return self.inner.parse_err("invalid escape in string literal"),
                        }
                    },
                    ch => {
                        self.inner.append_to_buf(ch)?;
                    }
                }
            }
            else {
                return self.inner.parse_err("unterminated string literal");
            }
        }

        // the buffer contains the string's contents - the next character determines whether this
        //  is key or a string value. Recall that we don't check for valid JSON.

        self.consume_whitespace()?;
        match self.read_next_byte()? {
            Some(b':') => {
                match self.inner.state {
                    ReaderState::Initial |
                    ReaderState::BeforeEntry => {
                        self.inner.state = ReaderState::AfterKey;
                    }
                    ReaderState::AfterKey => {
                        return self.inner.parse_err("two keys without value");
                    }
                    ReaderState::AfterValue => {
                        return self.inner.parse_err("missing comma");
                    }
                }
                Ok(JsonReadEvent::Key(self.inner.buf_as_str()?))
            },
            other => {
                self.inner.state_change_for_value()?;
                self.inner.parked_next = other;
                Ok(JsonReadEvent::StringLiteral(self.inner.buf_as_str()?))
            }
        }
    }

    fn parse_unicode_codepoint(&mut self) -> JsonParseResult<u16, R::Error> {
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
                        return self.inner.parse_err("not a four-digit hex number after \\u");
                    }
                }
            }
            else {
                return self.inner.parse_err("incomplete UTF codepoint in string literal");
            }
        }
        Ok(cp)
    }

    fn parse_number_literal(&mut self, b: u8) -> JsonParseResult<JsonReadEvent<'_>, R::Error> {
        self.inner.buf.as_mut()[0] = b;
        self.inner.ind_end_buf = 1;

        while let Some(next) = self.read_next_byte()? {
            match next {
                b'0'..=b'9' |
                b'+' | b'-' | b'e' | b'E' |
                b'.' => {
                    self.inner.append_to_buf(next)?;
                }
                other => {
                    self.inner.parked_next = Some(other);
                    break;
                }
            }
        }
        Ok(JsonReadEvent::NumberLiteral(JsonNumber(self.inner.buf_as_str().unwrap())))
    }

    #[inline]
    pub fn location(&self) -> Location {
        self.inner.cur_location
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use super::*;
    use rstest::*;
    use std::io::Cursor;

    fn assert_is_similar_error(actual: &JsonParseError<std::io::Error>, expected: &JsonParseError<std::io::Error>) {

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

    fn test_parse(#[case] input: &str, #[case] expected: Vec<JsonReadEvent>, #[case] expected_error: Option<JsonParseError<std::io::Error>>) {
        let input_with_whitespace = format!(" \r\n\t{} \r\n\t", input);

        {
            let r = Cursor::new(input.as_bytes().to_vec());
            let mut parser = JsonReader::new(64, r);
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
            let mut parser = JsonReader::new(64, r);
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

    #[test]
    fn test_provided_buffer_fits() -> Result<(), JsonParseError<io::Error>> {
        let mut reader = JsonReader::new(8, Cursor::new(b"123".to_vec()));
        assert_eq!(reader.next()?, JsonReadEvent::NumberLiteral(JsonNumber("123")));
        assert_eq!(reader.next()?, JsonReadEvent::EndOfStream);
        Ok(())
    }

    #[test]
    fn test_provided_buffer_overflow() -> Result<(), JsonParseError<io::Error>> {
        let mut reader = JsonReader::new(8, Cursor::new(b"\"123 123 x\"".to_vec()));
        match reader.next() {
            Ok(_) => panic!("expected an error"),
            Err(e) => assert_is_similar_error(&e, &JsonParseError::BufferOverflow(Location::start())),
        }
        Ok(())
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
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_number(#[case] json: &str, #[case] expected_num: JsonParseResult<u8, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_opt_number(#[case] json: &str, #[case] expected_num: JsonParseResult<Option<u8>, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_string(#[case] json: &str, #[case] expected: JsonParseResult<&str, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_opt_string(#[case] json: &str, #[case] expected: JsonParseResult<Option<&str>, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_bool(#[case] json: &str, #[case] expected: JsonParseResult<bool, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_opt_bool(#[case] json: &str, #[case] expected: JsonParseResult<Option<bool>, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_start_object(#[case] json: &str, #[case] expected: JsonParseResult<(), io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_opt_start_object(#[case] json: &str, #[case] expected: JsonParseResult<Option<()>, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_start_array(#[case] json: &str, #[case] expected: JsonParseResult<(), io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
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
    fn test_expect_next_opt_start_array(#[case] json: &str, #[case] expected: JsonParseResult<Option<()>, io::Error>) {
        let r = Cursor::new(json.as_bytes().to_vec());
        let mut parser = JsonReader::new(64, r);
        match parser.expect_next_opt_start_array() {
            Ok(n) => assert_eq!(n, expected.unwrap()),
            Err(act_e) => match expected {
                Ok(_) => panic!("unexpected error: {}", act_e),
                Err(exp_e) => assert_is_similar_error(&act_e, &exp_e),
            }
        }
    }
}
