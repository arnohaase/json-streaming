use crate::blocking::array::JsonArray;
use crate::blocking::io::BlockingWrite;
use crate::blocking::json_writer::JsonWriter;
use crate::blocking::JsonFormatter;

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
pub struct JsonObject<'a, W: BlockingWrite, F: JsonFormatter> {
    writer: &'a mut JsonWriter<W, F>,
    is_initial: bool,
    is_ended: bool,
}
impl<'a, W: BlockingWrite, F: JsonFormatter> JsonObject<'a, W, F> {
    /// Create a new [JsonObject] instance. Application code can do this explicitly only initially
    ///  as a starting point for writing JSON. Nested objects are created by the library.
    pub fn new(writer: &'a mut JsonWriter<W, F>) -> Result<Self, W::Error> {
        writer.write_bytes(b"{")?;
        writer.write_format_after_start_nested()?;
        Ok(JsonObject {
            writer,
            is_initial: true,
            is_ended: false,
        })
    }

    fn write_key(&mut self, key: &str) -> Result<(), W::Error> {
        if !self.is_initial {
            self.writer.write_bytes(b",")?;
            self.writer.write_format_after_element()?;
        }
        self.is_initial = false;
        self.writer.write_format_indent()?;
        self.writer.write_escaped_string(key)?;
        self.writer.write_bytes(b":")?;
        self.writer.write_format_after_key()
    }

    /// Write a key/value pair with element type 'string', escaping the provided string value.
    pub fn write_string_value(&mut self, key: &str, value: &str) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_escaped_string(value)
    }

    /// Write a key/value pair with element type 'bool'
    pub fn write_bool_value(&mut self, key: &str, value: bool) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_bool(value)
    }

    /// Write a key with a null literal as its value
    pub fn write_null_value(&mut self, key: &str) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_bytes(b"null")
    }

    /// Write a key/value pair with an f64 value. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the whole key/value
    ///  pair for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub fn write_f64_value(&mut self, key: &str, value: f64) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_f64(value)
    }

    /// Write a key/value pair with an f32 value. If the value is not finite (i.e. infinite or NaN),
    ///  a null literal is written instead. Different behavior (e.g. leaving out the whole key/value
    ///  pair for non-finite numbers, representing them in some other way etc.) is the responsibility
    ///  of application code.
    pub fn write_f32_value(&mut self, key: &str, value: f32) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_f32(value)
    }

    /// Start a nested object under a given key. This function returns a new [JsonObject] instance
    ///  for writing elements to the nested object. When the returned [JsonObject] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub fn start_object(&mut self, key: &str) -> Result<JsonObject<W, F>, W::Error> {
        self.write_key(key)?;
        JsonObject::new(&mut self.writer)
    }

    /// Start a nested array under a given key. This function returns a new [JsonArray] instance
    ///  for writing elements to the nested object. When the returned [JsonArray] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested array is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub fn start_array(&mut self, key: &str) -> Result<JsonArray<W, F>, W::Error> {
        self.write_key(key)?;
        JsonArray::new(self.writer)
    }

    /// Explicitly end this object's lifetime and write the closing bracket.
    pub fn end(self) -> Result<(), W::Error> {
        let mut mut_self = self;
        mut_self._end()
    }

    fn _end(&mut self) -> Result<(), W::Error> {
        self.writer.write_format_before_end_nested(self.is_initial)?;
        self.writer.write_bytes(b"}")?;
        self.is_ended = true;
        Ok(())
    }
}

