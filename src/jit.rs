//! Lowering a kaelo `Chunk` to Cranelift IR.
//!
//! Scope: integer-only functions with parameters, locals, arithmetic,
//! comparisons, `if`/`while` control flow and `return`. Anything else (globals,
//! calls, arrays, structs, floats) makes `compile` return None and the caller
//! falls back to the interpreter.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{AbiParam, Block, Function, InstBuilder, MemFlagsData, Signature, UserFuncName};
use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use std::collections::HashSet;
use std::sync::Arc;

use crate::vm::{Chunk, Op, Value};

pub struct JitCode {
    pub ptr: *const u8,
    pub len: usize,
}

impl JitCode {
    /// Call with `n` i64 arguments; `fns` is the call table (one slot per
    // global id). Returns the first result.
    pub unsafe fn call(&self, args: &[i64], fns: *const *const u8) -> i64 {
        let f: extern "C" fn(usize, *const i64, *const *const u8) -> i64 =
            std::mem::transmute(self.ptr);
        f(args.len(), args.as_ptr(), fns)
    }
}

extern "system" {
    fn VirtualAlloc(addr: *const u8, size: usize, ty: u32, protect: u32) -> *mut u8;
}
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

fn alloc_exec(len: usize) -> *mut u8 {
    unsafe {
        let p = VirtualAlloc(std::ptr::null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        assert!(!p.is_null(), "VirtualAlloc failed");
        p
    }
}

pub fn isa() -> Option<Arc<dyn isa::TargetIsa>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "false").ok()?;
    isa::lookup(target_lexicon::Triple::host())
        .ok()?
        .finish(settings::Flags::new(flag_builder))
        .ok()
}

type V = cranelift_codegen::ir::Value;

