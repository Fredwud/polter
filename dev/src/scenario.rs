// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scenarios: a start, the inputs, and what must be true at which ticks, in a text file that an
//! agent writes, a human's recording becomes, or a judge keeps hidden. `assert` runs them and
//! checks every expectation on every tick it covers, because a game can end well after breaking
//! its rules on the way.
//!
//! ```text
//! # GIRDER: the bot's run collects every bolt
//! seed 1
//! haunt ../haunts/bot_run.haunt     # the inputs, relative to this file
//! input 10-20 right a               # more input, a line as in a .haunt file
//! frames 800                        # how long (default: the last tick named below)
//! at 600: bolts >= 6                # at one tick
//! from 1 to 650: mode != "title"    # on every tick of a range
//! always: bolts <= 8                # on every tick
//! ```
//!
//! An expectation compares a key of the game's state (`state()`) with a number, `true`,
//! `false`, `null` or a "text", by `==`, `!=`, `<`, `<=`, `>` or `>=`.

use polter::haunt::Haunt;
use std::path::Path;

/// A value in a game's state.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Bool(bool),
    Text(String),
    Null,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Num(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Text(v) => write!(f, "{v:?}"),
            Value::Null => write!(f, "null"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// The ticks an expectation covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum When {
    At(u64),
    During(u64, u64),
    Always,
}

impl When {
    pub fn covers(self, frame: u64) -> bool {
        match self {
            When::At(f) => frame == f,
            When::During(a, b) => (a..=b).contains(&frame),
            When::Always => true,
        }
    }
}

/// One line of what must be true.
#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    pub when: When,
    pub key: String,
    pub op: Op,
    pub value: Value,
    /// The line in the file, and its text, for the report.
    pub line: usize,
    pub text: String,
}

impl Expect {
    /// Whether the expectation holds in a state; Err says why it can't be judged.
    pub fn holds(&self, state: &[(String, Value)]) -> Result<bool, String> {
        let Some((_, actual)) = state.iter().find(|(k, _)| *k == self.key) else {
            let keys: Vec<&str> = state.iter().map(|(k, _)| k.as_str()).collect();
            return Err(format!(
                "the state has no key {:?} (it has: {})",
                self.key,
                keys.join(", ")
            ));
        };
        Ok(match (actual, &self.value) {
            (Value::Num(a), Value::Num(b)) => match self.op {
                Op::Eq => a == b,
                Op::Ne => a != b,
                Op::Lt => a < b,
                Op::Le => a <= b,
                Op::Gt => a > b,
                Op::Ge => a >= b,
            },
            (a, b) => match self.op {
                Op::Eq => a == b,
                Op::Ne => a != b,
                _ => {
                    return Err(format!(
                        "{} {actual} can't be compared with {} (only numbers can be ordered)",
                        self.key, self.value
                    ));
                }
            },
        })
    }

    /// The value the state has for this expectation's key.
    pub fn actual(&self, state: &[(String, Value)]) -> Option<Value> {
        state
            .iter()
            .find(|(k, _)| *k == self.key)
            .map(|(_, v)| v.clone())
    }
}

/// A scenario, read from a file.
#[derive(Debug, Default)]
pub struct Scenario {
    pub seed: u64,
    pub frames: u64,
    pub haunt: Haunt,
    pub expects: Vec<Expect>,
}

/// A line without its comment: from a `#` that is not inside a "text".
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn number(s: &str, line: usize, what: &str) -> Result<u64, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("line {line}: {what} needs a whole number, got {s:?}"))
}

fn parse_value(s: &str) -> Option<Value> {
    let s = s.trim();
    Some(match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') => {
            Value::Text(s[1..s.len() - 1].to_string())
        }
        _ => Value::Num(s.parse().ok()?),
    })
}

/// `KEY OP VALUE`, e.g. `bolts >= 6` or `mode != "title"`.
fn parse_test(s: &str, line: usize) -> Result<(String, Op, Value), String> {
    let s = s.trim();
    // the longest operators first, so `<=` isn't read as `<`
    for (sym, op) in [
        ("==", Op::Eq),
        ("!=", Op::Ne),
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("<", Op::Lt),
        (">", Op::Gt),
    ] {
        if let Some(at) = s.find(sym) {
            let key = s[..at].trim();
            let value = &s[at + sym.len()..];
            if key.is_empty() || key.contains(char::is_whitespace) {
                return Err(format!(
                    "line {line}: expected a state key before {sym}, got {key:?}"
                ));
            }
            let value = parse_value(value).ok_or_else(|| {
                format!("line {line}: {value:?} is not a number, true, false, null or a \"text\"")
            })?;
            return Ok((key.to_string(), op, value));
        }
    }
    Err(format!(
        "line {line}: expected KEY OP VALUE with OP one of == != < <= > >=, got {s:?}"
    ))
}

