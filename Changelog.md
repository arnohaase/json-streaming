# Changelog

## Version 1.0.3
* add `JsonReader::expect_end_of_stream()`

## Version 1.0.2
* fix typo in hyperlink to changelog

## Version 1.0.1
* add convenience API for creating parse errors in `JsonReader`
* merge `JsonParseError::Token` and `JsonParseError::Parse`
* add `JsonReader::expect_end_object()` and `JsonReader::expect_end_array()`
* rename `JsonReader::expect_next_*` to `JsonReader::expect_*`
* add convenience API for reading homogeneous arrays: `JsonReader::expect_*_or_end_array()`

