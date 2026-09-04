use std::collections::HashSet;

use crate::bail;
use crate::error::Result;

#[derive(Clone)]
enum Node {
    Char(char),
    Any,
    Class(Vec<(char, char)>, bool),
    Start,
    End,
    Group(Vec<Node>),
    Alt(Vec<Vec<Node>>),
    Star(Box<Node>),
    Plus(Box<Node>),
    Opt(Box<Node>),
}

pub struct Regex {
    pattern: Vec<Node>,
}

struct Parser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn parse_pattern(&mut self) -> Result<Vec<Node>> {
        let mut branches = vec![self.parse_concat()?];
        while self.peek() == Some('|') {
            self.next();
            branches.push(self.parse_concat()?);
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            Ok(vec![Node::Alt(branches)])
        }
    }

    fn parse_concat(&mut self) -> Result<Vec<Node>> {
        let mut nodes = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            nodes.push(self.parse_repeat()?);
        }
        Ok(nodes)
    }

    fn parse_repeat(&mut self) -> Result<Node> {
        let atom = self.parse_atom()?;
        match self.peek() {
            Some('*') => {
                self.next();
                Ok(Node::Star(Box::new(atom)))
            }
            Some('+') => {
                self.next();
                Ok(Node::Plus(Box::new(atom)))
            }
            Some('?') => {
                self.next();
                Ok(Node::Opt(Box::new(atom)))
            }
            _ => Ok(atom),
        }
    }

    fn parse_atom(&mut self) -> Result<Node> {
        match self.next() {
            Some('(') => {
                let inner = self.parse_pattern()?;
                if self.next() != Some(')') {
                    bail!("regex: unclosed group");
                }
                Ok(Node::Group(inner))
            }
            Some('[') => self.parse_class(),
            Some('.') => Ok(Node::Any),
            Some('^') => Ok(Node::Start),
            Some('$') => Ok(Node::End),
            Some('\\') => {
                let c = self
                    .next()
                    .ok_or_else(|| crate::error::Error::new("regex: trailing backslash"))?;
                Ok(escape_node(c))
            }
            Some(c) => Ok(Node::Char(c)),
            None => bail!("regex: unexpected end of pattern"),
        }
    }

    fn parse_class(&mut self) -> Result<Node> {
        let negate = if self.peek() == Some('^') {
            self.next();
            true
        } else {
            false
        };
        let mut ranges = Vec::new();
        let mut first = true;
        loop {
            match self.peek() {
                None => bail!("regex: unclosed character class"),
                Some(']') if !first => {
                    self.next();
                    break;
                }
                _ => {
                    first = false;
                    let lo = self.parse_class_char()?;
                    if self.peek() == Some('-') && self.chars.get(self.pos + 1) != Some(&']') {
                        self.next();
                        let hi = self.parse_class_char()?;
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
            }
        }
        Ok(Node::Class(ranges, negate))
    }

    fn parse_class_char(&mut self) -> Result<char> {
        match self.next() {
            Some('\\') => self
                .next()
                .map(unescape_char)
                .ok_or_else(|| crate::error::Error::new("regex: trailing backslash in class")),
            Some(c) => Ok(c),
            None => bail!("regex: unclosed character class"),
        }
    }
}

fn unescape_char(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        other => other,
    }
}

