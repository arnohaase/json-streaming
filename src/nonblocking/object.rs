use crate::shared::float_format::FloatFormat;
use crate::shared::json_formatter::JsonFormatter;
use crate::nonblocking::array::JsonArray;
use crate::nonblocking::io::NonBlockingWrite;
use crate::nonblocking::json_writer::JsonWriter;

/// An [JsonObject] is the API for writing a JSON object, i.e. a sequence of key/value pairs. The
///  closing `}` is written when the [JsonObject] instance goes out of scope, or when its `end()`
///  function is called.
///
/// For nested objects or arrays, the function calls return new [JsonObject] or [JsonArray] instances,
///  respectively. Rust's type system ensures that applications can only interact with the innermost
///  such instance, and call outer instances only when all nested instances have gone out of scope.
///
/// A typical use of the library is to create a [JsonWriter] and then wrap it in a top-level
///  [JsonObject] instance.
pub struct JsonObject<'a, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> {
    writer: &'a mut JsonWriter<W, F, FF>,
    is_initial: bool,
    is_ended: bool,
}
impl<'a, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonObject<'a, W, F, FF> {
    /// Create a new [JsonObject] instance. Application code can do this explicitly only initially
    ///  as a starting point for writing JSON. Nested objects are created by the library.
    pub async fn new(writer: &'a mut JsonWriter<W, F, FF>) -> Result<Self, W::Error> {
        writer.write_bytes(b"{").await?;
        writer.write_format_after_start_nested().await?;
        Ok(JsonObject {
            writer,
            is_initial: true,
            is_ended: false,
        })
    }

    async fn write_key(&mut self, key: &str) -> Result<(), W::Error> {
        if !self.is_initial {
            self.writer.write_bytes(b",").await?;
            self.writer.write_format_after_element().await?;
        }
        self.is_initial = false;
        self.writer.write_format_indent().await?;
        self.writer.write_escaped_string(key).await?;
        self.writer.write_bytes(b":").await?;
        self.writer.write_format_after_key().await
    }

    /// Write a key/value pair with element type 'string', escaping the provided string value.
    pub async fn write_string_value(&mut self, key: &str, value: &str) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_escaped_string(value).await
    }

    /// Write a key/value pair with element type 'bool'
    pub async fn write_bool_value(&mut self, key: &str, value: bool) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_bool(value).await
    }

    /// Write a key with a null literal as its value
    pub async fn write_null_value(&mut self, key: &str) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_bytes(b"null").await
    }

    /// Write a key/value pair with an f64 value. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the whole key/value
    ///  pair for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub async fn write_f64_value(&mut self, key: &str, value: f64) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_f64(value).await
    }

    /// Write a key/value pair with an f32 value. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the whole key/value
    ///  pair for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub async fn write_f32_value(&mut self, key: &str, value: f32) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_f32(value).await
    }

    /// Start a nested object under a given key. This function returns a new [JsonObject] instance
    ///  for writing elements to the nested object. When the returned [JsonObject] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub async fn start_object(&mut self, key: &str) -> Result<JsonObject<'_, W, F, FF>, W::Error> {
        self.write_key(key).await?;
        JsonObject::new(&mut self.writer).await
    }

    /// Start a nested array under a given key. This function returns a new [JsonArray] instance
    ///  for writing elements to the nested object. When the returned [JsonArray] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested array is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub async fn start_array(&mut self, key: &str) -> Result<JsonArray<'_, W, F, FF>, W::Error> {
        self.write_key(key).await?;
        JsonArray::new(self.writer).await
    }

    /// Explicitly end this object's lifetime and write the closing bracket.
    pub async fn end(self) -> Result<(), W::Error> {
        let mut mut_self = self;
        mut_self._end().await
    }

    async fn _end(&mut self) -> Result<(), W::Error> {
        self.writer.write_format_before_end_nested(self.is_initial).await?;
        self.writer.write_bytes(b"}").await?;
        self.is_ended = true;
        Ok(())
    }
}

