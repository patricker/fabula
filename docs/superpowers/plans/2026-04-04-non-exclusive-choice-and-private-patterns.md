# Non-Exclusive Choice (7.6) & Private Patterns (7.7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add DSL syntax for non-exclusive choice composition and a `private` pattern modifier that suppresses a pattern's matches from engine output.

**Architecture:** Both features touch the same vertical: pattern struct → DSL parser/compiler → engine output. Non-exclusive choice requires only a DSL/AST change (the Rust API `compose::choice()` already accepts `exclusive: bool`; all DSL choices are currently hardcoded to `true`). Private patterns add a boolean field to `Pattern`, a DSL keyword, and output filtering in the engine.

**Tech Stack:** Rust (fabula core, fabula-dsl). No new dependencies.

---

## File Map

| File | Changes |
|------|---------|
| `crates/fabula-dsl/src/ast.rs` | Add `exclusive: bool` to `ComposeBody::Choice`. Add `private: bool` to `PatternAst`. |
| `crates/fabula-dsl/src/parser.rs` | Parse trailing `nonexclusive` keyword on choice. Parse leading `private` keyword on patterns. |
| `crates/fabula-dsl/src/compiler.rs` | Pass `exclusive` flag to `compose::choice()`. Set `pattern.private` from AST. |
| `crates/fabula/src/pattern.rs` | Add `pub private: bool` field to `Pattern<L, V>`. |
| `crates/fabula/src/builder.rs` | Add `.private()` method to `PatternBuilder`. |
| `crates/fabula/src/compose.rs` | Preserve `private` in `rename_vars`. Propagate through `sequence`/`repeat`/`choice`. |
| `crates/fabula/src/engine/eval.rs` | Filter events for private patterns before returning from `on_edge_added`. |
| `crates/fabula/src/engine/mod.rs` | Filter matches in `evaluate()` and `drain_completed()` for private patterns. |
| `crates/fabula-test-suite/src/scenarios/composition.rs` | New golden tests for both features. |
| `crates/fabula-test-suite/tests/golden.rs` | Add new scenario names to `golden_tests!` macro. |
| `crates/fabula-dsl/tests/integration.rs` | DSL parse/compile tests for both features. |

---

## Task 1: Non-Exclusive Choice — AST and Parser

**Files:**
- Modify: `crates/fabula-dsl/src/ast.rs:50` — `ComposeBody::Choice`
- Modify: `crates/fabula-dsl/src/parser.rs:320-335` — choice parsing branch
- Test: `crates/fabula-dsl/tests/integration.rs`

- [ ] **Step 1: Write failing test for `nonexclusive` parsing**

Add to `crates/fabula-dsl/tests/integration.rs`:

```rust
#[test]
fn parse_compose_choice_nonexclusive() {
    let src = r#"
        pattern war { stage e1 { e1.type = "war" } }
        pattern famine { stage e1 { e1.type = "famine" } }
        compose crisis = war | famine nonexclusive
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    // Non-exclusive: all patterns should have group = None
    for p in &doc.patterns {
        if p.name.starts_with("crisis_") {
            assert_eq!(p.group, None, "non-exclusive choice should have no group: {}", p.name);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-dsl parse_compose_choice_nonexclusive -- --nocapture`
Expected: FAIL — `nonexclusive` not recognized as valid syntax (parse error).

- [ ] **Step 3: Add `exclusive` field to AST**

In `crates/fabula-dsl/src/ast.rs`, change:

```rust
Choice { alternatives: Vec<String> },
```

to:

```rust
Choice { alternatives: Vec<String>, exclusive: bool },
```

- [ ] **Step 4: Fix all match arms that destructure `Choice`**

There are two sites that destructure `ComposeBody::Choice`:

In `crates/fabula-dsl/src/parser.rs` (~line 325), where `Choice` is constructed:

```rust
ComposeBody::Choice { alternatives, exclusive: true }
```

In `crates/fabula-dsl/src/compiler.rs` (~line 564), where `Choice` is matched:

```rust
ComposeBody::Choice { alternatives, exclusive } => {
    let pats = alternatives
        .iter()
        .map(|name| resolve(name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(compose::choice(&ast.name, &pats, *exclusive))
}
```

Run: `cargo check -p fabula-dsl` to verify the `exclusive` field is handled everywhere. Fix any remaining match exhaustiveness errors.

