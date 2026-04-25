---
title: DSL Reference
description: Complete syntax reference for the fabula pattern and graph DSL
---

# DSL Reference

Fabula provides a text DSL for defining patterns and graphs. The DSL compiles to the same types as the Rust builder API — every construct maps 1:1.

## Pattern Syntax

```
[private] pattern <name> [importance <weight>] {
  stage <event_var> {
    <clause>+
  }+

  [concurrent {
    stage <event_var> { <clause>+ }
    stage <event_var> { <clause>+ }
    ...
  }]*

  [unless between <start_var> <end_var> { <clause>+ }]*
  [unless after <start_var> { <clause>+ }]*
  [unless { <clause>+ }]*
  [temporal <left_var> <relation> <right_var>]*
  [meta("<key>", "<value>")]*
  [deadline <ticks>]
}
```

### Private patterns

Prefix a pattern with `private` to suppress its matches and events from engine output. The engine still evaluates the pattern internally — useful for composition building blocks.

```
private pattern helper {
  stage e1 { e1.type = "setup" }
}
pattern main_flow {
  stage e1 { e1.type = "action" }
}
compose full = helper >> main_flow sharing(...)
```

Only `full` matches appear in output. `helper` matches are suppressed.

### Importance

Set a pattern's importance weight with `importance <weight>` after the pattern name. Higher values cause the pattern's matches to be weighted more heavily in narrative scoring (via `assemble_signals_weighted()` in fabula-narratives). Default is `1.0`.

```
pattern climax importance 10.0 {
  stage e1 { e1.type = "final_battle" }
}
```

Maps to `PatternBuilder::importance(weight)`.

### `advance_in_place`

Pattern-level modifier. Written as a keyword preceding `pattern`:

```
advance_in_place pattern my_pattern {
    stage a { a.type = "enter" }
    stage b { b.type = "leave" }
}
```

