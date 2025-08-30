use json_api::blocking::{JsonObject, JsonReader, JsonWriter};
use json_api::shared::read::{JsonParseError, JsonParseResult, JsonReadEvent};
use std::io;
use std::io::Cursor;

fn main() {
    let json_lines = write().unwrap();
    read(json_lines).unwrap();
}

fn read(json_lines: String) -> JsonParseResult<(), io::Error> {
    let buf = json_lines.into_bytes();
    let read = Cursor::new(buf);

    let mut json_reader = JsonReader::new_with_lenient_comma_handling(1024, read);

    while let JsonReadEvent::StartObject = json_reader.next()? {
        println!("start object");
        loop {
            match json_reader.next()? {
                JsonReadEvent::Key("a") => println!("  a={:?}", json_reader.expect_next_string()?),
                JsonReadEvent::Key("b") => println!("  b={:?}", json_reader.expect_next_number::<u32>()?),
                JsonReadEvent::EndObject => break,
                _ => return Err(JsonParseError::UnexpectedEvent(json_reader.location())),
            }
        }
        println!("end object");
    }

    Ok(())
}

fn write() -> io::Result<String> {
    let mut writer = JsonWriter::new_compact(Vec::new());

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

    let buf = writer.into_inner().unwrap();
    let s = String::from_utf8(buf).unwrap();
    Ok(s)
}