- [ ] **Step 5: Parse optional `nonexclusive` keyword**

In `crates/fabula-dsl/src/parser.rs`, in the choice parsing branch (after collecting alternatives), add:

```rust
let exclusive = if self.check_ident("nonexclusive") {
    self.advance();
    false
} else {
    true
};
ComposeBody::Choice { alternatives, exclusive }
```

Note: `check_ident("nonexclusive")` checks if the next token is an `Ident` with that value. If the parser doesn't have a `check_ident` helper, add one or use:

```rust
let exclusive = match self.peek().kind {
    TokenKind::Ident(ref s) if s == "nonexclusive" => {
        self.advance();
        false
    }
    _ => true,
};
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p fabula-dsl parse_compose_choice_nonexclusive -- --nocapture`
Expected: PASS

- [ ] **Step 7: Write test that exclusive choice still works (backward compat)**

Add to `crates/fabula-dsl/tests/integration.rs`:

```rust
#[test]
fn parse_compose_choice_exclusive_default() {
    let src = r#"
        pattern war { stage e1 { e1.type = "war" } }
        pattern famine { stage e1 { e1.type = "famine" } }
        compose crisis = war | famine
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    for p in &doc.patterns {
        if p.name.starts_with("crisis_") {
            assert_eq!(p.group, Some("crisis".to_string()),
                "default choice should be exclusive: {}", p.name);
        }
    }
}
```

- [ ] **Step 8: Run to verify backward compat**

Run: `cargo test -p fabula-dsl parse_compose_choice -- --nocapture`
Expected: Both `parse_compose_choice_nonexclusive` and `parse_compose_choice_exclusive_default` PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/fabula-dsl/src/ast.rs crates/fabula-dsl/src/parser.rs \
       crates/fabula-dsl/src/compiler.rs crates/fabula-dsl/tests/integration.rs
