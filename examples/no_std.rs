//! JSON is usually associated with I/O which in turn requires a standard library. Nonetheless,
//!  the json-streaming library is designed to work in a no-std environment and can be included
//!  as #![no_std] using `default-features=false, features=["blocking"]`.
//!
//! To support this, it uses its own traits [BlockingRead] and [BlockingWrite] with
//!  blanket implementations for [std::io::Read] and [std::io::Write] that are available if the
//!  "std" feature is active.
//!
//! This example shows how the library works in a no-std environment.

use core::error::Error;
use std::fmt::Display;
use json_streaming::blocking::*;

/// The reader / writer abstraction requires the presence of some error type. Our implementations
///  can't fail with an error, so we define a NoError type for use there.
#[derive(Debug)]
enum NoError {}
impl Display for NoError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}
impl Error for NoError {}

/// A minimal fixed-buffer implementation of a writer
struct FixedWriteBuffer {
    data: [u8; 4096],
    index: usize,
}
impl FixedWriteBuffer {
    fn new() -> Self {
        FixedWriteBuffer {
            data: [0u8;4096],
            index: 0,
        }
    }
}

/// This is the trait implementation that makes our fixed buffer implementation usable for
///  writing JSON
impl BlockingWrite for FixedWriteBuffer {
    type Error = NoError;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.data[self.index..self.index+buf.len()].copy_from_slice(buf);
        self.index += buf.len();
        Ok(())
    }
}

/// A minimal slice-based read buffer implementation
struct SliceReadBuffer<'a> {
    data: &'a [u8],
    index: usize,
}
/// This is the trait implementation that makes the buffer implementation usable for reading JSON
impl BlockingRead for SliceReadBuffer<'_> {
    type Error = NoError;

    fn read(&mut self) -> Result<Option<u8>, Self::Error> {
        let result = if self.index < self.data.len() {
            Some(self.data[self.index])
        }
        else {
            None
        };
        self.index += 1;
        Ok(result)
    }
}


fn main() -> Result<(), NoError> {
    // With the no-std read and write buffer implementations in place, we can read and write JSON
    //  pretty much the same way as with a standard library.

    let mut write_buf = FixedWriteBuffer::new();
    let mut json_writer = JsonWriter::new_pretty(&mut write_buf);

    // We can use the full scoped API for writing JSON objects and arrays in a no-std environment:
    //  JsonObject is created on the stack and wraps the writer -> no alloc required
    let mut obj = JsonObject::new(&mut json_writer)?;
    // note how both key and value are passed as string slices: no alloc required
    obj.write_string_value("name", "John Doe")?;
    obj.write_u32_value("age", 42)?;
    obj.end()?;
    json_writer.flush()?;

    // we use a slice of the write buffer as our read buffer - this would be some other slice of
    //  data in real-world code, obviously
    let mut read_json_buffer = SliceReadBuffer {
        data: &write_buf.data[..write_buf.index],
        index: 0,
    };

    // Other examples use the convenience function JsonReader::new(), but that function allocates
    //  the JsonReader's internal buffer on the heap which we can't do in a no-std environment. For
    //  that purpose, there is a function JsonReader::new_with_provided_buffer() which takes its
    //  internal read buffer as a parameter.
    // First, we need to allocate the buffer. The buffer must be big enough to hold a single token
    //  (e.g. a string value), but there is no reason for it to be bigger than that. In our case,
    //  64 bytes are plenty, and we allocate it on the stack
    let mut reader_internal_buffer = [0u8;64];
    // Now we pass both the internal buffer and the reader with JSON data (which happens to be a
    //  buffer in our case as well) to the newly created JsonReader.
    // The third parameter is unrelated and exists here because 'new_with_provided_buffer' is the
    //  most low-level and comprehensive API for creating a JsonReader.
    let mut json_reader = JsonReader::new_with_provided_buffer(&mut reader_internal_buffer, &mut read_json_buffer, false);

    do_read(&mut json_reader)
        .expect("no errors in this example - real world code would do some error handling here");

    Ok(())
}

/// The actual parsing code is the same as with a standard library. It is extracted into a separate
///  function to simplify error handling.
fn do_read(json_reader: &mut JsonReader<&mut [u8;64], SliceReadBuffer>) -> JsonParseResult<(), NoError> {
    json_reader.expect_next_start_object()?;
    loop {
        match json_reader.expect_next_key()? {
            Some("name") => {
                // note that reading a string is alloc free: internally, the string is accumulated
                //  in the read buffer, and the 'expect_next_string' function returns a string
                //  slice pointing into that
                let _name = json_reader.expect_next_string()?;
                // do something with the name
            }
            Some("age") => {
                // note that reading a number is alloc free: internally, the number literal is
                //  accumulated in the read buffer, and then it is parsed based on a slice into
                //  that
                let _age = json_reader.expect_next_number::<u32>()?;
                // do something with the age
            }
            Some(_) => {
                return Err(JsonParseError::Parse("unexpected key", json_reader.location()));
            }
            None => {
                // end of the object
                break;
            }
        }
    }
    Ok(())
}