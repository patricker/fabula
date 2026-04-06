---
sidebar_position: 1
title: Getting Started
---

# Getting Started

**Learning objective:** Build and evaluate a temporal graph pattern using fabula in under 10 minutes, starting from an empty Rust project.

| | |
|---|---|
| **Time** | ~10 minutes |
| **Difficulty** | Beginner |
| **Prerequisites** | Rust 1.74+, cargo |

:::note Not using Rust?
Fabula has [WebAssembly bindings](/docs/guides/language-integration) for JavaScript/TypeScript,
and the interactive [Playground](/docs/playground/pattern-playground) requires no installation at all.
See the [Language Integration](/docs/guides/language-integration) guide for Python, C, and game engine options.
:::

You will build a pattern that detects a suspicious login: a user logs in from one location, then logs in from a *different* location within a short time, with no logout between. By the end, you will run the pattern in both batch and incremental mode and see the results.

---

## Step 1: Create the project

Create a new Rust binary project and add fabula as a dependency.

```bash
cargo new fabula-demo
cd fabula-demo
```

Open `Cargo.toml` and add the two crates you need — the core library and the in-memory graph adapter:

```toml
[dependencies]
fabula = "0.1"
fabula-memory = "0.1"
```

:::tip
If you are working from a local checkout of the fabula repository, use path dependencies instead:

```toml
[dependencies]
fabula = { path = "../fabula/crates/fabula" }
fabula-memory = { path = "../fabula/crates/fabula-memory" }
```
:::

Build to confirm everything resolves:

```bash
cargo build
```

Expected output:

```text
   Compiling fabula v0.1.0
   Compiling fabula-memory v0.1.0
   Compiling fabula-demo v0.1.0 (/path/to/fabula-demo)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s
```

---

## How events become edges

Before writing code, understand how fabula models data. Every event in your system maps to one or more **edges** in a temporal graph:

| Your data | Fabula concept | Example |
|-----------|---------------|---------|
| Event ID | Source node | `"ev1"` |
| Event type | Edge label | `"login"` |
| Related entity | Edge target (node ref) | `"alice"` |
| Property value | Edge target (string/number) | `"Seattle"` |
| Timestamp | Interval start | `1` |

A single event like `{id: "ev1", type: "login", user: "alice", location: "Seattle", time: 1}` becomes three edges:

```text
ev1 --[type]--> "login"      @ time 1
ev1 --[user]--> alice         @ time 1
ev1 --[location]--> "Seattle" @ time 1
```

This edge-based model works for any domain: game events, audit logs, network telemetry, process steps.

---

## Step 2: Build a graph

Replace the contents of `src/main.rs` with the code below. This creates a small temporal graph that models login/logout events:

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn main() {
```

```rust reference file=tests/getting_started.rs#graph_setup
```

```rust
    println!("Graph has {} edges", graph.edge_count());
}
```

Each call to `add_str` creates an edge from a source node to a string value with a label and a start time. Each `add_ref` creates an edge to another node (a traversable reference). Together they form event nodes with properties:

| Edge | Meaning |
|---|---|
| `login1 --[type]--> "login"` | This event is a login. |
| `login1 --[user]--> @alice` | The user is Alice (a node reference, so we can join on it later). |
| `login1 --[location]--> "new_york"` | The login came from New York. |

Run the program:

```bash
cargo run
```

Expected output:

```text
Graph has 15 edges
```

---

## Step 3: Define a pattern

Now add the pattern definition after the graph setup, before the final `println!`:

```rust reference file=tests/getting_started.rs#build_pattern
```

Here is what each piece does:

- **Stage `login_a`**: Finds an event with `type = "login"`, binds its `user` edge to the variable `user`, and binds its `location` edge to `loc_a`.
- **Stage `login_b`**: Finds a *later* event (stages are implicitly time-ordered) that is also a login. The `user` variable is already bound from stage 1, so this clause acts as a **join** — it only matches if the same user appears.  The `location` binds to `loc_b`.
- **`unless_between`**: Between `login_a` and `login_b`, there must be no event with `type = "logout"` whose `user` matches the bound `user` variable. If such an event exists, the match is killed.

:::tip DSL alternative
The same pattern in fabula's text DSL:

```fabula
pattern suspicious_login {
  stage e1 {
    e1.type = "login"
    e1.user -> ?user
    e1.location -> ?loc_a
  }
  stage e2 {
    e2.type = "login"
    e2.user -> ?user
    e2.location -> ?loc_b
  }
  unless between e1 e2 {
    logout.type = "logout"
    logout.user -> ?user
  }
  temporal e1 before e2
}
```

The Rust `PatternBuilder` API and the text DSL produce equivalent patterns that match the same events. Use whichever fits your workflow — the DSL is often easier for designers and configuration files, while the builder API integrates naturally into Rust code. See the [DSL Reference](/docs/reference/dsl) for full syntax.
:::

:::tip
Want to experiment without a Rust project? Try this pattern in the [Pattern Playground](/docs/playground/pattern-playground).
:::

Run:

```bash
cargo run
```

Expected output:

```text
Graph has 15 edges
Pattern 'suspicious_login' has 2 stages
```

---

## Step 4: Evaluate in batch

Batch evaluation scans the entire graph and returns all complete matches. Add this after the pattern definition:

```rust reference file=tests/getting_started.rs#batch_eval
```

Run:

```bash
cargo run
```

Expected output:

```text
Graph has 15 edges
Pattern 'suspicious_login' has 2 stages

