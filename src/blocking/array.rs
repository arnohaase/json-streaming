use crate::blocking::io::BlockingWrite;
use crate::blocking::json_writer::JsonWriter;
use crate::blocking::object::JsonObject;
use crate::blocking::JsonFormatter;


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
pub struct JsonArray<'a, W: BlockingWrite, F: JsonFormatter> {
    writer: &'a mut JsonWriter<W, F>,
    is_initial: bool,
    is_ended: bool,
}

impl<'a, W: BlockingWrite, F: JsonFormatter> JsonArray<'a, W, F> {
    /// Create a new [JsonArray] instance. Application code can do this explicitly only initially
    ///  as a starting point for writing JSON. Nested arrays are created by the library.
    pub fn new(writer: &'a mut JsonWriter<W, F>) -> Result<Self, W::Error> {
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

    /// Write an element with a generic int value. This function fits most Rust integral
    ///  types; for the exceptions, there are separate functions.
    pub fn write_int_value(&mut self, value: impl Into<i128>) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bytes(format!("{}", value.into()).as_bytes())
    }

    /// Write a u128 as an element.
    pub fn write_u128_value(&mut self, value: u128) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bytes(format!("{}", value).as_bytes())
    }

    /// Write a usize as an element.
    pub fn write_usize_value(&mut self, value: usize) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bytes(format!("{}", value).as_bytes())
    }

    /// Write an isize as an element.
    pub fn write_isize_value(&mut self, value: isize) -> Result<(), W::Error> {
        self.handle_initial()?;
        self.writer.write_bytes(format!("{}", value).as_bytes())
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
    pub fn start_object(&mut self) -> Result<JsonObject<W, F>, W::Error> {
        self.handle_initial()?;
        JsonObject::new(self.writer)
    }

    /// Start a nested array as an element. This function returns a new [JsonArray] instance
    ///  for writing elements to the nested object. When the returned [JsonArray] goes out of scope
    ///  (per syntactic scope or an explicit call to `end()`), the nested object is closed, and
    ///  application code can continue adding elements to the owning `self` object.
    pub fn start_array(&mut self) -> Result<JsonArray<W, F>, W::Error> {
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

impl <'a, W: BlockingWrite, F: JsonFormatter> Drop for JsonArray<'a, W, F> {
    fn drop(&mut self) {
        if !self.is_ended {
            let _ = self._end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocking::CompactFormatter;
    use rstest::*;
    use std::io;

    type AS<'a> = JsonArray<'a, Vec<u8>, CompactFormatter>;

    #[rstest]
    #[case::empty(Box::new(|_ser: &mut AS| Ok(())), "[]")]
    #[case::single(Box::new(|ser: &mut AS| ser.write_null_value()), "[null]")]
    #[case::two(Box::new(|ser: &mut AS| { ser.write_int_value(1)?; ser.write_int_value(2) }), "[1,2]")]
    #[case::nested_arr(Box::new(|ser: &mut AS| {ser.start_array()?.end() }), "[[]]")]
    #[case::nested_arr_with_el(Box::new(|ser: &mut AS| {let mut n = ser.start_array()?; n.write_int_value(5) }), "[[5]]")]
    #[case::nested_arr_first(Box::new(|ser: &mut AS| { ser.start_array()?.end()?; ser.write_int_value(4) }), "[[],4]")]
    #[case::nested_arr_last(Box::new(|ser: &mut AS| { ser.write_int_value(6)?; ser.start_array()?.end() }), "[6,[]]")]
    #[case::nested_arr_between(Box::new(|ser: &mut AS| { ser.write_int_value(7)?; ser.start_array()?.end()?; ser.write_int_value(9) }), "[7,[],9]")]
    #[case::two_nested_arrays(Box::new(|ser: &mut AS| { ser.start_array()?.end()?; ser.start_array()?.end() }), "[[],[]]")]
    #[case::nested_obj(Box::new(|ser: &mut AS| ser.start_object()?.end()), "[{}]")]
    #[case::nested_obj_with_el(Box::new(|ser: &mut AS| { let mut n=ser.start_object()?; n.write_int_value("a", 3) }), r#"[{"a":3}]"#)]
    #[case::nested_obj_first(Box::new(|ser: &mut AS| { ser.start_object()?.end()?; ser.write_int_value(0) }), r#"[{},0]"#)]
    #[case::nested_obj_last(Box::new(|ser: &mut AS| { ser.write_int_value(2)?; ser.start_object()?.end() }), r#"[2,{}]"#)]
    #[case::two_nested_objects(Box::new(|ser: &mut AS| { ser.start_object()?.end()?; ser.start_object()?.end() }), r#"[{},{}]"#)]
    fn test_array(#[case] code: Box<dyn Fn(&mut AS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        let mut writer = JsonWriter::new_compact(Vec::new());
        {
            let mut array_ser = JsonArray::new(&mut writer)?;
            code(&mut array_ser)?;
        }

        let actual = String::from_utf8(writer.into_inner()?).unwrap();
        assert_eq!(actual, expected);
        Ok(())
    }

    #[rstest]
    #[case::null(Box::new(|w: &mut AS| w.write_null_value()), "null")]
    #[case::bool_true(Box::new(|w: &mut AS| w.write_bool_value(true)), "true")]
    #[case::bool_false(Box::new(|w: &mut AS| w.write_bool_value(false)), "false")]
    #[case::string(Box::new(|w: &mut AS| w.write_string_value("asdf")), r#""asdf""#)]
    #[case::string_escaped(Box::new(|w: &mut AS| w.write_string_value("\r\n")), r#""\r\n""#)]
    #[case::u8(Box::new(|w: &mut AS| w.write_int_value(2u8)), "2")]
    #[case::i8(Box::new(|w: &mut AS| w.write_int_value(-3i8)), "-3")]
    #[case::u16(Box::new(|w: &mut AS| w.write_int_value(4u16)), "4")]
    #[case::i16(Box::new(|w: &mut AS| w.write_int_value(-5i16)), "-5")]
    #[case::u32(Box::new(|w: &mut AS| w.write_int_value(6u32)), "6")]
    #[case::i32(Box::new(|w: &mut AS| w.write_int_value(-7i32)), "-7")]
    #[case::u64(Box::new(|w: &mut AS| w.write_int_value(8u64)), "8")]
    #[case::i64(Box::new(|w: &mut AS| w.write_int_value(-9i64)), "-9")]
    #[case::usize(Box::new(|w: &mut AS| w.write_usize_value(10usize)), "10")]
    #[case::isize(Box::new(|w: &mut AS| w.write_isize_value(-11isize)), "-11")]
    #[case::u128(Box::new(|w: &mut AS| w.write_u128_value(12u128)), "12")]
    #[case::i128(Box::new(|w: &mut AS| w.write_int_value(-13i128)), "-13")]
    #[case::f64(Box::new(|w: &mut AS| w.write_f64_value(2.0)), "2.0")]
    #[case::f64_exp(Box::new(|w: &mut AS| w.write_f64_value(1.234e10)), "12340000000.0")]
    #[case::f64_exp_20(Box::new(|w: &mut AS| w.write_f64_value(1.234e20)), "1.234e20")]
    #[case::f64_neg_exp(Box::new(|w: &mut AS| w.write_f64_value(1.234e-10)), "1.234e-10")]
    #[case::f64_inf(Box::new(|w: &mut AS| w.write_f64_value(f64::INFINITY)), "null")]
    #[case::f64_neg_inf(Box::new(|w: &mut AS| w.write_f64_value(f64::NEG_INFINITY)), "null")]
    #[case::f64_nan(Box::new(|w: &mut AS| w.write_f64_value(f64::NAN)), "null")]
    #[case::f32(Box::new(|w: &mut AS| w.write_f32_value(2.0)), "2.0")]
    #[case::f32_exp(Box::new(|w: &mut AS| w.write_f32_value(1.234e10)), "12340000000.0")]
    #[case::f32_exp_20(Box::new(|w: &mut AS| w.write_f32_value(1.234e20)), "1.234e20")]
    #[case::f32_neg_exp(Box::new(|w: &mut AS| w.write_f32_value(1.234e-10)), "1.234e-10")]
    #[case::f32_inf(Box::new(|w: &mut AS| w.write_f32_value(f32::INFINITY)), "null")]
    #[case::f32_neg_inf(Box::new(|w: &mut AS| w.write_f32_value(f32::NEG_INFINITY)), "null")]
    #[case::f32_nan(Box::new(|w: &mut AS| w.write_f32_value(f32::NAN)), "null")]
    fn test_write_value(#[case] code: Box<dyn Fn(&mut AS) -> io::Result<()>>, #[case] expected: &str) -> io::Result<()> {
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut array_ser = JsonArray::new(&mut writer)?;
                code(&mut array_ser)?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!("[{}]", expected);
            assert_eq!(actual, expected);
        }

        // test with and without preceding element to verify that 'initial' is handled correctly
        {
            let mut writer = JsonWriter::new_compact(Vec::new());
            {
                let mut array_ser = JsonArray::new(&mut writer)?;
                array_ser.write_null_value()?;
                code(&mut array_ser)?;
            }

            let actual = String::from_utf8(writer.into_inner()?).unwrap();
            let expected = format!("[null,{}]", expected);
            assert_eq!(actual, expected);
        }

        Ok(())
    }
}
