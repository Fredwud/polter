// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A small JSON reader without dependencies, for the files art tools write (Aseprite's sheets,
//! later LDtk's levels). Objects keep their keys in the order they were written. Errors name the
//! line and column.

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Json>),
    /// Keys and values in the order the file has them.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Reads a JSON text.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser {
            s: text.as_bytes(),
            i: 0,
        };
        p.space();
        let v = p.value(0)?;
        p.space();
        if p.i < p.s.len() {
            return Err(p.error("more after the value"));
        }
        Ok(v)
    }

    /// A key of an object (`None` for a missing key or a value that is no object).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// A whole number (a number without a fraction, within `i64`).
    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64()
            .filter(|n| n.fract() == 0.0 && n.abs() < 9.0e15)
            .map(|n| n as i64)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The items of a list, or the values of an object in order (Aseprite writes its frames
    /// either way).
    pub fn items(&self) -> Vec<&Json> {
        match self {
            Json::List(v) => v.iter().collect(),
            Json::Object(pairs) => pairs.iter().map(|(_, v)| v).collect(),
            _ => Vec::new(),
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

/// How deep lists and objects may nest (a guard against damaged or hostile files).
const MOST_DEPTH: usize = 256;

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        let before = &self.s[..self.i.min(self.s.len())];
        let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = before.iter().rev().take_while(|&&b| b != b'\n').count() + 1;
        format!("JSON, line {line}, column {column}: {what}")
    }

    fn space(&mut self) {
        while self
            .s
            .get(self.i)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.i += 1;
        }
    }

    fn expect(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.error("an unknown word"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MOST_DEPTH {
            return Err(self.error("nested too deep"));
        }
        match self.s.get(self.i) {
            None => Err(self.error("the text ends where a value should be")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.list(depth),
            Some(b'"') => Ok(Json::Text(self.text()?)),
            Some(b't') => self.expect("true", Json::Bool(true)),
            Some(b'f') => self.expect("false", Json::Bool(false)),
            Some(b'n') => self.expect("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("not a value")),
        }
    }

    fn list(&mut self, depth: usize) -> Result<Json, String> {
        self.i += 1;
        let mut out = Vec::new();
        self.space();
        if self.s.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Json::List(out));
        }
        loop {
            self.space();
            out.push(self.value(depth + 1)?);
            self.space();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::List(out));
                }
                _ => return Err(self.error("a list wants , or ]")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.i += 1;
        let mut out = Vec::new();
        self.space();
        if self.s.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Json::Object(out));
        }
        loop {
            self.space();
            if self.s.get(self.i) != Some(&b'"') {
                return Err(self.error("an object's key must be a \"text\""));
            }
            let key = self.text()?;
            self.space();
            if self.s.get(self.i) != Some(&b':') {
                return Err(self.error("a key wants a : after it"));
            }
            self.i += 1;
            self.space();
            out.push((key, self.value(depth + 1)?));
            self.space();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Object(out));
                }
                _ => return Err(self.error("an object wants , or }")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let Some(digits) = self.s.get(self.i..self.i + 4) else {
            return Err(self.error("\\u wants four hex digits"));
        };
        let text = std::str::from_utf8(digits).map_err(|_| self.error("\\u wants hex digits"))?;
        let v = u32::from_str_radix(text, 16).map_err(|_| self.error("\\u wants hex digits"))?;
        self.i += 4;
        Ok(v)
    }

    fn text(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|&b| b != b'"' && b != b'\\' && b >= 0x20)
            {
                self.i += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.error("not UTF-8"))?,
            );
            match self.s.get(self.i) {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let Some(&e) = self.s.get(self.i) else {
                        return Err(self.error("the text ends inside an escape"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut c = self.hex4()?;
                            // a surrogate pair: two escapes for one character
                            if (0xd800..0xdc00).contains(&c)
                                && self.s.get(self.i..self.i + 2) == Some(b"\\u")
                            {
                                self.i += 2;
                                let low = self.hex4()?;
                                c = 0x10000
                                    + ((c - 0xd800) << 10)
                                    + (low.wrapping_sub(0xdc00) & 0x3ff);
                            }
                            out.push(char::from_u32(c).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.error("an unknown escape")),
                    }
                }
                Some(_) => return Err(self.error("a control character inside a text")),
                None => return Err(self.error("the text ends inside a \"text\"")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| self.error(&format!("{text:?} is not a number")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_json_has() {
        let j = Json::parse(
            r#" { "b": [1, -2.5, 3e2, true, false, null], "a": "x\"y\\z\nä😀", "o": {} } "#,
        )
        .unwrap();
        // keys keep their order
        let Json::Object(pairs) = &j else { panic!() };
        assert_eq!(
            pairs.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["b", "a", "o"]
        );
        let b = j.get("b").unwrap().items();
        assert_eq!(b[0].as_i64(), Some(1));
        assert_eq!(b[1].as_f64(), Some(-2.5));
        assert_eq!(b[2].as_i64(), Some(300));
        assert_eq!((b[3].as_bool(), b[5]), (Some(true), &Json::Null));
        assert_eq!(j.get("a").unwrap().as_str(), Some("x\"y\\z\nä😀"));
        assert_eq!(j.get("o"), Some(&Json::Object(vec![])));
        assert_eq!(j.get("nope"), None);
    }

    #[test]
    fn says_where_it_is_wrong() {
        for (text, says) in [
            ("{\"a\": 1,\n  \"b\" 2}", "line 2, column 7"),
            ("[1, 2", "a list wants"),
            ("{\"a\": tru}", "unknown word"),
            ("\"open", "ends inside"),
            ("1 2", "more after"),
            ("", "ends where a value"),
        ] {
            let err = Json::parse(text).unwrap_err();
            assert!(err.contains(says), "{text:?}: {err}");
        }
        let deep = "[".repeat(300) + &"]".repeat(300);
        assert!(Json::parse(&deep).unwrap_err().contains("too deep"));
    }
}
