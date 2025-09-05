//! JSON has a single 'number' type that does not distinguish between integers and floating point
//!  numbers and allows an arbitrary number of digits.
//!
//! This example showcases and explains how json-streaming approaches this.

use std::io;
use std::io::Cursor;
use json_streaming::blocking::*;
use json_streaming::shared::*;

fn main() -> JsonParseResult<(), io::Error> {
    // We start with an array of numbers so we have data to work with
    let json = "[1, 2, 3, 4, 5, -6, 7, 8, 9, 10]";

    let mut r = Cursor::new(json.as_bytes());
    let mut json_reader = JsonReader::new(128, &mut r);

    // get the start of the array out of the way
    json_reader.expect_next_start_array()?;

    // The most convenient way of reading a number is to call 'expect_next_number()'. This function
    //  requires the actual numeric type as a generic parameter so it knows which number type
    //  to parse the number into.
    println!("{}", json_reader.expect_next_number::<u32>()?);

    // If the compiler can infer the numeric type, there is no need to provide it explicitly
    let n: u32 = json_reader.expect_next_number()?;
    println!("{}", n);

    // This works for signed numeric types as well as unsigned types
    println!("{}", json_reader.expect_next_number::<i16>()?);

    // and also for floating point types
    let f: f64 = json_reader.expect_next_number()?;
    println!("{:1.2?}", f);

    // This actually works for any type that implements core::str::FromStr, so it should work
    //  with your 'big integer' / 'big decimal' / fixed point arithmetic library of choice out
    //  of the box, or should be easy to get to work

    // All of the above is syntactic sugar around JsonNumber, which is the data structure the
    //  parser pulls from the JSON stream. A JsonNumber is a wrapper around a string slice.
    let num = json_reader.expect_next_raw_number()?;
    println!("raw: {:?}", num.0);

    // JsonNumber has a 'parse()' function based on a type parameter with a bound of
    //  core::str::FromStr.
    let f: f32 = num.parse().unwrap();
    println!("{:1.1?}", f);

    // If the string JSON number literal cannot be parsed to the required numeric type, the call
    //  fails. Reasons for this include a number outside the type's numeric range, negative numbers
    //  for non-negative types, or floating point numbers parsed into integer types.
    match json_reader.expect_next_number::<u32>() {
        Ok(n) => println!("num: {}", n),
        Err(JsonParseError::Parse(msg, location)) => println!("not a u32 number: {}@{}", msg, location),
        Err(_) => panic!("err"),
    }

    Ok(())
}