use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::rc::Rc;

#[derive(Debug)]
pub struct RtError {
    pub msg: String,
}

impl RtError {
    pub fn new(msg: impl Into<String>) -> Self {
        RtError { msg: msg.into() }
    }
}

impl fmt::Display for RtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub type R<T> = Result<T, RtError>;

pub struct ArrayVal {
    pub items: RefCell<Vec<Value>>,
    pub frozen: bool,
}

pub struct FuncObj {
    pub name: Rc<str>,
    pub chunk: Rc<Chunk>,
    pub n_params: u32,
}

pub struct NativeFn(pub Box<dyn Fn(&mut Vm, &[Value]) -> R<Value>>);

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<str>),
    Array(Rc<ArrayVal>),
    Struct(Rc<RefCell<HashMap<String, Value>>>),
    Func(Rc<FuncObj>),
    Native(Rc<NativeFn>),
    None,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
            Value::Struct(_) => "struct",
            Value::Func(_) => "function",
            Value::Native(_) => "function",
            Value::None => "none",
        }
    }

    pub fn truthy(&self) -> R<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(RtError::new(format!(
                "expected bool in condition, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn render(&self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::None => "none".to_string(),
            Value::Array(a) => {
                let items: Vec<String> = a.items.borrow().iter().map(|v| v.render()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Struct(m) => {
                let m = m.borrow();
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                let items: Vec<String> = keys.iter().map(|k| format!("{}: {}", k, m[*k].render())).collect();
                format!("{{{}}}", items.join(", "))
            }
            other => format!("{:?}", other),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(x) => write!(f, "{}", x),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{:?}", s),
            Value::Array(a) => write!(f, "array(len {})", a.items.borrow().len()),
            Value::Struct(_) => write!(f, "struct"),
            Value::Func(fun) => write!(f, "<fn {}>", fun.name),
            Value::Native(_) => write!(f, "<native>"),
            Value::None => write!(f, "none"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Const(u32),
    Pop,
    True,
    False,
    None,
    LoadLocal(u32),
    StoreLocal(u32),
    LoadGlobal(u32),
    StoreGlobal(u32),
    StoreGlobalConst(u32),
    DefGlobal(u32),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Jump(i32),
    JumpIfFalse(i32),
    Loop(i32),
    Call(u32),
    Return,
    Array(u32, bool),
    Struct(u32, u32),
    Field(u32),
    FieldSet(u32),
    Index,
    IndexSet,
    Len,
    Dup,
}

pub struct Chunk {
    pub code: Vec<Op>,
    pub consts: Vec<Value>,
    pub names: Vec<Rc<str>>,
    pub n_locals: u32,
    pub n_params: u32,
}

pub struct Frame {
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub base: usize,
}

pub struct Vm {
    pub globals: Vec<Value>,
    global_kind: Vec<u8>, // 0 = undeclared, 1 = let, 2 = var
    global_ids: HashMap<String, u32>,
    global_names: Vec<Rc<str>>,
    frames: Vec<Frame>,
    stack: Vec<Value>,
    pub out: Rc<RefCell<dyn Write>>,
    pub steps: u64,
    rng: u64,
    pub structs: HashMap<String, Vec<String>>,
    main: Option<Rc<Chunk>>,
}

const MAX_STEPS: u64 = 200_000_000;

impl Vm {
    pub fn new() -> Self {
        let mut vm = Vm {
            globals: Vec::new(),
            global_kind: Vec::new(),
            global_ids: HashMap::new(),
            global_names: Vec::new(),
            frames: Vec::new(),
            stack: Vec::new(),
            out: Rc::new(RefCell::new(std::io::stdout())),
            steps: 0,
            rng: 0x2545F4914F6CDD1D,
            structs: HashMap::new(),
            main: None,
        };
        vm.install_builtins();
        vm
    }

    pub fn define_global(&mut self, name: &str, v: Value) -> u32 {
        let id = self.global_id(name) as usize;
        self.ensure_global(id);
        self.globals[id] = v;
        self.global_kind[id] = 2;
        id as u32
    }

    fn ensure_global(&mut self, id: usize) {
        if id >= self.globals.len() {
            self.globals.resize(id + 1, Value::None);
            self.global_kind.resize(id + 1, 0);
        }
    }

    fn global_id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.global_ids.get(name) {
            return *id;
        }
        let id = self.global_names.len() as u32;
        self.global_names.push(Rc::from(name));
        self.global_ids.insert(name.to_string(), id);
        id
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.rng = (seed as u64) | 1;
    }

    pub fn set_main(&mut self, chunk: Rc<Chunk>) {
        self.main = Some(chunk);
    }

    /// Bind the compiler's name ids to this VM's global ids (builtins are
    /// already registered, so ids have to be remapped before the chunk runs).
    pub fn link(&mut self, chunk: &mut Chunk) {
        let remap = self.remap_for(&chunk.names);
        for op in chunk.code.iter_mut() {
            match op {
                Op::LoadGlobal(id) | Op::StoreGlobal(id) | Op::StoreGlobalConst(id) | Op::DefGlobal(id) => {
                    *id = remap[*id as usize];
                }
                _ => {}
            }
        }
    }

    fn remap_for(&mut self, names: &[Rc<str>]) -> Vec<u32> {
        let mut remap = Vec::with_capacity(names.len());
        for n in names {
            let id = match self.global_ids.get(n.as_ref()) {
                Some(id) => *id,
                None => {
                    let id = self.global_names.len() as u32;
                    self.global_names.push(n.clone());
                    self.global_ids.insert(n.to_string(), id);
                    self.ensure_global(id as usize);
                    id
                }
            };
            remap.push(id);
        }
        remap
    }

    pub fn main_chunk(&self) -> Option<Rc<Chunk>> {
        self.main.clone()
    }

    fn next_rand(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let v = x.wrapping_mul(0x2545F4914F6CDD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }

    fn install_builtins(&mut self) {
        macro_rules! def {
            ($name:expr, $f:expr) => {{
                let f: Box<dyn Fn(&mut Vm, &[Value]) -> R<Value>> = Box::new($f);
                self.define_global($name, Value::Native(Rc::new(NativeFn(f))));
            }};
        }

        def!("print", |vm, args| {
            let parts: Vec<String> = args.iter().map(|v| v.render()).collect();
            let _ = writeln!(vm.out.borrow_mut(), "{}", parts.join(" "));
            Ok(Value::None)
        });

        def!("len", |_, args| match args.first() {
            Some(Value::Str(s)) => Ok(Value::Int(s.len() as i64)),
            Some(Value::Array(a)) => Ok(Value::Int(a.items.borrow().len() as i64)),
            Some(other) => Err(RtError::new(format!("len() needs string or array, got {}", other.type_name()))),
            None => Err(RtError::new("len() needs 1 argument")),
        });

        def!("push", |_, args| match (args.first(), args.get(1)) {
            (Some(Value::Array(a)), Some(v)) => {
                if a.frozen {
                    return Err(RtError::new("cannot mutate an immutable array (declared with let)"));
                }
                a.items.borrow_mut().push(v.clone());
                Ok(Value::None)
            }
            (Some(other), _) => Err(RtError::new(format!("push() needs array, got {}", other.type_name()))),
            _ => Err(RtError::new("push() needs 2 arguments")),
        });

        def!("pop", |_, args| match args.first() {
            Some(Value::Array(a)) => {
                if a.frozen {
                    return Err(RtError::new("cannot mutate an immutable array (declared with let)"));
                }
                Ok(a.items.borrow_mut().pop().unwrap_or(Value::None))
            }
            Some(other) => Err(RtError::new(format!("pop() needs array, got {}", other.type_name()))),
            None => Err(RtError::new("pop() needs 1 argument")),
        });

        def!("str", |_, args| match args.first() {
            Some(v) => Ok(Value::Str(Rc::from(v.render().as_str()))),
            None => Err(RtError::new("str() needs 1 argument")),
        });

        def!("int", |_, args| match args.first() {
            Some(Value::Int(i)) => Ok(Value::Int(*i)),
            Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
            Some(Value::Str(s)) => s.trim().parse::<i64>().map(Value::Int).map_err(|_| {
                RtError::new(format!("int() cannot parse {:?}", s))
            }),
            Some(other) => Err(RtError::new(format!("int() needs number or string, got {}", other.type_name()))),
            None => Err(RtError::new("int() needs 1 argument")),
        });

        def!("float", |_, args| match args.first() {
            Some(Value::Float(f)) => Ok(Value::Float(*f)),
            Some(Value::Int(i)) => Ok(Value::Float(*i as f64)),
            Some(Value::Str(s)) => s.trim().parse::<f64>().map(Value::Float).map_err(|_| {
                RtError::new(format!("float() cannot parse {:?}", s))
            }),
            Some(other) => Err(RtError::new(format!("float() needs number or string, got {}", other.type_name()))),
            None => Err(RtError::new("float() needs 1 argument")),
        });

        macro_rules! math1 {
            ($name:expr, $f:expr) => {
                def!($name, move |_, args| match args.first() {
                    Some(Value::Int(i)) => Ok(Value::Float($f(*i as f64))),
                    Some(Value::Float(x)) => Ok(Value::Float($f(*x))),
                    Some(other) => Err(RtError::new(format!("{}() needs number, got {}", $name, other.type_name()))),
                    None => Err(RtError::new(format!("{}() needs 1 argument", $name))),
                });
            };
        }

        math1!("sqrt", |x: f64| x.sqrt());
        math1!("sin", |x: f64| x.sin());
        math1!("cos", |x: f64| x.cos());
        math1!("abs", |x: f64| x.abs());
        math1!("floor", |x: f64| x.floor());
        math1!("ceil", |x: f64| x.ceil());

        def!("round", |_, args| match args.first() {
            Some(Value::Float(x)) => Ok(Value::Int(x.round() as i64)),
            Some(Value::Int(i)) => Ok(Value::Int(*i)),
            Some(other) => Err(RtError::new(format!("round() needs number, got {}", other.type_name()))),
            None => Err(RtError::new("round() needs 1 argument")),
        });

        def!("min", |_, args| match (args.first(), args.get(1)) {
            (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).min(*b))),
            (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a.min(*b))),
            _ => Err(RtError::new("min() needs two numbers")),
        });

        def!("max", |_, args| match (args.first(), args.get(1)) {
            (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).max(*b))),
            (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a.max(*b))),
            _ => Err(RtError::new("max() needs two numbers")),
        });

        def!("rand", |vm, _| Ok(Value::Float(vm.next_rand())));

        def!("rand_int", |vm, args| match (args.first(), args.get(1)) {
            (Some(Value::Int(lo)), Some(Value::Int(hi))) => {
                if hi < lo {
                    return Err(RtError::new("rand_int() high < low"));
                }
                let span = (*hi - *lo + 1) as f64;
                Ok(Value::Int(*lo + (vm.next_rand() * span) as i64))
            }
            _ => Err(RtError::new("rand_int() needs two ints")),
        });

        def!("seed", |vm, args| {
            if let Some(Value::Int(s)) = args.first() {
                vm.rng = (*s as u64) | 1;
            }
            Ok(Value::None)
        });

        def!("now_ms", |_, _| {
            let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            Ok(Value::Int(d.as_millis() as i64))
        });
    }

    pub fn run(&mut self, chunk: Rc<Chunk>) -> R<Value> {
        self.stack.clear();
        self.frames.clear();
        let base = self.stack.len();
        let n = chunk.n_locals as usize;
        self.stack.resize(base + n, Value::None);
        self.frames.push(Frame { chunk, ip: 0, base });
        self.step()
    }

    fn enter(&mut self, chunk: Rc<Chunk>, base: usize) {
        let end = base + chunk.n_locals as usize;
        if self.stack.len() < end {
            self.stack.resize(end, Value::None);
        }
        self.frames.push(Frame { chunk, ip: 0, base });
    }

    pub fn call_fn(&mut self, name: &str, args: &[Value]) -> R<Value> {
        let id = match self.global_ids.get(name) {
            Some(id) => *id as usize,
            None => return Err(RtError::new(format!("undefined function '{}'", name))),
        };
        let f = match self.globals.get(id) {
            Some(Value::Func(f)) => f.clone(),
            Some(_) => return Err(RtError::new(format!("'{}' is not a function", name))),
            None => return Err(RtError::new(format!("undefined function '{}'", name))),
        };
        if args.len() != f.n_params as usize {
            return Err(RtError::new(format!(
                "{}() expects {} argument(s), got {}",
                name,
                f.n_params,
                args.len()
            )));
        }
        let base = self.stack.len();
        for a in args {
            self.stack.push(a.clone());
        }
        self.enter(f.chunk.clone(), base);
        self.step()
    }

