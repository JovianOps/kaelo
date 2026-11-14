#[derive(Debug, Clone)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    NoneLit,
    Var(String),
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Logical(LogicOp, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    StructInit(String, Vec<(String, Expr)>),
    Assign(String, Box<Expr>),
    FieldAssign(Box<Expr>, String, Box<Expr>),
    IndexAssign(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, mutable: bool, init: Option<Expr> },
    Fn { name: String, params: Vec<String>, body: Vec<Stmt> },
    Struct { name: String, fields: Vec<String> },
    Return(Option<Expr>),
    If { cond: Expr, then: Vec<Stmt>, els: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    ForIn { name: String, iter: Expr, body: Vec<Stmt> },
    Break,
    Continue,
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Program(pub Vec<Stmt>);