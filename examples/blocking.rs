//! This example presents the API for reading and writing JSON using blocking I/O.
//!
//! There are separate functions for reading and writing which take [std::io::Read] and
//!  [std::io::Write] instances respectively. The main function takes care of the wiring which
//!  is not part of the library's functionality.

use std::io;
use std::io::Cursor;
use json_api::blocking::{JsonObject, JsonReader, JsonWriter};
use json_api::shared::read::{JsonParseError, JsonParseResult, JsonReadToken};

fn main() {
    let mut buf = Vec::new();

    // we write our sample JSON to a Vec<u8>
    write(&mut buf).unwrap();

    // for reading, we wrap the Vec into a Cursor because that implements 'Read'
    let mut r = Cursor::new(buf);
    read(&mut r).unwrap();
}

/// Write the following JSON to a `Write` instance:
/// ```json
/// {
///   "name": "John Smith",
///   "age": 49,
///   "favorite-colors": [ "red", "blue", "yellow" ]
/// }
/// ```
fn write(w: &mut impl io::Write) -> io::Result<()> {
    // The first step when writing JSON is to wrap the raw Write instance in a JsonWriter. The
    //  JsonWriter takes care of (among other things) formatting the output.
    // For this example, we use the 'pretty' format that adds indentation for human readability.
    let mut json_writer = JsonWriter::new_pretty(w);

    // Actually JSON is written through instances of JsonObject or JsonArray. The JSON we
    //  want to write has an object at its top level, so we start by creating a JsonObject.
    // Creating the object writes the opening `{`, so it can return with an I/O error.
    let mut obj = JsonObject::new(&mut json_writer)?;

    // We write the 'name' element to the object. Since we are writing to a JsonObject, we must
    //  pass both a key and a value.
    obj.write_string_value("name", "John Smith")?;

    // Next we write the age, again with both key and value.
    obj.write_u32_value("age", 49)?;

    // To start the array of favorite colors, we provide the name of the JSON key and receive
    //  an instance of JsonArray for writing data to that nested array. That is the way the
    //  json-streaming library handles nested JSON structures.
    let mut colors_arr = obj.start_array("favorite-colors")?;

    // We use this JsonArray object to write the array's values. Note that since this is an array,
    //  we write strings without providing a key.
    colors_arr.write_string_value("red")?;
    colors_arr.write_string_value("blue")?;
    colors_arr.write_string_value("yellow")?;

    // We close the 'colors' array by calling 'end()', writing the closing ']'.
    colors_arr.end()?;

    // Finally, we end the person object.
    obj.end()?;

    Ok(())
}

/// Read a JSON stream, expecting the above JSON document structure but being lenient about e.g.
///  the order of the elements
fn read(r: &mut impl io::Read) -> JsonParseResult<(), io::Error> {
    // The first step is to wrap the underlying 'Read' in a JsonReader which provides a stream
    //  of JSON tokens.
    // Note that we need to pass a buffer size to the JsonReader: This is the maximum number of
    //  bytes that can go into a single token, e.g. a string value. This is a safeguard for
    //  parsing a JSON stream from an untrusted source. It is also the size of the single
    //  pre-allocated buffer used by the JsonReader.
    let mut json_reader = JsonReader::new(1024, r);

    // The JsonReader is pull-based, i.e. client code calls one of its 'next' functions to read
    //  the next token.
    // Often client code 'knows' what kind of token 'should' come next - in this example, we
    //  expect a JSON object at the top level. So we call 'expect_next_start_object()' which
    //  reads the next token and fails unless that token actually is the start of a JSON object.
    json_reader.expect_next_start_object()?;

    loop {
        // Now that we are 'inside' the object, we iterate until the object ends.
        // 'expect_next_key()' exists for this purpose: It returns Some(key) if the next token
        //  is a key, and None if it reached the end of the object
        match json_reader.expect_next_key()? {
            // We can match on the name of the key - this makes our JSON handling independent
            //  of the order in which the keys occur
            Some("name") => {
                // for the 'name' element, we expect a string value
                let name = json_reader.expect_next_string()?;
                println!("name: {}", name);
            },
            Some("age") => {
                // for the 'age' element, we expect a number.
                // Note that we need to specify the actual type so the JsonReader
                //  can parse the JSON number literal to a u32 (or fail if the number is a
                //  floating point, negative number or too big)
                let age: u32 = json_reader.expect_next_number()?;
                println!("age: {}", age);
            },
            Some("favorite-colors") => {
                // we expect an array here. We could read it here, but we delegate to a separate
                //  function to showcase that :-)
                read_favorite_colors(&mut json_reader)?;
            }
            Some(_other) => {
                // This means there is some unexpected JSON element in the object. For this example,
                //  we decide to fail. For details on how to skip data safely, see the 'skipping'
                //  example.
                return Err(JsonParseError::Parse("unexpected key parsing 'person'", json_reader.location()));
            },
            None => break,
        }
    }
    Ok(())
}

/// To delegate reading the list of favorite colors to a separate function, we pass the JsonReader
///  as an argument.
///
/// It has two generic arguments which we must provide as part of the signature:
///  * The representation of its internal read buffer, which we don't really care about. Its type
///     bound is `AsMut<[u8]>`.
///  * The concrete type of the underlying reader.
/// It is safe and usually the most concise way to generify over them, as we do here.
fn read_favorite_colors<B: AsMut<[u8]>, R: io::Read>(r: &mut JsonReader<B, R>) -> JsonParseResult<(), io::Error> {
    // The next token must be the start of a JSON array
    r.expect_next_start_array()?;
    loop {
        // Calling 'next()' returns the next token, making no assumptions about its type. We call
        //  it repeatedly until we reach the end of the array
        match r.next()? {
            JsonReadToken::StringLiteral(color) => {
                // We have a string -> this is what we expect
                println!("  favorite color: {}", color);
            }
            JsonReadToken::EndArray => {
                // end of the array: we're done
                break;
            },
            _other => {
                // This means there is non-string elements in the array. For this example,
                //  we decide to fail. For details on how to skip data safely, see the 'skipping'
                //  example.
                return Err(JsonParseError::Parse("non-strings in 'favorite-colors'", r.location()));
            },
        }
    }
    Ok(())
}

















