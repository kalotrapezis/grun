//! A tiny calculator provider. If the query looks like arithmetic, it shows the
//! result; pressing Enter copies it to the clipboard.
//!
//! Supports + - * / % parentheses and unary minus, with normal precedence.

use super::Provider;
use crate::matching::{Action, Match};

pub struct CalcProvider;

impl Provider for CalcProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let s = input.trim();
        // Only engage if it looks like math: has a digit and only math chars.
        if !s.chars().any(|c| c.is_ascii_digit()) {
            return Vec::new();
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_digit() || "+-*/(). %".contains(c))
        {
            return Vec::new();
        }
        match eval(s) {
            Some(result) if result.is_finite() => {
                let value = format_number(result);
                vec![Match::new(
                    value.clone(),
                    "Press Enter to copy".to_string(),
                    Some("accessories-calculator".to_string()),
                    2.0,
                    "Calculator",
                    Action::Copy(value),
                )]
            }
            _ => Vec::new(),
        }
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{:.6}", n);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[derive(Clone, PartialEq)]
enum Tok {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' => i += 1,
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '%' => {
                toks.push(Tok::Percent);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num: String = chars[start..i].iter().collect();
                toks.push(Tok::Num(num.parse().ok()?));
            }
            _ => return None,
        }
    }
    Some(toks)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(t) = self.peek() {
            match t {
                Tok::Plus => {
                    self.pos += 1;
                    v += self.term()?;
                }
                Tok::Minus => {
                    self.pos += 1;
                    v -= self.term()?;
                }
                _ => break,
            }
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        while let Some(t) = self.peek() {
            match t {
                Tok::Star => {
                    self.pos += 1;
                    v *= self.factor()?;
                }
                Tok::Slash => {
                    self.pos += 1;
                    v /= self.factor()?;
                }
                Tok::Percent => {
                    self.pos += 1;
                    v %= self.factor()?;
                }
                _ => break,
            }
        }
        Some(v)
    }

    fn factor(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(n) => Some(n),
            Tok::Minus => Some(-self.factor()?),
            Tok::Plus => self.factor(),
            Tok::LParen => {
                let v = self.expr()?;
                match self.next()? {
                    Tok::RParen => Some(v),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn eval(s: &str) -> Option<f64> {
    let tokens = tokenize(s)?;
    if tokens.is_empty() {
        return None;
    }
    let mut p = Parser { tokens, pos: 0 };
    let v = p.expr()?;
    // Reject trailing garbage like "1 2".
    if p.pos == p.tokens.len() {
        Some(v)
    } else {
        None
    }
}
