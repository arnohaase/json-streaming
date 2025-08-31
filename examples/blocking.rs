//! This example presents the API for reading and writing JSON using blocking I/O. 
//! 
//! There are separate functions for reading and writing which take [std::io::Read] and 
//!  [std::io::Write] instances respectively. The main function takes care of the wiring which
//!  is not part of the library's functionality.

use std::io;
use json_api::blocking::{JsonObject, JsonWriter};

fn main() {
    
    
    todo!()
}

/// Write the following JSON to a `Write` instance:
/// ```json
/// {
///   "name": "John Smith",
///   "age": 49,
///   "favorite-colors": [ "red", "blue", "yellow" ]
/// }
/// ```
fn write(out: impl io::Write) -> io::Result<()> {
    // The first step when writing JSON is to wrap the raw Write instance in a JsonWriter. The
    //  JsonWriter takes care of (among other things) formatting the output.    
    // For this example, we use the 'pretty' format that adds indentation for human readability.
    let mut json_writer = JsonWriter::new_pretty(out);
    
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