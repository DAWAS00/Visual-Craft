//! The text form of class members, typed inline on the canvas card and in the panel:
//!
//! - attribute: `[vis] [static] name [: Type] [[multiplicity]] [= default]`
//! - operation: `[vis] [static] [abstract] name(p: T, …) [: Return]`
//! - literal: a name.
//!
//! `vis` is one of `+ - # ~` (public when left out). Parsing never panics and reports where the
//! problem is (a character index into the input). `format_*` writes the canonical form, which
//! parses back to the same member.

use super::{Attribute, MemberId, Operation, Param, Visibility, default_problem, text_problem};
use super::{Literal, MAX_PARAMS, MAX_TEXT};

/// Why a line didn't parse: what is wrong, and the character index it starts at.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} (at character {at})")]
pub struct TextError {
    pub at: usize,
    pub message: &'static str,
}

fn err<T>(at: usize, message: &'static str) -> Result<T, TextError> {
    Err(TextError { at, message })
}

/// The longest line accepted, in characters: every part at its longest, with room for markers.
const MAX_LINE: usize = MAX_TEXT * (3 + 2 * MAX_PARAMS) + 64;

/// A cursor over a line by characters, so every index is a character index and slicing is
/// always on a boundary.
struct Line {
    chars: Vec<char>,
    pos: usize,
}

impl Line {
    fn new(s: &str) -> Result<Self, TextError> {
        let chars: Vec<char> = s.chars().take(MAX_LINE + 1).collect();
        if chars.len() > MAX_LINE {
            return err(MAX_LINE, "the line is too long");
        }
        if let Some(i) = chars.iter().position(|c| super::breaks_line(*c)) {
            return err(i, "a member is one line");
        }
        Ok(Self { chars, pos: 0 })
    }

