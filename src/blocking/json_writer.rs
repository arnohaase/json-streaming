use core::fmt::Display;
use crate::blocking::io::BlockingWrite;

/// [JsonWriter] is the starting point for serializing JSON with this library. It is a thin wrapper
///  around a [Write], adding some JSON specifics and also formatting.
/// 
/// Application code should usually not have to interact with [JsonWriter] directly, but through
///  [ObjectSer] or [ArraySer] wrapped around it.
pub struct JsonWriter <W: BlockingWrite, F: JsonFormatter> {
    inner: W,
    formatter: F,
    /// ending an object / array through RAII can cause an IO error that can not be propagated
    ///  to calling code because [Drop] can not return errors. Such errors are stored in this
    ///  field and returned from the next I/O operation.
    ///
    /// For this to work reliably, it is necessary to call [JsonWriter::flush()] or
    ///  [JsonWriter::into_inner()] before it goes out of scope, in analogy to `BufWriter`'s API.
    unreported_error: Option<W::Error>,
}

impl <W: BlockingWrite, F: JsonFormatter> JsonWriter<W, F> {
    /// Create a new [JsonWriter] instance for given [Write] instance and an explicitly provided
    ///  [JsonFormatter]. It gives full flexibility; for most cases, `new_compact()` and 
    ///  `new_pretty()` functions are more convenient. 
    pub fn new(inner: W, formatter: F) -> JsonWriter<W, F> {
        JsonWriter {
            inner,
            formatter,
            unreported_error: None,
        }
    }

    /// Internal API for writing raw bytes to the underlying [Write].
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), W::Error> {
        self.flush()?;
        self.inner.write_all(data)
    }

    /// Internal API for writing a string as an escaped JSON string.
    pub fn write_escaped_string(&mut self, s: &str) -> Result<(), W::Error> {
        self.write_bytes(b"\"")?;
        for b in s.bytes() {
            match b {
                b'"' => self.write_bytes(b"\\\"")?,
                b'\\' => self.write_bytes(b"\\\\")?,
                b'\x08' => self.write_bytes(b"\\b")?,
                b'\x0c' => self.write_bytes(b"\\f")?,
                b'\n' => self.write_bytes(b"\\n")?,
                b'\r' => self.write_bytes(b"\\r")?,
                b'\t' => self.write_bytes(b"\\t")?,
                b if b < 0x20 => {
                    static HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";
                    let bytes = &[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX_DIGITS[(b >> 4) as usize],
                        HEX_DIGITS[(b & 0xF) as usize],
                    ];
                    self.write_bytes(bytes)?
                },
                b => self.write_bytes(&[b])?,
            }
        }
        self.write_bytes(b"\"")?;
        Ok(())
    }

    /// Internal API for writing a `bool`.
    pub fn write_bool(&mut self, value: bool) -> Result<(), W::Error> {
        if value {
            self.write_bytes(b"true")
        }
        else {
            self.write_bytes(b"false")
        }
    }

    /// Internal API for writing a floating point number, representing non-finite numbers as `null`. 
    pub fn write_f64(&mut self, value: f64) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_f64(value)
    }

    /// Internal API for writing a floating point number, representing non-finite numbers as `null`. 
    pub fn write_f32(&mut self, value: f32) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_f32(value)
    }

    /// internal API for writing raw int values
    pub fn write_raw_num(&mut self, value: impl Display) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_raw(value)
    }

    /// Internal API for interacting with the formatter
    pub fn write_format_after_key(&mut self) -> Result<(), W::Error> {
        self.flush()?;
        self.formatter.after_key(&mut self.inner)
    }

    /// Internal API for interacting with the formatter
    pub fn write_format_after_start_nested(&mut self) -> Result<(), W::Error> {
        self.flush()?;
        self.formatter.after_start_nested(&mut self.inner)
    }

    /// Internal API for interacting with the formatter
    pub fn write_format_after_element(&mut self) -> Result<(), W::Error> {
        self.flush()?;
        self.formatter.after_element(&mut self.inner)
    }

    /// Internal API for interacting with the formatter
    pub fn write_format_before_end_nested(&mut self, is_empty: bool) -> Result<(), W::Error> {
        self.flush()?;
        self.formatter.before_end_nested(is_empty, &mut self.inner)
    }

    /// Internal API for interacting with the formatter
    pub fn write_format_indent(&mut self) -> Result<(), W::Error> {
        self.flush()?;
        self.formatter.indent(&mut self.inner)
    }

    /// Check and return any unreported error that occurred when an object / array went out of 
    ///  scope. Applications should call this function when serialization is complete to ensure
    ///  that no errors get lost.    
    pub fn flush(&mut self) -> Result<(), W::Error> {
        if let Some(e) = self.unreported_error.take() {
            return Err(e);
        }
        Ok(())
    }

    /// End this [JsonWriter]'s lifetime, returning the [Write] instance it owned. This function
    ///  returns any unreported errors.
    pub fn into_inner(mut self) -> Result<W, W::Error> {
        self.flush()?;
        Ok(self.inner)
    }

    pub(crate) fn set_unreported_error(&mut self, unreported_error: W::Error) {
        self.unreported_error = Some(unreported_error);
    }
}