macro_rules! write_obj_int {
    ($t:ty ; $f:ident) => {
impl<'a, W: BlockingWrite, F: JsonFormatter> JsonObject<'a, W, F> {
    /// Write a key/value pair with an int value of type $t.
    pub fn $f(&mut self, key: &str, value: $t) -> Result<(), W::Error> {
        self.write_key(key)?;
        self.writer.write_raw_num(value)
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

impl <'a, W: BlockingWrite, F: JsonFormatter> Drop for JsonObject<'a, W, F> {
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
    use crate::blocking::CompactFormatter;
    use rstest::*;
    use std::io;

    type OS<'a> = JsonObject<'a, Vec<u8>, CompactFormatter>;

    #[rstest]
    #[case::empty(Box::new(|_ser: &mut OS| Ok(())), "{}")]
    #[case::single(Box::new(|ser: &mut OS| ser.write_null_value("a")), r#"{"a":null}"#)]
    #[case::two(Box::new(|ser: &mut OS| { ser.write_u32_value("a", 1)?; ser.write_u32_value("b", 2) }), r#"{"a":1,"b":2}"#)]
    #[case::nested_arr(Box::new(|ser: &mut OS| { ser.start_array("x")?.end() }), r#"{"x":[]}"#)]
    #[case::nested_arr_with_el(Box::new(|ser: &mut OS| { let mut n = ser.start_array("y")?; n.write_u32_value(5) }), r#"{"y":[5]}"#)]
    #[case::nested_arr_first(Box::new(|ser: &mut OS| { ser.start_array("z")?.end()?; ser.write_u32_value("q", 4) }), r#"{"z":[],"q":4}"#)]
    #[case::nested_arr_last(Box::new(|ser: &mut OS| { ser.write_u32_value("q", 6)?; ser.start_array("z")?.end() }), r#"{"q":6,"z":[]}"#)]
    #[case::nested_arr_between(Box::new(|ser: &mut OS| { ser.write_u32_value("a", 7)?; ser.start_array("b")?.end()?; ser.write_u32_value("c", 9) }), r#"{"a":7,"b":[],"c":9}"#)]
    #[case::two_nested_arrays(Box::new(|ser: &mut OS| { ser.start_array("d")?.end()?; ser.start_array("e")?.end() }), r#"{"d":[],"e":[]}"#)]
    #[case::nested_obj(Box::new(|ser: &mut OS| { ser.start_object("f")?.end() }), r#"{"f":{}}"#)]
    #[case::nested_obj_with_el(Box::new(|ser: &mut OS| { let mut n=ser.start_object("g")?; n.write_u32_value("a", 3) }), r#"{"g":{"a":3}}"#)]
    #[case::nested_obj_first(Box::new(|ser: &mut OS| { ser.start_object("h")?.end()?; ser.write_u32_value("i", 0) }), r#"{"h":{},"i":0}"#)]
    #[case::nested_obj_last(Box::new(|ser: &mut OS| { ser.write_u32_value("j", 2)?; ser.start_object("k")?.end() }), r#"{"j":2,"k":{}}"#)]
    #[case::two_nested_objects(Box::new(|ser: &mut OS| { ser.start_object("l")?.end()?; ser.start_object("m")?.end() }), r#"{"l":{},"m":{}}"#)]
    fn test_object(#[case] code: Box<dyn Fn(&mut OS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        let mut writer = JsonWriter::new_compact(Vec::new());
        {
            let mut object_ser = JsonObject::new(&mut writer)?;
            code(&mut object_ser)?;
        }

        let actual = String::from_utf8(writer.into_inner()?).unwrap();
        assert_eq!(actual, expected);
        Ok(())
    }

    #[rstest]
    #[case::null(Box::new(|w: &mut OS| w.write_null_value("a")), "null")]
    #[case::bool_true(Box::new(|w: &mut OS| w.write_bool_value("a", true)), "true")]
    #[case::bool_false(Box::new(|w: &mut OS| w.write_bool_value("a", false)), "false")]
    #[case::string(Box::new(|w: &mut OS| w.write_string_value("a", "asdf")), r#""asdf""#)]
    #[case::string_escaped(Box::new(|w: &mut OS| w.write_string_value("a", "\r\n")), r#""\r\n""#)]
    #[case::u8(Box::new(|w: &mut OS| w.write_u8_value("a", 2u8)), "2")]
    #[case::i8(Box::new(|w: &mut OS| w.write_i8_value("a", -3i8)), "-3")]
    #[case::u16(Box::new(|w: &mut OS| w.write_u16_value("a", 4u16)), "4")]
    #[case::i16(Box::new(|w: &mut OS| w.write_i16_value("a", -5i16)), "-5")]
    #[case::u32(Box::new(|w: &mut OS| w.write_u32_value("a", 6u32)), "6")]
    #[case::i32(Box::new(|w: &mut OS| w.write_i32_value("a", -7i32)), "-7")]
    #[case::u64(Box::new(|w: &mut OS| w.write_u64_value("a", 8u64)), "8")]
    #[case::i64(Box::new(|w: &mut OS| w.write_i64_value("a", -9i64)), "-9")]
    #[case::u128(Box::new(|w: &mut OS| w.write_u128_value("a", 12u128)), "12")]
    #[case::i128(Box::new(|w: &mut OS| w.write_i128_value("a", -13i128)), "-13")]
    #[case::usize(Box::new(|w: &mut OS| w.write_usize_value("a", 10usize)), "10")]
    #[case::isize(Box::new(|w: &mut OS| w.write_isize_value("a", -11isize)), "-11")]
    #[case::f64(Box::new(|w: &mut OS| w.write_f64_value("a", 2.0)), "2")]
    #[case::f64_exp_5(Box::new(|w: &mut OS| w.write_f64_value("a", 1.234e5)), "123400")]
    #[case::f64_exp_10(Box::new(|w: &mut OS| w.write_f64_value("a", 1.234e10)), "1.234e10")]
    #[case::f64_exp_20(Box::new(|w: &mut OS| w.write_f64_value("a", 1.234e20)), "1.234e20")]
    #[case::f64_exp_neg_3(Box::new(|w: &mut OS| w.write_f64_value("a", 1.234e-3)), "0.001234")]
    #[case::f64_exp_neg_10(Box::new(|w: &mut OS| w.write_f64_value("a", 1.234e-10)), "1.234e-10")]
    #[case::f64_neg(Box::new(|w: &mut OS| w.write_f64_value("a", -2.0)), "-2")]
    #[case::f64_neg_exp_5(Box::new(|w: &mut OS| w.write_f64_value("a", -1.234e5)), "-123400")]
    #[case::f64_neg_exp_10(Box::new(|w: &mut OS| w.write_f64_value("a", -1.234e10)), "-1.234e10")]
    #[case::f64_neg_exp_20(Box::new(|w: &mut OS| w.write_f64_value("a", -1.234e20)), "-1.234e20")]
    #[case::f64_neg_exp_neg_3(Box::new(|w: &mut OS| w.write_f64_value("a", -1.234e-3)), "-0.001234")]
    #[case::f64_neg_exp_neg_10(Box::new(|w: &mut OS| w.write_f64_value("a", -1.234e-10)), "-1.234e-10")]
    #[case::f64_inf(Box::new(|w: &mut OS| w.write_f64_value("a", f64::INFINITY)), "null")]
    #[case::f64_neg_inf(Box::new(|w: &mut OS| w.write_f64_value("a", f64::NEG_INFINITY)), "null")]
    #[case::f64_nan(Box::new(|w: &mut OS| w.write_f64_value("a", f64::NAN)), "null")]
    #[case::f32(Box::new(|w: &mut OS| w.write_f32_value("a", 2.0)), "2")]
    #[case::f32_exp_5(Box::new(|w: &mut OS| w.write_f32_value("a", 1.234e5)), "123400")]
    #[case::f32_exp_10(Box::new(|w: &mut OS| w.write_f32_value("a", 1.234e10)), "1.234e10")]
    #[case::f32_exp_20(Box::new(|w: &mut OS| w.write_f32_value("a", 1.234e20)), "1.234e20")]
    #[case::f32_exp_neg_3(Box::new(|w: &mut OS| w.write_f32_value("a", 1.234e-3)), "0.001234")]
    #[case::f32_exp_neg_10(Box::new(|w: &mut OS| w.write_f32_value("a", 1.234e-10)), "1.234e-10")]
    #[case::f32_neg(Box::new(|w: &mut OS| w.write_f32_value("a", -2.0)), "-2")]
    #[case::f32_neg_exp_5(Box::new(|w: &mut OS| w.write_f32_value("a", -1.234e5)), "-123400")]
    #[case::f32_neg_exp_10(Box::new(|w: &mut OS| w.write_f32_value("a", -1.234e10)), "-1.234e10")]
    #[case::f32_neg_exp_20(Box::new(|w: &mut OS| w.write_f32_value("a", -1.234e20)), "-1.234e20")]
    #[case::f32_neg_exp_neg_3(Box::new(|w: &mut OS| w.write_f32_value("a", -1.234e-3)), "-0.001234")]
    #[case::f32_neg_exp_neg_10(Box::new(|w: &mut OS| w.write_f32_value("a", -1.234e-10)), "-1.234e-10")]
    #[case::f32_inf(Box::new(|w: &mut OS| w.write_f32_value("a", f32::INFINITY)), "null")]
    #[case::f32_neg_inf(Box::new(|w: &mut OS| w.write_f32_value("a", f32::NEG_INFINITY)), "null")]
    #[case::f32_nan(Box::new(|w: &mut OS| w.write_f32_value("a", f32::NAN)), "null")]
    fn test_write_value(#[case] code: Box<dyn Fn(&mut OS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut object_ser = JsonObject::new(&mut writer)?;
                code(&mut object_ser)?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!(r#"{}"a":{}{}"#, "{", expected, "}");
            assert_eq!(actual, expected);
        }

        // test with and without preceding element to verify that 'initial' is handled correctly
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut object_ser = JsonObject::new(&mut writer)?;
                object_ser.write_null_value("x")?;
                code(&mut object_ser)?;
                object_ser.write_u32_value("y", 5)?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!(r#"{}"x":null,"a":{},"y":5{}"#, "{", expected, "}");
            assert_eq!(actual, expected);
        }

        Ok(())
    }
}