---
title: DSL Reference
description: Complete syntax reference for the fabula pattern and graph DSL
---

# DSL Reference

Fabula provides a text DSL for defining patterns and graphs. The DSL compiles to the same types as the Rust builder API — every construct maps 1:1.

## Pattern Syntax

```
pattern <name> {
  stage <event_var> {
    <clause>+
  }+

  [unless between <start_var> <end_var> { <clause>+ }]*
  [unless after <start_var> { <clause>+ }]*
  [unless { <clause>+ }]*
  [temporal <left_var> <relation> <right_var>]*
  [meta("<key>", "<value>")]*
  [deadline <ticks>]
}
```

### Sources

The left side of the dot identifies which node to query edges from. There are three kinds:

| Syntax | Meaning | Example |
|--------|---------|---------|
| `e1.label` | **Stage anchor** — the node this stage matches against | `e1.eventType = "enter"` inside `stage e1 { ... }` |
| `alice.label` | **Literal node** — a specific named node in the graph | `alice.trait = "impulsive"` |
| `?char.label` | **Bound variable** — must have been bound by `-> ?char` earlier | `?char.trait = "impulsive"` after `e1.actor -> ?char` |

Stage anchors do not need `?` — they are implicitly variables within their stage. Bare identifiers that are not the stage anchor are treated as literal node names. Use `?` to reference a bound variable.

Using `?` on an unbound variable is a compile error:

```
stage e1 {
  e1.eventType = "betray"
  ?char.trait = "impulsive"  // ERROR: ?char not yet bound
}
```

### Clauses

| Syntax | Meaning | Builder equivalent |
|--------|---------|-------------------|
| `e1.label = "value"` | Literal string match (stage anchor) | `.edge("e1", label, Str(value))` |
| `?char.label = "value"` | Match on bound variable | `.edge("char", label, Str(value))` |
| `alice.label = "value"` | Match on literal node | `.edge("alice", label, Str(value))` |
| `source.label = 42` | Literal number match | `.edge(source, label, Num(42.0))` |
| `source.label = true` | Literal boolean match | `.edge(source, label, Bool(true))` |
| `source.label -> ?var` | Bind target to variable | `.edge_bind(source, label, var)` |
| `source.label -> node` | Match node reference | `.edge(source, label, Node(node))` |
| `source.label < 0.5` | Value constraint (Lt) | `.edge_constrained(source, label, Lt(0.5))` |
| `source.label > 10` | Value constraint (Gt) | `.edge_constrained(source, label, Gt(10))` |
| `source.label <= 100` | Value constraint (Lte) | `.edge_constrained(source, label, Lte(100))` |
| `source.label >= 0` | Value constraint (Gte) | `.edge_constrained(source, label, Gte(0))` |
| `! source.label = "value"` | Negated clause (literals/refs only) | `.not_edge(source, label, value)` |

Negation (`!`) works with literal values (`= "value"`, `= 42`, `= true`) and node references (`-> node`). It is **not** supported with value constraints (`<`, `>`, `<=`, `>=`) or variable bindings (`-> ?var`) — rewrite as the inverse constraint instead (e.g., `! e.x < 0.5` becomes `e.x >= 0.5`).

### Negation Windows

| Syntax | Meaning |
|--------|---------|
| `unless between e1 e3 { ... }` | Clauses must NOT match between e1 and e3 |
| `unless after e1 { ... }` | Clauses must NOT match after e1 (open-ended) |
| `unless { ... }` | Global negation (first stage to last stage) |

### Temporal Constraints

```
temporal e1 before e2                   // qualitative only
temporal e1 before e2 gap 3..10         // gap in [3, 10]
temporal e1 before e2 gap ..10          // gap in [0, 10]
temporal e1 before e2 gap 3..           // gap in [3, infinity)
temporal e1 during e2 gap 5..50         // start margin in [5, 50]
```

All 13 Allen relations are supported: `before`, `after`, `meets`, `met_by`, `overlaps`, `overlapped_by`, `during`, `contains`, `starts`, `started_by`, `finishes`, `finished_by`, `equals`.

The optional `gap` keyword adds a metric bound (STN-style bounded difference constraint). The meaning of "gap" depends on the Allen relation — for `before` it's the separation between end(A) and start(B); for `during` it's the start margin; for `overlaps` it's the overlap duration. See the [Temporal Model](/docs/concepts/temporal-model) for details.

### Metadata

Attach key-value metadata to a pattern with `meta`. Metadata is propagated to `Match`, `SiftEvent`, and scored match types. Place `meta` directives inside the pattern body, alongside stages, negations, and temporal constraints.

```
pattern betrayal {
  stage e1 {
    e1.eventType = "betray"
    e1.actor -> ?char
  }
  meta("thread_type", "conflict")
  meta("priority", "high")
}
```

Multiple `meta` directives are allowed. If the same key appears twice, the last value wins. Maps to `PatternBuilder::metadata(key, value)`.

### Deadline

Set a tick deadline for partial match expiration with `deadline`. If a partial match has been alive for more than this many ticks without completing, it is killed with `SiftEvent::Expired` during `end_tick()`.

