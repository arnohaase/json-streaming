pub(crate) mod array;
pub(crate) mod io;
pub(crate) mod json_writer;
pub(crate) mod object;
pub(crate) mod read;

#[cfg(not(test))]
#[allow(unused_imports)]
pub use array::*;
#[allow(unused_imports)]
pub use io::*;
#[allow(unused_imports)]
pub use json_writer::*;
#[cfg(not(test))]
#[allow(unused_imports)]
pub use object::*;

#[allow(unused_imports)]
pub use read::*;


#[cfg(test)]
mod tests {
    use crate::nonblocking::json_writer::JsonWriter;
    use crate::nonblocking::object::JsonObject;
    use crate::shared::*;
    use std::io;

    async fn do_write_json<F: JsonFormatter>(o: &mut JsonObject<'_, '_, Vec<u8>, F, DefaultFloatFormat>) -> io::Result<()> {
        o.write_string_value("abc", "yo").await?;
        o.write_string_value("xyz", "yo").await?;

        let mut na = o.start_array("aaaa").await?;
        na.write_string_value("111").await?;
        na.write_string_value("11").await?;
        na.start_object().await?.end().await?;
        na.start_array().await?.end().await?;

        na.write_null_value().await?;
        na.write_bool_value(true).await?;
        na.write_bool_value(false).await?;
        na.write_i32_value(-23987).await?;
        na.write_u128_value(23987u128).await?;
        na.write_f64_value(23.235).await?;
        na.write_f64_value(f64::INFINITY).await?;
        na.write_f64_value(f64::NAN).await?;
        na.write_f32_value(23.235).await?;
        na.write_f32_value(f32::INFINITY).await?;
        na.write_f32_value(f32::NAN).await?;
        na.end().await?;

        let mut nested = o.start_object("ooo").await?;
        nested.write_string_value("lll", "whatever").await?;
        nested.start_array("ar").await?.end().await?;
        nested.end().await?;

        Ok(())
    }

    async fn do_test_combined<F: JsonFormatter>(mut writer: JsonWriter<'_, Vec<u8>, F, DefaultFloatFormat>, expected: &str) -> io::Result<()> {
        let mut root = JsonObject::new(&mut writer).await?;
        do_write_json(&mut root).await?;
        root.end().await?;

        let s = writer.into_inner()?.to_vec();
        let s = String::from_utf8(s).unwrap();

        assert_eq!(s, expected);
        Ok(())
    }

    #[tokio::test]
    async fn test_write_combined_compact() -> io::Result<()> {
        do_test_combined(JsonWriter::new_compact(&mut Vec::new()),
                         r#"{"abc":"yo","xyz":"yo","aaaa":["111","11",{},[],null,true,false,-23987,23987,23.235,null,null,23.235,null,null],"ooo":{"lll":"whatever","ar":[]}}"#
        ).await
    }

    #[tokio::test]
    async fn test_write_combined_pretty() -> io::Result<()> {
        do_test_combined(JsonWriter::new_pretty(&mut Vec::new()),
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
        ).await
    }
}