/// Compile `chunk` as `fn(argc: i64, argv: *const i64) -> i64`.
/// Returns None when the chunk uses anything this backend does not support.
pub fn compile(
    chunk: &Chunk,
    n_params: usize,
    fn_gids: &std::collections::HashSet<u32>,
) -> Option<JitCode> {
    for c in &chunk.consts {
        match c {
            Value::Int(_) => {}
            _ => return None, // strings, arrays and floats are not lowered yet
        }
    }

    let isa = isa()?;
    let mut ctx = Context::new();
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64)); // argc
    sig.params.push(AbiParam::new(types::I64)); // argv
    sig.params.push(AbiParam::new(types::I64)); // call table: *const *const u8
    sig.returns.push(AbiParam::new(types::I64));
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let code = &chunk.code;
    let n_locals = chunk.n_locals as usize;

    // every jump target gets its own block
    let mut targets: HashSet<usize> = HashSet::new();
    for (i, op) in code.iter().enumerate() {
        let off = match op {
            Op::Jump(o) | Op::JumpIfFalse(o) | Op::Loop(o) => *o,
            _ => continue,
        };
        let t = i as i64 + 1 + off as i64;
        if t < 0 || t as usize > code.len() {
            return None;
        }
        targets.insert(t as usize);
    }

    let mut fn_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_ctx);

    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    let argv_v = builder.block_params(entry)[1];
    let table_v = builder.block_params(entry)[2];

    let vars: Vec<Variable> = (0..n_locals).map(|_| builder.declare_var(types::I64)).collect();

    // one Cranelift block per kaelo instruction: trivially correct, and the
    // cost is one well-predicted jump per instruction
    let blocks: Vec<Block> = (0..code.len()).map(|_| builder.create_block()).collect();
    let done = builder.create_block();

    // entry block loads the parameters and jumps into the first instruction
    builder.switch_to_block(entry);
    for slot in 0..n_locals {
        let v = if slot < n_params {
            let addr = builder.ins().iadd_imm(argv_v, (slot as i64) * 8);
            builder.ins().load(types::I64, MemFlagsData::new(), addr, 0)
        } else {
            builder.ins().iconst(types::I64, 0)
        };
        builder.def_var(vars[slot], v);
    }
    match blocks.first() {
        Some(first) => {
            builder.ins().jump(*first, &[]);
        }
        None => {
            builder.ins().jump(done, &[]);
        }
    }

    let mut st: Vec<V> = Vec::new();
    let mut result: Option<V> = None;
    let mut last_global: Option<u32> = None;
    let callee_sig = builder.import_signature({
        let mut s = Signature::new(isa.default_call_conv());
        s.params.push(AbiParam::new(types::I64)); // argc
        s.params.push(AbiParam::new(types::I64)); // argv
        s.params.push(AbiParam::new(types::I64)); // call table
        s.returns.push(AbiParam::new(types::I64));
        s
    });

    for (ip, op) in code.iter().enumerate() {
        builder.switch_to_block(blocks[ip]);
        let mut terminated = false;

        macro_rules! pop {
            () => {
                st.pop()
            };
        }

        match op {
            Op::Const(i) => match &chunk.consts[*i as usize] {
                Value::Int(n) => st.push(builder.ins().iconst(types::I64, *n)),
                _ => return None,
            },
            Op::Pop => {
                pop!();
            }
            Op::True => st.push(builder.ins().iconst(types::I64, 1)),
            Op::False | Op::None => st.push(builder.ins().iconst(types::I64, 0)),
            Op::LoadLocal(s) => {
                let v = builder.use_var(vars[*s as usize]);
                st.push(v);
            }
            Op::StoreLocal(s) => {
                let v = match pop!() {
                    Some(v) => v,
                    None => return None,
                };
                builder.def_var(vars[*s as usize], v);
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Eq | Op::Ne | Op::Lt | Op::Le
            | Op::Gt | Op::Ge => {
                let b = match pop!() {
                    Some(v) => v,
                    None => return None,
                };
                let a = match pop!() {
                    Some(v) => v,
                    None => return None,
                };
                let v = match op {
                    Op::Add => builder.ins().iadd(a, b),
                    Op::Sub => builder.ins().isub(a, b),
                    Op::Mul => builder.ins().imul(a, b),
                    Op::Div => builder.ins().sdiv(a, b),
                    Op::Mod => builder.ins().srem(a, b),
                    Op::Eq => builder.ins().icmp(IntCC::Equal, a, b),
                    Op::Ne => builder.ins().icmp(IntCC::NotEqual, a, b),
                    Op::Lt => builder.ins().icmp(IntCC::SignedLessThan, a, b),
                    Op::Le => builder.ins().icmp(IntCC::SignedLessThanOrEqual, a, b),
                    Op::Gt => builder.ins().icmp(IntCC::SignedGreaterThan, a, b),
                    Op::Ge => builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, a, b),
                    _ => return None,
                };
                st.push(v);
            }
            Op::LoadGlobal(g) => {
                // only allowed in callee position: args, LoadGlobal, Call
                if !matches!(code.get(ip + 1), Some(Op::Call(_))) {
                    return None; // a real global read: not lowered yet
                }
                last_global = Some(*g);
                st.push(builder.ins().iconst(types::I64, 0)); // callee slot
            }
            Op::Call(argc) => {
                let gid = last_global?;
                if !fn_gids.contains(&gid) {
                    return None; // callee is not a function: not compilable
                }
                let idx = gid as i64;
                let argc = *argc as usize;
                // args are on the stack above the callee slot; the compiler emits
                // args then LoadGlobal, so drop nothing and take the top argc
                let mut argv_vals: Vec<V> = Vec::with_capacity(argc);
                for _ in 0..argc {
                    argv_vals.push(st.pop()?);
                }
                argv_vals.reverse();
                // the callee value sits below the args: drop it
                if st.pop().is_none() {
                    return None;
                }
                let addr = builder.ins().iadd_imm(table_v, idx * 8);
                let fp = builder.ins().load(types::I64, MemFlagsData::new(), addr, 0);
                let argc_v = builder.ins().iconst(types::I64, argc as i64);
                let mut args: Vec<V> = vec![argc_v];
                args.extend(argv_vals);
                let call = builder.ins().call_indirect(callee_sig, fp, &args);
                st.push(builder.inst_results(call)[0]);
            }
            Op::Jump(off) => {
                let t = (ip as i64 + 1 + *off as i64) as usize;
                let target = *blocks.get(t)?;
                builder.ins().jump(target, &[]);
                terminated = true;
            }
            Op::JumpIfFalse(off) => {
                let a = match pop!() {
                    Some(v) => v,
                    None => return None,
                };
                let t = (ip as i64 + 1 + *off as i64) as usize;
                let target = *blocks.get(t)?;
                let fall = *blocks.get(ip + 1).unwrap_or(&done);
                // brif takes the TRUE branch first: the condition was "jump to
                // target when false", so the fallthrough is the true side
                builder.ins().brif(a, fall, &[], target, &[]);
                terminated = true;
            }
            Op::Loop(off) => {
                let t = (ip as i64 + 1 + *off as i64) as usize;
                let target = *blocks.get(t)?;
                builder.ins().jump(target, &[]);
                terminated = true;
            }
            Op::Return => {
                let v = st.pop().unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
                builder.ins().return_(&[v]);
                terminated = true;
                result = None;
            }
            _ => return None,
        }

        if !terminated {
            match blocks.get(ip + 1) {
                Some(next) => {
                    builder.ins().jump(*next, &[]);
                }
                None => {
                    builder.ins().jump(done, &[]);
                }
            }
        }
    }

    // completion value: top of the stack at the end, or 0
    builder.switch_to_block(done);
    let ret = result
        .or_else(|| st.last().copied())
        .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
    builder.ins().return_(&[ret]);

    builder.seal_all_blocks();
    builder.finalize(isa.frontend_config());

    let mut ctrl = ControlPlane::default();
    let compiled = match ctx.compile(&*isa, &mut ctrl) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("jit compile error: {:?}", e.inner);
            return None;
        }
    };
    let buf = compiled.code_buffer();
    if buf.is_empty() {
        return None;
    }
    let mem = alloc_exec(buf.len());
    unsafe {
        std::ptr::copy_nonoverlapping(buf.as_ptr(), mem, buf.len());
        Some(JitCode { ptr: mem, len: buf.len() })
    }
}