impl Scenario {
    /// Reads a scenario; `dir` is where relative `haunt` files are looked up.
    pub fn parse(text: &str, dir: &Path) -> Result<Scenario, String> {
        let mut seed = 1;
        let mut frames = None;
        let mut haunt_text = String::new();
        let mut expects = Vec::new();
        for (k, raw) in text.lines().enumerate() {
            let line = k + 1;
            let content = strip_comment(raw).trim();
            if content.is_empty() {
                continue;
            }
            let (word, rest) = content
                .split_once(char::is_whitespace)
                .unwrap_or((content, ""));
            let rest = rest.trim();
            let (when, test) = match word.trim_end_matches(':') {
                "seed" => {
                    seed = number(rest, line, "seed")?;
                    continue;
                }
                "frames" => {
                    frames = Some(number(rest, line, "frames")?.max(1));
                    continue;
                }
                "haunt" => {
                    let path = dir.join(rest);
                    let t = std::fs::read_to_string(&path).map_err(|e| {
                        format!("line {line}: cannot read the haunt {}: {e}", path.display())
                    })?;
                    haunt_text.push_str(&t);
                    haunt_text.push('\n');
                    continue;
                }
                "input" => {
                    haunt_text.push_str(rest);
                    haunt_text.push('\n');
                    continue;
                }
                "at" => {
                    let (f, test) = rest
                        .split_once(':')
                        .ok_or(format!("line {line}: expected `at TICK: KEY OP VALUE`"))?;
                    (When::At(number(f, line, "at")?), test)
                }
                "from" => {
                    let (range, test) = rest
                        .split_once(':')
                        .ok_or(format!("line {line}: expected `from A to B: KEY OP VALUE`"))?;
                    let (a, b) = range
                        .split_once(" to ")
                        .ok_or(format!("line {line}: expected `from A to B: …`"))?;
                    let (a, b) = (number(a, line, "from")?, number(b, line, "to")?);
                    if b < a {
                        return Err(format!(
                            "line {line}: the range ends ({b}) before it starts ({a})"
                        ));
                    }
                    (When::During(a, b), test)
                }
                "always" => (When::Always, rest),
                other => {
                    return Err(format!(
                        "line {line}: unknown word {other:?} (seed, frames, haunt, input, at, from, always)"
                    ));
                }
            };
            let (key, op, value) = parse_test(test, line)?;
            expects.push(Expect {
                when,
                key,
                op,
                value,
                line,
                text: content.to_string(),
            });
        }
        let haunt = Haunt::parse(&haunt_text).map_err(|e| format!("input: {e}"))?;
        // without `frames`, long enough to reach every tick the expectations name; with it, an
        // expectation after the end is a mistake in the file, not a silent pass or fail
        let last = |e: &Expect| match e.when {
            When::At(f) | When::During(_, f) => f,
            When::Always => 0,
        };
        let end = expects.iter().map(last).max().unwrap_or(0);
        let frames = match frames {
            Some(n) => {
                if let Some(late) = expects.iter().find(|e| last(e) > n) {
                    return Err(format!(
                        "line {}: frame {} is after the scenario's end (frames {n})",
                        late.line,
                        last(late)
                    ));
                }
                n
            }
            None if end > 0 => end,
            None => 120,
        };
        Ok(Scenario {
            seed,
            frames,
            haunt,
            expects,
        })
    }

    pub fn load(path: &Path) -> Result<Scenario, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
        Scenario::parse(&text, path.parent().unwrap_or(Path::new(".")))
    }
}