```
pattern time_sensitive {
  stage e1 { e1.type = "offer" }
  stage e2 { e2.type = "accept" }
  deadline 10
}
```

The deadline must be a positive integer (>= 1). The compiler rejects `deadline 0` and negative values. Maps to `PatternBuilder::deadline(ticks)`.

## Graph Syntax

```
graph {
  @<timestamp> <source>.<label> = <value>
  @<timestamp> <source>.<label> -> <node>
  @<start>..<end> <source>.<label> = <value>
  now = <number>
}
```

### Edge Types

| Syntax | Value Type |
|--------|-----------|
| `@1 e.type = "enter"` | String |
| `@1 e.score = 42.5` | Number |
| `@1 e.active = true` | Boolean |
| `@1 e.actor -> alice` | Node reference |
| `@1..10 e.type = "siege"` | Bounded interval `[1, 10)` |

Edges without `..` create open-ended intervals `[start, infinity)`.

### `now`

The `now = <number>` statement sets the graph's current time. If omitted, the playground uses `max(timestamps) + 1`.

## Comments

Line comments start with `//`:

```
// This is a comment
pattern test {
  stage e1 {
    e1.type = "hello" // end-of-line comment
  }
}
```

## Complete Example

```
pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest      // joins on ?guest from e1
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host        // same host as e2
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest     // same guest — if they leave, no violation
  }
}

graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  @3 e3.eventType = "harm"
  @3 e3.actor -> bob
  @3 e3.target -> alice
  now = 10
}
```

### Variable source example

This pattern uses `?char` to check a property on a bound variable:

```
pattern two_impulsive_betrayals {
  stage e1 {
    e1.eventType = "betray"
    e1.actor -> ?char           // bind ?char
    ?char.trait = "impulsive"   // follow ?char, check its trait
  }
  stage e2 {
    e2.eventType = "betray"
    e2.actor -> ?char           // same character betrays again
  }
  unless {
    mid.eventType = "reconcile"
    mid.actor -> ?char          // mid is a scan root (no ?), ?char is a target binding
  }
}
```

Note that `mid` in the negation block has no `?` — it is a scan root (the engine searches for any node matching the clauses). But `?char` on the right side of `->` references the bound variable from the parent pattern.

Try this in the [Pattern Playground](/docs/playground/pattern-playground).

## Compose Syntax

Compose directives combine named patterns into larger patterns. Three operators are supported.

```
compose <name> = <pattern_a> >> <pattern_b> sharing(<var>, ...)   // sequence
compose <name> = <pattern_a> | <pattern_b> | <pattern_c>         // exclusive choice
compose <name> = <pattern> * <count> sharing(<var>, ...)          // repeat
```

### Sequence (`>>`)

Creates a new pattern whose stages are `A`'s stages followed by `B`'s stages. Variables listed in `sharing(...)` are joined across the two patterns.

```
pattern setup {
  stage e1 { e1.eventType = "promise"  e1.actor -> ?char }
}
pattern payoff {
  stage e2 { e2.eventType = "fulfill"  e2.actor -> ?char }
}

compose promise_kept = setup >> payoff sharing(char)
```

### Choice (`|`)

Registers all alternatives as separate patterns with a shared `group`. When one alternative completes, the engine kills active PMs for all sibling alternatives (exclusive choice).

```
compose crisis = war | famine | plague
```

### Repeat (`*`)

Creates a sequence of N copies of the same pattern. Variables listed in `sharing(...)` are joined across all copies (the same actor in each repetition).

```
compose three_strikes = offense * 3 sharing(offender)
```

### Rules

- All referenced patterns must be defined before the compose directive (no forward references).
- `sharing(...)` is required for sequence and repeat when you want cross-pattern variable joins. Omit it for independent patterns.
- Compose chains work: `compose ab = a >> b` then `compose abc = ab >> c`.
- Variables are automatically renamed to avoid collisions (e.g., `e1` becomes `e1_0`, `e1_1`).

---

## TypeMapper

By default, the DSL compiles patterns to `Pattern<String, MemValue>`. The `TypeMapper` trait lets you compile directly to a different type system.

### `TypeMapper` trait

```rust
pub trait TypeMapper {
    type L: Clone + Debug;  // label type
    type V: Clone + Debug;  // value type

    fn label(&self, s: &str) -> Result<Self::L, String>;
    fn string_value(&self, s: &str) -> Result<Self::V, String>;
    fn num_value(&self, n: f64) -> Result<Self::V, String>;
    fn bool_value(&self, b: bool) -> Result<Self::V, String>;
    fn node_ref(&self, name: &str) -> Result<Self::V, String>;
}
```

All methods return `Result` to support fallible mappings (e.g., looking up a label in a predicate registry).

### `MemMapper`

The default mapper, producing `Pattern<String, MemValue>`. Used by `parse_pattern()` and `parse_document()`.

### Custom mappers

