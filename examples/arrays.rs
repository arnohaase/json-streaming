//! This example showcases the 'expect_*_or_end_array' convenience API for reading arrays.
//!
//! It is possible to parse arrays without this convenience API, and other examples show this. That
//!  involves looping over the tokens inside the array, checking each token for element type and
//!  'end of array'.
//!
//! For this scenario, JsonReader has convenience functions that expect *either* a given element
//!  type *or* end-of-array, returning the former as `Some` the latter as `None`. For arrays with
//!  a single, given element type, that can make for more readable code.
//!
//! These convenience functions exist as both 'expect_X' and 'expect_opt_X'. The latter returns
//!  `Result<Option<Option<X>>, ...>` which may look intimidating at first glance - but the outer
//!  `Option` distinguishes between 'another element' and 'end of array', while the inner `Option`
//!  is the element type. This nesting makes for concise and readable code, see this example.

use json_streaming::blocking::*;
use json_streaming::shared::*;
use std::io;
use std::io::Cursor;

fn main() -> JsonParseResult<(), io::Error>{
    // parse an array that only contains numbers
    println!("required numbers: {:?}", parse_required_number_array("[1, 2, 3, 4, 5]")?);
    // parse the same array as optional: All elements are actual numbers, none are missing, but
    //  we parse them into Option<i32>
    println!("required numbers parsed as optional: {:?}", parse_optional_number_array("[1, 2, 3, 4, 5]")?);
    // parse an array with missing (i.e. 'null') numbers
    println!("optional numbers: {:?}", parse_optional_number_array("[1, null, null, 3, 4, 5]")?);

    // attempt to parse a JSON array that contains 'true' as well as numbers. Since our parser code
    //  accepts numbers only, this will fail (although it is of course valid JSON)
    println!("trying to parse with wrong element type (will fail!):");
    parse_required_number_array("[1, 2, 3, true, 4]")?;

    Ok(())
}

/// Expects a JSON array of i32 and parses it.
fn parse_required_number_array(json: &str) -> JsonParseResult<Vec<i32>, io::Error> {
    // Numbers will be collected in this Vec as they occur
    let mut result = Vec::new();

    // Set up the JsonReader
    let mut r = Cursor::new(json.as_bytes());
    let mut json_reader = JsonReader::new(64, &mut r);

    // Consume the '[' that starts the array
    json_reader.expect_start_array()?;

    // Loop over the elements in the array until reaching the end of the array.
    //
    // There are three possible outcomes for 'expect_number_or_end_array' in valid JSON:
    //  * If the next token is a number (that can be parsed as i32), it returns 'Some(n)'.
    //  * If the next token is the ']' that ends the array, it returns None. This representation
    //     allows the 'while let Some(n) = ...' convenience syntax used here
    //  * If the next token is some other JSON value (e.g. 'true'), the function fails so that
    //     application code need not handle different kinds of values. When consuming a
    //     heterogeneous array, calling 'next()' in a loop and matching over it works well
    //     and would hard to improve upon.
    while let Some(n) = json_reader.expect_number_or_end_array()? {
        // 'n' is the parsed number - push it to the result Vec
        result.push(n);
    }

    Ok(result)
}

// Expects a JSON array of i32 or null values and parses it into Vec<Option<i32>>
fn parse_optional_number_array(json: &str) -> JsonParseResult<Vec<Option<i32>>, io::Error> {
    // Numbers will be collected in this Vec as they occur
    let mut result = Vec::new();

    // Set up the JsonReader
    let mut r = Cursor::new(json.as_bytes());
    let mut json_reader = JsonReader::new(64, &mut r);

    // Consume the '[' that starts the array
    json_reader.expect_start_array()?;

    // Loop over the elements in the array until reaching the end of the array.
    //
    // There are four possible outcomes for 'expect_opt_number_or_end_array' in valid JSON:
    //  * If the next token is a number (that can be parsed as i32), it returns 'Some(Some(n))'.
    //  * If the next token is 'null', it returns 'Some(None)'. These two cases are the valid
    //     elements in the array
    //  * If the next token is the ']' that ends the array, it returns None. This representation
    //     allows the 'while let Some(n) = ...' convenience syntax used here
    //  * If the next token is some other JSON value (e.g. 'true'), the function fails so that
    //     application code need not handle different kinds of values. When consuming a
    //     heterogeneous array, calling 'next()' in a loop and matching over it works well
    //     and would hard to improve upon.
    while let Some(n) = json_reader.expect_opt_number_or_end_array()? {
        // 'n' is the parsed optional number, represented as Option<i32> - push it to the result Vec
        result.push(n);
    }

    Ok(result)
}