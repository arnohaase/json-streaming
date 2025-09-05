//! A given floating point number can be formatted in wide range of ways, all of them loss-free
//!  and valid in terms of both JSON semantics and parseability from e.g. Rust's standard library.
//!
//! The number 0.1f64 for example has (among others) the following valid string representations:
//!  * 0.1
//!  * 1e-1
//!  * 1.0e-1
//!
//! The same holds for 1000f64 - it can be formatted as 1000, 1000.0, 1e3, 1e+3 or 1.0e3 (and many
//!  other ways).
//!
//! All of these are correct and will be parsed by any correct JSON parser into the exact same
//!  number. But many JSON documents are intended to be human-readable, and people have different
//!  preferences for how they would like numbers formatted.
//!
//! For this reason, json-streaming has a pluggable [FloatFormat] abstraction with a default
//!  implementation that formats numbers from 0.001 to 1000000.0 in regular decimal representation
//!  and numbers outside that range using exponential representation.
//!
//! This example shows how to customize floating point formatting.

use core::fmt::Write;
use json_streaming::blocking::{JsonArray, JsonWriter};
use json_streaming::shared::*;
use std::io;

/// [ExponentialFloatFormat] formats all numbers in exponential representation
struct ExponentialFloatFormat;
impl FloatFormat for ExponentialFloatFormat {
    fn write_f64(f: &mut impl Write, value: f64) -> std::fmt::Result {
        // JSON can not represent INFINITY, NEG_INFINITY or NAN values as numbers, so they need
        //  special handling.
        // We represent them as null literals; representing them as a default number like 0.0
        //  would also work (if such a default fits the domain)
        if value.is_finite() {
            write!(f, "{:e}", value)
        }
        else {
            write!(f, "null")
        }
    }

    fn write_f32(f: &mut impl Write, value: f32) -> std::fmt::Result {
        if value.is_finite() {
            write!(f, "{:e}", value)
        }
        else {
            write!(f, "null")
        }
    }
}

fn main() -> io::Result<()> {
    let mut buf = Vec::new();

    // we use the 'new' function for creating the JsonWriter, explicitly providing the 'compact'
    // JsonFormatter (we don't care about pretty printing in this example) and our
    // ExponentialFloatFormatter.
    let mut json_writer = JsonWriter::new(&mut buf, CompactFormatter, ExponentialFloatFormat);

    let mut arr = JsonArray::new(&mut json_writer)?;
    // we write floating point numbers using the regular API; the formatter is applied internally
    arr.write_f64_value(1.0)?;
    arr.write_f64_value(10.0)?;
    arr.write_f64_value(0.1)?;
    arr.end()?;

    // All three floating point numbers are now formatted in exponential representation. This does
    //  not make for good human readability, but it illustrates how to control floating point
    //  formatting
    println!("formatted exponentially: {:?}", String::from_utf8(buf).unwrap());

    Ok(())
}