```rust
use fabula_dsl::{TypeMapper, parse_pattern_with};

struct WkMapper { labels: HashMap<String, u32> }

impl TypeMapper for WkMapper {
    type L = u32;
    type V = paracausality::Value;

    fn label(&self, s: &str) -> Result<u32, String> {
        self.labels.get(s).copied()
            .ok_or_else(|| format!("unknown predicate '{}'", s))
    }
    fn string_value(&self, s: &str) -> Result<Value, String> { Ok(Value::Str(s.into())) }
    fn num_value(&self, n: f64) -> Result<Value, String> { Ok(Value::Num(n)) }
    fn bool_value(&self, b: bool) -> Result<Value, String> { Ok(Value::Bool(b)) }
    fn node_ref(&self, name: &str) -> Result<Value, String> { Ok(Value::Entity(name.parse()?)) }
}

let pattern = parse_pattern_with("pattern test { stage e { e.type = \"harm\" } }", &WkMapper { .. })?;
// pattern is Pattern<u32, Value>
```

### `ParsedDocument<L, V>`

`parse_document()` returns `ParsedDocument` (defaults to `<String, MemValue>`). `parse_document_with(input, &mapper)` returns `ParsedDocument<M::L, M::V>`.

```rust
pub struct ParsedDocument<L = String, V = MemValue> {
    pub patterns: Vec<Pattern<L, V>>,
    pub graphs: Vec<MemGraph>,   // always MemGraph (graphs are test-only)
}
```

### Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `parse_pattern(input)` | `Result<Pattern<String, MemValue>>` | Parse a single pattern with `MemMapper`. |
| `parse_pattern_with(input, mapper)` | `Result<Pattern<M::L, M::V>>` | Parse a single pattern with a custom mapper. |
| `parse_graph(input)` | `Result<MemGraph>` | Parse a graph definition. |
| `parse_document(input)` | `Result<ParsedDocument>` | Parse a full document (patterns + graphs + composes) with `MemMapper`. |
| `parse_document_with(input, mapper)` | `Result<ParsedDocument<M::L, M::V>>` | Parse a full document with a custom mapper. |
| `compile_pattern_body(name, body)` | `Result<Pattern<String, MemValue>>` | Compile a `PatternBody` (from `parse_pattern_body()`) with a name, using `MemMapper`. |
| `compile_pattern_body_with(name, body, mapper)` | `Result<Pattern<M::L, M::V>>` | Compile a `PatternBody` with a custom mapper. |

---

## Composable Parser API

The parser exposes entry points for downstream DSLs that embed fabula pattern syntax within their own blocks. This lets a salience-DSL or storylet-DSL tokenize with fabula's lexer, delegate pattern parsing to fabula's parser, and resume parsing its own syntax afterward.

### `PatternBody`

The interior of a pattern — stages, negations, temporal constraints, metadata, and deadline — without the `pattern name { }` wrapper.

```rust
pub struct PatternBody {
    pub stages: Vec<StageAst>,
    pub negations: Vec<NegationAst>,
    pub temporals: Vec<TemporalAst>,
    pub metadata: Vec<(String, String)>,
    pub deadline: Option<f64>,
}
```

### Parser methods

#### `Parser::parse_pattern_body`

Parse the body of a pattern — stages, negations, temporal constraints, metadata, and deadline — without the `pattern name { }` wrapper. Stops when it sees `}` or EOF but does NOT consume the closing brace.

This is the primary composability entry point for downstream DSLs.

```rust
pub fn parse_pattern_body(&mut self) -> Result<PatternBody, ParseError>
```

#### `Parser::from_tokens_at`

Create a parser starting at a specific position in a token stream. Use this to resume parsing from where a previous parser left off.

```rust
pub fn from_tokens_at(tokens: Vec<Token>, pos: usize) -> Self
```

#### `Parser::pos`

Current cursor position in the token stream.

```rust
pub fn pos(&self) -> usize
```

#### `Parser::into_inner`

Consume the parser, returning the token stream and cursor position. Use this to recover the tokens after parsing a sub-section.

```rust
pub fn into_inner(self) -> (Vec<Token>, usize)
```

### Downstream usage example

```rust
use fabula_dsl::lexer::Lexer;
use fabula_dsl::parser::Parser;
use fabula_dsl::compiler::compile_pattern_body_with;

// 1. Tokenize everything with fabula's lexer
let tokens = Lexer::new(storylet_source).tokenize()?;
let mut parser = Parser::new(tokens);

// 2. Parse your own DSL syntax up to the embedded pattern body
//    (using parser.peek(), parser.advance(), parser.expect(), etc.)

// 3. Hand off to fabula's parser for the pattern body
let body = parser.parse_pattern_body()?;
parser.expect(TokenKind::RBrace)?; // consume closing brace

// 4. Compile the pattern body with a name
let pattern = compile_pattern_body_with("my_pattern", &body, &my_mapper)?;

// 5. Resume parsing your own DSL syntax
//    The parser cursor is right after the pattern body
```

All parsing primitives (`parse_stage`, `parse_negation`, `parse_temporal`, `parse_clause`, `peek`, `advance`, `at_eof`, `check`, `expect`, `expect_ident`, `error`) are public for fine-grained control.
