use kaelo::{Engine, Value};

fn run(src: &str) -> Value {
    let mut e = Engine::new();
    e.run(src).expect("program should run")
}

fn run_err(src: &str) -> String {
    let mut e = Engine::new();
    match e.run(src) {
        Ok(_) => panic!("expected an error for: {}", src),
        Err(e) => e,
    }
}

fn int(v: Value) -> i64 {
    match v {
        Value::Int(i) => i,
        other => panic!("expected int, got {:?}", other),
    }
}

#[test]
fn arithmetic() {
    assert_eq!(int(run("1 + 2 * 3;")), 7);
    assert_eq!(int(run("(1 + 2) * 3;")), 9);
    assert_eq!(int(run("10 / 4;")), 2);
    assert_eq!(int(run("10 % 4;")), 2);
    assert_eq!(int(run("-5 + 2;")), -3);
}

#[test]
fn immutable_bindings() {
    let e = run_err("let x = 1; x = 2;");
    assert!(e.contains("cannot assign"), "got: {}", e);
    // var is reassignable
    assert_eq!(int(run("var x = 1; x = 2; x;")), 2);
}

#[test]
fn no_null() {
    // there is no null literal; absence is the `none` value
    assert!(run_err("x = none;").contains("undefined"));
    assert_eq!(run("none == none;").render(), "true");
}

#[test]
fn bounds_are_checked() {
    let e = run_err("var a = [1, 2, 3]; a[9];");
    assert!(e.contains("out of bounds"), "got: {}", e);
    let e = run_err("var a = [1, 2, 3]; a[0 - 1];");
    assert!(e.contains("negative index"), "got: {}", e);
}

#[test]
fn integer_overflow_is_an_error() {
    let big = "9223372036854775807";
    let e = run_err(&format!("{} + 1;", big));
    assert!(e.contains("overflow"), "got: {}", e);
}

#[test]
fn division_by_zero_is_an_error() {
    assert!(run_err("1 / 0;").contains("division by zero"));
    assert!(run_err("1 % 0;").contains("modulo by zero"));
}

#[test]
fn functions_and_recursion() {
    let v = run(
        "fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } fib(20);",
    );
    assert_eq!(int(v), 6765);
}

#[test]
fn structs() {
    let v = run(
        "struct P { x, y, }
         var p = P { x: 3, y: 4 };
         p.x = 6;
         p.x + p.y;",
    );
    assert_eq!(int(v), 10);
}

#[test]
fn arrays_and_loops() {
    let v = run(
        "var a = [];
         var i = 0;
         while i < 5 {
             push(a, i * i);
             i = i + 1;
         }
         var s = 0;
         for x in a { s = s + x; }
         s;",
    );
    assert_eq!(int(v), 30);
}

#[test]
fn immutable_arrays_reject_mutation() {
    let e = run_err("let a = [1, 2]; push(a, 3);");
    assert!(e.contains("immutable"), "got: {}", e);
}

#[test]
fn strings() {
    let v = run("\"foo\" + \"bar\";");
    assert_eq!(v.render(), "foobar");
    assert_eq!(int(run("len(\"hello\");")), 5);
}

#[test]
fn engine_call_and_host_functions() {
    let mut e = Engine::new();
    e.load("fn add(a, b) { return a + b; } fn bump(n) { return n * 2; }")
        .expect("load");
    assert_eq!(int(e.call("add", &[Value::Int(2), Value::Int(3)]).unwrap()), 5);
    e.host_fn("triple", Box::new(|_, args| match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n * 3)),
        _ => Ok(Value::None),
    }));
    e.load("fn use_host(n) { return triple(n); }").expect("load host");
    assert_eq!(int(e.call("use_host", &[Value::Int(4)]).unwrap()), 12);
    assert_eq!(int(e.call("bump", &[Value::Int(5)]).unwrap()), 10);
}

#[test]
fn step_limit_stops_infinite_loops() {
    let e = run_err("while true { }");
    assert!(e.contains("step limit"), "got: {}", e);
}

#[test]
fn parse_errors_are_reported() {
    let mut e = Engine::new();
    assert!(e.run("fn (").is_err());
    assert!(e.run("let = 3;").is_err());
}

#[test]
fn deterministic_random() {
    let a = run("seed(42); rand_int(0, 100);");
    let b = run("seed(42); rand_int(0, 100);");
    assert_eq!(a.render(), b.render());
    let c = run("seed(7); rand_int(0, 100);");
    assert_ne!(a.render(), c.render());
}