    fn skip_spaces(&mut self) {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Takes `word` when it comes next followed by a space and then a name (`static name`), not as
    /// a prefix of a longer name (`statics`) nor as a member's own name (`static : Bool`).
    fn keyword(&mut self, word: &str) -> bool {
        let w: Vec<char> = word.chars().collect();
        let end = self.pos + w.len();
        let matches = self.chars.get(self.pos..end).is_some_and(|s| s == w.as_slice());
        if !matches || !self.chars.get(end).is_some_and(|c| c.is_whitespace()) {
            return false;
        }
        let mut next = end;
        while self.chars.get(next).is_some_and(|c| c.is_whitespace()) {
            next += 1;
        }
        match self.chars.get(next) {
            Some(c) if !super::FORBIDDEN.contains(c) => {
                self.pos = next;
                true
            }
            _ => false,
        }
    }

    /// The characters from `a` to `b`, trimmed, with the index of the first kept one.
    fn part(&self, a: usize, b: usize) -> (String, usize) {
        let slice = self.chars.get(a..b).unwrap_or(&[]);
        let lead = slice.iter().take_while(|c| c.is_whitespace()).count();
        let s: String = slice.iter().collect();
        (s.trim().to_string(), a + lead)
    }

    /// The index of the first `c` at or after `from`.
    fn find(&self, c: char, from: usize) -> Option<usize> {
        self.chars.iter().enumerate().skip(from).find(|(_, x)| **x == c).map(|(i, _)| i)
    }
}

/// The visibility marker and the `static` / `abstract` keywords at the start.
fn prefix(l: &mut Line) -> (Visibility, bool, bool) {
    l.skip_spaces();
    let mut vis = Visibility::Public;
    if let Some(v) = l.peek().and_then(Visibility::from_symbol) {
        vis = v;
        l.pos += 1;
        l.skip_spaces();
    }
    let (mut is_static, mut is_abstract) = (false, false);
    loop {
        if !is_static && l.keyword("static") {
            is_static = true;
        } else if !is_abstract && l.keyword("abstract") {
            is_abstract = true;
        } else {
            break;
        }
    }
    (vis, is_static, is_abstract)
}

fn name_at(s: &str, at: usize) -> Result<String, TextError> {
    match text_problem(s, false) {
        None => Ok(s.to_string()),
        Some(_) if s.is_empty() => err(at, "a name is needed here"),
        Some(why) => err(at, why_message(why)),
    }
}

fn type_at(s: &str, at: usize) -> Result<String, TextError> {
    match text_problem(s, true) {
        None => Ok(s.to_string()),
        Some(_) if s.is_empty() => err(at, "a type is needed after the colon"),
        Some(why) => err(at, why_message(why)),
    }
}

/// [`text_problem`]'s reasons, as messages for the editor.
fn why_message(why: &'static str) -> &'static str {
    match why {
        "it is too long" => "this is too long",
        "it has a space" => "a name has no spaces",
        "it has one of : ( ) , = [ ] \"" => "names and types can't use : ( ) , = [ ] or quotes",
        _ => "this can't be used here",
    }
}

/// Parses an attribute line. `id` is the member's ID (parsing doesn't allocate one).
pub fn parse_attribute(id: MemberId, s: &str) -> Result<Attribute, TextError> {
    let mut l = Line::new(s)?;
    let (visibility, is_static, is_abstract) = prefix(&mut l);
    if is_abstract {
        return err(l.pos, "an attribute can't be abstract");
    }
    let start = l.pos;
    let end = l.chars.len();
    // The default is everything after the first `=` (names and types can't hold one).
    let (head_end, default) = match l.find('=', start) {
        Some(eq) => {
            let (d, at) = l.part(eq + 1, end);
            if d.is_empty() {
                return err(eq + 1, "a default is needed after =");
            }
            if let Some(why) = default_problem(&d) {
                return err(at, default_message(why));
            }
            (eq, Some(d))
        }
        None => (end, None),
    };
    // A multiplicity in brackets at the end of the head.
    let (body_end, multiplicity) = match l.find('[', start).filter(|i| *i < head_end) {
        Some(open) => {
            let (rest, _) = l.part(open, head_end);
            if !rest.ends_with(']') {
                return err(open, "a multiplicity ends with ]");
            }
            let (inner, at) = l.part(open + 1, open + rest.chars().count().saturating_sub(1));
            if !crate::use_case::multiplicity_ok(&inner) {
                return err(at, "a multiplicity is n, n..m, n..* or *");
            }
            (open, Some(inner))
        }
        None => (head_end, None),
    };
    let (name, type_name) = match l.find(':', start).filter(|i| *i < body_end) {
        Some(colon) => {
            let (n, at) = l.part(start, colon);
            let (t, tat) = l.part(colon + 1, body_end);
            (name_at(&n, at)?, Some(type_at(&t, tat)?))
        }
        None => {
            let (n, at) = l.part(start, body_end);
            (name_at(&n, at)?, None)
        }
    };
    Ok(Attribute { id, visibility, name, type_name, multiplicity, default, is_static })
}

fn default_message(why: &'static str) -> &'static str {
    match why {
        "it is too long" => "the default is too long",
        "a quoted default only escapes \\\" and \\\\" => "a quoted default only escapes \\\" and \\\\",
        "a quoted default ends with its closing quote" => "a quoted default ends with its closing quote",
        "quotes belong around the whole default" => "quotes belong around the whole default",
        _ => "this default can't be used",
    }
}

/// Parses an operation line.
pub fn parse_operation(id: MemberId, s: &str) -> Result<Operation, TextError> {
    let mut l = Line::new(s)?;
    let (visibility, is_static, is_abstract) = prefix(&mut l);
    let start = l.pos;
    let Some(open) = l.find('(', start) else { return err(l.chars.len(), "an operation has ( ) after its name") };
    let Some(close) = l.find(')', open) else { return err(l.chars.len(), "the parameters end with )") };
    let (n, at) = l.part(start, open);
    let name = name_at(&n, at)?;
    let mut params = Vec::new();
    let (inside, _) = l.part(open + 1, close);
    if !inside.is_empty() {
        let mut from = open + 1;
        loop {
            let comma = l.find(',', from).filter(|i| *i < close);
            let to = comma.unwrap_or(close);
            if params.len() >= MAX_PARAMS {
                return err(from, "an operation has at most 8 parameters");
            }
            let (p, pat) = l.part(from, to);
            let param = match p.split_once(':') {
                Some((pn, pt)) => {
                    let colon = pat + pn.chars().count();
                    let tat = colon + 1 + pt.chars().take_while(|c| c.is_whitespace()).count();
                    Param { name: name_at(pn.trim(), pat)?, type_name: Some(type_at(pt.trim(), tat)?) }
                }
                None => Param { name: name_at(&p, pat)?, type_name: None },
            };
            params.push(param);
            match comma {
                Some(c) => from = c + 1,
                None => break,
            }
        }
    }
    let (rest, rat) = l.part(close + 1, l.chars.len());
    let return_type = if rest.is_empty() {
        None
    } else if let Some(t) = rest.strip_prefix(':') {
        let lead = t.chars().take_while(|c| c.is_whitespace()).count();
        Some(type_at(t.trim(), rat + 1 + lead)?)
    } else {
        return err(rat, "only : and a return type may follow the parameters");
    };
    Ok(Operation { id, visibility, name, params, return_type, is_static, is_abstract })
}

/// Parses an enumeration literal.
pub fn parse_literal(id: MemberId, s: &str) -> Result<Literal, TextError> {
    let l = Line::new(s)?;
    let (n, at) = l.part(0, l.chars.len());
    Ok(Literal { id, name: name_at(&n, at)? })
}

/// The canonical text of an attribute: `+ static name : Type [m] = default`.
pub fn format_attribute(a: &Attribute) -> String {
    let mut s = format!("{} ", a.visibility.symbol());
    if a.is_static {
        s.push_str("static ");
    }
    s.push_str(&a.name);
    if let Some(t) = &a.type_name {
        s.push_str(" : ");
        s.push_str(t);
    }
    if let Some(m) = &a.multiplicity {
        s.push_str(" [");
        s.push_str(m);
        s.push(']');
    }
    if let Some(d) = &a.default {
        s.push_str(" = ");
        s.push_str(d);
    }
    s
}

/// The canonical text of an operation: `+ static abstract name(p : T, q) : R`.
pub fn format_operation(o: &Operation) -> String {
    let mut s = format!("{} ", o.visibility.symbol());
    if o.is_static {
        s.push_str("static ");
    }
    if o.is_abstract {
        s.push_str("abstract ");
    }
    s.push_str(&o.name);
    s.push('(');
    let params: Vec<String> = o
        .params
        .iter()
        .map(|p| match &p.type_name {
            Some(t) => format!("{} : {t}", p.name),
            None => p.name.clone(),
        })
        .collect();
    s.push_str(&params.join(", "));
    s.push(')');
    if let Some(r) = &o.return_type {
        s.push_str(" : ");
        s.push_str(r);
    }
    s
}

/// The text of a literal: its name.
pub fn format_literal(l: &Literal) -> String {
    l.name.clone()
}