=== Batch results: 1 match(es) ===
  Pattern: suspicious_login
    user = Node("alice")
    login_a = Node("login1")
    loc_a = Value(Str("new_york"))
    login_b = Node("login2")
    loc_b = Value(Str("tokyo"))
```

:::note
The variable ordering in the bindings map is non-deterministic (it is a `HashMap`). Your output may list the variables in a different order. The values will be the same.
:::

Alice matched because she logged in from New York (time 1) and then from Tokyo (time 3) with no logout between. Bob did *not* match — even though he logged in from two different locations (London and Paris), he logged out at time 4 between those logins, so the negation clause killed the match.

---

## Step 5: Evaluate incrementally

Incremental evaluation tracks partial matches as edges arrive one at a time, emitting events when patterns advance, complete, or get killed by negation. This is how you use fabula in a live simulation or event stream.

Replace the batch evaluation section with the code below (or add it alongside):

```rust reference file=tests/getting_started.rs#incremental_eval
```

Run:

```bash
cargo run
```

Expected output (abbreviated — the exact match IDs may vary):

```text
=== Incremental replay ===

-- Alice logs in from New York (t=1) --
  Advanced { pattern: "suspicious_login", match_id: 0, stage_index: 0 }

-- Bob logs in from London (t=2) --
  Advanced { pattern: "suspicious_login", match_id: 1, stage_index: 0 }

-- Alice logs in from Tokyo (t=3) --
  Advanced { pattern: "suspicious_login", match_id: 2, stage_index: 0 }
  Completed { pattern: "suspicious_login", match_id: 3, bindings: {"user": Node("alice"), ...} }

-- Bob logs out (t=4) --
  Negated { pattern: "suspicious_login", match_id: 1, clause_label: "type", trigger_source: "logout1" }

-- Bob logs in from Paris (t=5) --
  Advanced { pattern: "suspicious_login", match_id: 4, stage_index: 0 }
```

Walk through what happened:

1. **Alice logs in (t=1)**: The engine creates a partial match with stage 0 satisfied — `Advanced`.
2. **Bob logs in (t=2)**: A second partial match starts for Bob — `Advanced`.
3. **Alice logs in from Tokyo (t=3)**: The engine advances Alice's partial match to stage 1 and finds a complete match — `Completed`. Alice's bindings show `user = alice`, `loc_a = new_york`, `loc_b = tokyo`.
4. **Bob logs out (t=4)**: The logout edge matches the negation clause for Bob's partial match — `Negated`. Bob's partial match is killed.
5. **Bob logs in from Paris (t=5)**: A new partial match starts (stage 0), but Bob's earlier partial match is already dead. No completion.

---

## Complete example

Here is the full `src/main.rs` with both batch and incremental evaluation:

```rust
use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