impl <W: BlockingWrite> JsonWriter<W, CompactFormatter> {
    /// Create a [JsonWriter] that generates pretty-printed JSON output.
    pub fn new_compact(inner: W) -> JsonWriter<W, CompactFormatter> {
        JsonWriter::new(inner, CompactFormatter)
    }
}

impl <W: BlockingWrite> JsonWriter<W, PrettyFormatter> {
    /// Create a [JsonWriter] that generates compact JSON output, i.e. with a minimum of whitespace.
    pub fn new_pretty(inner: W) -> JsonWriter<W, PrettyFormatter> {
        JsonWriter::new(inner, PrettyFormatter::new())
    }
}

/// [JsonFormatter] controls how whitespace is added between JSON elements in the output. It does not
///  affect the JSON's semantics, but only its looks and size.
pub trait JsonFormatter {
    /// optional whitespace after the ':' of a JSON object's key.
    fn after_key<W: BlockingWrite>(&self, w: &mut W) -> Result<(), W::Error>;
    /// optional newline after the start of an object or array; adds a level of nesting
    fn after_start_nested<W: BlockingWrite>(&mut self, w: &mut W) -> Result<(), W::Error>;
    /// optional newline after an element
    fn after_element<W: BlockingWrite>(&self, w: &mut W) -> Result<(), W::Error>;
    /// optional indent before then ending character of a nested object or array; removes a level of nesting
    fn before_end_nested<W: BlockingWrite>(&mut self, is_empty: bool, w: &mut W) -> Result<(), W::Error>;
    /// indentation, if any
    fn indent<W: BlockingWrite>(&self, w: &mut W) -> Result<(), W::Error>;
}

/// Write a minimum of whitespace, minimizing output size
pub struct CompactFormatter;
impl JsonFormatter for CompactFormatter {
    fn after_key<W: BlockingWrite>(&self, _w: &mut W) -> Result<(), W::Error> { Ok(())}
    fn after_start_nested<W: BlockingWrite>(&mut self, _w: &mut W) -> Result<(), W::Error> { Ok(()) }
    fn after_element<W: BlockingWrite>(&self, _w: &mut W) -> Result<(), W::Error> { Ok(()) }
    fn before_end_nested<W: BlockingWrite>(&mut self, _is_empty: bool, _w: &mut W) -> Result<(), W::Error> { Ok(()) }
    fn indent<W: BlockingWrite>(&self, _w: &mut W) -> Result<(), W::Error> { Ok(()) }
}

/// Write some whitespace and indentation to improve human readability
pub struct PrettyFormatter {
    indent_level: usize,
}
impl PrettyFormatter {
    pub fn new() -> PrettyFormatter {
        PrettyFormatter {
            indent_level: 0,
        }
    }
}
impl JsonFormatter for PrettyFormatter {
    fn after_key<W: BlockingWrite>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_all(b" ")
    }

    fn after_start_nested<W: BlockingWrite>(&mut self, _w: &mut W) -> Result<(), W::Error> {
        self.indent_level += 1;
        Ok(())
    }

    fn after_element<W: BlockingWrite>(&self, _w: &mut W) -> Result<(), W::Error> {
        Ok(())
    }

    fn before_end_nested<W: BlockingWrite>(&mut self, is_empty: bool, w: &mut W) -> Result<(), W::Error> {
        self.indent_level -= 1;
        if !is_empty {
            self.indent(w)?;
        }
        Ok(())
    }

    fn indent<W: BlockingWrite>(&self, w: &mut W) -> Result<(), W::Error> {
        static INDENT: &'static str = "\n                                                                                                                                                                                                                                                 ";
        w.write_all(&INDENT.as_bytes()[..2*self.indent_level + 1])
    }
}