fn step(&mut self) -> R<Value> {
        let mut tick: u64 = 0;
        loop {
            tick += 1;
            if tick & 0xFFFF == 0 {
                self.steps += tick;
                tick = 0;
                if self.steps > MAX_STEPS {
                    return Err(RtError::new("step limit exceeded (possible infinite loop)"));
                }
            }
            // one frame lookup per instruction
            let fi = self.frames.len() - 1;
            let (ip, base) = {
                let f = &self.frames[fi];
                (f.ip, f.base)
            };
            let op = match self.frames[fi].chunk.code.get(ip) {
                Some(op) => *op,
                None => return Ok(Value::None),
            };
            self.frames[fi].ip = ip + 1;

            match op {
                Op::Const(i) => {
                    let v = self.frames[fi].chunk.consts[i as usize].clone();
                    self.stack.push(v);
                }
                Op::Pop => {
                    self.stack.pop();
                }
                Op::True => self.stack.push(Value::Bool(true)),
                Op::False => self.stack.push(Value::Bool(false)),
                Op::None => self.stack.push(Value::None),
                Op::Dup => {
                    let v = self.stack.last().cloned().unwrap_or(Value::None);
                    self.stack.push(v);
                }
                Op::LoadLocal(slot) => {
                    let v = self.stack.get(base + slot as usize).cloned().unwrap_or(Value::None);
                    self.stack.push(v);
                }
                Op::StoreLocal(slot) => {
                    let v = self.stack.pop().unwrap_or(Value::None);
                    let at = base + slot as usize;
                    if at < self.stack.len() {
                        self.stack[at] = v;
                    } else {
                        self.stack.push(v);
                    }
                }
                Op::LoadGlobal(id) => {
                    let v = self.globals.get(id as usize).cloned().unwrap_or(Value::None);
                    self.stack.push(v);
                }
                Op::StoreGlobal(id) => {
                    let v = self.stack.pop().unwrap_or(Value::None);
                    let id = id as usize;
                    self.ensure_global(id);
                    if self.global_kind[id] == 0 {
                        return Err(RtError::new(format!(
                            "undefined variable '{}'",
                            self.global_names.get(id).cloned().unwrap_or_else(|| Rc::from("?"))
                        )));
                    }
                    if self.global_kind[id] == 1 {
                        return Err(RtError::new(format!(
                            "cannot assign to '{}': declared with let (use var)",
                            self.global_names.get(id).cloned().unwrap_or_else(|| Rc::from("?"))
                        )));
                    }
                    self.globals[id] = v;
                    self.global_kind[id] = 2;
                }
                Op::StoreGlobalConst(id) => {
                    let v = self.stack.pop().unwrap_or(Value::None);
                    let id = id as usize;
                    self.ensure_global(id);
                    if self.global_kind[id] != 0 {
                        return Err(RtError::new(format!(
                            "cannot reassign '{}': declared with let",
                            self.global_names.get(id).cloned().unwrap_or_else(|| Rc::from("?"))
                        )));
                    }
                    self.globals[id] = v;
                    self.global_kind[id] = 1;
                }
                Op::DefGlobal(id) => {
                    let v = self.stack.pop().unwrap_or(Value::None);
                    let id = id as usize;
                    self.ensure_global(id);
                    self.globals[id] = v;
                    self.global_kind[id] = 2;
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                    let b = self.stack.pop().unwrap_or(Value::None);
                    let a = self.stack.pop().unwrap_or(Value::None);
                    let v = self.binop(op, a, b)?;
                    self.stack.push(v);
                }
                Op::Neg => {
                    let a = self.stack.pop().unwrap_or(Value::None);
                    let v = match a {
                        Value::Int(i) => i.checked_neg().map(Value::Int).ok_or_else(|| RtError::new("integer overflow in negation"))?,
                        Value::Float(f) => Value::Float(-f),
                        other => return Err(RtError::new(format!("cannot negate {}", other.type_name()))),
                    };
                    self.stack.push(v);
                }
                Op::Not => {
                    let a = self.stack.pop().unwrap_or(Value::None);
                    let t = a.truthy()?;
                    self.stack.push(Value::Bool(!t));
                }
                Op::Jump(off) => {
                    self.frames[fi].ip = (self.frames[fi].ip as i32 + off) as usize;
                }
                Op::JumpIfFalse(off) => {
                    let a = self.stack.pop().unwrap_or(Value::None);
                    if !a.truthy()? {
                        self.frames[fi].ip = (self.frames[fi].ip as i32 + off) as usize;
                    }
                }
                Op::Loop(off) => {
                    self.frames[fi].ip = (self.frames[fi].ip as i32 + off) as usize;
                }
                Op::Len => {
                    let a = self.stack.pop().unwrap_or(Value::None);
                    let n = match &a {
                        Value::Str(s) => s.len() as i64,
                        Value::Array(arr) => arr.items.borrow().len() as i64,
                        other => return Err(RtError::new(format!("len needs string or array, got {}", other.type_name()))),
                    };
                    self.stack.push(Value::Int(n));
                }
                Op::Array(n, frozen) => {
                    let mut items = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        items.push(self.stack.pop().unwrap_or(Value::None));
                    }
                    items.reverse();
                    self.stack.push(Value::Array(Rc::new(ArrayVal { items: RefCell::new(items), frozen })));
                }
                Op::Struct(name_id, n) => {
                    let name = self.frames[fi].chunk.names[name_id as usize].to_string();
                    let decl = match self.structs.get(&name) {
                        Some(d) => d.clone(),
                        None => return Err(RtError::new(format!("unknown struct '{}'", name))),
                    };
                    let mut vals = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        vals.push(self.stack.pop().unwrap_or(Value::None));
                    }
                    vals.reverse();
                    let mut map = HashMap::new();
                    for (i, f) in decl.iter().enumerate() {
                        map.insert(f.clone(), vals.get(i).cloned().unwrap_or(Value::None));
                    }
                    self.stack.push(Value::Struct(Rc::new(RefCell::new(map))));
                }
                Op::Field(name_id) => {
                    let name = self.frames[fi].chunk.names[name_id as usize].to_string();
                    let base_v = self.stack.pop().unwrap_or(Value::None);
                    match base_v {
                        Value::Struct(m) => {
                            let m = m.borrow();
                            match m.get(&name) {
                                Some(v) => self.stack.push(v.clone()),
                                None => return Err(RtError::new(format!("struct has no field '{}'", name))),
                            }
                        }
                        other => return Err(RtError::new(format!("cannot read field '{}' on {}", name, other.type_name()))),
                    }
                }
                Op::FieldSet(name_id) => {
                    let name = self.frames[fi].chunk.names[name_id as usize].to_string();
                    let val = self.stack.pop().unwrap_or(Value::None);
                    let base_v = self.stack.pop().unwrap_or(Value::None);
                    match base_v {
                        Value::Struct(m) => {
                            let mut m = m.borrow_mut();
                            if !m.contains_key(&name) {
                                return Err(RtError::new(format!("struct has no field '{}'", name)));
                            }
                            m.insert(name, val.clone());
                        }
                        other => return Err(RtError::new(format!("cannot write field on {}", other.type_name()))),
                    }
                    self.stack.push(val);
                }
                Op::Index => {
                    let idx = self.stack.pop().unwrap_or(Value::None);
                    let base_v = self.stack.pop().unwrap_or(Value::None);
                    match (base_v, idx) {
                        (Value::Array(a), Value::Int(n)) => {
                            if n < 0 {
                                return Err(RtError::new(format!("negative index {}", n)));
                            }
                            let a = a.items.borrow();
                            match a.get(n as usize) {
                                Some(v) => self.stack.push(v.clone()),
                                None => return Err(RtError::new(format!("index out of bounds: {} (length {})", n, a.len()))),
                            }
                        }
                        (Value::Str(s), Value::Int(n)) => {
                            if n < 0 {
                                return Err(RtError::new(format!("negative index {}", n)));
                            }
                            let chars: Vec<char> = s.chars().collect();
                            match chars.get(n as usize) {
                                Some(c) => self.stack.push(Value::Str(Rc::from(c.to_string().as_str()))),
                                None => return Err(RtError::new(format!("index out of bounds: {} (length {})", n, chars.len()))),
                            }
                        }
                        (other, _) => return Err(RtError::new(format!("cannot index {}", other.type_name()))),
                    }
                }
                Op::IndexSet => {
                    let val = self.stack.pop().unwrap_or(Value::None);
                    let idx = self.stack.pop().unwrap_or(Value::None);
                    let base_v = self.stack.pop().unwrap_or(Value::None);
                    match (base_v, idx) {
                        (Value::Array(a), Value::Int(n)) => {
                            if a.frozen {
                                return Err(RtError::new("cannot mutate an immutable array (declared with let)"));
                            }
                            if n < 0 {
                                return Err(RtError::new(format!("negative index {}", n)));
                            }
                            let mut a = a.items.borrow_mut();
                            if n as usize >= a.len() {
                                return Err(RtError::new(format!("index out of bounds: {} (length {})", n, a.len())));
                            }
                            a[n as usize] = val.clone();
                        }
                        (other, _) => return Err(RtError::new(format!("cannot assign into {}", other.type_name()))),
                    }
                    self.stack.push(val);
                }
                Op::Call(argc) => {
                    // stack layout at the call: [arg0 .. argN-1, callee]
                    let argc = argc as usize;
                    let callee = match self.stack.pop() {
                        Some(c) => c,
                        None => return Err(RtError::new("stack underflow at call")),
                    };
                    match callee {
                        Value::Native(n) => {
                            let args: Vec<Value> = self.stack.split_off(self.stack.len().saturating_sub(argc));
                            let v = (n.0)(self, &args)?;
                            self.stack.push(v);
                        }
                        Value::Func(f) => {
                            if argc != f.n_params as usize {
                                return Err(RtError::new(format!(
                                    "{}() expects {} argument(s), got {}",
                                    f.name, f.n_params, argc
                                )));
                            }
                            let base = self.stack.len() - argc;
                            let chunk = f.chunk.clone();
                            self.enter(chunk, base);
                        }
                        other => {
                            return Err(RtError::new(format!("{} is not callable", other.type_name())));
                        }
                    }
                }
                Op::Return => {
                    let ret = self.stack.pop().unwrap_or(Value::None);
                    let frame = self.frames.pop().unwrap();
                    self.stack.truncate(frame.base);
                    self.stack.push(ret.clone());
                    if self.frames.is_empty() {
                        return Ok(ret);
                    }
                }
            }
        }
    }
    fn binop(&mut self, op: Op, a: Value, b: Value) -> R<Value> {
        use Op::*;
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => {
                let v = match op {
                    Add => x.checked_add(*y),
                    Sub => x.checked_sub(*y),
                    Mul => x.checked_mul(*y),
                    Div => {
                        if *y == 0 {
                            return Err(RtError::new("division by zero"));
                        }
                        x.checked_div(*y)
                    }
                    Mod => {
                        if *y == 0 {
                            return Err(RtError::new("modulo by zero"));
                        }
                        x.checked_rem(*y)
                    }
                    Eq => return Ok(Value::Bool(x == y)),
                    Ne => return Ok(Value::Bool(x != y)),
                    Lt => return Ok(Value::Bool(x < y)),
                    Le => return Ok(Value::Bool(x <= y)),
                    Gt => return Ok(Value::Bool(x > y)),
                    Ge => return Ok(Value::Bool(x >= y)),
                    _ => return Err(RtError::new("bad opcode")),
                };
                v.map(Value::Int).ok_or_else(|| RtError::new("integer overflow (checked arithmetic is on)"))
            }
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(match op {
                Add => x + y,
                Sub => x - y,
                Mul => x * y,
                Div => {
                    if *y == 0.0 {
                        return Err(RtError::new("division by zero"));
                    }
                    x / y
                }
                Mod => x % y,
                Eq => return Ok(Value::Bool(x == y)),
                Ne => return Ok(Value::Bool(x != y)),
                Lt => return Ok(Value::Bool(x < y)),
                Le => return Ok(Value::Bool(x <= y)),
                Gt => return Ok(Value::Bool(x > y)),
                Ge => return Ok(Value::Bool(x >= y)),
                _ => return Err(RtError::new("bad opcode")),
            })),
            (Value::Str(x), Value::Str(y)) => match op {
                Add => Ok(Value::Str(Rc::from(format!("{}{}", x, y).as_str()))),
                Eq => Ok(Value::Bool(x == y)),
                Ne => Ok(Value::Bool(x != y)),
                Lt => Ok(Value::Bool(x < y)),
                Le => Ok(Value::Bool(x <= y)),
                Gt => Ok(Value::Bool(x > y)),
                Ge => Ok(Value::Bool(x >= y)),
                _ => Err(RtError::new("string does not support this operator")),
            },
            (Value::Bool(x), Value::Bool(y)) => match op {
                Eq => Ok(Value::Bool(x == y)),
                Ne => Ok(Value::Bool(x != y)),
                _ => Err(RtError::new("bool does not support this operator")),
            },
            (Value::None, Value::None) => match op {
                Eq => Ok(Value::Bool(true)),
                Ne => Ok(Value::Bool(false)),
                _ => Err(RtError::new("none does not support this operator")),
            },
            _ => Err(RtError::new(format!(
                "operator {:?} not defined for {} and {}",
                op,
                a.type_name(),
                b.type_name()
            ))),
        }
    }
}