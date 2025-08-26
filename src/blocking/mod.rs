//! This is an API for writing JSON directly to a (blocking) [std::io::Write], i.e. without
//!  creating an in-memory representation and writing that through serde's `Serializer` mechanism.
//!
//! It serves two main use cases:
//!  * writing big JSON data structures, potentially in a streaming fashion
//!  * writing a JSON representation that is very different from the data's in-memory representation,
//!     e.g. flattening maps into fields based on domain knowledge
//! The idea is to use `serde` for common use cases, and to have an alternative where the mapping
//!  approach becomes unwieldy.
//!
//! Here's a simple example of how to use the library, with explanations following the code:
//! ```
//! use json_api::blocking::*;
//!
//! fn write_something() -> std::io::Result<()> {
//!     let mut writer = JsonWriter::new_pretty(std::io::stdout());
//!     {
//!         let mut o = JsonObject::new(&mut writer)?;
//!         o.write_string_value("a", "hello")?;
//!         o.write_string_value("b", "world")?;
//!     }
//!
//!     writer.flush()
//! }
//! ```
//!
//! The starting point for this library is [JsonWriter]. This is a thin wrapper around
//!  a [std::io::Write] with some support for handling JSON handling of data types. It also holds
//!  the [JsonFormatter], which determines how the generated JSON is formatted. While it is
//!  possible for applications to provide their own implementations, the library provides two
//!  variants that are expected to cover the vast majority of cases (given that the format does
//!  not affect the generated JSON's semantics):
//! * [CompactFormatter] writes a minimum of whitespace
//! * [PrettyFormatter] adds some whitespace and indentation for human readability
//!
//! For actually writing a JSON object, code needs to create an [JsonObject] instance based on the
//!  writer (or an [JsonArray] for writing a top-level array). That instance then has an API for
//!  writing key/value pairs, nested objects or arrays etc.
//!
//! When [JsonObject] or [JsonArray] instances go out of scope, they write their respective closing
//!  brackets automatically (although they also have optional `end()` functions). This is
//!  convenient, but it means there is no explicit function call for returning potential
//!  `io::Error`s. These errors are stored internally so they don't get lost, and the [JsonWriter]
//!  has a `flush()` function that returns such an error if it exists. The idiom is the same that
//!  e.g. the standard library's `BufRead` uses.

pub(crate) mod json_writer;
pub(crate) mod object;
pub(crate) mod array;
pub mod read;
mod io;
//TODO feature flag for no-std - define bridge to std::io::Write only in its absence
//TODO feature flag for async / blocking support
//TODO object-per-line

#[allow(unused_imports)]
pub use array::*;
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

    fn do_write_json<F: JsonFormatter>(o: &mut JsonObject<Vec<u8>, F>) -> io::Result<()> {
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
            na.write_int_value(-23987)?;
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

    fn do_test_combined<F: JsonFormatter>(mut writer: JsonWriter<Vec<u8>, F>, expected: &str) -> io::Result<()> {
        do_write_json(&mut JsonObject::new(&mut writer)?)?;

        let s = writer.into_inner()?;
        let s = String::from_utf8(s).unwrap();

        assert_eq!(s, expected);
        Ok(())
    }

    #[test]
    fn test_write_combined_compact() -> io::Result<()> {
        do_test_combined(JsonWriter::new_compact(Vec::new()),
            r#"{"abc":"yo","xyz":"yo","aaaa":["111","11",{},[],null,true,false,-23987,23987,23.235,null,null,23.235,null,null],"ooo":{"lll":"whatever","ar":[]}}"#
        )
    }

    #[test]
    fn test_write_combined_pretty() -> io::Result<()> {
        do_test_combined(JsonWriter::new_pretty(Vec::new()),
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