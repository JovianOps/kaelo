//! JIT spike: compile `fn add(a: i64, b: i64) -> i64 { a + b }` with Cranelift,
//! copy the emitted code into RWX memory, and call it.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, Signature, UserFuncName, types};
use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

extern "system" {
    fn VirtualAlloc(addr: *const u8, size: usize, ty: u32, protect: u32) -> *mut u8;
}
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

unsafe fn alloc_exec(len: usize) -> *mut u8 {
    let p = VirtualAlloc(std::ptr::null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    assert!(!p.is_null(), "VirtualAlloc failed");
    p
}

fn make_isa() -> Option<std::sync::Arc<dyn isa::TargetIsa>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "false").ok()?;
    let triple = target_lexicon::Triple::host();
    let builder = isa::lookup(triple).ok()?;
    builder.finish(settings::Flags::new(flag_builder)).ok()
}

fn compile_add() -> Option<extern "C" fn(i64, i64) -> i64> {
    let isa = make_isa()?;
    let mut ctx = Context::new();

    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fn_ctx = FunctionBuilderContext::new();
    let block = {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let (a, b) = {
            let p = builder.block_params(block);
            (p[0], p[1])
        };
        let sum = builder.ins().iadd(a, b);
        builder.ins().return_(&[sum]);
        builder.seal_all_blocks();
        builder.finalize(isa.frontend_config());
        block
    };
    let _ = block;

    let mut ctrl = ControlPlane::default();
    let code = ctx.compile(&*isa, &mut ctrl).ok()?;
    let buf = code.code_buffer();
    assert!(!buf.is_empty(), "empty code buffer");
    println!("emitted {} bytes of machine code", buf.len());
    print!("bytes:");
    for b in buf.iter() { print!(" {:02x}", b); }
    println!();

    let mem = unsafe { alloc_exec(buf.len()) };
    unsafe {
        std::ptr::copy_nonoverlapping(buf.as_ptr(), mem, buf.len());
        Some(std::mem::transmute::<*mut u8, extern "C" fn(i64, i64) -> i64>(mem))
    }
}

fn main() {
    let f = match compile_add() {
        Some(f) => f,
        None => {
            println!("compile failed");
            return;
        }
    };
    let a = f(2, 3);
    let b = f(20, 22);
    println!("jit add(2,3) = {}", a);
    println!("jit add(20,22) = {}", b);
    assert_eq!((a, b), (5, 42));
    println!("JIT spike OK");
}

