---
sidebar_position: 12
title: DSL templates
description: Define parameterized pattern templates with `template name(params) { ... }` and instantiate them with `instantiate name(args)`.
---

# DSL templates

A template is a reusable parameterized pattern body. Define it once, instantiate
it with different arguments to produce many concrete patterns.

## Defining a template

```fabula
template harm_arc(aggressor, victim) {
    stage e1 {
        e1.eventType = "harm"
        e1.actor -> ?aggressor
        e1.target -> ?victim
    }
    stage e2 {
        e2.eventType = "retaliation"
        e2.actor -> ?victim
        e2.target -> ?aggressor
    }
}
```

- Parameter list is parens-delimited and comma-separated. Zero parameters is
  legal: `template empty() { ... }`.
- The body must contain at least one stage.
- Templates cannot themselves contain `instantiate` directives (no nested
  templating in this version).

## Instantiating a template

```fabula
pattern alice_revenge {
    instantiate harm_arc("alice", "bob")
}
```

- Arguments are string literals. Numeric or `?var` arguments are not
  supported in this version.
- Arity must match the template's parameter list exactly. Mismatch is a
  compile error.
- A pattern body may contain multiple `instantiate` directives, mixed with
  regular `stage` blocks. The instantiated stages are spliced in at the
  position of the directive in document order.

## What gets substituted

Parameter names are textually replaced with the corresponding argument
string at every occurrence in:

| Position | Example before | Example after `instantiate t("alice", "bob")` |
|---|---|---|
| Clause source | `?aggressor.eventType = "harm"` | `?alice.eventType = "harm"` |
| Bind target | `e1.actor -> ?victim` | `e1.actor -> ?bob` |
| `*Var` constraint target | `e1.actor = ?aggressor` | `e1.actor = ?alice` |
| Node-ref target | `e1.actor -> aggressor` | `e1.actor -> alice` |
| String-literal target | `e1.role = "aggressor"` | `e1.role = "alice"` |
| `OneOf` member | `e1.tag in ["aggressor", "ritual"]` | `e1.tag in ["alice", "ritual"]` |

**Labels are NOT substituted.** A label like `actor` in `e1.actor -> ?aggressor`
identifies the predicate name in your graph schema, not a parameterizable
position. If a parameter happened to share a name with a label, substituting
the label would silently rewire the clause to query a non-existent edge —
so the substitution skips labels entirely. Pick parameter names that don't
collide with the labels you want to keep (most schemas use noun-like labels
`actor`, `target`, `eventType` and parameter names tend to be role-like
`aggressor`, `victim`, `witness` — collisions are uncommon in practice).

Numeric and boolean literals are *not* substituted.

Stage anchor names are also not parameter-substituted, but they ARE renamed
per-instantiation to a fresh prefix (`inst0__e1`, `inst1__e1`, …) so two
instantiations of the same template don't collide on anchor scope.

Stage `let` bindings inside templates are not yet substituted — if you put
a parameterized expression in a `let` binding inside a template, the result
is unspecified. For the current version, write `let` bindings outside
template bodies in the calling pattern.

## Multiple instantiations

One template can be instantiated multiple times in a single document:

```fabula
pattern carol_revenge {
    instantiate harm_arc("carol", "dave")
}
```

Each instantiation produces an independent expansion — changes to one
pattern do not affect others.

## Mixed bodies

A pattern can mix `instantiate` directives with its own `stage` blocks.
Stages from the instantiation are spliced in at the position of the directive:

```fabula
template setup(actor) {
    stage prep {
        prep.actor -> ?actor
        prep.eventType = "prepare"
    }
}

pattern full_arc {
    instantiate setup("alice")
    stage climax {
        climax.actor -> ?alice
        climax.eventType = "strike"
    }
}
```

Here `full_arc` compiles to a two-stage pattern: `prep` then `climax`.

## Errors

- `unknown template \`foo\`` — instantiating a name that was not defined in the document.
- `template \`foo\` expects N argument(s), got M` — arity mismatch.
- `template must contain at least one stage` — empty body rejected at parse time.

## Limitations (current version)

- **No cross-document imports** — templates are document-scoped.
- **No nested templates** — `instantiate` cannot appear inside a template body.
- **Arguments must be string literals** — not numbers, booleans, or `?var` references.
- **Labels are not parameterizable** — pick parameter names that don't collide
  with predicate names in your schema (see "What gets substituted" above).
- **`let` bindings inside template bodies are not substituted** — if you need
  a parameterized `let`, define it in the calling pattern after the
  `instantiate` directive.

These are intentional simplifications. File an issue with a concrete use case
if you need one of these constraints lifted.
