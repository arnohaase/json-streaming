use crate::shared::float_format::{DefaultFloatFormat, FloatFormat};
use crate::shared::json_formatter::{CompactFormatter, JsonFormatter, PrettyFormatter};
use crate::nonblocking::io::NonBlockingWrite;
use core::fmt::Display;
use core::marker::PhantomData;

/// [JsonWriter] is the starting point for serializing JSON with this library. It is a thin wrapper
///  around a [Write], adding some JSON specifics and also formatting.
/// 
/// Application code should usually not have to interact with [JsonWriter] directly, but through
///  [ObjectSer] or [ArraySer] wrapped around it.
pub struct JsonWriter <W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> {
    inner: W, //TODO &mut W
    formatter: F,
    number_write_buf: NumWriteBuf,
    _float_format: FF,
}

impl <W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonWriter<W, F, FF> {
    /// Create a new [JsonWriter] instance for given [Write] instance and an explicitly provided
    ///  [JsonFormatter]. It gives full flexibility; for most cases, `new_compact()` and 
    ///  `new_pretty()` functions are more convenient. 
    pub fn new(inner: W, formatter: F, float_format: FF) -> JsonWriter<W, F, FF> {
        JsonWriter {
            inner,
            formatter,
            number_write_buf: NumWriteBuf::new(),
            _float_format: float_format,
        }
    }

    /// Internal API for writing raw bytes to the underlying [Write].
    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), W::Error> {
        self.inner.write_all(data).await
    }

    /// Internal API for writing a string as an escaped JSON string.
    pub async fn write_escaped_string(&mut self, s: &str) -> Result<(), W::Error> {
        self.write_bytes(b"\"").await?;
        for b in s.bytes() {
            match b {
                b'"' => self.write_bytes(b"\\\"").await?,
                b'\\' => self.write_bytes(b"\\\\").await?,
                b'\x08' => self.write_bytes(b"\\b").await?,
                b'\x0c' => self.write_bytes(b"\\f").await?,
                b'\n' => self.write_bytes(b"\\n").await?,
                b'\r' => self.write_bytes(b"\\r").await?,
                b'\t' => self.write_bytes(b"\\t").await?,
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
                    self.write_bytes(bytes).await?
                },
                b => self.write_bytes(&[b]).await?,
            }
        }
        self.write_bytes(b"\"").await?;
        Ok(())
    }

    /// Internal API for writing a `bool`.
    pub async fn write_bool(&mut self, value: bool) -> Result<(), W::Error> {
        if value {
            self.write_bytes(b"true").await
        }
        else {
            self.write_bytes(b"false").await
        }
    }

    /// Internal API for writing a floating point number, representing non-finite numbers as `null`. 
    pub async fn write_f64(&mut self, value: f64) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_f64(value).await
    }

    /// Internal API for writing a floating point number, representing non-finite numbers as `null`. 
    pub async fn write_f32(&mut self, value: f32) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_f32(value).await
    }

    /// internal API for writing raw int values
    pub async fn write_raw_num(&mut self, value: impl Display) -> Result<(), W::Error> {
        FormatWrapper::new(self)
            .write_raw(value).await
    }

    /// Internal API for interacting with the formatter
    pub async fn write_format_after_key(&mut self) -> Result<(), W::Error> {
        self.inner.write_all(self.formatter.after_key().as_bytes()).await
    }

    /// Internal API for interacting with the formatter
    pub async fn write_format_after_start_nested(&mut self) -> Result<(), W::Error> {
        self.inner.write_all(self.formatter.after_start_nested().as_bytes()).await
    }

    /// Internal API for interacting with the formatter
    pub async fn write_format_after_element(&mut self) -> Result<(), W::Error> {
        self.inner.write_all(self.formatter.after_element().as_bytes()).await
    }

    /// Internal API for interacting with the formatter
    pub async fn write_format_before_end_nested(&mut self, is_empty: bool) -> Result<(), W::Error> {
        self.inner.write_all(self.formatter.before_end_nested(is_empty).as_bytes()).await
    }

    /// Internal API for interacting with the formatter
    pub async fn write_format_indent(&mut self) -> Result<(), W::Error> {
        self.inner.write_all(self.formatter.indent().as_bytes()).await
    }

    /// End this [JsonWriter]'s lifetime, returning the [Write] instance it owned. This function
    ///  returns any unreported errors.
    pub fn into_inner(self) -> Result<W, W::Error> {
        Ok(self.inner)
    }
}

//TODO Rust Doc, move convenience to prominent place, documentation

impl <W: NonBlockingWrite> JsonWriter<W, CompactFormatter, DefaultFloatFormat> {
    pub fn new_compact(inner: W) -> Self {
        JsonWriter::new(inner, CompactFormatter, DefaultFloatFormat)
    }
}