fn main() {
    // -- Build the event graph --
    let mut graph = MemGraph::new();

    graph.add_str("login1", "type", "login", 1);
    graph.add_ref("login1", "user", "alice", 1);
    graph.add_str("login1", "location", "new_york", 1);

    graph.add_str("login2", "type", "login", 3);
    graph.add_ref("login2", "user", "alice", 3);
    graph.add_str("login2", "location", "tokyo", 3);

    graph.add_str("login3", "type", "login", 2);
    graph.add_ref("login3", "user", "bob", 2);
    graph.add_str("login3", "location", "london", 2);

    graph.add_str("logout1", "type", "logout", 4);
    graph.add_ref("logout1", "user", "bob", 4);

    graph.add_str("login4", "type", "login", 5);
    graph.add_ref("login4", "user", "bob", 5);
    graph.add_str("login4", "location", "paris", 5);

    graph.set_time(10);
    println!("Graph has {} edges", graph.edge_count());

    // -- Define the pattern --
    let pattern = PatternBuilder::<String, MemValue>::new("suspicious_login")
        .stage("login_a", |s| s
            .edge("login_a", "type".into(), MemValue::Str("login".into()))
            .edge_bind("login_a", "user".into(), "user")
            .edge_bind("login_a", "location".into(), "loc_a"))
        .stage("login_b", |s| s
            .edge("login_b", "type".into(), MemValue::Str("login".into()))
            .edge_bind("login_b", "user".into(), "user")
            .edge_bind("login_b", "location".into(), "loc_b"))
        .unless_between("login_a", "login_b", |neg| neg
            .edge("logout_evt", "type".into(), MemValue::Str("logout".into()))
            .edge_bind("logout_evt", "user".into(), "user"))
        .build();

    println!("Pattern '{}' has {} stages", pattern.name, pattern.stages.len());

    // -- Batch evaluation --
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern);

    let matches = engine.evaluate(&graph);
    println!("\n=== Batch results: {} match(es) ===", matches.len());
    for m in &matches {
        println!("  Pattern: {}", m.pattern);
        for (var, val) in &m.bindings {
            println!("    {} = {:?}", var, val);
        }
    }

    // -- Incremental evaluation --
    let pattern = PatternBuilder::<String, MemValue>::new("suspicious_login")
        .stage("login_a", |s| s
            .edge("login_a", "type".into(), MemValue::Str("login".into()))
            .edge_bind("login_a", "user".into(), "user")
            .edge_bind("login_a", "location".into(), "loc_a"))
        .stage("login_b", |s| s
            .edge("login_b", "type".into(), MemValue::Str("login".into()))
            .edge_bind("login_b", "user".into(), "user")
            .edge_bind("login_b", "location".into(), "loc_b"))
        .unless_between("login_a", "login_b", |neg| neg
            .edge("logout_evt", "type".into(), MemValue::Str("logout".into()))
            .edge_bind("logout_evt", "user".into(), "user"))
        .build();

    let mut inc_engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    inc_engine.register(pattern);

    let mut inc_graph = MemGraph::new();
    inc_graph.set_time(10);

    let mut add_event = |graph: &mut MemGraph,
                          engine: &mut SiftEngineFor<MemGraph>,
                          id: &str, typ: &str, user: &str,
                          extra_label: &str, extra_val: &str,
                          t: i64| {
        graph.add_str(id, "type", typ, t);
        graph.add_ref(id, "user", user, t);
        let interval = fabula::interval::Interval::open(t);

        let mut events = Vec::new();
        events.extend(engine.on_edge_added(
            graph, &id.to_string(), &"type".to_string(),
            &MemValue::Str(typ.into()), &interval,
        ));
        events.extend(engine.on_edge_added(
            graph, &id.to_string(), &"user".to_string(),
            &MemValue::Node(user.into()), &interval,
        ));

        if !extra_label.is_empty() {
            graph.add_str(id, extra_label, extra_val, t);
            events.extend(engine.on_edge_added(
                graph, &id.to_string(), &extra_label.to_string(),
                &MemValue::Str(extra_val.into()), &interval,
            ));
        }

        for evt in &events {
            println!("  {:?}", evt);
        }
    };

    println!("\n=== Incremental replay ===");

    println!("\n-- Alice logs in from New York (t=1) --");
    add_event(&mut inc_graph, &mut inc_engine,
              "login1", "login", "alice", "location", "new_york", 1);

    println!("\n-- Bob logs in from London (t=2) --");
    add_event(&mut inc_graph, &mut inc_engine,
              "login3", "login", "bob", "location", "london", 2);

    println!("\n-- Alice logs in from Tokyo (t=3) --");
    add_event(&mut inc_graph, &mut inc_engine,
              "login2", "login", "alice", "location", "tokyo", 3);

    println!("\n-- Bob logs out (t=4) --");
    add_event(&mut inc_graph, &mut inc_engine,
              "logout1", "logout", "bob", "", "", 4);

    println!("\n-- Bob logs in from Paris (t=5) --");
    add_event(&mut inc_graph, &mut inc_engine,
              "login4", "login", "bob", "location", "paris", 5);
}
```

---

## What you learned

- **Patterns** are built from stages (ordered event matches) with variable joins and negation windows, using the `PatternBuilder` API.
- **Batch evaluation** (`engine.evaluate`) scans the entire graph and returns all complete matches at once.
- **Incremental evaluation** (`engine.on_edge_added`) tracks partial matches in real time, emitting `SiftEvent::Advanced`, `SiftEvent::Completed`, and `SiftEvent::Negated` as edges arrive.

## Next steps

- [Concepts overview](concepts/overview) — understand the core model (edges, patterns, intervals, negation) in depth.
- [Pattern cookbook](guides/pattern-cookbook) — worked recipes for common pattern types: repeated behavior, numeric thresholds, overlapping events, absence detection.
- [Pattern reference](reference/patterns) — full API details for `Pattern`, `Stage`, `Clause`, and `PatternBuilder`.
