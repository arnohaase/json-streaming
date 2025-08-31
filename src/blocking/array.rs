use crate::blocking::io::BlockingWrite;
use crate::blocking::json_writer::JsonWriter;
use crate::blocking::object::JsonObject;
use crate::shared::float_format::FloatFormat;
use crate::shared::json_formatter::JsonFormatter;

/// An [JsonArray] is the API for writing a JSON array, i.e. a sequence of elements. The 
///  closing `]` is written when the [JsonArray] instance goes out of scope, or when its `end()`
///  function is called.
///
/// For nested objects or arrays, the function calls return new [JsonObject] or [JsonArray] instances,
///  respectively. Rust's type system ensures that applications can only interact with the innermost
///  such instance, and call outer instances only when all nested instances have gone out of scope.   
///
/// A typical use of the library is to create a [JsonWriter] and then wrap it in a top-level 
///  [JsonArray] instance.
pub struct JsonArray<'a, 'b, W: BlockingWrite, F: JsonFormatter, FF: FloatFormat> {
    writer: &'a mut JsonWriter<'b, W, F, FF>,
    is_initial: bool,
    is_ended: bool,
}

impl<'a, 'b, W: BlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonArray<'a, 'b, W, F, FF> {
    /// Create a new [JsonArray] instance. Application code can do this explicitly only initially
    ///  as a starting point for writing JSON. Nested arrays are created by the library.
    pub fn new(writer: &'a mut JsonWriter<'b, W, F, FF>) -> Result<Self, W::Error> {
        writer.write_bytes(b"[")?;
        writer.write_format_after_start_nested()?;

        Ok(JsonArray {
            writer,
            is_initial: true,
            is_ended: false,
        })
    }

    fn handle_initial(&mut self) -> Result<(), W::Error> {
        if self.is_initial {
            self.is_initial = false;
        }
        else {
            self.writer.write_bytes(b",")?;
            self.writer.write_format_after_element()?;
        }
        self.writer.write_format_indent()?;
        Ok(())
    }

    /// Write an element of type 'string', escaping the provided string value.
    pub fn write_string_value(&mut self, value: &str) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_escaped_string(value)
    }

    /// Write an element of type 'bool'.
    pub fn write_bool_value(&mut self, value: bool) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bool(value)
    }

    /// Write a null literal as an element.
    pub fn write_null_value(&mut self) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bytes(b"null")
    }

    /// Write an f64 value as an element. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the element
    ///  for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub fn write_f64_value(&mut self, value: f64) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_f64(value)
    }

    /// Write an f32 value as an element. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the element
    ///  for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub fn write_f32_value(&mut self, value: f32) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_f32(value)
    }

    /// Start a nested object as an element. This function returns a new [JsonObject] instance
    ///  for writing elements to the nested object. When the returned [JsonObject] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub fn start_object<'c, 'x>(&'x mut self) -> Result<JsonObject<'c, 'b, W, F, FF>, W::Error>
    where 'a: 'c, 'x: 'c
    {
        self.handle_initial()?;
        JsonObject::new(self.writer)
    }

    /// Start a nested array as an element. This function returns a new [JsonArray] instance
    ///  for writing elements to the nested object. When the returned [JsonArray] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub fn start_array<'c, 'x>(&'x mut self) -> Result<JsonArray<'c, 'b, W, F, FF>, W::Error>
    where 'a: 'c, 'x: 'c
    {
        self.handle_initial()?;
        JsonArray::new(self.writer)
    }

    /// Explicitly end this array's lifetime and write the closing bracket.
    pub fn end(self) -> Result<(), W::Error> {
        let mut mut_self = self;
        mut_self._end()
    }

    fn _end(&mut self) -> Result<(), W::Error> {
        self.writer.write_format_before_end_nested(self.is_initial)?;
        self.writer.write_bytes(b"]")?;
        self.is_ended = true;
        Ok(())
    }
}

