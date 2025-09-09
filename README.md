# json-streaming - reading and writing JSON in a streaming fashion

> This is a library for writing and reading JSON through a streaming API, i.e. without data structures
> that are then "mapped".

It fills a niche: for most typical use cases, the `serde` ecosystem is the better and more natural choice, and if
`serde` does what you need, you should probably choose it over `json-streaming`. This library is written to cover
special use cases:
* Writing and processing big data structures without materializing them
* Writing and reading JSON representations that have significant structural differences to the in-memory representation,
   e.g. flattening nested in-memory maps to a flat JSON object based on domain knowledge
* Fine-grained control over how JSON is written
* Working with JSON in a `no-std`, no-alloc environment

All APIs exist in both blocking and non-blocking variants.

For a change history and versions, see the [Changelog](Changelog.md).

## Getting started

Here's a simple example of how to write JSON using the library:

```
use json_streaming::blocking::*;

fn write_something() -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    let mut writer = JsonWriter::new_pretty(&mut stdout);

    let mut o = JsonObject::new(&mut writer)?;
    o.write_string_value("a", "hello")?;
    o.write_string_value("b", "world")?;
    o.end()?;

    writer.flush()
}
```

Or reading the same data:
```
use json_streaming::blocking::*;

fn read_something(r: &mut impl io::Read) -> JsonParseResult<(), io::Error> {
    let mut json_reader = JsonReader::new(1024, r);

    json_reader.expect_start_object()?;
    loop {
        match json_reader.expect_key()? {
            Some("a") => println!("a: {}", json_reader.expect_string()?),
            Some("b") => println!("b: {}", json_reader.expect_string()?),
            Some(_other) => {
                return Err(JsonParseError::Parse("unexpected key parsing 'person'", json_reader.location()));
            },
            None => break,
        }
    }
    Ok(())
}
```

See the examples (`blocking.rs` and `non_blocking.rs` are good starting points) for more comprehensive code with
lots of comments to explain.

## Feature Flags

### default

By default, both blocking and non-blocking APIs are included. Adapters to `std::io::Read` and `std::io::Write` are
included, making the default library depend on `std` by default. 

The `tokio` adapters for non-blocking APIs are not included and require the `tokio` feature flag. This is done to 
avoid pulling in the `tokio` dependency by default.

> default = ["blocking", "non-blocking", "std"]

### blocking and std

The `blocking` feature flag is active by default; for detailed control, `default-features` must be disabled. The
`blocking` feature flag by itself adds the blocking APIs themselves (which do *not* depend on *std*), but not the adapters
for `std::io::Read` and `std::io::Write` (which do).

So in order to work with JSON in a `no-std` environment, disable `default-features` and add the `blocking` feature
flag. You will have to provide your own implementations of the `BlockingRead` or `BlockingWrite` trait to adapt to
your environment's data sources or sinks. See the `no_std.rs` example for a showcase.

### non-blocking and tokio

The non-blocking API is included by default, but without the adapters for Tokio's `tokio::io::AsyncRead` and 
`tokio::io::AsyncWrite` traits - those require the `tokio` feature flag, which adds a dependency on the Tokio library.






