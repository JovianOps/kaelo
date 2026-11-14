use crate::token::{Tok, Token};

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

#[derive(Debug)]
pub struct LexError {
    pub msg: String,
    pub line: u32,
    pub col: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0, line: 1, col: 1 }
    }

    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }
    fn peek_at(&self, n: usize) -> u8 {
        *self.src.get(self.pos + n).unwrap_or(&0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token()?;
            let end = t.tok == Tok::Eof;
            out.push(t);
            if end {
                return Ok(out);
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        loop {
            let c = self.peek();
            if c == 0 {
                return Ok(Token { tok: Tok::Eof, line: self.line, col: self.col });
            }
            if c.is_ascii_whitespace() {
                self.bump();
                continue;
            }
            if c == b'#' {
                while self.peek() != b'\n' && self.peek() != 0 {
                    self.bump();
                }
                continue;
            }
            if c == b'/' && self.peek_at(1) == b'/' {
                while self.peek() != b'\n' && self.peek() != 0 {
                    self.bump();
                }
                continue;
            }
            break;
        }

        let line = self.line;
        let col = self.col;
        let c = self.peek();

        // identifiers / keywords
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = self.pos;
            while {
                let d = self.peek();
                d.is_ascii_alphanumeric() || d == b'_'
            } {
                self.bump();
            }
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
            let tok = match s.as_str() {
                "let" => Tok::Let,
                "var" => Tok::Var,
                "fn" => Tok::Fn,
                "if" => Tok::If,
                "else" => Tok::Else,
                "while" => Tok::While,
                "for" => Tok::For,
                "in" => Tok::In,
                "break" => Tok::Break,
                "continue" => Tok::Continue,
                "return" => Tok::Return,
                "true" => Tok::True,
                "false" => Tok::False,
                "none" => Tok::None_,
                "struct" => Tok::Struct,
                _ => Tok::Ident(s),
            };
            return Ok(Token { tok, line, col });
        }

        // numbers
        if c.is_ascii_digit() {
            let start = self.pos;
            let mut is_float = false;
            while self.peek().is_ascii_digit() {
                self.bump();
            }
            if self.peek() == b'.' && self.peek_at(1).is_ascii_digit() {
                is_float = true;
                self.bump();
                while self.peek().is_ascii_digit() {
                    self.bump();
                }
            }
            let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
            let tok = if is_float {
                Tok::Float(text.parse().map_err(|_| LexError { msg: format!("bad float {}", text), line, col })?)
            } else {
                Tok::Int(text.parse().map_err(|_| LexError { msg: format!("bad int {}", text), line, col })?)
            };
            return Ok(Token { tok, line, col });
        }

        // strings
        if c == b'"' {
            self.bump();
            let mut s = String::new();
            loop {
                let d = self.peek();
                if d == 0 || d == b'\n' {
                    return Err(LexError { msg: "unterminated string".into(), line, col });
                }
                if d == b'"' {
                    self.bump();
                    break;
                }
                if d == b'\\' {
                    self.bump();
                    let e = self.bump();
                    s.push(match e {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'0' => '\0',
                        other => other as char,
                    });
                    continue;
                }
                s.push(d as char);
                self.bump();
            }
            return Ok(Token { tok: Tok::Str(s), line, col });
        }

        // operators
        macro_rules! two {
            ($ch:expr, $one:ident, $two_:ident) => {{
                self.bump();
                if self.peek() == $ch {
                    self.bump();
                    Tok::$two_
                } else {
                    Tok::$one
                }
            }};
        }

        let tok = match c {
            b'+' => {
                self.bump();
                if self.peek() == b'=' { self.bump(); Tok::PlusEq } else { Tok::Plus }
            }
            b'-' => {
                self.bump();
                if self.peek() == b'>' { self.bump(); Tok::Arrow } else { Tok::Minus }
            }
            b'*' => { self.bump(); Tok::Star }
            b'/' => { self.bump(); Tok::Slash }
            b'%' => { self.bump(); Tok::Percent }
            b'=' => {
                self.bump();
                if self.peek() == b'>' { self.bump(); Tok::FatArrow }
                else if self.peek() == b'=' { self.bump(); Tok::EqEq }
                else { Tok::Assign }
            }
            b'!' => two!(b'=', Not, NotEq),
            b'<' => two!(b'=', Lt, LtEq),
            b'>' => two!(b'=', Gt, GtEq),
            b'&' => { self.bump(); if self.peek() == b'&' { self.bump(); } Tok::And }
            b'|' => { self.bump(); if self.peek() == b'|' { self.bump(); } Tok::Or }
            b',' => { self.bump(); Tok::Comma }
            b';' => { self.bump(); Tok::Semicolon }
            b':' => { self.bump(); Tok::Colon }
            b'.' => {
                self.bump();
                if self.peek() == b'.' { self.bump(); Tok::Dot2 } else { Tok::Dot }
            }
            b'(' => { self.bump(); Tok::LParen }
            b')' => { self.bump(); Tok::RParen }
            b'{' => { self.bump(); Tok::LBrace }
            b'}' => { self.bump(); Tok::RBrace }
            b'[' => { self.bump(); Tok::LBracket }
            b']' => { self.bump(); Tok::RBracket }
            other => {
                return Err(LexError { msg: format!("unexpected character '{}'", other as char), line, col })
            }
        };
        Ok(Token { tok, line, col })
    }
}