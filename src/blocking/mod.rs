//! TODO move this to the structs
//! When [JsonObject] or [JsonArray] instances go out of scope, they write their respective closing
//!  brackets automatically (although they also have optional `end()` functions). This is
//!  convenient, but it means there is no explicit function call for returning potential
//!  `io::Error`s. These errors are stored internally so they don't get lost, and the [JsonWriter]
//!  has a `flush()` function that returns such an error if it exists. The idiom is the same that
//!  e.g. the standard library's `BufRead` uses.

pub(crate) mod json_writer;
pub(crate) mod object;
pub(crate) mod array;
pub(crate) mod read;
pub (crate) mod io;

#[allow(unused_imports)]
pub use array::*;
#[allow(unused_imports)]
pub use io::*;
#[allow(unused_imports)]
pub use json_writer::*;
#[allow(unused_imports)]
pub use object::*;

#[allow(unused_imports)]
pub use read::*;


#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use crate::shared::*;

    fn do_write_json<F: JsonFormatter>(o: &mut JsonObject<Vec<u8>, F, DefaultFloatFormat>) -> io::Result<()> {
        o.write_string_value("abc", "yo")?;
        o.write_string_value("xyz", "yo")?;

        {
            let mut na = o.start_array("aaaa")?;
            na.write_string_value("111")?;
            na.write_string_value("11")?;
            na.start_object()?.end()?;
            na.start_array()?.end()?;

            na.write_null_value()?;
            na.write_bool_value(true)?;
            na.write_bool_value(false)?;
            na.write_i32_value(-23987)?;
            na.write_u128_value(23987u128)?;
            na.write_f64_value(23.235)?;
            na.write_f64_value(f64::INFINITY)?;
            na.write_f64_value(f64::NAN)?;
            na.write_f32_value(23.235)?;
            na.write_f32_value(f32::INFINITY)?;
            na.write_f32_value(f32::NAN)?;
        }

        {
            let mut nested = o.start_object("ooo")?;
            nested.write_string_value("lll", "whatever")?;
            nested.start_array("ar")?;
        }

        Ok(())
    }

    fn do_test_combined<F: JsonFormatter>(mut writer: JsonWriter<Vec<u8>, F, DefaultFloatFormat>, expected: &str) -> io::Result<()> {
        do_write_json(&mut JsonObject::new(&mut writer)?)?;

        let s = writer.into_inner()?.to_vec();
        let s = String::from_utf8(s).unwrap();

        assert_eq!(s, expected);
        Ok(())
    }

    #[test]
    fn test_write_combined_compact() -> io::Result<()> {
        let mut buf = Vec::new();
        do_test_combined(JsonWriter::new_compact(&mut buf),
            r#"{"abc":"yo","xyz":"yo","aaaa":["111","11",{},[],null,true,false,-23987,23987,23.235,null,null,23.235,null,null],"ooo":{"lll":"whatever","ar":[]}}"#
        )
    }

    #[test]
    fn test_write_combined_pretty() -> io::Result<()> {
        let mut buf = Vec::new();
        do_test_combined(JsonWriter::new_pretty(&mut buf),
            r#"{
  "abc": "yo",
  "xyz": "yo",
  "aaaa": [
    "111",
    "11",
    {},
    [],
    null,
    true,
    false,
    -23987,
    23987,
    23.235,
    null,
    null,
    23.235,
    null,
    null
  ],
  "ooo": {
    "lll": "whatever",
    "ar": []
  }
}"#
        )
    }
}