# Changelog

## Version 1.0.1
* add convenience API for creating parse errors in JsonReader
* merge JsonParseError::Token and JsonParseError::Parse
* add JsonReader::expect_end_object() and JsonReader::expect_end_array()
* rename JsonReader::expect_next_* to JsonReader::expect_*