git commit -m "DSL: non-exclusive choice — 'compose x = a | b nonexclusive' syntax"
```

---

## Task 2: Non-Exclusive Choice — Golden Test

**Files:**
- Modify: `crates/fabula-test-suite/src/scenarios/composition.rs`
- Modify: `crates/fabula-test-suite/tests/golden.rs`

- [ ] **Step 1: Write golden test for non-exclusive choice**

Add to `crates/fabula-test-suite/src/scenarios/composition.rs`:

```rust
pub fn incremental_choice_nonexclusive<G: TestGraph>() {
    use fabula::compose;
    use fabula::prelude::*;

    let mut graph = G::new();

    // Two single-stage patterns
    let war = PatternBuilder::new("war")
        .stage("e1", |s| {
            s.edge("e1", G::label("type"), G::value("war"))
        })
        .build();
    let famine = PatternBuilder::new("famine")
        .stage("e1", |s| {
            s.edge("e1", G::label("type"), G::value("famine"))
        })
        .build();

    // Non-exclusive: both can complete independently
    let alternatives = compose::choice("crisis", &[&war, &famine], false);

    let mut engine = SiftEngine::new();
    for p in alternatives {
        engine.register(p);
    }

    // Add a "war" event at t=1
    let war_node = G::node("ev1");
    let interval = Interval::open(G::time(1));
    G::add_edge(&mut graph, &war_node, "type", "war", G::time(1));
    let events = engine.on_edge_added(
        &graph, &war_node, &G::label("type"), &G::value("war"), &interval,
    );
    let completed: Vec<_> = events.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).collect();
    assert_eq!(completed.len(), 1, "war should complete");

    // Add a "famine" event at t=2 — should ALSO complete (non-exclusive)
    let famine_node = G::node("ev2");
    let interval2 = Interval::open(G::time(2));
    G::add_edge(&mut graph, &famine_node, "type", "famine", G::time(2));
    let events2 = engine.on_edge_added(
        &graph, &famine_node, &G::label("type"), &G::value("famine"), &interval2,
    );
    let completed2: Vec<_> = events2.iter().filter(|e| matches!(e, SiftEvent::Completed { .. })).collect();
    assert_eq!(completed2.len(), 1, "famine should also complete (non-exclusive)");
}
```

Note: Adapt the `G::add_edge`, `G::node`, `G::label`, `G::value`, `G::time` calls to match the actual `TestGraph` trait API. Read `crates/fabula-test-suite/src/lib.rs` to confirm the helper method names.

- [ ] **Step 2: Add to `golden_tests!` macro**

In `crates/fabula-test-suite/tests/golden.rs`, add `incremental_choice_nonexclusive,` to the macro invocation. Also re-export from `scenarios/mod.rs` if composition scenarios are in a submodule.

- [ ] **Step 3: Run to verify**

Run: `cargo test -p fabula-test-suite choice_nonexclusive`
Expected: PASS on all 3 adapters (mem, pet, grafeo — grafeo will be ignored if rustc < 1.91).

- [ ] **Step 4: Commit**

```bash
git add crates/fabula-test-suite/
git commit -m "Test: golden test for non-exclusive choice composition"
```

---

## Task 3: Private Patterns — Pattern Struct and Builder

**Files:**
- Modify: `crates/fabula/src/pattern.rs:104-140` — `Pattern` struct
- Modify: `crates/fabula/src/builder.rs` — `PatternBuilder`
- Modify: `crates/fabula/src/compose.rs` — preserve `private` through composition

- [ ] **Step 1: Write failing test**

Add to `crates/fabula/tests/` (or an existing test file — check where builder tests live):

```rust
#[test]
fn private_pattern_field() {
    let pattern = PatternBuilder::<String, String>::new("helper")
        .stage("e1", |s| s.edge("e1", "type".into(), "test".into()))
        .private()
        .build();
    assert!(pattern.private);

    let public = PatternBuilder::<String, String>::new("visible")
        .stage("e1", |s| s.edge("e1", "type".into(), "test".into()))
        .build();
    assert!(!public.private);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fabula private_pattern_field`
Expected: FAIL — no `private` field on Pattern, no `.private()` on builder.

- [ ] **Step 3: Add `private` field to Pattern**

In `crates/fabula/src/pattern.rs`, add to the `Pattern` struct:

```rust
    /// If true, this pattern's matches and events are suppressed from engine
    /// output. The engine still evaluates the pattern internally (for
    /// composition and exclusive group handling), but `evaluate()`,
    /// `drain_completed()`, and `on_edge_added()` filter out its results.
    pub private: bool,
```

Set default to `false` in the `new()` constructor or wherever `Pattern` instances are created. Check all construction sites — search for `Pattern {` in the codebase to find them all.

- [ ] **Step 4: Add `.private()` to PatternBuilder**

In `crates/fabula/src/builder.rs`, add:

```rust
    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }
```

And add `private: bool` to the builder struct with default `false`. Set it on `build()`:

```rust
    Pattern {
        // ... existing fields ...
        private: self.private,
    }
```

- [ ] **Step 5: Preserve `private` in compose.rs**

In `crates/fabula/src/compose.rs`:

- In `rename_vars()`: add `private: pattern.private` to the returned Pattern construction.
- In `sequence()`: set `private: false` on composed patterns (composed pattern is a new entity).
- In `choice()`: preserve each alternative's `private` flag (clone from original).
- In `repeat()` and `repeat_range()`: set `private: false` on composed pattern.

Search for all `Pattern {` constructions in compose.rs and add `private:` field.

- [ ] **Step 6: Preserve in `map_types()`**

In `crates/fabula/src/pattern.rs`, find `map_types()` and add `private: self.private` to the returned Pattern.

- [ ] **Step 7: Run test**

Run: `cargo test -p fabula private_pattern_field`
Expected: PASS

Run: `cargo test -p fabula` and `cargo clippy -p fabula -- -D warnings`
Expected: All pass, zero warnings. Fix any missed construction sites.

- [ ] **Step 8: Commit**

```bash
git add crates/fabula/src/pattern.rs crates/fabula/src/builder.rs crates/fabula/src/compose.rs
git commit -m "API: add Pattern.private field and PatternBuilder::private()"
```

---

## Task 4: Private Patterns — Engine Filtering

**Files:**
- Modify: `crates/fabula/src/engine/eval.rs` — filter events in `on_edge_added`
- Modify: `crates/fabula/src/engine/mod.rs` — filter in `evaluate()` and `drain_completed()`

- [ ] **Step 1: Write failing golden test**

Add to `crates/fabula-test-suite/src/scenarios/composition.rs`:

```rust
pub fn private_pattern_suppresses_events<G: TestGraph>() {
    use fabula::prelude::*;

    let mut graph = G::new();

    // A private pattern — matches should not appear in output
    let private_pat = PatternBuilder::new("helper")
        .stage("e1", |s| {
            s.edge("e1", G::label("type"), G::value("setup"))
        })
        .private()
        .build();

    // A public pattern — matches should appear
    let public_pat = PatternBuilder::new("visible")
        .stage("e1", |s| {
            s.edge("e1", G::label("type"), G::value("setup"))
        })
        .build();

    let mut engine = SiftEngine::new();
    engine.register(private_pat);
    engine.register(public_pat);

    // Add edge that matches both patterns
    let node = G::node("ev1");
    let interval = Interval::open(G::time(1));
    G::add_edge(&mut graph, &node, "type", "setup", G::time(1));
    let events = engine.on_edge_added(
        &graph, &node, &G::label("type"), &G::value("setup"), &interval,
    );

    // Only the public pattern's event should appear
    let names: Vec<&str> = events.iter().filter_map(|e| match e {
        SiftEvent::Completed { pattern, .. } => Some(pattern.as_str()),
        _ => None,
    }).collect();
    assert!(names.contains(&"visible"), "public pattern should appear");
    assert!(!names.contains(&"helper"), "private pattern should be suppressed");

    // Batch evaluation should also filter
    let matches = engine.evaluate(&graph);
    let match_names: Vec<&str> = matches.iter().map(|m| m.pattern.as_str()).collect();
    assert!(match_names.contains(&"visible"));
    assert!(!match_names.contains(&"helper"));
}
```

Add to `golden_tests!` macro and re-export.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fabula-test-suite private_pattern`
Expected: FAIL — both patterns appear in output (no filtering yet).

- [ ] **Step 3: Filter events in `on_edge_added`**

In `crates/fabula/src/engine/eval.rs`, at the end of `on_edge_added()` (just before returning the events vec), add filtering:

```rust
    // Filter out events from private patterns
    events.retain(|e| {
        let pattern_name = match e {
            SiftEvent::Advanced { pattern, .. } => pattern,
            SiftEvent::Completed { pattern, .. } => pattern,
            SiftEvent::Negated { pattern, .. } => pattern,
            SiftEvent::Expired { pattern, .. } => pattern,
        };
        !self.patterns.iter().any(|p| p.name == *pattern_name && p.private)
    });
```

Important: this filtering must happen AFTER exclusive group handling (the dead-PM killing in Phase 4 of eval). Private patterns still participate in group logic — their completion kills other group members. The filtering only removes them from the returned event list.

- [ ] **Step 4: Filter matches in `evaluate()`**

In `crates/fabula/src/engine/mod.rs`, in the `evaluate()` method, after collecting matches, add:

```rust
    matches.retain(|m| {
        !self.patterns.iter().any(|p| p.name == m.pattern && p.private)
    });
```

- [ ] **Step 5: Filter in `drain_completed()`**

In `crates/fabula/src/engine/mod.rs`, in `drain_completed()`, add the same filter on the returned matches.

- [ ] **Step 6: Run test**

Run: `cargo test -p fabula-test-suite private_pattern`
Expected: PASS

Run: `cargo test -p fabula` and `cargo clippy -p fabula -- -D warnings`
Expected: All pass, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/fabula/src/engine/
git commit -m "API: private patterns — suppress events and matches from engine output"
```

---

## Task 5: Private Patterns — DSL Syntax

**Files:**
- Modify: `crates/fabula-dsl/src/ast.rs` — add `private: bool` to `PatternAst`
- Modify: `crates/fabula-dsl/src/parser.rs` — parse `private pattern ...`
- Modify: `crates/fabula-dsl/src/compiler.rs` — set `private` on compiled pattern
- Test: `crates/fabula-dsl/tests/integration.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/fabula-dsl/tests/integration.rs`:

```rust
#[test]
fn parse_private_pattern() {
    let src = r#"
        private pattern helper {
            stage e1 { e1.type = "setup" }
        }
        pattern visible {
            stage e1 { e1.type = "setup" }
        }
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();
    assert_eq!(doc.patterns.len(), 2);

    let helper = doc.patterns.iter().find(|p| p.name == "helper").unwrap();
    assert!(helper.private, "helper should be private");

    let visible = doc.patterns.iter().find(|p| p.name == "visible").unwrap();
    assert!(!visible.private, "visible should be public");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fabula-dsl parse_private_pattern`
Expected: FAIL — `private` not recognized as valid syntax.

- [ ] **Step 3: Add `private` field to PatternAst**

In `crates/fabula-dsl/src/ast.rs`, add to `PatternAst`:

```rust
    pub private: bool,
```

Set default to `false` wherever `PatternAst` is constructed in the parser.

- [ ] **Step 4: Parse `private` keyword**

In `crates/fabula-dsl/src/parser.rs`, in the pattern parsing logic (where `TokenKind::Pattern` is matched), add detection for a leading `private` keyword.

The `private` keyword is not a dedicated token — it's parsed as an `Ident`. Before the `TokenKind::Pattern` match, check:

```rust
TokenKind::Ident(ref s) if s == "private" => {
    self.advance(); // consume "private"
    // Expect "pattern" keyword next
    let mut pat = self.parse_pattern()?;
    pat.private = true;
    // ... handle appropriately in the document parsing loop
}
```

The exact integration depends on the parser structure. Read the `parse_document()` method to find where `TokenKind::Pattern` is dispatched and add the `private` handling there. The `parse_pattern()` method handles the `pattern name { ... }` part; the `private` keyword comes before it.

- [ ] **Step 5: Set `private` in compiler**

In `crates/fabula-dsl/src/compiler.rs`, in `compile_pattern_with()` or `compile_pattern()`, after constructing the Pattern, add:

```rust
    pattern.private = ast.private;
```

Also update `compile_pattern_body_with()` if it constructs a PatternAst internally — ensure `private` defaults to `false`.

- [ ] **Step 6: Run test**

Run: `cargo test -p fabula-dsl parse_private_pattern`
Expected: PASS

Run: `cargo test -p fabula-dsl` and `cargo clippy -p fabula-dsl -- -D warnings`
Expected: All pass, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/fabula-dsl/src/
git commit -m "DSL: 'private pattern' syntax — suppresses pattern from output"
```

---

## Task 6: DSL Integration Test — End-to-End

**Files:**
- Test: `crates/fabula-dsl/tests/integration.rs`

- [ ] **Step 1: Write end-to-end DSL test combining both features**

```rust
#[test]
fn private_pattern_with_nonexclusive_choice() {
    let src = r#"
        private pattern setup { stage e1 { e1.type = "setup" } }
        pattern action_a { stage e1 { e1.type = "action_a" } }
        pattern action_b { stage e1 { e1.type = "action_b" } }
        compose options = action_a | action_b nonexclusive
    "#;
    let doc = fabula_dsl::parse_document(src).unwrap();

    // setup is private
    let setup = doc.patterns.iter().find(|p| p.name == "setup").unwrap();
    assert!(setup.private);

    // choice alternatives are non-exclusive (no group)
    let choices: Vec<_> = doc.patterns.iter()
        .filter(|p| p.name.starts_with("options_"))
        .collect();
    assert_eq!(choices.len(), 2);
    for c in &choices {
        assert_eq!(c.group, None, "non-exclusive should have no group");
        assert!(!c.private, "choice alternatives should not inherit private");
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p fabula-dsl private_pattern_with_nonexclusive`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/fabula-dsl/tests/integration.rs
git commit -m "Test: end-to-end DSL test for private + non-exclusive choice"
```

---

## Task 7: Full Verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p fabula -p fabula-dsl -p fabula-memory -p fabula-petgraph \
           -p fabula-narratives -p fabula-examples
```

Expected: All pass, zero failures.

- [ ] **Step 2: Clippy**

```bash
cargo clippy -p fabula -p fabula-dsl -- -D warnings
```

Expected: Zero warnings.

- [ ] **Step 3: Doc build**

```bash
cd docs && npm run build
```

Expected: Build succeeds (no doc changes needed — these are API additions).

- [ ] **Step 4: Update reference docs**

Add to `docs/docs/reference/patterns.md`:
- `private: bool` field on Pattern struct
- `PatternBuilder::private()` method

Add to `docs/docs/reference/dsl.md`:
- `private pattern name { ... }` syntax
- `compose name = a | b nonexclusive` syntax

- [ ] **Step 5: Commit**

```bash
git add docs/docs/reference/patterns.md docs/docs/reference/dsl.md
git commit -m "Docs: reference updates for private patterns and non-exclusive choice"
```