Equivalent to calling `PatternBuilder::advance_in_place()` from Rust. See the [patterns reference](./patterns#advance_in_place) for full semantics.

Composes with `private`: both keywords may appear in any order before `pattern`.

```
private advance_in_place pattern internal_fast { ... }
advance_in_place private pattern internal_fast { ... }
```

:::note
`inactivity_threshold` (auto-prune PMs after N idle ticks) has no DSL syntax. Set it via the builder API: `PatternBuilder::inactivity_threshold(ticks)`. See [Pattern reference](/reference/patterns#inactivity_threshold).
:::

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
| `source.label > ?var` | Cross-stage comparison (Gt) — target must be greater than the value bound to `?var` | `.edge_gt_var(source, label, "var")` |
| `source.label < ?var` | Cross-stage comparison (Lt) | `.edge_lt_var(source, label, "var")` |
| `source.label = ?var` | Cross-stage comparison (Eq) — target must equal bound value (not a binding — use `->` to bind) | `.edge_eq_var(source, label, "var")` |
| `source.label >= ?var` | Cross-stage comparison (Gte) | `.edge_gte_var(source, label, "var")` |
| `source.label <= ?var` | Cross-stage comparison (Lte) | `.edge_lte_var(source, label, "var")` |
| `source.label in ["a", "b"]` | Value disjunction (OneOf) | `.edge_one_of(source, label, vec![...])` |
| `! source.label in ["a", "b"]` | Negated disjunction | `.not_edge_one_of(source, label, vec![...])` |
| `! source.label = "value"` | Negated clause (literals/refs only) | `.not_edge(source, label, value)` |

**Important**: `= ?var` (equality comparison against a bound variable's value) is distinct from `-> ?var` (bind or join). Use `=` when you want to compare a numeric/string value against a previously bound value. Use `->` when you want to traverse and bind a node reference.

Negation (`!`) works with literal values (`= "value"`, `= 42`, `= true`), node references (`-> node`), and value disjunctions (`in [...]`). It is **not** supported with value constraints (`<`, `>`, `<=`, `>=`), variable comparisons (`> ?var`), or variable bindings (`-> ?var`) — rewrite as the inverse constraint instead (e.g., `! e.x < 0.5` becomes `e.x >= 0.5`).

**Value disjunction** (`in [...]`): Match an edge target against any value in a list. Supports strings, numbers, booleans, and node references. Elements are separated by commas.

```
// Match any harmful event type
e1.eventType in ["attack", "betray", "steal"]

// Negated: target must NOT be any of the listed values
! e1.status in ["resolved", "dismissed"]
```

### Computed Bindings (`let`)

Declare a derived variable from already-bound variables and literals. The let attaches to the most recently parsed `stage` block and evaluates after that stage's clauses match.

```fab
pattern arrival_with_deadline {
    stage e1 {
        e1.type = "world"
        e1.pulse_count -> ?ts
    }
    let deadline = ?ts + 5
    stage e2 {
        e2.type = "world"
        e2.pulse_count = ?deadline
    }
}
```

#### Grammar

```
let_stmt = "let" ident "=" expr
expr     = term { ("+" | "-") term }*
term     = factor { ("*" | "/") factor }*
factor   = number | string | "?" ident | "(" expr ")"
```

`*` and `/` bind tighter than `+` and `-`. Parentheses override precedence. All operators are left-associative.

#### Rules

- A let must follow at least one `stage` block within the pattern body. A let before any stage is a parse error.
- Every `?var` reference must already be bound -- by an earlier stage's clauses (`-> ?var`), by the let's own stage's clauses (the let evaluates after the stage's clauses match), or by an earlier let on the same stage.
- A let cannot reuse the name of any already-bound variable (no shadowing).
- If the expression cannot be evaluated at runtime (unbound var, type mismatch, division by zero), the enclosing stage match fails silently -- the same behavior as any other unsatisfied clause.

See also: [Computed Bindings how-to guide](/guides/computed-bindings).

### Concurrent Groups

Stages inside a `concurrent { }` block can match in any order. The engine tracks which stages in the group have been satisfied using a bitmask and advances the pattern past the group when all stages are matched.

```
pattern multi_signal_alert {
  stage e1 {
    e1.type = "anomaly_detected"
    e1.sensor -> ?sensor
  }
  concurrent {
    stage e2 {
      e2.type = "temperature_spike"
      e2.sensor -> ?sensor
    }
    stage e3 {
      e3.type = "pressure_drop"
      e3.sensor -> ?sensor
    }
  }
  stage e4 {
    e4.type = "shutdown"
    e4.sensor -> ?sensor
  }
}
```

In this pattern, after `e1` matches, both `e2` and `e3` must match (in either order) before `e4` can match. The shared variable `?sensor` ensures all stages refer to the same sensor.

**Rules:**
- A concurrent group must contain at least 2 stages.
- Stages in the group can be interleaved with stages outside the group (the overall pattern order is: stages before the group, then the group stages in any order, then stages after the group).
- Temporal ordering within a concurrent group is relaxed — stages in the same group are not required to be time-ordered relative to each other.
- `unless between` cannot use two anchors that are both inside the same concurrent group (undefined temporal ordering). The compiler rejects this with an error.
- Variables bound in any stage of a concurrent group are visible to all sibling stages in the same group (symmetric scoping).
- `let` statements cannot appear inside a `concurrent { }` block — only stages are permitted. The Rust API (`PatternBuilder::unordered_group`) does not enforce this; lets attached to grouped stages have order-sensitive semantics. See [Computed Bindings — Concurrent groups](../guides/computed-bindings#concurrent-groups).

Maps to `PatternBuilder::unordered_group()` in the Rust builder API.

---

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
compose <name> = <pattern_a> | <pattern_b> nonexclusive          // non-exclusive choice
compose <name> = <pattern> * <count> sharing(<var>, ...)          // exact repeat
compose <name> = <pattern> * <min>..<max> sharing(<var>, ...)     // repeat range
compose <name> = <pattern> * <min>.. sharing(<var>, ...)          // repeat (unbounded)
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

Add `nonexclusive` after the alternatives to allow all of them to match independently (no group, no mutual kill):

```
compose any_crisis = war | famine | plague nonexclusive
```

### Repeat (`*`)

**Exact repeat** (`* N`): creates a sequence of N copies of the same pattern. Variables listed in `sharing(...)` are joined across all copies (the same actor in each repetition). Each copy gets distinct `repN_` prefixed variable names, so you can inspect individual repetition bindings.

```
compose three_strikes = offense * 3 sharing(offender)
```

**Repeat range** (`* N..M`): matches the sub-pattern at least N times, up to M times. Uses a looping engine with first/last bookends instead of unrolling. The first match binds `first_` prefixed variables, subsequent matches overwrite `last_` prefixed variables. Shared variables persist across all iterations. Completion is emitted at N occurrences; the engine continues matching up to M.

```
compose brute_force = login_fail * 5..10 sharing(account)   // 5 to 10 attempts
compose escalation = price_hike * 3..5 sharing(item)        // 3 to 5 hikes
```

**Unbounded repeat** (`* N..`): matches at least N times with no upper limit. The engine loops indefinitely, emitting a completion at each occurrence >= N.

```
compose persistent = anomaly * 3.. sharing(sensor)   // 3 or more anomalies
```

Bindings available in repeat-range matches:
- `first_*` — variables from the first match (e.g., `first_actor`, `first_e`)
- `last_*` — variables from the most recent match (overwritten each iteration)
- Shared variables — unprefixed, consistent across all iterations
- `repetition_count` — available on the `PartialMatch` for inspection

`let` bindings inside a repeat-range looping segment are re-evaluated on each iteration; non-shared let results do not persist across iterations.

### Rules

- All referenced patterns must be defined before the compose directive (no forward references).
- `sharing(...)` is required for sequence and repeat when you want cross-pattern variable joins. Omit it for independent patterns.
- Compose chains work: `compose ab = a >> b` then `compose abc = ab >> c`.
- Variables are automatically renamed to avoid collisions (e.g., `e1` becomes `e1_0`, `e1_1`).
- Repeat range (`* N..M`, `* N..`) requires `N >= 1`. For bounded ranges, `M >= N`.

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
    pub unordered_groups: Vec<Vec<usize>>,
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

All parsing primitives (`parse_stage`, `parse_negation`, `parse_temporal`, `parse_clause`, `peek`, `advance`, `at_eof`, `check`, `expect`, `expect_ident`, `expect_number`, `error`) are public stable API for downstream DSL consumers.

### Lexer Token Reference

The `Lexer` produces a stream of `Token` values. Downstream DSLs that reuse fabula's lexer (via `Lexer::new(source).tokenize()`) have access to all token types. The tokens used by fabula's own parser are marked; unmarked tokens exist for downstream DSL consumers.

**Keywords**: `Pattern`, `Stage`, `Unless`, `Between`, `After`, `Graph`, `Now`, `Temporal`, `True`, `False`, `Compose`, `Sharing`, `Concurrent`, `In`, `Importance`

**Symbols** (used by fabula):

| Token | Character(s) |
|-------|-------------|
| `LBrace` / `RBrace` | `{` `}` |
| `LParen` / `RParen` | `(` `)` |
| `LBracket` / `RBracket` | `[` `]` |
| `Dot` / `DotDot` | `.` `..` |
| `Arrow` | `->` |
| `Eq` | `=` (also `==`) |
| `Lt` / `Gt` / `Lte` / `Gte` | `<` `>` `<=` `>=` |
| `GtGt` | `>>` |
| `Bang` | `!` |
| `At` | `@` |
| `Question` | `?` |
| `Pipe` | `\|` |
| `Star` | `*` |
| `Comma` | `,` |

**Symbols** (for downstream DSLs — not consumed by fabula's parser):

| Token | Character(s) | Intended use |
|-------|-------------|-------------|
| `Plus` | `+` | Delta semantics (`adjust ?e2.depth + 1`) |
| `Minus` | `-` | Subtraction / negative prefix. Note: `-5` lexes as `Minus, Number(5.0)`, not `Number(-5.0)`. The parser's `expect_number()` handles optional leading `Minus`. |
| `Colon` | `:` | Key-value syntax (`lifecycle: oneshot`) |
| `Semicolon` | `;` | Statement separator |

**Literals**: `Ident(String)`, `String(String)`, `Number(f64)`, `Eof`

**Strings**: Both single-quoted (`"..."`) and triple-quoted (`"""..."""`) strings produce the same `TokenKind::String`. Triple-quoted strings allow newlines and embedded `"` characters — useful for multi-line content blocks like prompt templates.
