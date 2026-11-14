//! Kaelo — a fast, memory-safe, embeddable language for games and tools.
//!
//! ```ignore
//! let mut k = kaelo::Engine::new();
//! k.load("fn add(a, b) { return a + b; }").unwrap();
//! let v = k.call("add", &[1.into(), 2.into()]).unwrap();
//! ```

pub mod ast;
pub mod compiler;
pub mod ffi;
pub mod jit;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod vm;

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

pub use vm::{RtError, Value, Vm};

pub struct Engine {
    pub vm: Vm,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine { vm: Vm::new() }
    }

    /// Compile and run a whole program.
    pub fn run(&mut self, src: &str) -> Result<Value, String> {
        self.compile(src)?;
        let chunk = self.chunk();
        self.vm.run(chunk).map_err(|e| e.msg)
    }

    /// Compile a program and keep it loaded, ready for `call`.
    pub fn load(&mut self, src: &str) -> Result<(), String> {
        self.compile(src)?;
        let chunk = self.chunk();
        self.vm.run(chunk).map(|_| ()).map_err(|e| e.msg)
    }

    fn compile(&mut self, src: &str) -> Result<(), String> {
        let prog = parser::parse(src)?;
        let mut c = compiler::Compiler::new();
        let compiled = c.compile(&prog)?;

        let mut fns = compiled.fns;
        for f in fns.iter_mut() {
            self.vm.link(&mut f.chunk);
        }
        let mut main = compiled.chunk;
        self.vm.link(&mut main);
        // now that ids are final, attach the function objects
        for f in fns {
            let obj = vm::FuncObj {
                name: Rc::from(f.name.as_str()),
                chunk: Rc::new(f.chunk),
                n_params: f.params.len() as u32,
                jit: std::cell::RefCell::new(None),
            };
            main.consts[f.const_idx] = Value::Func(Rc::new(obj));
        }
        self.vm.structs = compiled.structs.clone();
        self.vm.set_main(Rc::new(main));
        Ok(())
    }

    fn chunk(&self) -> Rc<vm::Chunk> {
        self.vm.main_chunk().expect("compiled chunk")
    }

    /// Call a loaded script function by name (game-loop entry point).
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.vm.call_fn(name, args).map_err(|e| e.msg)
    }

    /// Register a host function the script can call — the embedding hook for
    /// engine APIs (draw, input, audio...).
    pub fn host_fn(&mut self, name: &str, f: Box<dyn Fn(&mut Vm, &[Value]) -> Result<Value, RtError>>) {
        self.vm.define_global(name, Value::Native(Rc::new(vm::NativeFn(f, None))));
    }

    pub fn set_output(&mut self, out: Rc<RefCell<dyn Write>>) {
        self.vm.out = out;
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.vm.set_seed(seed);
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(Rc::new(v.to_string()))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}