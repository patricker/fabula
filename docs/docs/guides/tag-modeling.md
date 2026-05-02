---
sidebar_position: 15
title: Modeling events with tags
description: How to attach polymorphic tags to events for one-pattern-matches-many-types matching.
---

# Modeling events with tags

When a simulation has rich event taxonomies — *attack, betrayal, scolding* are
all *harm* events; *attack, duel* are both *violent*; *betrayal, scolding* are
both *social* — you don't want one pattern per event type. You want one pattern
per tag.

Fabula supports this via a simple convention: **events are nodes, tags are
edges with a chosen label**.

## The convention

For each event:

- **The event is a node** — an opaque id like `"ev1"` or `"event_2026_05_02_143022"`.
- **The event type is one edge**: `event_id --"eventType"--> "attack"`
- **Each tag is an edge** with label `"tag"`: `event_id --"tag"--> "violent"`, etc.

The label name `"tag"` is a convention; you can pick anything (`"category"`, `"kind"`)
as long as you're consistent within a graph.

## The helper

`fabula-memory`'s `MemGraph` provides `add_event` to bundle event-creation +
tag-attachment into one call:

```rust
use fabula_memory::MemGraph;

let mut g = MemGraph::new();
g.add_event("ev1", "attack",   &["violent", "harm", "physical"], 1);
g.add_event("ev2", "betrayal", &["harm", "social"],              5);
g.add_event("ev3", "scolding", &["harm", "social"],              10);
```

This is exactly equivalent to:

```rust
g.add_edge("ev1", "eventType", MemValue::Str("attack".into()), 1);
g.add_edge("ev1", "tag", MemValue::Str("violent".into()), 1);
g.add_edge("ev1", "tag", MemValue::Str("harm".into()), 1);
g.add_edge("ev1", "tag", MemValue::Str("physical".into()), 1);
// ... and so on for ev2, ev3
```

Custom adapters can either expose their own `add_event`-style helper or document
how the convention maps onto their native graph API.

## Matching by tag

### "Any event with tag `harm`"

```fabula
pattern any_harm {
    stage e1 {
        e1.tag = "harm"
    }
}
```

This finds events whose tag set *contains* "harm". An event with tags
`["violent", "harm", "physical"]` matches because at least one outgoing `"tag"`
edge has target `"harm"`.

### "Any event with tag `harm` OR `violent`"

```fabula
pattern dramatic {
    stage e1 {
        e1.tag in ["harm", "violent"]
    }
}
```

The `in [...]` syntax is a `ValueConstraint::OneOf` — matches if any of the
listed values is in the tag set.

### "Event with tags `harm` AND `social`"

```fabula
pattern social_harm {
    stage e1 {
        e1.tag = "harm"
        e1.tag = "social"
    }
}
```

Two clauses in the same stage both must match. The engine joins them on the
shared anchor (`e1`), so both clauses look at the same event's outgoing edges.

### "Event without tag `peaceful`"

```fabula
pattern not_peaceful {
    stage e1 {
        e1.eventType = "ritual"
        ! e1.tag = "peaceful"
    }
}
```

The `!` prefix negates the clause. Combined with another positive clause (here,
`eventType = "ritual"`), this matches rituals that *aren't* tagged peaceful.
The negation binds to the whole clause, including the variable resolution.

## Common patterns

| Goal | Clause |
|---|---|
| Tagged X | `e.tag = "X"` |
| Tagged X or Y | `e.tag in ["X", "Y"]` |
| Tagged X **and** Y | `e.tag = "X"` + `e.tag = "Y"` (two clauses, same stage) |
| Tagged X but **not** Y | `e.tag = "X"` + `! e.tag = "Y"` |
| Tagged at least one of a list, **not** any of another | `e.tag in [allowed]` + `! e.tag in [forbidden]` |

## Why this is just convention

Fabula's pattern engine doesn't know what a tag is. It sees edges with a label
and matches them like any other edge. The "tag" convention is just a shared
vocabulary between the simulation (which writes tag edges) and the patterns
(which read them). You can change the label, model multi-level taxonomies
(`"genre"` + `"subgenre"` edges), or skip tags entirely if your event types are
flat enough.

For richer event taxonomies — hierarchical tags, tag synonyms, weighted
membership — author a small post-processor on the graph or a custom
`DataSource` adapter; the matching primitives don't need to change.
