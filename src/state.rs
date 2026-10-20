// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What a cart reports to `probe` and `twin`: a flat JSON object, built without any dependencies.

/// Builder for a cart's state report.
///
/// ```
/// let s = polter::State::new().num("x", 12.5).int("lives", 3).flag("on_ground", true).text("mode", "play");
/// assert_eq!(s.to_json(), r#"{"x":12.5,"lives":3,"on_ground":true,"mode":"play"}"#);
/// ```
#[derive(Clone, Debug, Default)]
pub struct State {
    parts: Vec<String>,
}

impl State {
    pub fn new() -> State {
        State::default()
    }

    /// A number, rounded to 4 decimals. NaN and infinity become `null`.
    pub fn num(mut self, key: &str, v: f32) -> State {
        let value = if v.is_finite() {
            format!("{}", (v * 10_000.0).round() / 10_000.0 + 0.0) // + 0.0 turns -0 into 0
        } else {
            "null".into()
        };
        self.parts.push(format!("{}:{}", quote(key), value));
        self
    }

    pub fn int(mut self, key: &str, v: i64) -> State {
        self.parts.push(format!("{}:{}", quote(key), v));
        self
    }

    pub fn flag(mut self, key: &str, v: bool) -> State {
        self.parts.push(format!("{}:{}", quote(key), v));
        self
    }

    pub fn text(mut self, key: &str, v: &str) -> State {
        self.parts.push(format!("{}:{}", quote(key), quote(v)));
        self
    }

    pub fn to_json(&self) -> String {
        format!("{{{}}}", self.parts.join(","))
    }
}

/// A JSON string literal.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
