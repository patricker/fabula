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

```rust reference file=tests/guides_dsl_in_rust.rs#step1_parse_pattern
```

`parse_pattern` returns `Pattern<String, MemValue>` — the same type the builder API produces. You can register it directly with a `SiftEngine<MemGraph>`.

## Step 2: Parse a full document

A document can contain multiple patterns, graphs, and compose directives:

```rust reference file=tests/guides_dsl_in_rust.rs#step2_parse_document
```

Compose directives produce new patterns in the `patterns` list. The composed pattern (`promise_kept`) is a regular `Pattern` — the composition is resolved at parse time.

## Step 3: Evaluate

Register parsed patterns with the engine and evaluate:

```rust reference file=tests/guides_dsl_in_rust.rs#step3_evaluate
```

## Step 4: Custom type systems with TypeMapper

The default `parse_pattern` produces `Pattern<String, MemValue>`. If your application uses different types (e.g., `u32` labels, custom value enums), implement `TypeMapper`:

```rust reference file=tests/guides_dsl_in_rust.rs#step4_type_mapper
```

Each `TypeMapper` method returns `Result` — return `Err` to reject invalid labels or values at compile time.

## Step 5: Composable parsing for downstream DSLs

If you're building a DSL that embeds fabula pattern syntax (e.g., a storylet DSL), use the composable parser API:

```rust reference file=tests/guides_dsl_in_rust.rs#step5_composable
```

The parser exposes `pos()`, `from_tokens_at()`, and `into_inner()` for cursor management. All parsing primitives (`parse_stage`, `parse_negation`, `parse_temporal`, `peek`, `advance`, `check`, `expect`, `expect_ident`) are public API.

## Error handling

All parse functions return `Result<T, ParseError>`. `ParseError` includes the line and column of the failure:

```rust reference file=tests/guides_dsl_in_rust.rs#error_handling
```

## Where to go next

- [DSL Reference](/docs/reference/dsl) — Complete syntax reference for patterns, graphs, compose, and TypeMapper.
- [Pattern Playground](/docs/playground/pattern-playground) — Try DSL patterns interactively.
- [Composing Patterns](/docs/guides/composing-patterns) — Practical composition recipes.
