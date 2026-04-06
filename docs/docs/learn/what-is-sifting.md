---
sidebar_position: 1
title: What is Sifting?
---

# What is Sifting?

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

<PatternPlayground
  defaultPattern={`pattern access_after_revocation {
  stage e1 {
    e1.type = "revoke_access"
    e1.user -> ?user
    e1.resource -> ?resource
  }
  stage e2 {
    e2.type = "access"
    e2.user -> ?user
    e2.resource -> ?resource
  }
  unless between e1 e2 {
    mid.type = "reauthorize"
    mid.user -> ?user
    mid.resource -> ?resource
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "revoke_access"
  @1 e1.user -> alice
  @1 e1.resource -> db_prod

  @3 e2.type = "access"
  @3 e2.user -> alice
  @3 e2.resource -> db_prod

  @5 e3.type = "revoke_access"
  @5 e3.user -> bob
  @5 e3.resource -> db_staging

  @6 mid.type = "reauthorize"
  @6 mid.user -> bob
  @6 mid.resource -> db_staging

  @8 e4.type = "access"
  @8 e4.user -> bob
  @8 e4.resource -> db_staging

  now = 10
}`}
  compact
/>

**Story sifting** is the process of searching through a collection of simulated events to find sequences that match a pattern of interest -- a "narratively compelling" event chain in games, a policy violation in compliance, or an attack signature in security. Think of it as a regular expression engine, but for events on a temporal graph instead of characters in a string.

The playground shows a **pattern** (left) matched against a **graph** (right). The pattern describes a sequence of events to find; the graph contains the actual events. One match: Alice accessed `db_prod` after her access was revoked, with no re-authorization between. Bob accessed `db_staging` too, but he was re-authorized first — so no violation.

You just ran a sifting pattern. Try editing the graph — remove the `reauthorize` event for Bob and watch a second match appear.

---

## What just happened

You defined a template: a sequence of events with variables, ordering constraints, and an exception clause. The engine found every instance of that template in the data, respecting time and binding the same entity (`?user`, `?resource`) across stages.

That's sifting. A regular expression engine, but instead of matching character sequences in strings, it matches event sequences in temporal graphs. (A temporal graph is a set of edges where each edge has a time interval saying when the relationship held.)

## The idea

Systems produce events. Events have structure. Structure has meaning. But finding meaningful structure in a stream of events is hard — especially when the pattern you're looking for spans multiple events, involves the same entities, and has exceptions ("unless X happened between A and B").

Sifting is ordered subgraph template matching with variable joins over temporal data. You describe a sequence of connected events, and the engine finds every occurrence in your data.

## Where sifting applies

The pattern above is a compliance check. The same concept works across domains:

**Narrative detection.** A social simulation produces events — characters meet, betray, reconcile. Sifting finds the interesting subsequences: "Two betrayals by the same impulsive character with no reconciliation between them."

**Distributed tracing.** Service A calls B, B calls C, C returns a timeout. No recovery event within 5 ticks. Sifting finds the cascade failure and traces the call chain through shared variables.

**Process mining.** An order is placed, shipped, but never delivery-confirmed — with no cancellation between. Sifting finds the broken processes and `why_not` explains the near-misses.

**Cybersecurity.** An attacker moves through 3+ hosts using the same stolen credential, with no MFA challenge between any hop. Sifting finds the lateral movement chain.

The code is domain-agnostic. The examples differ in vocabulary, not in mechanism. Every domain listed above uses the same three primitives: stages (ordered event slots), variable joins (same entity across stages), and negation windows (events that must NOT occur).

## The key insight about negation

"Expected event that never happened" is the core detection signal across security, clinical, compliance, and observability. It's not enough to find sequences that DID happen — you need to find sequences where a critical event is MISSING.

Fabula's negation windows (`unless between`, `unless after`, `unless global`) express this directly. The `unless` clause in the demo above is what distinguishes "violation" from "authorized access." Without negation, sifting is just sequence matching. With it, you can express exceptions, safeguards, and recovery conditions.

## What makes fabula different

Most tools solve part of this problem:

- **CEP systems** (Flink, Esper) match event sequences but don't traverse graph structure or bind variables across joins.
- **Datalog engines** (DataScript) query graph structure but don't do incremental matching as events stream in.
- **Regular expressions** match sequences but have no notion of temporal intervals or entity identity.
- **SIEM tools** correlate events but don't provide clause-level gap analysis (a clause-by-clause breakdown of why a pattern *didn't* match -- which stages succeeded, which failed, and why) explaining *why* a pattern didn't match.

Fabula combines three capabilities in one library:

1. **Graph + Allen intervals + negation windows.** Temporal pattern matching over graph-structured data with 13 interval relations and scoped negation.
2. **`why_not` gap analysis.** When a pattern doesn't match, fabula reports which clause failed and why — clause by clause. No production CEP, SIEM, process mining, or monitoring tool does this.
3. **Clone-speculate-discard.** Clone the engine, add hypothetical events, see what would match, discard the clone. This enables what-if analysis, procedural content generation, and AI-driven narrative management.

## Where to go next

- [Sifting by Example](sifting-by-example) — The same pattern concept across 4 domains, with interactive playgrounds.
- [Getting Started](/docs/getting-started) — Build and evaluate your first pattern in Rust, from `cargo new` to working code.
- [Research Lineage](/docs/research) — The academic foundations: Felt, Winnow, StU, Allen interval algebra.
