/// [JsonFormatter] controls how whitespace is added between JSON elements in the output. It does not
///  affect the JSON's semantics, but only its looks and size.
pub trait JsonFormatter {
    /// optional whitespace after the ':' of a JSON object's key.
    fn after_key(&self) -> &str;
    /// optional newline after the start of an object or array; adds a level of nesting
    fn after_start_nested(&mut self) -> &str;
    /// optional newline after an element
    fn after_element(&self) -> &str;
    /// optional indent before then ending character of a nested object or array; removes a level of nesting
    fn before_end_nested(&mut self, is_empty: bool) -> &str;
    /// indentation, if any
    fn indent(&self) -> &str;
}

/// Write a minimum of whitespace, minimizing output size
pub struct CompactFormatter;
impl JsonFormatter for CompactFormatter {
    fn after_key(&self) -> &str { "" }
    fn after_start_nested(&mut self) -> &str { "" }
    fn after_element(&self) -> &str { "" }
    fn before_end_nested(&mut self, _is_empty: bool) -> &str { "" }
    fn indent(&self) -> &str { "" }
}

/// Write some whitespace and indentation to improve human readability
pub struct PrettyFormatter {
    indent_level: usize,
}
impl PrettyFormatter {
    pub fn new() -> PrettyFormatter {
        PrettyFormatter {
            indent_level: 0,
        }
    }
}
impl JsonFormatter for PrettyFormatter {
    fn after_key(&self) -> &str {
        " "
    }

    fn after_start_nested(&mut self) -> &str {
        self.indent_level += 1;
        ""
    }

    fn after_element(&self) -> &str {
        ""
    }

    fn before_end_nested(&mut self, is_empty: bool) -> &str {
        self.indent_level -= 1;
        if is_empty {
            ""
        }
        else {
            self.indent()
        }
    }

    fn indent(&self) -> &str {
        static INDENT: &'static str = "\n                                                                                                                                                                                                                                                 ";
        &INDENT[..2*self.indent_level + 1]
    }
}
