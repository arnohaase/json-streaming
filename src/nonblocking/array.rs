use crate::nonblocking::io::NonBlockingWrite;
use crate::nonblocking::json_writer::JsonWriter;
use crate::nonblocking::object::JsonObject;
use crate::shared::*;

/// A [JsonArray] is the API for writing a JSON array, i.e. a sequence of elements. The
///  closing `]` is written when the [JsonArray] instance goes out of scope, or when its `end()`
///  function is called.
///
/// For nested objects or arrays, the function calls return new [JsonObject] or [JsonArray] instances,
///  respectively. Rust's type system ensures that applications can only interact with the innermost
///  such instance, and call outer instances only when all nested instances have gone out of scope.   
///
/// A typical use of the library is to create a [JsonWriter] and then wrap it in a top-level 
///  [JsonArray] instance.
pub struct JsonArray<'a, 'b, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> {
    writer: &'a mut JsonWriter<'b, W, F, FF>,
    is_initial: bool,
    is_ended: bool,
}

impl<'a, 'b, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonArray<'a, 'b, W, F, FF> {
    /// Create a new [JsonArray] instance. Application code can do this explicitly only initially
    ///  as a starting point for writing JSON. Nested arrays are created by the library.
    pub async fn new(writer: &'a mut JsonWriter<'b, W, F, FF>) -> Result<Self, W::Error> {
        writer.write_bytes(b"[").await?;
        writer.write_format_after_start_nested().await?;

        Ok(JsonArray {
            writer,
            is_initial: true,
            is_ended: false,
        })
    }

    async fn handle_initial(&mut self) -> Result<(), W::Error> {
        if self.is_initial {
            self.is_initial = false;
        }
        else {
            self.writer.write_bytes(b",").await?;
            self.writer.write_format_after_element().await?;
        }
        self.writer.write_format_indent().await?;
        Ok(())
    }

    /// Write an element of type 'string', escaping the provided string value.
    pub async fn write_string_value(&mut self, value: &str) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_escaped_string(value).await
    }

    /// Write an element of type 'bool'.
    pub async fn write_bool_value(&mut self, value: bool) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_bool(value).await
    }

    /// Write a null literal as an element.
    pub async fn write_null_value(&mut self) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_bytes(b"null").await
    }

    /// Write an f64 value as an element. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the element
    ///  for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub async fn write_f64_value(&mut self, value: f64) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_f64(value).await
    }

    /// Write an f32 value as an element. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the element
    ///  for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub async fn write_f32_value(&mut self, value: f32) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_f32(value).await
    }

    /// Start a nested object as an element. This function returns a new [JsonObject] instance
    ///  for writing elements to the nested object. When the returned [JsonObject] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub async fn start_object<'c, 'x>(&'x mut self) -> Result<JsonObject<'c, 'b, W, F, FF>, W::Error>
    where 'a: 'c, 'x: 'c
    {
        self.handle_initial().await?;
        JsonObject::new(self.writer).await
    }

    /// Start a nested array as an element. This function returns a new [JsonArray] instance
    ///  for writing elements to the nested object. When the returned [JsonArray] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub async fn start_array<'c, 'x>(&'x mut self) -> Result<JsonArray<'c, 'b, W, F, FF>, W::Error>
    where 'a: 'c, 'x: 'c
    {
        self.handle_initial().await?;
        JsonArray::new(self.writer).await
    }

    /// Explicitly end this array's lifetime and write the closing bracket.
    pub async fn end(self) -> Result<(), W::Error> {
        let mut mut_self = self;
        mut_self._end().await
    }

    async fn _end(&mut self) -> Result<(), W::Error> {
        self.writer.write_format_before_end_nested(self.is_initial).await?;
        self.writer.write_bytes(b"]").await?;
        self.is_ended = true;
        Ok(())
    }
}

macro_rules! write_arr_int {
    ($t:ty ; $f:ident) => {
impl<'a, 'b, W: NonBlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonArray<'a, 'b, W, F, FF> {
    /// Write an element with a generic int value. This function fits most Rust integral
    ///  types; for the exceptions, there are separate functions.
    pub async fn $f(&mut self, value: $t) -> Result<(), W::Error> {
        self.handle_initial().await?;
        self.writer.write_raw_num(value).await
    }
}
    };
}
write_arr_int!(i8; write_i8_value);
write_arr_int!(u8; write_u8_value);
write_arr_int!(i16; write_i16_value);
write_arr_int!(u16; write_u16_value);
write_arr_int!(i32; write_i32_value);
write_arr_int!(u32; write_u32_value);
write_arr_int!(i64; write_i64_value);
write_arr_int!(u64; write_u64_value);
write_arr_int!(i128; write_i128_value);
write_arr_int!(u128; write_u128_value);
write_arr_int!(isize; write_isize_value);
write_arr_int!(usize; write_usize_value);




