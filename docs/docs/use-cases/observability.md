---
sidebar_position: 2
title: Observability
description: Detect cascade failures and anomalies in distributed systems
---

# Observability

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Distributed systems fail in sequences. Service A calls B, B calls C, C times out — and the root cause is three hops away from the symptom. Sifting patterns trace these chains through shared variables, detect missing recovery events, and flag SLA violations with gap constraints.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Cascade timeout

A call chain where the deepest service times out and no recovery follows.

<PatternPlayground
  defaultPattern={`pattern cascade_timeout {
  stage e1 {
    e1.type = "call"
    e1.caller -> ?svc_a
    e1.callee -> ?svc_b
  }
  stage e2 {
    e2.type = "call"
    e2.caller -> ?svc_b
    e2.callee -> ?svc_c
  }
  stage e3 {
    e3.type = "timeout"
    e3.service -> ?svc_c
  }
  unless after e3 {
    mid.type = "recovery"
    mid.service -> ?svc_c
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "call"
  @1 e1.caller -> api_gateway
  @1 e1.callee -> auth_service

  @2 e2.type = "call"
  @2 e2.caller -> auth_service
  @2 e2.callee -> user_db

  @3 e3.type = "timeout"
  @3 e3.service -> user_db

  @5 e4.type = "call"
  @5 e4.caller -> api_gateway
  @5 e4.callee -> cache_service

  @6 e5.type = "call"
  @6 e5.caller -> cache_service
  @6 e5.callee -> redis

  @7 e6.type = "timeout"
  @7 e6.service -> redis

  @8 mid.type = "recovery"
  @8 mid.service -> redis

  now = 10
}`}
  compact
/>

**Result:** 1 match — api_gateway → auth_service → user_db timed out with no recovery. The cache_service → redis chain also timed out, but redis recovered at time 8, killing that match.

**What to notice:** The variable chain `?svc_a → ?svc_b → ?svc_c` traces the call path through join semantics. Stage 2 reuses `?svc_b` from stage 1 as a caller — this is how sifting "follows" a dependency chain. The `unless after` is open-ended: it checks from the timeout forward, catching any future recovery.

---

## 2. Retry storm

A service retries a failed call more than once before the downstream recovers. This creates amplified load that can worsen the original failure.

<PatternPlayground
  defaultPattern={`pattern retry_storm {
  stage e1 {
    e1.type = "call_failed"
    e1.caller -> ?svc
    e1.callee -> ?target
  }
  stage e2 {
    e2.type = "retry"
    e2.caller -> ?svc
    e2.callee -> ?target
  }
  stage e3 {
    e3.type = "retry"
    e3.caller -> ?svc
    e3.callee -> ?target
  }
  unless between e1 e3 {
    mid.type = "success"
    mid.caller -> ?svc
    mid.callee -> ?target
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "call_failed"
  @1 e1.caller -> order_svc
  @1 e1.callee -> payment_api

  @2 e2.type = "retry"
  @2 e2.caller -> order_svc
  @2 e2.callee -> payment_api

  @3 e3.type = "retry"
  @3 e3.caller -> order_svc
  @3 e3.callee -> payment_api

  @4 e4.type = "call_failed"
  @4 e4.caller -> shipping_svc
  @4 e4.callee -> inventory_api

  @5 e5.type = "retry"
  @5 e5.caller -> shipping_svc
  @5 e5.callee -> inventory_api

  @6 mid.type = "success"
  @6 mid.caller -> shipping_svc
  @6 mid.callee -> inventory_api

  @7 e6.type = "retry"
  @7 e6.caller -> shipping_svc
  @7 e6.callee -> inventory_api

  now = 10
}`}
  compact
/>

**Result:** 1 match — order_svc retried payment_api twice with no success between. shipping_svc also failed and retried inventory_api, but succeeded at time 6 before the second retry, so `unless between` kills that match.

**What to notice:** The negation window spans from the initial failure to the second retry. A successful call between those bounds means the retry storm was resolved. Without the negation, you'd flag every service that ever retried — the negation makes it specific to *unresolved* retry storms.

---

## 3. SLA breach

A request takes longer than the allowed threshold to complete. Use a gap constraint to enforce the timing bound.

<PatternPlayground
  defaultPattern={`pattern sla_breach {
  stage e1 {
    e1.type = "request_start"
    e1.request -> ?req
    e1.service -> ?svc
  }
  stage e2 {
    e2.type = "request_end"
    e2.request -> ?req
    e2.service -> ?svc
  }
  temporal e1 before e2 gap 5..
}`}
  defaultGraph={`graph {
  @1 e1.type = "request_start"
  @1 e1.request -> req_100
  @1 e1.service -> checkout

  @8 e2.type = "request_end"
  @8 e2.request -> req_100
  @8 e2.service -> checkout

  @2 e3.type = "request_start"
  @2 e3.request -> req_101
  @2 e3.service -> checkout

  @4 e4.type = "request_end"
  @4 e4.request -> req_101
  @4 e4.service -> checkout

  now = 10
}`}
  compact
/>

**Result:** 1 match — req_100 took 7 ticks (1 to 8), exceeding the `gap 5..` threshold. req_101 took 2 ticks (2 to 4), within bounds.

**What to notice:** The `temporal e1 before e2 gap 5..` constraint adds a metric bound: the gap between stage 1's end and stage 2's start must be at least 5 ticks. This is STN-style bounded-difference constraint checking — you define the SLA threshold in the pattern itself, not in post-processing.

---

## The pattern across all three examples

| Pattern | Stages | Joins | Negation | Temporal |
|---------|--------|-------|----------|----------|
| Cascade timeout | call → call → timeout | caller/callee chain | no recovery after | implicit ordering |
| Retry storm | fail → retry → retry | same caller/target | no success between | implicit ordering |
| SLA breach | start → end | same request/service | none | gap constraint (min 5 ticks) |

## Integration with incremental mode

In production, you feed events from your tracing pipeline into fabula's incremental engine:

```rust
// Each span/event from your tracing system:
let events = engine.on_edge_added(&graph, &source, &label, &value, &interval);
for event in &events {
    match event {
        SiftEvent::Completed { pattern, bindings, .. } => {
            // Alert: cascade_timeout detected!
            // bindings["svc_a"], bindings["svc_b"], bindings["svc_c"]
            // contain the affected services.
        }
        SiftEvent::Negated { pattern, .. } => {
            // Recovery detected — a previously active alert is resolved.
        }
        _ => {}
    }
}
```

The engine tracks partial matches across thousands of concurrent requests. When a cascade completes, you get the full call chain in the bindings. When a recovery event arrives, partial matches are automatically killed.

## Where to go next

- [Getting Started](/docs/getting-started) — Build and evaluate patterns in Rust.
- [Incremental Integration](/docs/guides/incremental-integration) — Wire fabula into your event pipeline.
- [Scoring Reference](/docs/reference/scoring) — Rank alerts by surprise to reduce noise.
- [Pattern Cookbook](/docs/guides/pattern-cookbook) — More pattern recipes.
