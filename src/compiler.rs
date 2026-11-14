use crate::ast::*;
use crate::vm::{Chunk, Op, Value};
use std::collections::HashMap;
use std::rc::Rc;

pub struct FnDef {
    pub const_idx: usize,
    pub name: String,
    pub params: Vec<String>,
    pub chunk: Chunk,
}

pub struct Compiled {
    pub chunk: Chunk,
    pub structs: HashMap<String, Vec<String>>,
    pub fns: Vec<FnDef>,
}

struct FnCtx {
    code: Vec<Op>,
    consts: Vec<Value>,
    locals: Vec<(String, bool)>,
    scopes: Vec<usize>,
    n_params: usize,
    break_stack: Vec<Vec<usize>>,
    continue_stack: Vec<Vec<usize>>,
    freeze_arrays: bool,
}

impl FnCtx {
    fn new(n_params: usize) -> Self {
        FnCtx {
            code: Vec::new(),
            consts: Vec::new(),
            locals: Vec::new(),
            scopes: vec![0],
            n_params,
            break_stack: Vec::new(),
            continue_stack: Vec::new(),
            freeze_arrays: false,
        }
    }

    fn const_idx(&mut self, v: Value) -> u32 {
        self.consts.push(v);
        (self.consts.len() - 1) as u32
    }

    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }

    fn patch_jump(&mut self, at: usize) {
        let here = self.code.len() as i32;
        let target = at as i32 + 1;
        match self.code[at] {
            Op::Jump(_) => self.code[at] = Op::Jump(here - target),
            Op::JumpIfFalse(_) => self.code[at] = Op::JumpIfFalse(here - target),
            Op::Loop(_) => self.code[at] = Op::Loop(here - target),
            _ => {}
        }
    }

    fn patch_loop(&mut self, at: usize, body_start: usize) {
        let here = self.code.len() as i32;
        let target = body_start as i32;
        self.code[at] = Op::Loop(target - here);
    }

    fn resolve_local(&self, name: &str) -> Option<u32> {
        self.locals.iter().rposition(|(n, _)| n == name).map(|i| i as u32)
    }

    fn local_is_mutable(&self, name: &str) -> bool {
        self.locals.iter().rposition(|(n, _)| n == name).map(|i| self.locals[i].1).unwrap_or(false)
    }

    fn declare_local(&mut self, name: &str, mutable: bool) -> u32 {
        if let Some(slot) = self.resolve_local(name) {
            return slot;
        }
        self.locals.push((name.to_string(), mutable));
        (self.locals.len() - 1) as u32
    }

    fn begin_scope(&mut self) {
        self.scopes.push(self.locals.len());
    }
    fn end_scope(&mut self) {
        if let Some(n) = self.scopes.pop() {
            self.locals.truncate(n);
        }
    }
}

