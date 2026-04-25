use fabula_dsl::ast::{ConstraintValue, ExprAst, ExprBinOp};
use fabula_dsl::lexer::Lexer;
use fabula_dsl::parser::Parser;

fn parse_pattern(src: &str) -> fabula_dsl::ast::PatternAst {
    let tokens = Lexer::new(src).tokenize().expect("lex failed");
    let mut p = Parser::new(tokens);
    p.parse_pattern_only().expect("parse failed")
}

#[test]
fn parses_simple_let_attached_to_preceding_stage() {
    let src = r#"
        pattern foo {
            stage e1 {
                e1.type = "world"
                e1.pulse_count -> ?ts
            }
            let deadline = ?ts + 5
            stage e2 { e2.pulse_count = ?deadline }
        }
    "#;
    let ast = parse_pattern(src);
    assert_eq!(ast.stages.len(), 2);
    let lets = &ast.stages[0].let_bindings;
    assert_eq!(lets.len(), 1);
    assert_eq!(lets[0].name, "deadline");
    match &lets[0].expr {
        ExprAst::BinOp(ExprBinOp::Add, l, r) => {
            assert!(matches!(**l, ExprAst::Var(ref s) if s == "ts"));
            assert!(matches!(**r, ExprAst::Literal(ConstraintValue::Num(n)) if n == 5.0));
        }
        other => panic!("expected Add, got {:?}", other),
    }
}

#[test]
fn precedence_mul_before_add() {
    // ?a + ?b * 2  ==> Add(a, Mul(b, 2))
    let src = r#"
        pattern foo {
            stage e1 {
                e1.x -> ?a
                e1.y -> ?b
            }
            let z = ?a + ?b * 2
        }
    "#;
    let ast = parse_pattern(src);
    let e = &ast.stages[0].let_bindings[0].expr;
    match e {
        ExprAst::BinOp(ExprBinOp::Add, _, r) => {
            assert!(matches!(**r, ExprAst::BinOp(ExprBinOp::Mul, _, _)));
        }
        _ => panic!("expected Add at root: {:?}", e),
    }
}

#[test]
fn parens_override_precedence() {
    // (?a + ?b) * 2  ==> Mul(Add(a, b), 2)
    let src = r#"
        pattern foo {
            stage e1 {
                e1.x -> ?a
                e1.y -> ?b
            }
            let z = (?a + ?b) * 2
        }
    "#;
    let ast = parse_pattern(src);
    let e = &ast.stages[0].let_bindings[0].expr;
    match e {
        ExprAst::BinOp(ExprBinOp::Mul, l, _) => {
            assert!(matches!(**l, ExprAst::BinOp(ExprBinOp::Add, _, _)));
        }
        _ => panic!("expected Mul at root"),
    }
}

#[test]
fn division_operator_parses() {
    let src = r#"
        pattern foo {
            stage e1 { e1.x -> ?a }
            let z = ?a / 2
        }
    "#;
    let ast = parse_pattern(src);
    let e = &ast.stages[0].let_bindings[0].expr;
    assert!(matches!(e, ExprAst::BinOp(ExprBinOp::Div, _, _)));
}

#[test]
fn subtraction_left_associative() {
    // ?a - ?b - ?c  ==> Sub(Sub(a, b), c)
    let src = r#"
        pattern foo {
            stage e1 {
                e1.x -> ?a
                e1.y -> ?b
                e1.z -> ?c
            }
            let r = ?a - ?b - ?c
        }
    "#;
    let ast = parse_pattern(src);
    let e = &ast.stages[0].let_bindings[0].expr;
    match e {
        ExprAst::BinOp(ExprBinOp::Sub, l, r) => {
            assert!(matches!(**l, ExprAst::BinOp(ExprBinOp::Sub, _, _)));
            assert!(matches!(**r, ExprAst::Var(ref s) if s == "c"));
        }
        _ => panic!("expected outer Sub"),
    }
}

#[test]
fn let_before_first_stage_is_error() {
    let src = r#"
        pattern foo {
            let bad = 1 + 2
            stage e1 { e1.x -> ?a }
        }
    "#;
    let tokens = Lexer::new(src).tokenize().expect("lex failed");
    let mut p = Parser::new(tokens);
    let result = p.parse_pattern_only();
    assert!(result.is_err(), "expected parse error for let before any stage");
}

#[test]
fn string_literal_in_let_expr() {
    // String literals are syntactically allowed; whether they're semantically
    // useful for arithmetic depends on the value type. Parser-level shouldn't
    // care.
    let src = r#"
        pattern foo {
            stage e1 { e1.x -> ?a }
            let s = "hello"
        }
    "#;
    let ast = parse_pattern(src);
    let e = &ast.stages[0].let_bindings[0].expr;
    assert!(matches!(e, ExprAst::Literal(ConstraintValue::Str(ref s)) if s == "hello"));
}
