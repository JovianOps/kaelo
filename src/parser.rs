use crate::ast::*;
use crate::token::{Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    no_struct_lit: bool,
}

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
    pub line: u32,
    pub col: u32,
}

type PResult<T> = Result<T, ParseError>;

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser { toks, pos: 0, no_struct_lit: false }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn at(&self, n: usize) -> &Tok {
        self.toks.get(self.pos + n).map(|t| &t.tok).unwrap_or(&Tok::Eof)
    }
    fn line(&self) -> u32 {
        self.toks[self.pos].line
    }
    fn col(&self) -> u32 {
        self.toks[self.pos].col
    }
    fn err<T>(&self, msg: impl Into<String>) -> PResult<T> {
        Err(ParseError { msg: msg.into(), line: self.line(), col: self.col() })
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) -> PResult<()> {
        if self.eat(&t) {
            Ok(())
        } else {
            self.err(format!("expected {:?}, found {:?}", t, self.peek()))
        }
    }
    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            other => self.err(format!("expected name, found {:?}", other)),
        }
    }

    pub fn parse_program(mut self) -> PResult<Program> {
        let mut out = Vec::new();
        while *self.peek() != Tok::Eof {
            out.push(self.statement()?);
        }
        Ok(Program(out))
    }

    fn block(&mut self) -> PResult<Vec<Stmt>> {
        self.expect(Tok::LBrace)?;
        let mut out = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return self.err("unexpected end of input, expected '}'");
            }
            out.push(self.statement()?);
        }
        self.expect(Tok::RBrace)?;
        Ok(out)
    }

    fn statement(&mut self) -> PResult<Stmt> {
        match self.peek().clone() {
            Tok::Let | Tok::Var => {
                let mutable = matches!(self.peek(), Tok::Var);
                self.bump();
                let name = self.expect_ident()?;
                let init = if self.eat(&Tok::Assign) { Some(self.expression()?) } else { None };
                self.expect(Tok::Semicolon)?;
                Ok(Stmt::Let { name, mutable, init })
            }
            Tok::Fn => {
                self.bump();
                let name = self.expect_ident()?;
                self.expect(Tok::LParen)?;
                let mut params = Vec::new();
                while *self.peek() != Tok::RParen {
                    params.push(self.expect_ident()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RParen)?;
                let body = self.block()?;
                Ok(Stmt::Fn { name, params, body })
            }
            Tok::Struct => {
                self.bump();
                let name = self.expect_ident()?;
                self.expect(Tok::LBrace)?;
                let mut fields = Vec::new();
                while *self.peek() != Tok::RBrace {
                    fields.push(self.expect_ident()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RBrace)?;
                Ok(Stmt::Struct { name, fields })
            }
            Tok::Return => {
                self.bump();
                let val = if *self.peek() == Tok::Semicolon { None } else { Some(self.expression()?) };
                self.expect(Tok::Semicolon)?;
                Ok(Stmt::Return(val))
            }
            Tok::If => {
                self.bump();
                let cond = self.expression()?;
                let then = self.block()?;
                let els = if self.eat(&Tok::Else) {
                    if *self.peek() == Tok::If {
                        Some(vec![self.statement()?])
                    } else {
                        Some(self.block()?)
                    }
                } else {
                    None
                };
                Ok(Stmt::If { cond, then, els })
            }
            Tok::While => {
                self.bump();
                // `while x < n {` must not read the body brace as a struct literal
                let saved = self.no_struct_lit;
                self.no_struct_lit = true;
                let cond = self.expression();
                self.no_struct_lit = saved;
                let cond = cond?;
                let body = self.block()?;
                Ok(Stmt::While { cond, body })
            }
            Tok::For => {
                self.bump();
                let name = self.expect_ident()?;
                self.expect(Tok::In)?;
                // `for x in xs {` must not read the loop body brace as a struct literal
                let saved = self.no_struct_lit;
                self.no_struct_lit = true;
                let iter = self.expression();
                self.no_struct_lit = saved;
                let iter = iter?;
                let body = self.block()?;
                Ok(Stmt::ForIn { name, iter, body })
            }
            Tok::Break => {
                self.bump();
                self.expect(Tok::Semicolon)?;
                Ok(Stmt::Break)
            }
            Tok::Continue => {
                self.bump();
                self.expect(Tok::Semicolon)?;
                Ok(Stmt::Continue)
            }
            _ => {
                // compound append: name += expr;
                if let Tok::Ident(name) = self.peek().clone() {
                    if *self.at(1) == Tok::PlusEq {
                        self.bump();
                        self.bump();
                        let value = self.expression()?;
                        self.expect(Tok::Semicolon)?;
                        return Ok(Stmt::Append { name, value });
                    }
                    if *self.at(1) == Tok::Assign {
                        self.bump();
                        self.bump();
                        let val = self.expression()?;
                        self.expect(Tok::Semicolon)?;
                        return Ok(Stmt::Expr(Expr::Assign(name, Box::new(val))));
                    }
                }
                let expr = self.expression()?;
                self.expect(Tok::Semicolon)?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    pub fn expression(&mut self) -> PResult<Expr> {
        self.logic_or()
    }

    fn logic_or(&mut self) -> PResult<Expr> {
        let mut left = self.logic_and()?;
        while *self.peek() == Tok::Or {
            self.bump();
            let right = self.logic_and()?;
            left = Expr::Logical(LogicOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn logic_and(&mut self) -> PResult<Expr> {
        let mut left = self.equality()?;
        while *self.peek() == Tok::And {
            self.bump();
            let right = self.equality()?;
            left = Expr::Logical(LogicOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn equality(&mut self) -> PResult<Expr> {
        let mut left = self.comparison()?;
        loop {
            let op = match self.peek() {
                Tok::EqEq => BinOp::Eq,
                Tok::NotEq => BinOp::NotEq,
                _ => break,
            };
            self.bump();
            let right = self.comparison()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Tok::Lt => BinOp::Lt,
                Tok::LtEq => BinOp::LtEq,
                Tok::Gt => BinOp::Gt,
                Tok::GtEq => BinOp::GtEq,
                _ => break,
            };
            self.bump();
            let right = self.term()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> PResult<Expr> {
        let mut left = self.factor()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.factor()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn factor(&mut self) -> PResult<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<Expr> {
        match self.peek() {
            Tok::Minus => {
                self.bump();
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.unary()?)))
            }
            Tok::Not => {
                self.bump();
                Ok(Expr::Unary(UnOp::Not, Box::new(self.unary()?)))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            match self.peek() {
                Tok::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while *self.peek() != Tok::RParen {
                        args.push(self.expression()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen)?;
                    e = Expr::Call(Box::new(e), args);
                }
                Tok::Dot => {
                    self.bump();
                    let name = self.expect_ident()?;
                    if *self.peek() == Tok::Assign {
                        self.bump();
                        let val = self.expression()?;
                        e = Expr::FieldAssign(Box::new(e), name, Box::new(val));
                        return Ok(e);
                    }
                    e = Expr::Field(Box::new(e), name);
                }
                Tok::LBracket => {
                    self.bump();
                    let idx = self.expression()?;
                    self.expect(Tok::RBracket)?;
                    if *self.peek() == Tok::Assign {
                        self.bump();
                        let val = self.expression()?;
                        e = Expr::IndexAssign(Box::new(e), Box::new(idx), Box::new(val));
                        return Ok(e);
                    }
                    e = Expr::Index(Box::new(e), Box::new(idx));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn primary(&mut self) -> PResult<Expr> {
        let line = self.line();
        let col = self.col();
        match self.peek().clone() {
            Tok::Int(i) => {
                self.bump();
                Ok(Expr::Int(i))
            }
            Tok::Float(f) => {
                self.bump();
                Ok(Expr::Float(f))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            Tok::True => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            Tok::False => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            Tok::None_ => {
                self.bump();
                Ok(Expr::NoneLit)
            }
            Tok::Ident(name) => {
                self.bump();
                if *self.peek() == Tok::LBrace && !self.no_struct_lit {
                    // struct literal Name { a: 1, b: 2 }
                    self.bump();
                    let mut fields = Vec::new();
                    while *self.peek() != Tok::RBrace {
                        let f = self.expect_ident()?;
                        self.expect(Tok::Colon)?;
                        let v = self.expression()?;
                        fields.push((f, v));
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RBrace)?;
                    return Ok(Expr::StructInit(name, fields));
                }
                Ok(Expr::Var(name))
            }
            Tok::LParen => {
                self.bump();
                let e = self.expression()?;
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Tok::LBracket => {
                self.bump();
                let mut items = Vec::new();
                while *self.peek() != Tok::RBracket {
                    items.push(self.expression()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RBracket)?;
                Ok(Expr::Array(items))
            }
            other => self.err(format!("unexpected token {:?} at {}:{}", other, line, col)),
        }
    }
}

pub fn parse(src: &str) -> Result<Program, String> {
    let toks = crate::lexer::Lexer::new(src)
        .tokenize()
        .map_err(|e| format!("lex error at {}:{}: {}", e.line, e.col, e.msg))?;
    Parser::new(toks).parse_program().map_err(|e| format!("parse error at {}:{}: {}", e.line, e.col, e.msg))
}