struct FormatWrapper<'a, W: BlockingWrite, F: JsonFormatter> {
    inner: &'a mut JsonWriter<W, F>,
    cached_error: Option<W::Error>,
}
impl<'a, W: BlockingWrite, F: JsonFormatter> FormatWrapper<'a, W, F> {
    fn new(inner: &'a mut JsonWriter<W, F>) -> Self {
        Self {
            inner,
            cached_error: None,
        }
    }

    fn write_raw(&mut self, value: impl core::fmt::Display) -> Result<(), W::Error> {
        use core::fmt::Write;
        let _ = write!(self, "{}", value);
        match self.cached_error.take() {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    fn write_f64(&mut self, value: f64) -> Result<(), W::Error> {
        use core::fmt::Write;

        const UPPER_BOUND_LIT:f64 = 1e6;
        const LOWER_BOUND_LIT:f64 = 1e-3;

        if value.is_finite() {
            let _ = if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(self, "{}", value)
            }
            else {
                write!(self, "{:e}", value)
            };
            match self.cached_error.take() {
                None => Ok(()),
                Some(e) => Err(e),
            }
        }
        else {
            self.inner.write_bytes("null".as_bytes())
        }
    }

    fn write_f32(&mut self, value: f32) -> Result<(), W::Error> {
        use core::fmt::Write;

        const UPPER_BOUND_LIT:f32 = 1e6;
        const LOWER_BOUND_LIT:f32 = 1e-3;

        if value.is_finite() {
            let _ = if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(self, "{}", value)
            }
            else {
                write!(self, "{:e}", value)
            };
            match self.cached_error.take() {
                None => Ok(()),
                Some(e) => Err(e),
            }
        }
        else {
            self.inner.write_bytes("null".as_bytes())
        }
    }
}
impl<'a, W: BlockingWrite, F: JsonFormatter> core::fmt::Write for FormatWrapper<'a, W, F> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self.inner.write_bytes(s.as_bytes()) {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                self.cached_error = Some(e);
                Err(core::fmt::Error)
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::io;

    #[test]
    fn test_json_writer() {
        _test_json_writer(JsonWriter::new(Vec::new(), CompactFormatter));
    }

    #[test]
    fn test_json_writer_compact() {
        _test_json_writer(JsonWriter::new_compact(Vec::new()));
    }

    #[test]
    fn test_json_writer_pretty() {
        _test_json_writer(JsonWriter::new_pretty(Vec::new()));
    }

    fn _test_json_writer<F: JsonFormatter>(mut writer: JsonWriter<Vec<u8>, F>) {
        writer.write_bytes(b"a").unwrap();
        writer.write_bytes(b"b").unwrap();
        writer.write_bytes(b"cde").unwrap();

        writer.flush().unwrap();

        assert_eq!(as_written_string(writer), "abcde");
    }

    fn as_written_string<F: JsonFormatter>(writer: JsonWriter<Vec<u8>, F>) -> String {
        let s = writer.into_inner().unwrap();
        String::from_utf8(s).unwrap()
    }

