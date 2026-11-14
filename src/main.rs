use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        std::process::exit(1);
    }
    match args[0].as_str() {
        "-h" | "--help" | "help" => usage(),
        "-v" | "--version" => println!("kaelo {}", env!("CARGO_PKG_VERSION")),
        "eval" => {
            if args.len() < 2 {
                eprintln!("usage: kaelo eval <code>");
                std::process::exit(1);
            }
            match kaelo::Engine::new().run(&args[1]) {
                Ok(v) => println!("{}", v.render()),
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "run" => {
            if args.len() < 2 {
                eprintln!("usage: kaelo run <file.klo>");
                std::process::exit(1);
            }
            let src = match std::fs::read_to_string(&args[1]) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cannot read {}: {}", args[1], e);
                    std::process::exit(1);
                }
            };
            let t = Instant::now();
            let mut e = kaelo::Engine::new();
            if args.iter().any(|a| a == "--jit") {
                e.vm.jit_enabled = true;
            }
            match e.run(&src) {
                Ok(_) => {
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    eprintln!("[{:.3} ms]", ms);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "jit" => {
            // explicit opt-in: compile functions to native code and run
            if args.len() < 2 {
                eprintln!("usage: kaelo jit <file.klo>");
                std::process::exit(1);
            }
            let src = std::fs::read_to_string(&args[1]).expect("read file");
            let mut e = kaelo::Engine::new();
            e.vm.jit_enabled = true;
            let t0 = Instant::now();
            match e.run(&src) {
                Ok(_) => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    eprintln!("[{:.3} ms]", ms);
                }
                Err(er) => {
                    eprintln!("error: {}", er);
                    std::process::exit(1);
                }
            }
        }
        "bench" => {
            if args.len() < 2 {
                eprintln!("usage: kaelo bench <file.klo>");
                std::process::exit(1);
            }
            let src = std::fs::read_to_string(&args[1]).expect("read file");
            let mut best = f64::MAX;
            for _ in 0..3 {
                let t = Instant::now();
                let prog = kaelo::parser::parse(&src).expect("parse");
                let mut c = kaelo::compiler::Compiler::new();
                let compiled = c.compile(&prog).expect("compile");
                let mut vm = kaelo::Vm::new();
                vm.structs = compiled.structs.clone();
                let mut fns = compiled.fns;
                for f in fns.iter_mut() {
                    vm.link(&mut f.chunk);
                }
                let mut main = compiled.chunk;
                vm.link(&mut main);
                for f in fns {
                    let obj = kaelo::vm::FuncObj {
                        name: std::rc::Rc::from(f.name.as_str()),
                        chunk: std::rc::Rc::new(f.chunk),
                        n_params: f.params.len() as u32,
                        jit: std::cell::RefCell::new(None),
                    };
                    main.consts[f.const_idx] = kaelo::Value::Func(std::rc::Rc::new(obj));
                }
                vm.run(std::rc::Rc::new(main)).expect("run");
                let d = t.elapsed().as_secs_f64();
                if d < best {
                    best = d;
                }
            }
            println!("best of 3: {:.3} ms", best * 1000.0);
        }
        other => {
            eprintln!("unknown command '{}'\n", other);
            usage();
            std::process::exit(1);
        }
    }
}

fn usage() {
    println!(
        "kaelo {}\n\nUSAGE:\n  kaelo run <file.klo>       run a program\n  kaelo eval <code>          run code directly\n  kaelo jit <file.klo>       run with the native-code JIT (experimental)
  kaelo bench <file.klo>     time a program (best of 3)\n  kaelo --version            print version",
        env!("CARGO_PKG_VERSION")
    );
}