macro_rules! write_obj_int {
    ($t:ty ; $f:ident) => {
impl<'a, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonObject<'a, W, F, FF> {
    /// Write a key/value pair with an int value of type $t.
    pub async fn $f(&mut self, key: &str, value: $t) -> Result<(), W::Error> {
        self.write_key(key).await?;
        self.writer.write_raw_num(value).await
    }
}
    };
}
write_obj_int!(i8; write_i8_value);
write_obj_int!(u8; write_u8_value);
write_obj_int!(i16; write_i16_value);
write_obj_int!(u16; write_u16_value);
write_obj_int!(i32; write_i32_value);
write_obj_int!(u32; write_u32_value);
write_obj_int!(i64; write_i64_value);
write_obj_int!(u64; write_u64_value);
write_obj_int!(i128; write_i128_value);
write_obj_int!(u128; write_u128_value);
write_obj_int!(isize; write_isize_value);
write_obj_int!(usize; write_usize_value);



#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::shared::float_format::DefaultFloatFormat;
    use crate::shared::json_formatter::CompactFormatter;
    use crate::nonblocking::array::tests::ArrayCommand;
    use rstest::rstest;
    use std::io;

    pub enum ObjectCommand {
        Null(&'static str),
        Bool(&'static str, bool),
        String(&'static str, &'static str),
        U8(&'static str, u8),
        I8(&'static str, i8),
        U16(&'static str, u16),
        I16(&'static str, i16),
        U32(&'static str, u32),
        I32(&'static str, i32),
        U64(&'static str, u64),
        I64(&'static str, i64),
        U128(&'static str, u128),
        I128(&'static str, i128),
        Usize(&'static str, usize),
        Isize(&'static str, isize),
        F64(&'static str, f64),
        F32(&'static str, f32),
        Object(&'static str, Vec<ObjectCommand>),
        Array(&'static str, Vec<ArrayCommand>)
    }
    impl ObjectCommand {
        pub async fn apply(&self, obj: &mut JsonObject<'_, Vec<u8>, CompactFormatter, DefaultFloatFormat>) {
            match self {
                ObjectCommand::Null(key) => obj.write_null_value(key).await.unwrap(),
                ObjectCommand::Bool(key, b) => obj.write_bool_value(key, *b).await.unwrap(),
                ObjectCommand::String(key, s) => obj.write_string_value(key, s).await.unwrap(),
                ObjectCommand::U8(key, n) => obj.write_u8_value(key, *n).await.unwrap(),
                ObjectCommand::I8(key, n) => obj.write_i8_value(key, *n).await.unwrap(),
                ObjectCommand::U16(key, n) => obj.write_u16_value(key, *n).await.unwrap(),
                ObjectCommand::I16(key, n) => obj.write_i16_value(key, *n).await.unwrap(),
                ObjectCommand::U32(key, n) => obj.write_u32_value(key, *n).await.unwrap(),
                ObjectCommand::I32(key, n) => obj.write_i32_value(key, *n).await.unwrap(),
                ObjectCommand::U64(key, n) => obj.write_u64_value(key, *n).await.unwrap(),
                ObjectCommand::I64(key, n) => obj.write_i64_value(key, *n).await.unwrap(),
                ObjectCommand::U128(key, n) => obj.write_u128_value(key, *n).await.unwrap(),
                ObjectCommand::I128(key, n) => obj.write_i128_value(key, *n).await.unwrap(),
                ObjectCommand::Usize(key, n) => obj.write_usize_value(key, *n).await.unwrap(),
                ObjectCommand::Isize(key, n) => obj.write_isize_value(key, *n).await.unwrap(),
                ObjectCommand::F64(key, x) => obj.write_f64_value(key, *x).await.unwrap(),
                ObjectCommand::F32(key, x) => obj.write_f32_value(key, *x).await.unwrap(),
                ObjectCommand::Object(key, cmds) => {
                    let mut nested = obj.start_object(key).await.unwrap();
                    for cmd in cmds {
                        Box::pin(cmd.apply(&mut nested)).await;
                    }
                    nested.end().await.unwrap();
                }
                ObjectCommand::Array(key, cmds) => {
                    let mut nested = obj.start_array(key).await.unwrap();
                    for cmd in cmds {
                        Box::pin(cmd.apply(&mut nested)).await;
                    }
                    nested.end().await.unwrap();
                }
            }
        }
    }


    #[rstest]
    #[case::empty(vec![], "{}")]
    #[case::single(vec![ObjectCommand::Null("a")], r#"{"a":null}"#)]
    #[case::two(vec![ObjectCommand::U32("a", 1), ObjectCommand::U32("b", 2)], r#"{"a":1,"b":2}"#)]
    #[case::nested_arr(vec![ObjectCommand::Array("x", vec![])], r#"{"x":[]}"#)]
    #[case::nested_arr_with_el(vec![ObjectCommand::Array("y", vec![ArrayCommand::U32(5)])], r#"{"y":[5]}"#)]
    #[case::nested_arr_first(vec![ObjectCommand::Array("z", vec![]), ObjectCommand::U32("q", 4)], r#"{"z":[],"q":4}"#)]
    #[case::nested_arr_last(vec![ObjectCommand::U32("q", 6), ObjectCommand::Array("z", vec![])], r#"{"q":6,"z":[]}"#)]
    #[case::nested_arr_between(vec![ObjectCommand::U32("a", 7), ObjectCommand::Array("b", vec![]), ObjectCommand::U32("c", 9)], r#"{"a":7,"b":[],"c":9}"#)]
    #[case::two_nested_arrays(vec![ObjectCommand::Array("d", vec![]), ObjectCommand::Array("e", vec![])], r#"{"d":[],"e":[]}"#)]
    #[case::nested_obj(vec![ObjectCommand::Object("f", vec![])], r#"{"f":{}}"#)]
    #[case::nested_obj_with_el(vec![ObjectCommand::Object("g", vec![ObjectCommand::U32("a", 3)])], r#"{"g":{"a":3}}"#)]
    #[case::nested_obj_first(vec![ObjectCommand::Object("h", vec![]), ObjectCommand::U32("i", 0)], r#"{"h":{},"i":0}"#)]
    #[case::nested_obj_last(vec![ObjectCommand::U32("j", 2), ObjectCommand::Object("k", vec![])], r#"{"j":2,"k":{}}"#)]
    #[case::two_nested_objects(vec![ObjectCommand::Object("l", vec![]), ObjectCommand::Object("m", vec![])], r#"{"l":{},"m":{}}"#)]
    #[tokio::test]
    async fn test_object(#[case] cmds: Vec<ObjectCommand>, #[case] expected: &str) -> io::Result<()> {
        let mut writer = JsonWriter::new_compact(Vec::new());
        {
            let mut object_ser = JsonObject::new(&mut writer).await?;
            for cmd in cmds {
                cmd.apply(&mut object_ser).await;
            }
            object_ser.end().await?;
        }

        let actual = String::from_utf8(writer.into_inner()?).unwrap();
        assert_eq!(actual, expected);
        Ok(())
    }


    #[rstest]
    #[case::null(ObjectCommand::Null("a"), "null")]
    #[case::bool_true(ObjectCommand::Bool("a", true), "true")]
    #[case::bool_false(ObjectCommand::Bool("a", false), "false")]
    #[case::string(ObjectCommand::String("a", "asdf"), r#""asdf""#)]
    #[case::string_escaped(ObjectCommand::String("a", "\r\n"), r#""\r\n""#)]
    #[case::u8(ObjectCommand::U8("a", 2u8), "2")]
    #[case::i8(ObjectCommand::I8("a", -3i8), "-3")]
    #[case::u16(ObjectCommand::U16("a", 4u16), "4")]
    #[case::i16(ObjectCommand::I16("a", -5i16), "-5")]
    #[case::u32(ObjectCommand::U32("a", 6u32), "6")]
    #[case::i32(ObjectCommand::I32("a", -7i32), "-7")]
    #[case::u64(ObjectCommand::U64("a", 8u64), "8")]
    #[case::i64(ObjectCommand::I64("a", -9i64), "-9")]
    #[case::u128(ObjectCommand::U128("a", 12u128), "12")]
    #[case::i128(ObjectCommand::I128("a", -13i128), "-13")]
    #[case::usize(ObjectCommand::Usize("a", 10usize), "10")]
    #[case::isize(ObjectCommand::Isize("a", -11isize), "-11")]
    #[case::f64(ObjectCommand::F64("a", 2.0), "2")]
    #[case::f64_exp_5(ObjectCommand::F64("a", 1.234e5), "123400")]
    #[case::f64_exp_10(ObjectCommand::F64("a", 1.234e10), "1.234e10")]
    #[case::f64_exp_20(ObjectCommand::F64("a", 1.234e20), "1.234e20")]
    #[case::f64_exp_neg_3(ObjectCommand::F64("a", 1.234e-3), "0.001234")]
    #[case::f64_exp_neg_10(ObjectCommand::F64("a", 1.234e-10), "1.234e-10")]
    #[case::f64_neg(ObjectCommand::F64("a", -2.0), "-2")]
    #[case::f64_neg_exp_5(ObjectCommand::F64("a", -1.234e5), "-123400")]
    #[case::f64_neg_exp_10(ObjectCommand::F64("a", -1.234e10), "-1.234e10")]
    #[case::f64_neg_exp_20(ObjectCommand::F64("a", -1.234e20), "-1.234e20")]
    #[case::f64_neg_exp_neg_3(ObjectCommand::F64("a", -1.234e-3), "-0.001234")]
    #[case::f64_neg_exp_neg_10(ObjectCommand::F64("a", -1.234e-10), "-1.234e-10")]
    #[case::f64_inf(ObjectCommand::F64("a", f64::INFINITY), "null")]
    #[case::f64_neg_inf(ObjectCommand::F64("a", f64::NEG_INFINITY), "null")]
    #[case::f64_nan(ObjectCommand::F64("a", f64::NAN), "null")]
    #[case::f32(ObjectCommand::F32("a", 2.0), "2")]
    #[case::f32_exp_5(ObjectCommand::F32("a", 1.234e5), "123400")]
    #[case::f32_exp_10(ObjectCommand::F32("a", 1.234e10), "1.234e10")]
    #[case::f32_exp_20(ObjectCommand::F32("a", 1.234e20), "1.234e20")]
    #[case::f32_exp_neg_3(ObjectCommand::F32("a", 1.234e-3), "0.001234")]
    #[case::f32_exp_neg_10(ObjectCommand::F32("a", 1.234e-10), "1.234e-10")]
    #[case::f32_neg(ObjectCommand::F32("a", -2.0), "-2")]
    #[case::f32_neg_exp_5(ObjectCommand::F32("a", -1.234e5), "-123400")]
    #[case::f32_neg_exp_10(ObjectCommand::F32("a", -1.234e10), "-1.234e10")]
    #[case::f32_neg_exp_20(ObjectCommand::F32("a", -1.234e20), "-1.234e20")]
    #[case::f32_neg_exp_neg_3(ObjectCommand::F32("a", -1.234e-3), "-0.001234")]
    #[case::f32_neg_exp_neg_10(ObjectCommand::F32("a", -1.234e-10), "-1.234e-10")]
    #[case::f32_inf(ObjectCommand::F32("a", f32::INFINITY), "null")]
    #[case::f32_neg_inf(ObjectCommand::F32("a", f32::NEG_INFINITY), "null")]
    #[case::f32_nan(ObjectCommand::F32("a", f32::NAN), "null")]
    #[tokio::test]
    async fn test_write_value(#[case] cmd: ObjectCommand, #[case] expected: &str) -> io::Result<()> {
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut object_ser = JsonObject::new(&mut writer).await?;
                cmd.apply(&mut object_ser).await;
                object_ser.end().await?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!(r#"{}"a":{}{}"#, "{", expected, "}");
            assert_eq!(actual, expected);
        }

        // test with and without preceding element to verify that 'initial' is handled correctly
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut object_ser = JsonObject::new(&mut writer).await?;
                object_ser.write_null_value("x").await?;
                cmd.apply(&mut object_ser).await;
                object_ser.write_u32_value("y", 5).await?;
                object_ser.end().await?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!(r#"{}"x":null,"a":{},"y":5{}"#, "{", expected, "}");
            assert_eq!(actual, expected);
        }

        Ok(())
    }
}