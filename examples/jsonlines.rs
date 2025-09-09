//! Illustrate writing and reading the 'jsonlines' format (https://jsonlines.org), with a
//!  JSON object per line without a wrapping JSON array or separating commas.
//!
//! This format is sometimes used for streaming or sending large numbers of simple data, e.g.
//!  log events.
//!
//! While the json-streaming library has no explicit support for the 'jsonlines' format, it is
//!  straightforward to read and write in a fully streaming fashion.
//! * When writing, create a new top-level [JsonObject] for each line, and write the `\n` between
//!    lines
//! * When reading, use the [new_with_lenient_comma_handling] function to create the JsonReader to
//!    skip the check for commas between the objects

use json_streaming::blocking::*;
use json_streaming::shared::*;
use std::io;
use std::io::Cursor;

fn main() {
    let json_lines = write().unwrap();
    read(json_lines).unwrap();
}

fn read(json_lines: String) -> JsonParseResult<(), io::Error> {
    let buf = json_lines.into_bytes();
    let mut read = Cursor::new(buf);

    let mut json_reader = JsonReader::new_with_lenient_comma_handling(1024, &mut read);

    while let JsonReadToken::StartObject = json_reader.next()? {
        println!("start object");
        loop {
            match json_reader.next()? {
                JsonReadToken::Key("a") => println!("  a={:?}", json_reader.expect_next_string()?),
                JsonReadToken::Key("b") => println!("  b={:?}", json_reader.expect_next_number::<u32>()?),
                JsonReadToken::EndObject => break,
                _ => return Err(JsonParseError::Parse(JsonReadToken::EndObject.kind(), json_reader.location())),
            }
        }
        println!("end object");
    }

    Ok(())
}

fn write() -> io::Result<String> {
    let mut buf = Vec::new();
    let mut writer = JsonWriter::new_compact(&mut buf);

    let mut obj = JsonObject::new(&mut writer)?;
    obj.write_string_value("a", "yo")?;
    obj.write_u32_value("b", 123)?;
    obj.end()?;
    writer.write_bytes(b"\n")?;

    let mut obj = JsonObject::new(&mut writer)?;
    obj.write_u32_value("b", 456)?;
    obj.write_string_value("a", "hey")?;
    obj.end()?;
    writer.write_bytes(b"\n")?;

    let buf = writer.into_inner().unwrap().to_vec();
    let s = String::from_utf8(buf).unwrap();
    Ok(s)
}