    #[rstest]
    #[case::empty("", r#""""#)]
    #[case::text("yo", r#""yo""#)]
    #[case::non_ascii("äöü", r#""äöü""#)]
    #[case::quotation_mark("\"", r#""\"""#)]
    #[case::backquote("\\", r#""\\""#)]
    #[case::backspace("\x08", r#""\b""#)]
    #[case::form_feed("\x0c", r#""\f""#)]
    #[case::line_feed("\n", r#""\n""#)]
    #[case::carriage_return("\r", r#""\r""#)]
    #[case::tab("\t", r#""\t""#)]
    #[case::esc_00("\x00", r#""\u0000""#)]
    #[case::esc_01("\x01", r#""\u0001""#)]
    #[case::esc_02("\x02", r#""\u0002""#)]
    #[case::esc_03("\x03", r#""\u0003""#)]
    #[case::esc_04("\x04", r#""\u0004""#)]
    #[case::esc_05("\x05", r#""\u0005""#)]
    #[case::esc_06("\x06", r#""\u0006""#)]
    #[case::esc_07("\x07", r#""\u0007""#)]
    #[case::esc_08("\x08", r#""\b""#)]
    #[case::esc_09("\x09", r#""\t""#)]
    #[case::esc_0a("\x0a", r#""\n""#)]
    #[case::esc_0b("\x0b", r#""\u000b""#)]
    #[case::esc_0c("\x0c", r#""\f""#)]
    #[case::esc_0d("\x0d", r#""\r""#)]
    #[case::esc_0e("\x0e", r#""\u000e""#)]
    #[case::esc_0f("\x0f", r#""\u000f""#)]
    #[case::esc_10("\x10", r#""\u0010""#)]
    #[case::esc_11("\x11", r#""\u0011""#)]
    #[case::esc_12("\x12", r#""\u0012""#)]
    #[case::esc_13("\x13", r#""\u0013""#)]
    #[case::esc_14("\x14", r#""\u0014""#)]
    #[case::esc_15("\x15", r#""\u0015""#)]
    #[case::esc_16("\x16", r#""\u0016""#)]
    #[case::esc_17("\x17", r#""\u0017""#)]
    #[case::esc_18("\x18", r#""\u0018""#)]
    #[case::esc_19("\x19", r#""\u0019""#)]
    #[case::esc_1a("\x1a", r#""\u001a""#)]
    #[case::esc_1b("\x1b", r#""\u001b""#)]
    #[case::esc_1c("\x1c", r#""\u001c""#)]
    #[case::esc_1d("\x1d", r#""\u001d""#)]
    #[case::esc_1e("\x1e", r#""\u001e""#)]
    #[case::esc_1f("\x1f", r#""\u001f""#)]
    #[case::combination("asdf \n jklö \t!", r#""asdf \n jklö \t!""#)]
    fn test_write_escaped_string(#[case] s: &str, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_escaped_string(s).unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[rstest]
    #[case::bool_true(true, "true")]
    #[case::bool_false(false, "false")]
    fn test_write_bool(#[case] b: bool, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_bool(b).unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[rstest]
    #[case::simple(2.0, "2")]
    #[case::exp_5(1.234e5, "123400")]
    #[case::exp_10(1.234e10, "1.234e10")]
    #[case::exp_20(1.234e20, "1.234e20")]
    #[case::exp_neg_3(1.234e-3, "0.001234")]
    #[case::exp_neg_10(1.234e-10, "1.234e-10")]
    #[case::neg_simple(-2.0, "-2")]
    #[case::neg_exp_5(-1.234e5, "-123400")]
    #[case::neg_exp_10(-1.234e10, "-1.234e10")]
    #[case::neg_exp_20(-1.234e20, "-1.234e20")]
    #[case::neg_exp_neg_3(-1.234e-3, "-0.001234")]
    #[case::neg_exp_neg_10(-1.234e-10, "-1.234e-10")]
    #[case::inf(f64::INFINITY, "null")]
    #[case::neg_inf(f64::NEG_INFINITY, "null")]
    #[case::nan(f64::NAN, "null")]
    fn test_write_f64(#[case] value: f64, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_f64(value).unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[rstest]
    #[case::simple(2.0, "2")]
    #[case::exp_5(1.234e5, "123400")]
    #[case::exp_10(1.234e10, "1.234e10")]
    #[case::exp_20(1.234e20, "1.234e20")]
    #[case::exp_neg_3(1.234e-3, "0.001234")]
    #[case::exp_neg_10(1.234e-10, "1.234e-10")]
    #[case::neg_simple(-2.0, "-2")]
    #[case::neg_exp_5(-1.234e5, "-123400")]
    #[case::neg_exp_10(-1.234e10, "-1.234e10")]
    #[case::neg_exp_20(-1.234e20, "-1.234e20")]
    #[case::neg_exp_neg_3(-1.234e-3, "-0.001234")]
    #[case::neg_exp_neg_10(-1.234e-10, "-1.234e-10")]
    #[case::inf(f32::INFINITY, "null")]
    #[case::neg_inf(f32::NEG_INFINITY, "null")]
    #[case::nan(f32::NAN, "null")]
    fn test_write_f32(#[case] value: f32, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_f32(value).unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[test]
    fn test_set_reported_error() {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_bytes(b"yo").unwrap();

        writer.set_unreported_error(io::Error::new(io::ErrorKind::Other, "something went wrong"));
        match writer.write_bytes(b" after error") {
            Ok(_) => {
                panic!("previous error should have been returned");
            }
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Other);
                assert_eq!(e.to_string(), "something went wrong");
            },
        }

        assert_eq!(as_written_string(writer), "yo");
    }

    #[test]
    fn test_flush() {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_bytes(b"yo").unwrap();

        writer.set_unreported_error(io::Error::new(io::ErrorKind::Other, "something went wrong"));

        match writer.flush() {
            Ok(_) => {
                panic!("previous error should have been returned");
            }
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Other);
                assert_eq!(e.to_string(), "something went wrong");
            },
        }

        assert_eq!(as_written_string(writer), "yo");
    }
}
