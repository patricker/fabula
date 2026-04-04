---
sidebar_position: 6
title: DSL in Rust
---

# DSL in Rust

**Learning objective:** Parse, compile, and evaluate DSL patterns in a Rust project.

## Prerequisites

- `fabula`, `fabula-dsl`, and `fabula-memory` in `Cargo.toml`
- Familiarity with the [DSL syntax](/docs/reference/dsl)

## Step 1: Parse a single pattern

```rust
use fabula_dsl::parse_pattern;
use fabula_memory::MemValue;

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
```

`parse_pattern` returns `Pattern<String, MemValue>` — the same type the builder API produces. You can register it directly with a `SiftEngine<MemGraph>`.

## Step 2: Parse a full document

A document can contain multiple patterns, graphs, and compose directives:

```rust
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
```

Compose directives produce new patterns in the `patterns` list. The composed pattern (`promise_kept`) is a regular `Pattern` — the composition is resolved at parse time.

## Step 3: Evaluate

Register parsed patterns with the engine and evaluate:

```rust
use fabula::prelude::*;
use fabula_dsl::parse_document;
use fabula_memory::MemGraph;

let doc = parse_document(r#"
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
"#).unwrap();

let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
for pattern in doc.patterns {
    engine.register(pattern);
}

let matches = engine.evaluate(&doc.graphs[0]);
assert_eq!(matches.len(), 1);
assert_eq!(matches[0].pattern, "breach");
```

## Step 4: Custom type systems with TypeMapper

The default `parse_pattern` produces `Pattern<String, MemValue>`. If your application uses different types (e.g., `u32` labels, custom value enums), implement `TypeMapper`:

```rust
use fabula_dsl::{TypeMapper, parse_pattern_with};
use std::collections::HashMap;

struct MyMapper {
    labels: HashMap<String, u32>,
}

impl TypeMapper for MyMapper {
    type L = u32;
    type V = String; // simplified for this example

    fn label(&self, s: &str) -> Result<u32, String> {
        self.labels.get(s).copied()
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
).unwrap();
// pattern is Pattern<u32, String>
```

Each `TypeMapper` method returns `Result` — return `Err` to reject invalid labels or values at compile time.

## Step 5: Composable parsing for downstream DSLs

If you're building a DSL that embeds fabula pattern syntax (e.g., a storylet DSL), use the composable parser API:

```rust
use fabula_dsl::lexer::Lexer;
use fabula_dsl::parser::Parser;
use fabula_dsl::compiler::compile_pattern_body;

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
```

The parser exposes `pos()`, `from_tokens_at()`, and `into_inner()` for cursor management. All parsing primitives (`parse_stage`, `parse_negation`, `parse_temporal`, `peek`, `advance`, `check`, `expect`, `expect_ident`) are public API.

## Error handling

All parse functions return `Result<T, ParseError>`. `ParseError` includes the line and column of the failure:

```rust
let result = fabula_dsl::parse_pattern("pattern bad { }");
match result {
    Ok(pattern) => { /* use it */ }
    Err(e) => {
        eprintln!("Parse error at line {}, col {}: {}", e.line, e.column, e.message);
    }
}
```

## Where to go next

- [DSL Reference](/docs/reference/dsl) — Complete syntax reference for patterns, graphs, compose, and TypeMapper.
- [Pattern Playground](/docs/playground/pattern-playground) — Try DSL patterns interactively.
- [Composing Patterns](/docs/guides/composing-patterns) — Practical composition recipes.