#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::nonblocking::object::tests::ObjectCommand;
    use rstest::*;
    use std::io;

    pub enum ArrayCommand {
        Null,
        Bool(bool),
        String(&'static str),
        U8(u8),
        I8(i8),
        U16(u16),
        I16(i16),
        U32(u32),
        I32(i32),
        U64(u64),
        I64(i64),
        U128(u128),
        I128(i128),
        Usize(usize),
        Isize(isize),
        F64(f64),
        F32(f32),
        Object(Vec<ObjectCommand>),
        Array(Vec<ArrayCommand>)
    }
    impl ArrayCommand {
        pub async fn apply(&self, arr: &mut JsonArray<'_, '_, Vec<u8>, CompactFormatter, DefaultFloatFormat>) {
            match self {
                ArrayCommand::Null => arr.write_null_value().await.unwrap(),
                ArrayCommand::Bool(b) => arr.write_bool_value(*b).await.unwrap(),
                ArrayCommand::String(s) => arr.write_string_value(s).await.unwrap(),
                ArrayCommand::U8(n) => arr.write_u8_value(*n).await.unwrap(),
                ArrayCommand::I8(n) => arr.write_i8_value(*n).await.unwrap(),
                ArrayCommand::U16(n) => arr.write_u16_value(*n).await.unwrap(),
                ArrayCommand::I16(n) => arr.write_i16_value(*n).await.unwrap(),
                ArrayCommand::U32(n) => arr.write_u32_value(*n).await.unwrap(),
                ArrayCommand::I32(n) => arr.write_i32_value(*n).await.unwrap(),
                ArrayCommand::U64(n) => arr.write_u64_value(*n).await.unwrap(),
                ArrayCommand::I64(n) => arr.write_i64_value(*n).await.unwrap(),
                ArrayCommand::U128(n) => arr.write_u128_value(*n).await.unwrap(),
                ArrayCommand::I128(n) => arr.write_i128_value(*n).await.unwrap(),
                ArrayCommand::Usize(n) => arr.write_usize_value(*n).await.unwrap(),
                ArrayCommand::Isize(n) => arr.write_isize_value(*n).await.unwrap(),
                ArrayCommand::F64(x) => arr.write_f64_value(*x).await.unwrap(),
                ArrayCommand::F32(x) => arr.write_f32_value(*x).await.unwrap(),
                ArrayCommand::Object(cmds) => {
                    let mut nested = arr.start_object().await.unwrap();
                    for cmd in cmds {
                        Box::pin(cmd.apply(&mut nested)).await;
                    }
                    nested.end().await.unwrap();
                }
                ArrayCommand::Array(cmds) => {
                    let mut nested = arr.start_array().await.unwrap();
                    for cmd in cmds {
                        Box::pin(cmd.apply(&mut nested)).await;
                    }
                    nested.end().await.unwrap();
                }
            }
        }
    }

    #[rstest]
    #[case::empty(vec![], "[]")]
    #[case::single(vec![ArrayCommand::Null], "[null]")]
    #[case::two(vec![ArrayCommand::U32(1), ArrayCommand::U32(2)], "[1,2]")]
    #[case::nested_arr(vec![ArrayCommand::Array(vec![])], "[[]]")]
    #[case::nested_arr_with_el(vec![ArrayCommand::Array(vec![ArrayCommand::U32(5)])], "[[5]]")]
    #[case::nested_arr_first(vec![ArrayCommand::Array(vec![]), ArrayCommand::U32(4)], "[[],4]")]
    #[case::nested_arr_last(vec![ArrayCommand::U32(6), ArrayCommand::Array(vec![])], "[6,[]]")]
    #[case::nested_arr_between(vec![ArrayCommand::U32(7), ArrayCommand::Array(vec![]), ArrayCommand::U32(9)], "[7,[],9]")]
    #[case::two_nested_arrays(vec![ArrayCommand::Array(vec![]), ArrayCommand::Array(vec![])], "[[],[]]")]
    #[case::nested_obj(vec![ArrayCommand::Object(vec![])], "[{}]")]
    #[case::nested_obj_with_el(vec![ArrayCommand::Object(vec![ObjectCommand::U32("a", 3)])], r#"[{"a":3}]"#)]
    #[case::nested_obj_first(vec![ArrayCommand::Object(vec![]), ArrayCommand::U32(0)], r#"[{},0]"#)]
    #[case::nested_obj_last(vec![ArrayCommand::U32(2), ArrayCommand::Object(vec![])], r#"[2,{}]"#)]
    #[case::two_nested_objects(vec![ArrayCommand::Object(vec![]), ArrayCommand::Object(vec![])], r#"[{},{}]"#)]
    #[tokio::test]
    async fn test_array(#[case] code: Vec<ArrayCommand>, #[case] expected: &str) -> io::Result<()> {
        let mut buf = Vec::new();
        let mut writer = JsonWriter::new_compact(&mut buf);
        let mut array_ser = JsonArray::new(&mut writer).await?;
        for cmd in code {
            cmd.apply(&mut array_ser).await;
        }
        array_ser.end().await?;

        let actual = String::from_utf8(buf).unwrap();
        assert_eq!(actual, expected);
        Ok(())
    }

    #[rstest]
    #[case::null(ArrayCommand::Null, "null")]
    #[case::bool_true(ArrayCommand::Bool(true), "true")]
    #[case::bool_false(ArrayCommand::Bool(false), "false")]
    #[case::string(ArrayCommand::String("asdf"), r#""asdf""#)]
    #[case::string_escaped(ArrayCommand::String("\r\n"), r#""\r\n""#)]
    #[case::u8(ArrayCommand::U8(2), "2")]
    #[case::i8(ArrayCommand::I8(-3), "-3")]
    #[case::u16(ArrayCommand::U16(4), "4")]
    #[case::i16(ArrayCommand::I16(-5), "-5")]
    #[case::u32(ArrayCommand::U32(6), "6")]
    #[case::i32(ArrayCommand::I32(-7), "-7")]
    #[case::u64(ArrayCommand::U64(8), "8")]
    #[case::i64(ArrayCommand::I64(-9), "-9")]
    #[case::u128(ArrayCommand::U128(10), "10")]
    #[case::i128(ArrayCommand::I128(-11), "-11")]
    #[case::u128(ArrayCommand::Usize(12), "12")]
    #[case::i128(ArrayCommand::Isize(-13), "-13")]
    #[case::f64(ArrayCommand::F64(2.0), "2")]
    #[case::f64_exp_5(ArrayCommand::F64(1.234e5), "123400")]
    #[case::f64_exp_10(ArrayCommand::F64(1.234e10), "1.234e10")]
    #[case::f64_exp_20(ArrayCommand::F64(1.234e20), "1.234e20")]
    #[case::f64_exp_neg_3(ArrayCommand::F64(1.234e-3), "0.001234")]
    #[case::f64_exp_neg_10(ArrayCommand::F64(1.234e-10), "1.234e-10")]
    #[case::f64_neg(ArrayCommand::F64(-2.0), "-2")]
    #[case::f64_neg_exp_5(ArrayCommand::F64(-1.234e5), "-123400")]
    #[case::f64_neg_exp_10(ArrayCommand::F64(-1.234e10), "-1.234e10")]
    #[case::f64_neg_exp_20(ArrayCommand::F64(-1.234e20), "-1.234e20")]
    #[case::f64_neg_exp_neg_3(ArrayCommand::F64(-1.234e-3), "-0.001234")]
    #[case::f64_neg_exp_neg_10(ArrayCommand::F64(-1.234e-10), "-1.234e-10")]
    #[case::f64_inf(ArrayCommand::F64(f64::INFINITY), "null")]
    #[case::f64_neg_inf(ArrayCommand::F64(f64::NEG_INFINITY), "null")]
    #[case::f64_nan(ArrayCommand::F64(f64::NAN), "null")]
    #[case::f32(ArrayCommand::F32(2.0), "2")]
    #[case::f32_exp_5(ArrayCommand::F32(1.234e5), "123400")]
    #[case::f32_exp_10(ArrayCommand::F32(1.234e10), "1.234e10")]
    #[case::f32_exp_20(ArrayCommand::F32(1.234e20), "1.234e20")]
    #[case::f32_exp_neg_3(ArrayCommand::F32(1.234e-3), "0.001234")]
    #[case::f32_exp_neg_10(ArrayCommand::F32(1.234e-10), "1.234e-10")]
    #[case::f32_neg(ArrayCommand::F32(-2.0), "-2")]
    #[case::f32_neg_exp_5(ArrayCommand::F32(-1.234e5), "-123400")]
    #[case::f32_neg_exp_10(ArrayCommand::F32(-1.234e10), "-1.234e10")]
    #[case::f32_neg_exp_20(ArrayCommand::F32(-1.234e20), "-1.234e20")]
    #[case::f32_neg_exp_neg_3(ArrayCommand::F32(-1.234e-3), "-0.001234")]
    #[case::f32_neg_exp_neg_10(ArrayCommand::F32(-1.234e-10), "-1.234e-10")]
    #[case::f32_inf(ArrayCommand::F32(f32::INFINITY), "null")]
    #[case::f32_neg_inf(ArrayCommand::F32(f32::NEG_INFINITY), "null")]
    #[case::f32_nan(ArrayCommand::F32(f32::NAN), "null")]
    #[tokio::test]
    async fn test_write_value(#[case] cmd: ArrayCommand, #[case] expected: &str) -> io::Result<()> {
        {
            let mut buf = Vec::new();
            let mut writer = JsonWriter::new_compact(&mut buf);
            {
                let mut array_ser = JsonArray::new(&mut writer).await?;
                cmd.apply(&mut array_ser).await;
                array_ser.end().await?;
            }

            let actual = String::from_utf8(buf).unwrap();
            let expected = format!("[{}]", expected);
            assert_eq!(actual, expected);
        }

        // test with and without preceding element to verify that 'initial' is handled correctly
        {
            let mut buf = Vec::new();
            let mut writer = JsonWriter::new_compact(&mut buf);
            {
                let mut array_ser = JsonArray::new(&mut writer).await?;
                array_ser.write_null_value().await?;
                cmd.apply(&mut array_ser).await;
                array_ser.end().await?;
            }

            let actual = String::from_utf8(buf).unwrap();
            let expected = format!("[null,{}]", expected);
            assert_eq!(actual, expected);
        }

        Ok(())
    }
}
