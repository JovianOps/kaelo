use kaelo::Engine;

fn int(src: &str) -> i64 {
    match Engine::new().run(src).expect("run") {
        kaelo::Value::Int(n) => n,
        other => panic!("expected int, got {:?}", other),
    }
}

#[test]
fn string_helpers() {
    assert_eq!(int(r#"len(upper("ab"));"#), 2);
    assert_eq!(int(r#"len(trim("  ab  "));"#), 2);
    assert_eq!(int(r#"len(split("a,b,c", ","));"#), 3);
    assert_eq!(int(r#"len(chars("abc"));"#), 3);
    assert_eq!(int(r#"len(join(split("a,b", ","), "-"));"#), 3);
    assert_eq!(int(r#"len(replace("aXbXc", "X", "-"));"#), 5);
}

#[test]
fn string_predicates() {
    let t = |src: &str| Engine::new().run(src).expect("run").render() == "true";
    assert!(t(r#"starts_with("hello", "he");"#));
    assert!(t(r#"ends_with("hello", "lo");"#));
    assert!(t(r#"contains("hello", "ell");"#));
    assert!(t("contains([1,2,3], 2);"));
    assert!(!t("contains([1,2,3], 9);"));
}

#[test]
fn array_helpers() {
    assert_eq!(int("var a = [3,1,2]; sort(a); a[0];"), 1);
    assert_eq!(int("var a = [1,2,3]; reverse(a); a[0];"), 3);
    assert_eq!(int("var a = [1,2,3,4]; len(slice(a, 1, 3));"), 2);
}

#[test]
fn range_loops() {
    assert_eq!(int("var t = 0; for i in 0..5 { t = t + i; } t;"), 10);
    assert_eq!(int("var t = 0; for i in 3..8 { t = t + i; } t;"), 25);
    // an empty range runs zero times
    assert_eq!(int("var t = 0; for i in 5..5 { t = t + 1; } t;"), 0);
}

#[test]
fn type_names() {
    assert_eq!(Engine::new().run(r#"type(1);"#).unwrap().render(), "int");
    assert_eq!(Engine::new().run(r#"type("x");"#).unwrap().render(), "string");
    assert_eq!(Engine::new().run("type([1]);").unwrap().render(), "array");
    assert_eq!(Engine::new().run("type(none);").unwrap().render(), "none");
}

#[test]
fn immutable_collections_reject_mutation() {
    assert!(Engine::new().run("let a = [1,2]; sort(a);").is_err());
    assert!(Engine::new().run("let a = [1,2]; reverse(a);").is_err());
}