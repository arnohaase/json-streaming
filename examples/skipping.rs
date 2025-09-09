//! This example shows how to safely skip unexpected data in an objects and arrays:
//!
//!  * unexpected keys in an object
//!  * values of unexpected type in an array
//!
//! The tricky bit about this is that a JSON value can be an object or array with arbitrarily deep
//!  nesting inside it. The json-streaming library has convenience APIs for helping with this.
//!
//! The idioms and APIs are the same for blocking and non-blocking variants. This example uses
//!  blocking API, but working with non-blocking calls is pretty much identical.

use json_streaming::blocking::*;
use json_streaming::shared::*;
use std::io;
use std::io::Cursor;

fn main() {
    skip_unexpected_keys_in_object().unwrap();
    println!("--");
    skip_unexpected_keys_in_array().unwrap();
}

fn skip_unexpected_keys_in_object() -> JsonParseResult<(), io::Error> {
    // we parse an object with some keys that we can't handle and want to skip
    let json = r#"
    {
      "a": true,
      "unexpected_string": "abc",
      "unexpected_array": [ 1, 2, { "xyz": "yo" }, 4, [ 4, 5, 6], 4],
      "unexpected_object": { "x": null, "y": [1, 2, 3, {}], "z": {"a": [1, 2, 2] } },
      "b": 3
    }
    "#;

    let mut r = Cursor::new(json.as_bytes());
    let mut json_reader = JsonReader::new(1024, &mut r);

    // start of object
    json_reader.expect_next_start_object()?;
    loop {
        // and call 'expect_next_key()' in a loop as is typical for reading JSON objects
        match json_reader.expect_next_key()? {
            // known keys
            Some("a") => println!("a: {}", json_reader.expect_next_bool()?),
            Some("b") => println!("b: {}", json_reader.expect_next_number::<u32>()?),
            Some(other) => {
                println!("unexpected key {}, skipping", other);
                // An unexpected key: We want to skip it, so we need to consume the corresponding
                //  value. For a string, boolean or number value, that is straightforward. But for
                //  an array or object, we need to consume tokens until it ends.
                // That's what the 'skip_value' function does.
                json_reader.skip_value()?;
            }
            None => break,
        }
    }
    Ok(())
}

fn skip_unexpected_keys_in_array() -> JsonParseResult<(), io::Error> {
    // We parse an array that we expect to contain only strings. We want to skip over non-string
    //  elements in the array.
    let json = r#"
    [
      "a",
      "b",
      true,
      "c",
      1,
      "d",
      [ 1, 2, 3, {}, 4, 5, ["yo"]],
      "e",
      { "x": 1, "y": [9, 8, 7] },
      "f"
    ]
    "#;

    let mut r = Cursor::new(json.as_bytes());
    let mut json_reader = JsonReader::new(1024, &mut r);

    // start of array
    json_reader.expect_next_start_array()?;
    loop {
        // We call the generic 'next()' function to handle all kinds of elements
        match json_reader.next()? {
            JsonReadToken::StringLiteral(s) => {
                // strings are what we expect - this is the 'happy' case
                println!("array element: {}", s);
            }
            JsonReadToken::EndArray => {
                // end of array - we're done here
                break;
            }
            JsonReadToken::NumberLiteral(_) |
            JsonReadToken::BooleanLiteral(_) |
            JsonReadToken::NullLiteral => {
                // non-string single-token value - we already read that token, so we safely skipped
                //  them as far as the JSON read is concerned
                println!("unexpected array element, skipping");
            }
            JsonReadToken::StartObject |
            JsonReadToken::StartArray => {
                // We are now inside an object or array of unknown size. We need to consume all
                //  tokens until it ends if we want to skip over it.
                // That is what 'skip_to_end_of_current_scope()' is for: It consumes tokens until
                //  it reaches the closing bracket that corresponds to the scope we are currently
                //  in.
                // Note: We already consumed the opening bracket - and that is the key difference
                //  compared to 'skip_value()' we used when skipping over unknown keys in an
                //  object.
                println!("skipping over a nested structure");
                json_reader.skip_to_end_of_current_scope()?;
            }
            JsonReadToken::EndObject |
            JsonReadToken::Key(_) |
            JsonReadToken::EndOfStream => {
                return Err(JsonParseError::Parse(JsonReadToken::EndOfStream.kind(), json_reader.location()));
            }
        }
    }
    Ok(())
}