macro_rules! write_arr_int {
    ($t:ty ; $f:ident) => {
impl<'a, 'b, W: BlockingWrite, F: JsonFormatter, FF: FloatFormat> JsonArray<'a, 'b, W, F, FF> {
    /// Write an element with a generic int value. This function fits most Rust integral
    ///  types; for the exceptions, there are separate functions.
    pub fn $f(&mut self, value: $t) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_raw_num(value)
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



impl <'a, 'b, W: BlockingWrite, F: JsonFormatter, FF: FloatFormat> Drop for JsonArray<'a, 'b, W, F, FF> {
    fn drop(&mut self) {
        if !self.is_ended {
            if let Err(e) = self._end() {
                self.writer.set_unreported_error(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::float_format::DefaultFloatFormat;
    use rstest::*;
    use std::io;
    use crate::shared::json_formatter::CompactFormatter;

    type AS<'a, 'b> = JsonArray<'a, 'b, Vec<u8>, CompactFormatter, DefaultFloatFormat>;

    #[rstest]
    #[case::empty(Box::new(|_ser: &mut AS| Ok(())), "[]")]
    #[case::single(Box::new(|ser: &mut AS| ser.write_null_value()), "[null]")]
    #[case::two(Box::new(|ser: &mut AS| { ser.write_u32_value(1)?; ser.write_u32_value(2) }), "[1,2]")]
    #[case::nested_arr(Box::new(|ser: &mut AS| {ser.start_array()?.end() }), "[[]]")]
    #[case::nested_arr_with_el(Box::new(|ser: &mut AS| {let mut n = ser.start_array()?; n.write_u32_value(5) }), "[[5]]")]
    #[case::nested_arr_first(Box::new(|ser: &mut AS| { ser.start_array()?.end()?; ser.write_u32_value(4) }), "[[],4]")]
    #[case::nested_arr_last(Box::new(|ser: &mut AS| { ser.write_u32_value(6)?; ser.start_array()?.end() }), "[6,[]]")]
    #[case::nested_arr_between(Box::new(|ser: &mut AS| { ser.write_u32_value(7)?; ser.start_array()?.end()?; ser.write_u32_value(9) }), "[7,[],9]")]
    #[case::two_nested_arrays(Box::new(|ser: &mut AS| { ser.start_array()?.end()?; ser.start_array()?.end() }), "[[],[]]")]
    #[case::nested_obj(Box::new(|ser: &mut AS| ser.start_object()?.end()), "[{}]")]
    #[case::nested_obj_with_el(Box::new(|ser: &mut AS| { let mut n=ser.start_object()?; n.write_u32_value("a", 3) }), r#"[{"a":3}]"#)]
    #[case::nested_obj_first(Box::new(|ser: &mut AS| { ser.start_object()?.end()?; ser.write_u32_value(0) }), r#"[{},0]"#)]
    #[case::nested_obj_last(Box::new(|ser: &mut AS| { ser.write_u32_value(2)?; ser.start_object()?.end() }), r#"[2,{}]"#)]
    #[case::two_nested_objects(Box::new(|ser: &mut AS| { ser.start_object()?.end()?; ser.start_object()?.end() }), r#"[{},{}]"#)]
    fn test_array(#[case] code: Box<dyn Fn(&mut AS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        let mut buf = Vec::new();
        let mut writer = JsonWriter::new_compact(&mut buf);
        {
            let mut array_ser = JsonArray::new(&mut writer)?;
            code(&mut array_ser)?;
        }

        let actual = String::from_utf8(writer.into_inner()?.to_vec()).unwrap();
        assert_eq!(actual, expected);
        Ok(())
    }

    #[rstest]
    #[case::null(Box::new(|w: &mut AS| w.write_null_value()), "null")]
    #[case::bool_true(Box::new(|w: &mut AS| w.write_bool_value(true)), "true")]
    #[case::bool_false(Box::new(|w: &mut AS| w.write_bool_value(false)), "false")]
    #[case::string(Box::new(|w: &mut AS| w.write_string_value("asdf")), r#""asdf""#)]
    #[case::string_escaped(Box::new(|w: &mut AS| w.write_string_value("\r\n")), r#""\r\n""#)]
    #[case::u8(Box::new(|w: &mut AS| w.write_u8_value(2u8)), "2")]
    #[case::i8(Box::new(|w: &mut AS| w.write_i8_value(-3i8)), "-3")]
    #[case::u16(Box::new(|w: &mut AS| w.write_u16_value(4u16)), "4")]
    #[case::i16(Box::new(|w: &mut AS| w.write_i16_value(-5i16)), "-5")]
    #[case::u32(Box::new(|w: &mut AS| w.write_u32_value(6u32)), "6")]
    #[case::i32(Box::new(|w: &mut AS| w.write_i32_value(-7i32)), "-7")]
    #[case::u64(Box::new(|w: &mut AS| w.write_u64_value(8u64)), "8")]
    #[case::i64(Box::new(|w: &mut AS| w.write_i64_value(-9i64)), "-9")]
    #[case::u128(Box::new(|w: &mut AS| w.write_u128_value(12u128)), "12")]
    #[case::i128(Box::new(|w: &mut AS| w.write_i128_value(-13i128)), "-13")]
    #[case::usize(Box::new(|w: &mut AS| w.write_usize_value(10usize)), "10")]
    #[case::isize(Box::new(|w: &mut AS| w.write_isize_value(-11isize)), "-11")]
    #[case::f64(Box::new(|w: &mut AS| w.write_f64_value(2.0)), "2")]
    #[case::f64_exp_5(Box::new(|w: &mut AS| w.write_f64_value(1.234e5)), "123400")]
    #[case::f64_exp_10(Box::new(|w: &mut AS| w.write_f64_value(1.234e10)), "1.234e10")]
    #[case::f64_exp_20(Box::new(|w: &mut AS| w.write_f64_value(1.234e20)), "1.234e20")]
    #[case::f64_exp_neg_3(Box::new(|w: &mut AS| w.write_f64_value(1.234e-3)), "0.001234")]
    #[case::f64_exp_neg_10(Box::new(|w: &mut AS| w.write_f64_value(1.234e-10)), "1.234e-10")]
    #[case::f64_neg(Box::new(|w: &mut AS| w.write_f64_value(-2.0)), "-2")]
    #[case::f64_neg_exp_5(Box::new(|w: &mut AS| w.write_f64_value(-1.234e5)), "-123400")]
    #[case::f64_neg_exp_10(Box::new(|w: &mut AS| w.write_f64_value(-1.234e10)), "-1.234e10")]
    #[case::f64_neg_exp_20(Box::new(|w: &mut AS| w.write_f64_value(-1.234e20)), "-1.234e20")]
    #[case::f64_neg_exp_neg_3(Box::new(|w: &mut AS| w.write_f64_value(-1.234e-3)), "-0.001234")]
    #[case::f64_neg_exp_neg_10(Box::new(|w: &mut AS| w.write_f64_value(-1.234e-10)), "-1.234e-10")]
    #[case::f64_inf(Box::new(|w: &mut AS| w.write_f64_value(f64::INFINITY)), "null")]
    #[case::f64_neg_inf(Box::new(|w: &mut AS| w.write_f64_value(f64::NEG_INFINITY)), "null")]
    #[case::f64_nan(Box::new(|w: &mut AS| w.write_f64_value(f64::NAN)), "null")]
    #[case::f32(Box::new(|w: &mut AS| w.write_f32_value(2.0)), "2")]
    #[case::f32_exp_5(Box::new(|w: &mut AS| w.write_f32_value(1.234e5)), "123400")]
    #[case::f32_exp_10(Box::new(|w: &mut AS| w.write_f32_value(1.234e10)), "1.234e10")]
    #[case::f32_exp_20(Box::new(|w: &mut AS| w.write_f32_value(1.234e20)), "1.234e20")]
    #[case::f32_exp_neg_3(Box::new(|w: &mut AS| w.write_f32_value(1.234e-3)), "0.001234")]
    #[case::f32_exp_neg_10(Box::new(|w: &mut AS| w.write_f32_value(1.234e-10)), "1.234e-10")]
    #[case::f32_neg(Box::new(|w: &mut AS| w.write_f32_value(-2.0)), "-2")]
    #[case::f32_neg_exp_5(Box::new(|w: &mut AS| w.write_f32_value(-1.234e5)), "-123400")]
    #[case::f32_neg_exp_10(Box::new(|w: &mut AS| w.write_f32_value(-1.234e10)), "-1.234e10")]
    #[case::f32_neg_exp_20(Box::new(|w: &mut AS| w.write_f32_value(-1.234e20)), "-1.234e20")]
    #[case::f32_neg_exp_neg_3(Box::new(|w: &mut AS| w.write_f32_value(-1.234e-3)), "-0.001234")]
    #[case::f32_neg_exp_neg_10(Box::new(|w: &mut AS| w.write_f32_value(-1.234e-10)), "-1.234e-10")]
    #[case::f32_inf(Box::new(|w: &mut AS| w.write_f32_value(f32::INFINITY)), "null")]
    #[case::f32_neg_inf(Box::new(|w: &mut AS| w.write_f32_value(f32::NEG_INFINITY)), "null")]
    #[case::f32_nan(Box::new(|w: &mut AS| w.write_f32_value(f32::NAN)), "null")]
    fn test_write_value(#[case] code: Box<dyn Fn(&mut AS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        {
            let mut buf = Vec::new();
            let mut writer = JsonWriter::new_compact(&mut buf);
            {
                let mut array_ser = JsonArray::new(&mut writer)?;
                code(&mut array_ser)?;
            }

            let actual = String::from_utf8(writer.into_inner()?.to_vec()).unwrap(); //TODO here and elsewhere: just use 'buf'
            let expected = format!("[{}]", expected);
            assert_eq!(actual, expected);
        }

        // test with and without preceding element to verify that 'initial' is handled correctly
        {
            let mut buf = Vec::new();
            let mut writer = JsonWriter::new_compact(&mut buf);
            {
                let mut array_ser = JsonArray::new(&mut writer)?;
                array_ser.write_null_value()?;
                code(&mut array_ser)?;
            }

            let actual = String::from_utf8(writer.into_inner()?.to_vec()).unwrap();
            let expected = format!("[null,{}]", expected);
            assert_eq!(actual, expected);
        }

        Ok(())
    }
}
