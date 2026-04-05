use fabula::prelude::*;
use fabula_memory::MemGraph;

#[test]
fn step1_parse_single_pattern() {
    // #region step1_parse_pattern
    use fabula_dsl::parse_pattern;

    let input = r#"
    pattern suspicious_login {
      stage login_a {
        login_a.type = "login"
        login_a.user -> ?user
        login_a.location -> ?loc_a
      }
      stage login_b {
        login_b.type = "login"
        login_b.user -> ?user
        login_b.location -> ?loc_b
      }
      unless between login_a login_b {
        mid.type = "logout"
        mid.user -> ?user
      }
    }
    "#;

    let pattern = parse_pattern(input).expect("parse failed");
    assert_eq!(pattern.name, "suspicious_login");
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.negations.len(), 1);
    // #endregion
}

#[test]
fn step2_parse_document() {
    // #region step2_parse_document
    use fabula_dsl::parse_document;

    let input = r#"
    pattern setup {
      stage e1 { e1.type = "promise" e1.actor -> ?char }
    }
    pattern payoff {
      stage e2 { e2.type = "fulfill" e2.actor -> ?char }
    }
    compose promise_kept = setup >> payoff sharing(char)

    graph {
      @1 e1.type = "promise"
      @1 e1.actor -> alice
      @3 e2.type = "fulfill"
      @3 e2.actor -> alice
      now = 10
    }
    "#;

    let doc = parse_document(input).expect("parse failed");
    assert_eq!(doc.patterns.len(), 3); // setup, payoff, promise_kept
    assert_eq!(doc.graphs.len(), 1);
    // #endregion
}

#[test]
fn step3_evaluate() {
    // #region step3_evaluate
    use fabula_dsl::parse_document;

    let doc = parse_document(
        r#"
    pattern breach {
      stage e1 { e1.type = "revoke" e1.user -> ?user }
      stage e2 { e2.type = "access" e2.user -> ?user }
      unless between e1 e2 { mid.type = "reauth" mid.user -> ?user }
    }
    graph {
      @1 e1.type = "revoke"   @1 e1.user -> alice
      @3 e2.type = "access"   @3 e2.user -> alice
      now = 10
    }
    "#,
    )
    .unwrap();

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    for pattern in doc.patterns {
        engine.register(pattern);
    }

    let matches = engine.evaluate(&doc.graphs[0]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "breach");
    // #endregion
}

#[test]
fn step4_custom_type_mapper() {
    // #region step4_type_mapper
    use fabula_dsl::{parse_pattern_with, TypeMapper};
    use std::collections::HashMap;

    struct MyMapper {
        labels: HashMap<String, u32>,
    }

    impl TypeMapper for MyMapper {
        type L = u32;
        type V = String; // simplified for this example

        fn label(&self, s: &str) -> Result<u32, String> {
            self.labels
                .get(s)
                .copied()
                .ok_or_else(|| format!("unknown label: {}", s))
        }
        fn string_value(&self, s: &str) -> Result<String, String> {
            Ok(s.to_string())
        }
        fn num_value(&self, n: f64) -> Result<String, String> {
            Ok(n.to_string())
        }
        fn bool_value(&self, b: bool) -> Result<String, String> {
            Ok(b.to_string())
        }
        fn node_ref(&self, name: &str) -> Result<String, String> {
            Ok(name.to_string())
        }
    }

    let mut labels = HashMap::new();
    labels.insert("type".into(), 1);
    labels.insert("user".into(), 2);
    let mapper = MyMapper { labels };

    let pattern = parse_pattern_with(
        r#"pattern test { stage e { e.type = "login" e.user -> ?u } }"#,
        &mapper,
    )
    .unwrap();
    // pattern is Pattern<u32, String>
    // #endregion
    assert_eq!(pattern.name, "test");
    assert_eq!(pattern.stages.len(), 1);
}

#[test]
fn step5_composable_parsing() {
    // #region step5_composable
    use fabula_dsl::compiler::compile_pattern_body;
    use fabula_dsl::lexer::Lexer;
    use fabula_dsl::parser::Parser;

    let source = r#"
      stage e1 { e1.type = "login" e1.user -> ?user }
      stage e2 { e2.type = "logout" e2.user -> ?user }
    "#;

    // Tokenize
    let tokens = Lexer::new(source).tokenize().unwrap();

    // Parse just the pattern body (no `pattern name { }` wrapper)
    let mut parser = Parser::new(tokens);
    let body = parser.parse_pattern_body().unwrap();
    assert_eq!(body.stages.len(), 2);

    // Compile with a name you choose
    let pattern = compile_pattern_body("my_session", &body).unwrap();
    assert_eq!(pattern.name, "my_session");
    assert_eq!(pattern.stages.len(), 2);
    // #endregion
}

#[test]
fn error_handling() {
    // #region error_handling
    let result = fabula_dsl::parse_pattern("pattern bad { }");
    match result {
        Ok(_pattern) => { /* use it */ }
        Err(e) => {
            eprintln!(
                "Parse error at line {}, col {}: {}",
                e.line, e.column, e.message
            );
        }
    }
    // #endregion
}