pub struct Compiler {
    names: Vec<Rc<str>>,
    name_ids: HashMap<String, u32>,
    structs: HashMap<String, Vec<String>>,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler { names: Vec::new(), name_ids: HashMap::new(), structs: HashMap::new() }
    }

    fn name_id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.name_ids.get(name) {
            return *id;
        }
        let id = self.names.len() as u32;
        self.names.push(Rc::from(name));
        self.name_ids.insert(name.to_string(), id);
        id
    }

    pub fn compile(&mut self, prog: &Program) -> Result<Compiled, String> {
        // struct declarations are known before any code runs
        for stmt in &prog.0 {
            if let Stmt::Struct { name, fields } = stmt {
                self.structs.insert(name.clone(), fields.clone());
            }
        }

        let mut ctx = FnCtx::new(0);
        // top-level `fn` bodies are compiled after the main pass so every chunk
        // shares the final name table
        let mut pending: Vec<(usize, String, Vec<String>, Chunk)> = Vec::new();

        let last = prog.0.len().saturating_sub(1);
        for (idx, stmt) in prog.0.iter().enumerate() {
            if idx == last {
                if let Stmt::Expr(e) = stmt {
                    // the last expression is the program's value (REPL-style)
                    self.expr(e, &mut ctx)?;
                    continue;
                }
            }
            if let Stmt::Fn { name, params, body } = stmt {
                let vidx = ctx.const_idx(Value::None) as usize;
                ctx.emit(Op::Const(vidx as u32));
                let gid = self.name_id(name);
                ctx.emit(Op::DefGlobal(gid));
                let mut sub = FnCtx::new(params.len());
                for p in params {
                    sub.declare_local(p, true);
                }
                for st in body {
                    self.stmt(st, &mut sub, false)?;
                }
                sub.emit(Op::None);
                sub.emit(Op::Return);
                let chunk = Chunk {
                    code: sub.code,
                    consts: sub.consts,
                    names: self.names.clone(),
                    n_locals: sub.locals.len() as u32,
                    n_params: params.len() as u32,
                };
                pending.push((vidx, name.clone(), params.clone(), chunk));
                continue;
            }
            self.stmt(stmt, &mut ctx, true)?;
        }
        // Return yields whatever the last expression left, or none
        ctx.emit(Op::Return);

        let mut fns = Vec::new();
        for (vidx, name, params, chunk) in pending {
            fns.push(FnDef { const_idx: vidx, name, params, chunk });
        }

        let chunk = Chunk {
            code: ctx.code,
            consts: ctx.consts,
            names: self.names.clone(),
            n_locals: ctx.locals.len() as u32,
            n_params: 0,
        };

        Ok(Compiled { chunk, structs: self.structs.clone(), fns })
    }

    fn stmt(&mut self, s: &Stmt, ctx: &mut FnCtx, top: bool) -> Result<(), String> {
        match s {
            Stmt::Let { name, mutable, init } => {
                if top {
                    // top-level bindings are globals: functions can see them
                    let gid = self.name_id(name);
                    let prev = ctx.freeze_arrays;
                    if !mutable {
                        ctx.freeze_arrays = true;
                    }
                    match init {
                        Some(e) => self.expr(e, ctx)?,
                        None => {
                            ctx.emit(Op::None);
                        }
                    }
                    ctx.freeze_arrays = prev;
                    ctx.emit(if *mutable { Op::DefGlobal(gid) } else { Op::StoreGlobalConst(gid) });
                    return Ok(());
                }
                let slot = ctx.declare_local(name, *mutable);
                match init {
                    Some(e) => {
                        let prev = ctx.freeze_arrays;
                        if !mutable {
                            ctx.freeze_arrays = true;
                        }
                        self.expr(e, ctx)?;
                        ctx.freeze_arrays = prev;
                        ctx.emit(Op::StoreLocal(slot));
                    }
                    None => {
                        ctx.emit(Op::None);
                        ctx.emit(Op::StoreLocal(slot));
                    }
                }
                Ok(())
            }
            Stmt::Struct { .. } => Ok(()), // collected up front
            Stmt::Fn { .. } => Err("nested fn declarations are not supported".to_string()),
            Stmt::Return(e) => {
                match e {
                    Some(x) => self.expr(x, ctx)?,
                    None => {
                        ctx.emit(Op::None);
                    }
                }
                ctx.emit(Op::Return);
                Ok(())
            }
            Stmt::If { cond, then, els } => {
                self.expr(cond, ctx)?;
                let jf = ctx.emit(Op::JumpIfFalse(0));
                ctx.begin_scope();
                for st in then {
                    self.stmt(st, ctx, false)?;
                }
                ctx.end_scope();
                match els {
                    Some(els) => {
                        let j = ctx.emit(Op::Jump(0));
                        ctx.patch_jump(jf);
                        ctx.begin_scope();
                        for st in els {
                            self.stmt(st, ctx, false)?;
                        }
                        ctx.end_scope();
                        ctx.patch_jump(j);
                    }
                    None => ctx.patch_jump(jf),
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                let start = ctx.code.len();
                self.expr(cond, ctx)?;
                let jf = ctx.emit(Op::JumpIfFalse(0));
                ctx.begin_scope();
                ctx.break_stack.push(Vec::new());
                ctx.continue_stack.push(Vec::new());
                for st in body {
                    self.stmt(st, ctx, false)?;
                }
                ctx.end_scope();
                let end = ctx.code.len();
                ctx.emit(Op::Loop(0));
                ctx.patch_loop(ctx.code.len() - 1, start);
                ctx.patch_jump(jf);
                // break jumps land after the loop; continue jumps go back to the top
                for at in ctx.break_stack.pop().unwrap() {
                    ctx.code[at] = Op::Jump((end as i32) - (at as i32 + 1));
                }
                for at in ctx.continue_stack.pop().unwrap() {
                    ctx.code[at] = Op::Jump((start as i32) - (at as i32 + 1));
                }
                Ok(())
            }
            Stmt::ForIn { name, iter, body } => {
                // desugar: for x in it { body }  ->
                //   tmp_arr = it; tmp_i = 0;
                //   loop: if tmp_i >= len(tmp_arr) break;
                //         x = tmp_arr[tmp_i]; body; tmp_i = tmp_i + 1; loop
                let arr_slot = ctx.declare_local("@arr", true);
                let i_slot = ctx.declare_local("@i", true);
                let x_slot = ctx.declare_local(name, false);

                self.expr(iter, ctx)?;
                ctx.emit(Op::StoreLocal(arr_slot));
                let zero = ctx.const_idx(Value::Int(0));
                ctx.emit(Op::Const(zero));
                ctx.emit(Op::StoreLocal(i_slot));

                let start = ctx.code.len();
                ctx.emit(Op::LoadLocal(i_slot));
                ctx.emit(Op::LoadLocal(arr_slot));
                ctx.emit(Op::Len);
                ctx.emit(Op::Lt);
                let jf = ctx.emit(Op::JumpIfFalse(0));

                ctx.emit(Op::LoadLocal(arr_slot));
                ctx.emit(Op::LoadLocal(i_slot));
                ctx.emit(Op::Index);
                ctx.emit(Op::StoreLocal(x_slot));

                ctx.begin_scope();
                ctx.break_stack.push(Vec::new());
                ctx.continue_stack.push(Vec::new());
                for st in body {
                    self.stmt(st, ctx, false)?;
                }
                ctx.end_scope();

                let inc = ctx.code.len();
                ctx.emit(Op::LoadLocal(i_slot));
                let one = ctx.const_idx(Value::Int(1));
                ctx.emit(Op::Const(one));
                ctx.emit(Op::Add);
                ctx.emit(Op::StoreLocal(i_slot));
                ctx.emit(Op::Loop(0));
                ctx.patch_loop(ctx.code.len() - 1, start);
                ctx.patch_jump(jf);

                let end = ctx.code.len();
                for at in ctx.break_stack.pop().unwrap() {
                    ctx.code[at] = Op::Jump((end as i32) - (at as i32 + 1));
                }
                for at in ctx.continue_stack.pop().unwrap() {
                    ctx.code[at] = Op::Jump((inc as i32) - (at as i32 + 1));
                }
                Ok(())
            }
            Stmt::Append { name, value } => {
                if let Some(slot) = ctx.resolve_local(name) {
                    if !ctx.local_is_mutable(name) {
                        return Err(format!("cannot assign to '{}': declared with let (use var)", name));
                    }
                    self.expr(value, ctx)?;
                    ctx.emit(Op::AppendLocal(slot));
                    ctx.emit(Op::Pop); // statement form: drop the result
                } else {
                    let gid = self.name_id(name);
                    self.expr(value, ctx)?;
                    ctx.emit(Op::AppendGlobal(gid));
                    ctx.emit(Op::Pop); // statement form: drop the result
                }
                Ok(())
            }
            Stmt::Break => {
                let at = ctx.emit(Op::Jump(0));
                if let Some(list) = ctx.break_stack.last_mut() {
                    list.push(at);
                }
                Ok(())
            }
            Stmt::Continue => {
                let at = ctx.emit(Op::Jump(0));
                if let Some(list) = ctx.continue_stack.last_mut() {
                    list.push(at);
                }
                Ok(())
            }
            Stmt::Expr(e) => {
                self.expr(e, ctx)?;
                ctx.emit(Op::Pop);
                Ok(())
            }
        }
    }

    fn expr(&mut self, e: &Expr, ctx: &mut FnCtx) -> Result<(), String> {
        match e {
            Expr::Int(i) => {
                let idx = ctx.const_idx(Value::Int(*i));
                ctx.emit(Op::Const(idx));
            }
            Expr::Float(f) => {
                let idx = ctx.const_idx(Value::Float(*f));
                ctx.emit(Op::Const(idx));
            }
            Expr::Bool(b) => {
                ctx.emit(if *b { Op::True } else { Op::False });
            }
            Expr::Str(s) => {
                let idx = ctx.const_idx(Value::Str(Rc::new(s.clone())));
                ctx.emit(Op::Const(idx));
            }
            Expr::NoneLit => {
                ctx.emit(Op::None);
            }
            Expr::Var(name) => match ctx.resolve_local(name) {
                Some(slot) => {
                    ctx.emit(Op::LoadLocal(slot));
                }
                None => {
                    let gid = self.name_id(name);
                    ctx.emit(Op::LoadGlobal(gid));
                }
            },
            Expr::Assign(name, val) => {
                // assignment is an expression: store, then re-push the value
                if let Some(slot) = ctx.resolve_local(name) {
                    if !ctx.local_is_mutable(name) {
                        return Err(format!(
                            "cannot assign to '{}': declared with let (use var)",
                            name
                        ));
                    }
                    let _ = slot;
                }
                self.expr(val, ctx)?;
                match ctx.resolve_local(name) {
                    Some(slot) => {
                        ctx.emit(Op::StoreLocal(slot));
                        ctx.emit(Op::LoadLocal(slot));
                    }
                    None => {
                        let gid = self.name_id(name);
                        ctx.emit(Op::StoreGlobal(gid));
                        ctx.emit(Op::LoadGlobal(gid));
                    }
                }
            }
            Expr::Array(items) => {
                for i in items {
                    self.expr(i, ctx)?;
                }
                ctx.emit(Op::Array(items.len() as u32, ctx.freeze_arrays));
            }
            Expr::StructInit(name, fields) => {
                if !self.structs.contains_key(name) {
                    return Err(format!("unknown struct '{}'", name));
                }
                let decl = self.structs.get(name).unwrap().clone();
                let mut vals: Vec<Option<&Expr>> = vec![None; decl.len()];
                for (k, v) in fields {
                    match decl.iter().position(|f| f == k) {
                        Some(i) => vals[i] = Some(v),
                        None => return Err(format!("struct '{}' has no field '{}'", name, k)),
                    }
                }
                for v in vals {
                    match v {
                        Some(e) => self.expr(e, ctx)?,
                        None => {
                            ctx.emit(Op::None);
                        }
                    }
                }
                let nid = self.name_id(name);
                ctx.emit(Op::Struct(nid, decl.len() as u32));
            }
            Expr::Unary(op, x) => {
                self.expr(x, ctx)?;
                ctx.emit(match op {
                    UnOp::Neg => Op::Neg,
                    UnOp::Not => Op::Not,
                });
            }
            Expr::Binary(op, a, b) => {
                self.expr(a, ctx)?;
                self.expr(b, ctx)?;
                ctx.emit(match op {
                    BinOp::Add => Op::Add,
                    BinOp::Sub => Op::Sub,
                    BinOp::Mul => Op::Mul,
                    BinOp::Div => Op::Div,
                    BinOp::Mod => Op::Mod,
                    BinOp::Eq => Op::Eq,
                    BinOp::NotEq => Op::Ne,
                    BinOp::Lt => Op::Lt,
                    BinOp::LtEq => Op::Le,
                    BinOp::Gt => Op::Gt,
                    BinOp::GtEq => Op::Ge,
                });
            }
            Expr::Logical(op, a, b) => {
                self.expr(a, ctx)?;
                match op {
                    LogicOp::And => {
                        let jf = ctx.emit(Op::JumpIfFalse(0));
                        self.expr(b, ctx)?;
                        ctx.patch_jump(jf);
                    }
                    LogicOp::Or => {
                        // a || b  ==  if a { true } else { b }
                        let jf = ctx.emit(Op::JumpIfFalse(0));
                        ctx.emit(Op::True);
                        let j = ctx.emit(Op::Jump(0));
                        ctx.patch_jump(jf);
                        self.expr(b, ctx)?;
                        ctx.patch_jump(j);
                    }
                }
            }
            Expr::Field(base, name) => {
                self.expr(base, ctx)?;
                let nid = self.name_id(name);
                ctx.emit(Op::Field(nid));
            }
            Expr::FieldAssign(base, name, val) => {
                self.expr(base, ctx)?;
                self.expr(val, ctx)?;
                let nid = self.name_id(name);
                ctx.emit(Op::FieldSet(nid));
            }
            Expr::Index(base, idx) => {
                self.expr(base, ctx)?;
                self.expr(idx, ctx)?;
                ctx.emit(Op::Index);
            }
            Expr::IndexAssign(base, idx, val) => {
                self.expr(base, ctx)?;
                self.expr(idx, ctx)?;
                self.expr(val, ctx)?;
                ctx.emit(Op::IndexSet);
            }
            Expr::Call(callee, args) => {
                // calling a global function directly avoids a stack slot round-trip
                if let Expr::Var(name) = &**callee {
                    if ctx.resolve_local(name).is_none() {
                        for a in args {
                            self.expr(a, ctx)?;
                        }
                        let gid = self.name_id(name);
                        ctx.emit(Op::LoadGlobal(gid));
                        ctx.emit(Op::Call(args.len() as u32));
                        return Ok(());
                    }
                }
                self.expr(callee, ctx)?;
                for a in args {
                    self.expr(a, ctx)?;
                }
                ctx.emit(Op::Call(args.len() as u32));
            }
        }
        Ok(())
    }
}