/// A game's state report (a flat JSON object, as `State` writes it) as keys and values.
pub fn parse_state(json: &str) -> Result<Vec<(String, Value)>, String> {
    let b = json.trim().as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let err = |i: usize, what: &str| format!("state JSON, byte {i}: {what}");
    let skip = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    let string = |i: &mut usize| -> Result<String, String> {
        // at the opening quote
        *i += 1;
        let mut s = String::new();
        while *i < b.len() {
            match b[*i] {
                b'"' => {
                    *i += 1;
                    return Ok(s);
                }
                b'\\' => {
                    *i += 1;
                    match b.get(*i) {
                        Some(b'n') => s.push('\n'),
                        Some(b'u') => {
                            let hex = std::str::from_utf8(b.get(*i + 1..*i + 5).unwrap_or(&[]))
                                .ok()
                                .and_then(|h| u32::from_str_radix(h, 16).ok())
                                .and_then(char::from_u32)
                                .ok_or(err(*i, "a bad \\u escape"))?;
                            s.push(hex);
                            *i += 4;
                        }
                        Some(&c) => s.push(c as char),
                        None => return Err(err(*i, "an escape at the end")),
                    }
                    *i += 1;
                }
                _ => {
                    // a whole UTF-8 character
                    let rest = std::str::from_utf8(&b[*i..]).map_err(|_| err(*i, "not UTF-8"))?;
                    let c = rest.chars().next().unwrap_or('\0');
                    s.push(c);
                    *i += c.len_utf8();
                }
            }
        }
        Err(err(*i, "a string that doesn't end"))
    };
    skip(&mut i);
    if b.get(i) != Some(&b'{') {
        return Err(err(i, "expected {"));
    }
    i += 1;
    loop {
        skip(&mut i);
        match b.get(i) {
            Some(b'}') => return Ok(out),
            Some(b'"') => {}
            _ => return Err(err(i, "expected a key or }")),
        }
        let key = string(&mut i)?;
        skip(&mut i);
        if b.get(i) != Some(&b':') {
            return Err(err(i, "expected :"));
        }
        i += 1;
        skip(&mut i);
        let value = match b.get(i) {
            Some(b'"') => Value::Text(string(&mut i)?),
            Some(_) => {
                let start = i;
                while i < b.len() && !matches!(b[i], b',' | b'}') && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                let word =
                    std::str::from_utf8(&b[start..i]).map_err(|_| err(start, "not UTF-8"))?;
                match word {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "null" => Value::Null,
                    n => Value::Num(n.parse().map_err(|_| err(start, "a bad number"))?),
                }
            }
            None => return Err(err(i, "a key without a value")),
        };
        out.push((key, value));
        skip(&mut i);
        match b.get(i) {
            Some(b',') => i += 1,
            Some(b'}') => return Ok(out),
            _ => return Err(err(i, "expected , or }")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(json: &str) -> Vec<(String, Value)> {
        parse_state(json).expect("state")
    }

    #[test]
    fn the_state_reads_back_as_state_writes_it() {
        let json = polter::State::new()
            .num("x", 12.5)
            .int("lives", -3)
            .flag("on_ground", true)
            .text("mode", "pl\"ay\n·")
            .num("nan", f32::NAN)
            .to_json();
        assert_eq!(
            state(&json),
            vec![
                ("x".into(), Value::Num(12.5)),
                ("lives".into(), Value::Num(-3.0)),
                ("on_ground".into(), Value::Bool(true)),
                ("mode".into(), Value::Text("pl\"ay\n·".into())),
                ("nan".into(), Value::Null),
            ]
        );
        assert!(
            parse_state("{\"a\":1").is_err(),
            "an object that doesn't end"
        );
        assert!(parse_state("[1]").is_err(), "not an object");
        assert!(state("{}").is_empty());
    }

    #[test]
    fn a_scenario_reads_its_parts() {
        let s = Scenario::parse(
            "# a comment\nseed 7\ninput 1-3 right a\nat 5: x >= 2.5\nfrom 2 to 9: mode != \"title\"\nalways: lives > 0  # trailing comment\n",
            Path::new("."),
        )
        .expect("scenario");
        assert_eq!((s.seed, s.frames), (7, 9), "runs to the last tick named");
        assert_eq!(
            s.haunt.buttons(2),
            polter::Button::Right.bit() | polter::Button::A.bit()
        );
        assert_eq!(s.haunt.buttons(4), 0);
        assert_eq!(s.expects.len(), 3);
        assert_eq!(s.expects[0].when, When::At(5));
        assert_eq!(s.expects[1].when, When::During(2, 9));
        assert_eq!(s.expects[1].value, Value::Text("title".into()));
        assert_eq!(s.expects[2].op, Op::Gt);
        assert_eq!(s.expects[2].line, 6);
    }

    #[test]
    fn mistakes_name_their_line() {
        for (text, needle) in [
            ("at 5 x > 1", "line 1"),
            ("seed\nat x: y > 1", "line 1"),
            ("at 5: x ~ 1", "line 1"),
            ("frames 10\nfrom 9 to 3: x > 1", "line 2"),
            ("at 5: x > maybe", "line 1"),
            ("wiggle 3", "unknown word"),
            ("frames 300\nat 900: x > 1", "line 2: frame 900 is after"),
        ] {
            let e = Scenario::parse(text, Path::new(".")).expect_err(text);
            assert!(e.contains(needle), "{text:?} gave {e:?}");
        }
    }

    #[test]
    fn expectations_hold_or_say_why_not() {
        let s = state(r##"{"x":3,"mode":"play","on":true,"tag":"#1"}"##);
        let e = |t: &str| {
            let (key, op, value) = parse_test(t, 1).expect(t);
            Expect {
                when: When::Always,
                key,
                op,
                value,
                line: 1,
                text: t.into(),
            }
        };
        assert_eq!(e("x >= 3").holds(&s), Ok(true));
        assert_eq!(e("x < 3").holds(&s), Ok(false));
        assert_eq!(e("x == 3").holds(&s), Ok(true));
        assert_eq!(e("mode == \"play\"").holds(&s), Ok(true));
        assert_eq!(
            e(strip_comment("tag == \"#1\" # a comment").trim()).holds(&s),
            Ok(true)
        );
        assert_eq!(e("on != true").holds(&s), Ok(false));
        assert!(e("mode > 1").holds(&s).is_err(), "text can't be ordered");
        assert!(
            e("y == 1")
                .holds(&s)
                .unwrap_err()
                .contains("has no key \"y\"")
        );
    }
}