impl <W: NonBlockingWrite> JsonWriter<W, PrettyFormatter, DefaultFloatFormat> {
    pub fn new_pretty(inner: W) -> Self {
        JsonWriter::new(inner, PrettyFormatter::new(), DefaultFloatFormat)
    }
}

struct NumWriteBuf {
    buf: [u8;40],
    len: usize,
}

impl NumWriteBuf {
    fn new() -> Self {
        Self {
            buf: [0u8;40],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    fn reset(&mut self) {
        self.len = 0;
    }
}
impl core::fmt::Write for NumWriteBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let old_len = self.len;
        self.len += s.len();
        self.buf[old_len..self.len].copy_from_slice(s.as_bytes());
        Ok(())
    }
}

struct FormatWrapper<'a, W: NonBlockingWrite, FF: FloatFormat> {
    buf: &'a mut NumWriteBuf,
    writer: &'a mut W,
    pd: PhantomData<FF>,
}
impl<'a, W: NonBlockingWrite, FF: FloatFormat> FormatWrapper<'a, W, FF> {
    fn new<F: JsonFormatter>(inner: &'a mut JsonWriter<W, F, FF>) -> Self {
        inner.number_write_buf.reset();
        Self {
            buf: &mut inner.number_write_buf,
            writer: &mut inner.inner,
            pd: PhantomData::default(),
        }
    }

    async fn write_raw(&mut self, value: impl Display) -> Result<(), W::Error> {
        use core::fmt::Write;

        let _ = write!(&mut self.buf, "{}", value);
        self.writer.write_all(&self.buf.as_bytes()).await
    }

    async fn write_f64(&mut self, value: f64) -> Result<(), W::Error> {
        let _ = FF::write_f64(&mut self.buf, value);
        self.writer.write_all(&self.buf.as_bytes()).await
    }

    async fn write_f32(&mut self, value: f32) -> Result<(), W::Error> {
        let _ = FF::write_f32(&mut self.buf, value);
        self.writer.write_all(&self.buf.as_bytes()).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::fmt::Write;

    #[tokio::test]
    async fn test_json_writer() {
        _test_json_writer(JsonWriter::new(Vec::new(), CompactFormatter, DefaultFloatFormat)).await;
    }

    #[tokio::test]
    async fn test_json_writer_compact() {
        _test_json_writer(JsonWriter::new_compact(Vec::new())).await;
    }

    #[tokio::test]
    async fn test_json_writer_pretty() {
        _test_json_writer(JsonWriter::new_pretty(Vec::new())).await;
    }

    async fn _test_json_writer<F: JsonFormatter>(mut writer: JsonWriter<Vec<u8>, F, DefaultFloatFormat>) {
        writer.write_bytes(b"a").await.unwrap();
        writer.write_bytes(b"b").await.unwrap();
        writer.write_bytes(b"cde").await.unwrap();

        assert_eq!(as_written_string(writer), "abcde");
    }

    fn as_written_string<F: JsonFormatter>(writer: JsonWriter<Vec<u8>, F, DefaultFloatFormat>) -> String {
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
    #[tokio::test]
    async fn test_write_escaped_string(#[case] s: &str, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_escaped_string(s).await.unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[rstest]
    #[case::bool_true(true, "true")]
    #[case::bool_false(false, "false")]
    #[tokio::test]
    async fn test_write_bool(#[case] b: bool, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_bool(b).await.unwrap();
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
    #[tokio::test]
    async fn test_write_f64(#[case] value: f64, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_f64(value).await.unwrap();
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
    #[tokio::test]
    async fn test_write_f32(#[case] value: f32, #[case] expected: &str) {
        let mut writer = JsonWriter::new_compact(Vec::new());
        writer.write_f32(value).await.unwrap();
        assert_eq!(as_written_string(writer), expected);
    }

    #[tokio::test]
    async fn test_float_format() {
        struct OtherFf;
        impl FloatFormat for OtherFf {
            fn write_f64(f: &mut impl Write, value: f64) -> std::fmt::Result {
                write!(f, "_{}_64", value)
            }

            fn write_f32(f: &mut impl Write, value: f32) -> std::fmt::Result {
                write!(f, "_{}_32", value)
            }
        }

        let mut writer = JsonWriter::new(Vec::new(), CompactFormatter, OtherFf);
        writer.write_f64(1.2).await.unwrap();
        writer.write_f32(3.4).await.unwrap();
        let written = writer.into_inner().unwrap();
        assert_eq!(&written, b"_1.2_64_3.4_32");
    }
}
