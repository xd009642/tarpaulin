use std::default::Default;
use std::io;
use serde_json::ser::CharEscape;
use serde_json::ser::CompactFormatter;

struct SafeFormatter(CompactFormatter);

impl Default for SafeFormatter {
    fn default() -> Self {
        SafeFormatter(CompactFormatter)
    }
}

impl serde_json::ser::Formatter for SafeFormatter {
    fn write_string_fragment<W: ?Sized>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: io::Write,
    {
        let mut start = 0;
        let mut code_length = 0;
        for ch in fragment.chars() {
            code_length += ch.len_utf8();
            let escape = match ch {
                '<' | '>' | '&' => CharEscape::AsciiControl(ch as u8),
                _ => continue,
            };
            if start < code_length - 1 {
                self.0.write_string_fragment(writer, &fragment[start..code_length-1])?;
            }

            self.write_char_escape(writer, escape)?;

            start = code_length;
        }

        if start < code_length {
            self.0.write_string_fragment(writer, &fragment[start..])?;
        }
        Ok(())
    }
}

pub fn to_string_safe<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let mut writer = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut writer, SafeFormatter::default());
    value.serialize(&mut ser).map_err(|e| e.to_string())?;
    let string = String::from_utf8(writer).map_err(|e| e.to_string())?;
    Ok(string)
}

#[cfg(test)]
mod tests {
    use serde_json::{self, json};
    use super::*;

    #[test]
    fn test_json_without_html() {
        let x = json!({
            "a": 1,
            "b": "c",
            "d": "text with \"quotes\" inside",
        });
        assert_eq!(to_string_safe(&x).unwrap(), serde_json::to_string(&x).unwrap());
    }

    #[test]
    fn test_json_with_html() {
        let x = json!({
            "a": 1,
            "b": "c",
            "d": "text with \"quotes\" inside",
            "h": "some <script>alert(\"Alert\")</script> html",
        });
        assert_eq!(
            to_string_safe(&x).unwrap().as_str(),
            r#"{"a":1,"b":"c","d":"text with \"quotes\" inside","h":"some \u003cscript\u003ealert(\"Alert\")\u003c/script\u003e html"}"#
        );
    }

    #[test]
    fn test_json_unicode() {
        let x = json!({
            "a": 1,
            "b": "a<❌>b",
        });
        assert_eq!(
            to_string_safe(&x).unwrap().as_str(),
            r#"{"a":1,"b":"a\u003c❌\u003eb"}"#
        );
    }
}
