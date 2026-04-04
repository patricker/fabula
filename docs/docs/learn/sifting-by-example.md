---
sidebar_position: 2
title: Sifting by Example
---

# Sifting by Example

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Four domains. Same mechanism. Each example: a problem, a pattern, a graph, and a result you can run in the playground.

**Quick DSL guide:** `stage e1 { ... }` defines an event slot. `e1.type = "login"` matches an edge with that label and value. `e1.user -> ?user` binds the target to a variable. `?user` in a later stage creates a join (same entity). `@3` means time 3. `unless between e1 e2 { ... }` means "these events must NOT occur between stages e1 and e2."

| | |
|---|---|
| **Time** | ~10 minutes |
| **Prerequisites** | [What is Sifting?](what-is-sifting) |

---

## 1. Narrative: Broken promise

**Problem:** A character promises something, then breaks that promise, with no fulfillment between.

<PatternPlayground
  defaultPattern={`pattern broken_promise {
  stage e1 {
    e1.type = "promise"
    e1.actor -> ?char
    e1.target -> ?recipient
  }
  stage e2 {
    e2.type = "betray"
    e2.actor -> ?char
    e2.target -> ?recipient
  }
  unless between e1 e2 {
    mid.type = "fulfill"
    mid.actor -> ?char
    mid.target -> ?recipient
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "promise"
  @1 e1.actor -> macbeth
  @1 e1.target -> duncan

  @3 e2.type = "betray"
  @3 e2.actor -> macbeth
  @3 e2.target -> duncan

  @2 e3.type = "promise"
  @2 e3.actor -> banquo
  @2 e3.target -> macbeth

  @4 mid.type = "fulfill"
  @4 mid.actor -> banquo
  @4 mid.target -> macbeth

  @5 e4.type = "betray"
  @5 e4.actor -> banquo
  @5 e4.target -> macbeth

  now = 10
}`}
  compact
/>

**Result:** 1 match — Macbeth promised Duncan and then betrayed him. Banquo also promised and later betrayed Macbeth, but fulfilled the promise first (time 4), so the negation kills that match.

**What to notice:** The variable `?char` joins the actor across both stages. The `unless between` clause requires the *same* actor and recipient to have fulfilled the promise — a random fulfillment by someone else doesn't count.

---

## 2. Observability: Cascade failure

**Problem:** Service A calls B, B calls C, and C times out. No recovery event (successful retry) follows.

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

**Result:** 1 match — the api_gateway → auth_service → user_db chain timed out with no recovery. The cache_service → redis chain also timed out, but redis recovered at time 8, so `unless after` kills that match.

**What to notice:** The variable chain `?svc_a → ?svc_b → ?svc_c` traces the call path through shared bindings. The `unless after` negation is open-ended — it checks from the timeout forward, not between two fixed events.

---

## 3. Compliance: Four-eyes violation

**Problem:** The same person both initiates and approves a transaction — violating the four-eyes principle (dual control).

<PatternPlayground
  defaultPattern={`pattern four_eyes_violation {
  stage e1 {
    e1.type = "initiate"
    e1.actor -> ?person
    e1.transaction -> ?txn
  }
  stage e2 {
    e2.type = "approve"
    e2.actor -> ?person
    e2.transaction -> ?txn
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "initiate"
  @1 e1.actor -> alice
  @1 e1.transaction -> txn_100

  @2 e2.type = "approve"
  @2 e2.actor -> alice
  @2 e2.transaction -> txn_100

  @3 e3.type = "initiate"
  @3 e3.actor -> bob
  @3 e3.transaction -> txn_200

  @4 e4.type = "approve"
  @4 e4.actor -> carol
  @4 e4.transaction -> txn_200

  now = 10
}`}
  compact
/>

**Result:** 1 match — Alice both initiated and approved `txn_100`. Bob initiated `txn_200` but Carol approved it (different people), so no violation.

**What to notice:** This pattern has no negation — the match *itself* is the violation. The join on `?person` is what makes it work: the same actor in both stages means dual control failed. No negation needed because there is no "exception" here — the pattern completing IS the problem.

---

## 4. Process mining: Stuck order

**Problem:** An order was placed and shipped, but never confirmed as delivered — with no cancellation between.

<PatternPlayground
  defaultPattern={`pattern stuck_order {
  stage e1 {
    e1.type = "order_placed"
    e1.order -> ?order
  }
  stage e2 {
    e2.type = "shipped"
    e2.order -> ?order
  }
  unless after e2 {
    mid.type = "delivered"
    mid.order -> ?order
  }
  unless between e1 e2 {
    cancel.type = "cancelled"
    cancel.order -> ?order
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "order_placed"
  @1 e1.order -> order_500

  @3 e2.type = "shipped"
  @3 e2.order -> order_500

  @2 e3.type = "order_placed"
  @2 e3.order -> order_501

  @4 e4.type = "shipped"
  @4 e4.order -> order_501

  @6 mid.type = "delivered"
  @6 mid.order -> order_501

  @1 e5.type = "order_placed"
  @1 e5.order -> order_502

  @2 cancel.type = "cancelled"
  @2 cancel.order -> order_502

  now = 10
}`}
  compact
/>

**Result:** 1 match — `order_500` was placed and shipped but never delivered. `order_501` was delivered (time 6), so `unless after` kills that match. `order_502` was cancelled before shipping, so `unless between` prevents stage 2 from being reached.

**What to notice:** Two negation windows working together. `unless between e1 e2` catches cancellations during processing. `unless after e2` catches the missing delivery. The variable `?order` threads through everything, ensuring each negation checks the *same* order.

---

## The pattern across all four examples

Every example uses the same three primitives:

| Primitive | Narrative | Observability | Compliance | Process Mining |
|-----------|-----------|---------------|------------|----------------|
| **Stages** | promise → betray | call → call → timeout | initiate → approve | placed → shipped |
| **Variable joins** | same actor, same target | call chain via caller/callee | same person, same transaction | same order |
| **Negation** | no fulfillment between | no recovery after | (none needed) | no delivery after, no cancel between |

The mechanism is identical. The domain vocabulary differs.

## Where to go next

- [Getting Started](/docs/getting-started) — Build this in Rust, from `cargo new` to working code.
- [Pattern Cookbook](/docs/guides/pattern-cookbook) — More pattern recipes with matching and non-matching graphs.
- [Pattern Playground](/docs/playground/pattern-playground) — Full playground with 12 presets to explore.