fn escape_node(c: char) -> Node {
    match c {
        'd' => Node::Class(vec![('0', '9')], false),
        'D' => Node::Class(vec![('0', '9')], true),
        'w' => Node::Class(vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')], false),
        'W' => Node::Class(vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')], true),
        's' => Node::Class(
            vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
            false,
        ),
        'S' => Node::Class(
            vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
            true,
        ),
        other => Node::Char(unescape_char(other)),
    }
}

fn class_matches(ranges: &[(char, char)], negate: bool, c: char) -> bool {
    let hit = ranges.iter().any(|&(lo, hi)| c >= lo && c <= hi);
    hit != negate
}

fn node_positions(node: &Node, text: &[char], pos: usize) -> Vec<usize> {
    match node {
        Node::Char(c) => {
            if text.get(pos) == Some(c) {
                vec![pos + 1]
            } else {
                vec![]
            }
        }
        Node::Any => {
            if pos < text.len() {
                vec![pos + 1]
            } else {
                vec![]
            }
        }
        Node::Class(ranges, negate) => {
            if pos < text.len() && class_matches(ranges, *negate, text[pos]) {
                vec![pos + 1]
            } else {
                vec![]
            }
        }
        Node::Start => {
            if pos == 0 {
                vec![pos]
            } else {
                vec![]
            }
        }
        Node::End => {
            if pos == text.len() {
                vec![pos]
            } else {
                vec![]
            }
        }
        Node::Group(inner) => seq_positions(inner, text, pos),
        Node::Alt(branches) => {
            let mut set = HashSet::new();
            for branch in branches {
                for p in seq_positions(branch, text, pos) {
                    set.insert(p);
                }
            }
            set.into_iter().collect()
        }
        Node::Opt(inner) => {
            let mut set: HashSet<usize> = HashSet::new();
            set.insert(pos);
            for p in node_positions(inner, text, pos) {
                set.insert(p);
            }
            set.into_iter().collect()
        }
        Node::Star(inner) => star_positions(inner, text, pos),
        Node::Plus(inner) => {
            let mut set = HashSet::new();
            for p in node_positions(inner, text, pos) {
                for q in star_positions(inner, text, p) {
                    set.insert(q);
                }
            }
            set.into_iter().collect()
        }
    }
}

fn star_positions(inner: &Node, text: &[char], pos: usize) -> Vec<usize> {
    let mut reached: HashSet<usize> = HashSet::new();
    let mut frontier = vec![pos];
    reached.insert(pos);
    while let Some(p) = frontier.pop() {
        for q in node_positions(inner, text, p) {
            if q != p && reached.insert(q) {
                frontier.push(q);
            }
        }
    }
    reached.into_iter().collect()
}

fn seq_positions(nodes: &[Node], text: &[char], pos: usize) -> Vec<usize> {
    let mut positions: HashSet<usize> = HashSet::new();
    positions.insert(pos);
    for node in nodes {
        let mut next: HashSet<usize> = HashSet::new();
        for &p in &positions {
            for q in node_positions(node, text, p) {
                next.insert(q);
            }
        }
        positions = next;
        if positions.is_empty() {
            break;
        }
    }
    positions.into_iter().collect()
}

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut parser = Parser {
            chars: &chars,
            pos: 0,
        };
        let nodes = parser.parse_pattern()?;
        if parser.pos != chars.len() {
            bail!("regex: unexpected ')' in pattern");
        }
        Ok(Regex { pattern: nodes })
    }

    pub fn is_match(&self, haystack: &str) -> bool {
        let text: Vec<char> = haystack.chars().collect();
        for start in 0..=text.len() {
            if !seq_positions(&self.pattern, &text, start).is_empty() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_anchor() {
        let re = Regex::new("^abc$").unwrap();
        assert!(re.is_match("abc"));
        assert!(!re.is_match("xabc"));
        assert!(!re.is_match("abcx"));
    }

    #[test]
    fn extension_filter() {
        let re = Regex::new(r"\.txt$").unwrap();
        assert!(re.is_match("dir/file.txt"));
        assert!(!re.is_match("dir/file.bin"));
    }

    #[test]
    fn star_and_class() {
        let re = Regex::new(r"^maps/[a-z0-9_]+\.bsp$").unwrap();
        assert!(re.is_match("maps/de_dust2.bsp"));
        assert!(!re.is_match("maps/DE_DUST2.bsp"));
        assert!(!re.is_match("sounds/de_dust2.bsp"));
    }

    #[test]
    fn alternation_and_group() {
        let re = Regex::new(r"^(foo|bar)baz$").unwrap();
        assert!(re.is_match("foobaz"));
        assert!(re.is_match("barbaz"));
        assert!(!re.is_match("bazbaz"));
    }
}
