---
sidebar_position: 3
title: Process Mining
description: Detect deviations in business processes using temporal pattern matching
---

# Process Mining

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Business processes have expected flows: order → payment → shipment → delivery. Deviations — skipped steps, reversed order, timeout violations — hide in event logs. Sifting patterns describe the expected flow and its exceptions, finding every instance where the process broke.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Skipped approval

A purchase order is fulfilled without the required approval step.

<PatternPlayground
  defaultPattern={`pattern skipped_approval {
  stage e1 {
    e1.type = "purchase_request"
    e1.order -> ?order
    e1.requester -> ?requester
  }
  stage e2 {
    e2.type = "fulfillment"
    e2.order -> ?order
  }
  unless between e1 e2 {
    mid.type = "approval"
    mid.order -> ?order
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "purchase_request"
  @1 e1.order -> po_100
  @1 e1.requester -> alice

  @3 e2.type = "fulfillment"
  @3 e2.order -> po_100

  @2 e3.type = "purchase_request"
  @2 e3.order -> po_101
  @2 e3.requester -> bob

  @4 mid.type = "approval"
  @4 mid.order -> po_101

  @6 e4.type = "fulfillment"
  @6 e4.order -> po_101

  now = 10
}`}
  compact
/>

**Result:** 1 match — po_100 was fulfilled without approval. po_101 was approved at time 4 before fulfillment at time 6, so the negation kills that match.

**What to notice:** The join on `?order` threads through all three parts (request, negation, fulfillment). An approval for a *different* order doesn't satisfy the negation — it must be for the same PO.

---

## 2. Out-of-order processing

Payment received before the order was confirmed. The process flow should be: confirm → pay. Reversed order is a deviation.

<PatternPlayground
  defaultPattern={`pattern payment_before_confirmation {
  stage e1 {
    e1.type = "payment_received"
    e1.order -> ?order
    e1.amount -> ?amount
  }
  stage e2 {
    e2.type = "order_confirmed"
    e2.order -> ?order
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "payment_received"
  @1 e1.order -> ord_200
  @1 e1.amount = 150.00

  @3 e2.type = "order_confirmed"
  @3 e2.order -> ord_200

  @2 e3.type = "order_confirmed"
  @2 e3.order -> ord_201

  @4 e4.type = "payment_received"
  @4 e4.order -> ord_201
  @4 e4.amount = 75.00

  now = 10
}`}
  compact
/>

**Result:** 1 match — ord_200 received payment (time 1) before confirmation (time 3). ord_201 was confirmed first (time 2) then paid (time 4) — correct order, no match.

**What to notice:** Stages are time-ordered. Stage 1 must happen before stage 2. When you define `payment_received` as stage 1 and `order_confirmed` as stage 2, the pattern matches only when payment comes first. This is how you detect reversed process steps — put the wrong order in the pattern.

---

## 3. SLA timeout

An order was placed but not shipped within the SLA window. Use a gap constraint to enforce the time limit.

<PatternPlayground
  defaultPattern={`pattern shipping_sla_breach {
  stage e1 {
    e1.type = "order_placed"
    e1.order -> ?order
    e1.customer -> ?customer
  }
  stage e2 {
    e2.type = "shipped"
    e2.order -> ?order
  }
  temporal e1 before e2 gap 5..
  unless between e1 e2 {
    mid.type = "cancelled"
    mid.order -> ?order
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "order_placed"
  @1 e1.order -> ord_300
  @1 e1.customer -> alice

  @8 e2.type = "shipped"
  @8 e2.order -> ord_300

  @2 e3.type = "order_placed"
  @2 e3.order -> ord_301
  @2 e3.customer -> bob

  @4 e4.type = "shipped"
  @4 e4.order -> ord_301

  @3 e5.type = "order_placed"
  @3 e5.order -> ord_302
  @3 e5.customer -> carol

  @5 mid.type = "cancelled"
  @5 mid.order -> ord_302

  now = 10
}`}
  compact
/>

**Result:** 1 match — ord_300 took 7 ticks to ship (1 to 8), exceeding the `gap 5..` threshold. ord_301 shipped in 2 ticks (within bounds). ord_302 was cancelled before shipping, so the negation prevents a false positive.

**What to notice:** The `gap 5..` constraint defines the SLA in the pattern itself. Combined with `unless between` for cancellations, you get a precise definition: "order placed, shipped late, not cancelled." The gap constraint and negation work together — neither alone is sufficient.

---

## Batch auditing

Process mining typically works on complete logs. Use batch evaluation to scan an entire event log for all deviations:

```rust reference file=tests/use_cases_process_mining.rs#batch_auditing
```

Register multiple patterns and evaluate once. Every deviation in the log is surfaced, and `gap_analysis` highlights processes that were one step away from a deviation.

## The pattern across all three examples

| Pattern | What it detects | Stages | Key mechanism |
|---------|----------------|--------|---------------|
| Skipped approval | Missing step in process | 2 + negation | `unless between` checks for the missing step |
| Out-of-order | Reversed process steps | 2, no negation | Stage ordering = expected sequence; match = reversed |
| SLA timeout | Excessive processing time | 2 + gap + negation | `gap 5..` enforces time bound; negation excludes cancellations |

## Mapping your data

XES event log entries map to fabula edges as follows:

| Real-world field | Fabula edge |
|---|---|
| case:concept:name (case ID) | source node |
| concept:name (activity) | label value |
| org:resource (performer) | target node |
| time:timestamp | interval start |

In XES, all events in the same case share a case ID -- this becomes the source node, enabling joins across activities in the same process instance.

---

## How fabula compares

- **vs ProM / Disco / Celonis:** Petri net conformance checking and process discovery. These tools model the full process as a net and check event logs for conformance. No Allen algebra for temporal relations, no incremental streaming (batch-only replay), no variable-bound negation. Fabula is pattern-first: you describe the deviation, not the entire process.
- **vs Declare / LTLf:** Constraint-based process modeling with boolean satisfaction (a trace either satisfies a constraint or does not). No graduated matching -- no gap analysis showing *how close* a trace came to violating a rule. Fabula's `why_not` provides clause-by-clause breakdown of near-misses.

---

## Where to go next

- [Getting Started](/docs/getting-started) — Build and evaluate patterns in Rust.
- [Debugging Patterns](/docs/guides/debugging-patterns) — Troubleshoot with gap analysis.
- [DSL Reference](/docs/reference/dsl) — Full syntax for patterns, graphs, and compose operators.
- [Compliance Checking](/docs/use-cases/compliance-checking) — Related use case with a regulatory focus.
