---
sidebar_position: 4
title: Compliance Checking
description: Detect policy violations and audit event logs for forbidden sequences
---

# Compliance Checking

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Compliance rules are forbidden sequences: access after revocation, approval without review, data export after access removal. Sifting patterns express these rules directly. When a pattern matches, that's a violation. When it doesn't, `why_not` explains how close the system came.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Access after revocation

A user accesses a resource after their access was revoked, with no re-authorization between.

<PatternPlayground
  defaultPattern={`pattern unauthorized_access {
  stage e1 {
    e1.type = "revoke"
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
  @1 e1.type = "revoke"
  @1 e1.user -> alice
  @1 e1.resource -> db_prod

  @3 e2.type = "access"
  @3 e2.user -> alice
  @3 e2.resource -> db_prod

  @2 e3.type = "revoke"
  @2 e3.user -> bob
  @2 e3.resource -> api_keys

  @4 mid.type = "reauthorize"
  @4 mid.user -> bob
  @4 mid.resource -> api_keys

  @6 e4.type = "access"
  @6 e4.user -> bob
  @6 e4.resource -> api_keys

  now = 10
}`}
  compact
/>

**Result:** 1 match — Alice accessed db_prod after revocation with no re-authorization. Bob's access was revoked too, but he was re-authorized at time 4, so the negation prevents a match.

**What to notice:** The variables `?user` and `?resource` both join across stages AND the negation window. A re-authorization for a *different* user or *different* resource doesn't count — the negation must match the same entity pair.

---

## 2. Four-eyes principle violation

The same person both initiates and approves a transaction. The pattern completing IS the violation — no negation needed.

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
  @1 e1.transaction -> txn_500

  @2 e2.type = "approve"
  @2 e2.actor -> alice
  @2 e2.transaction -> txn_500

  @3 e3.type = "initiate"
  @3 e3.actor -> bob
  @3 e3.transaction -> txn_501

  @4 e4.type = "approve"
  @4 e4.actor -> carol
  @4 e4.transaction -> txn_501

  now = 10
}`}
  compact
/>

**Result:** 1 match — Alice initiated and approved txn_500. Bob initiated txn_501 but Carol approved it (different people), so dual control held.

**What to notice:** This is a conceptual inversion from narrative sifting. In narrative detection, a match means "something interesting happened." In compliance checking, a match means "a rule was broken." The mechanism is identical; the interpretation differs. The join on `?person` enforces that the same actor performed both actions.

---

## 3. Data export without approval

Data is exported from a sensitive system without a preceding approval for the same dataset. Model this as: approval event, then export event for the same dataset — with a negation ensuring the approval actually happened. If the export has no matching approval before it, the second pattern (export-only) catches it.

<PatternPlayground
  defaultPattern={`pattern unapproved_export {
  stage e1 {
    e1.type = "export"
    e1.actor -> ?actor
    e1.dataset -> ?data
  }
  unless after e1 {
    mid.type = "approve_export"
    mid.dataset -> ?data
  }
}`}
  defaultGraph={`graph {
  @3 e1.type = "export"
  @3 e1.actor -> alice
  @3 e1.dataset -> customer_pii

  @1 mid.type = "approve_export"
  @1 mid.dataset -> financial_reports

  @5 e2.type = "export"
  @5 e2.actor -> bob
  @5 e2.dataset -> financial_reports

  @2 mid2.type = "approve_export"
  @2 mid2.dataset -> financial_reports

  now = 10
}`}
  compact
/>

**Result:** 1 match — Alice exported customer_pii and no approval for that dataset exists anywhere. Bob exported financial_reports, which was approved at time 2.

**What to notice:** The `unless after e1` checks for an approval event after the export. Combined with the graph having approvals before exports, this catches cases where no approval exists at all. The join on `?data` ensures the approval covers the specific dataset — approving one dataset doesn't authorize exporting a different one.

---

## Auditing with gap analysis

When a pattern does NOT match, that's a good thing — the system is compliant. But near-misses matter. Use `why_not` (gap analysis) to find events that *almost* violated a rule:

```rust
use fabula::prelude::*;
use fabula_memory::MemGraph;

let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
engine.register(violation_pattern);

let matches = engine.evaluate(&graph);
if matches.is_empty() {
    // System is compliant. Check near-misses for each pattern:
    for pattern in engine.patterns() {
        let gap = gap_analysis(&graph, pattern);
        for stage in &gap.stages {
            match stage.status {
                StageStatus::Matched => {}
                StageStatus::Unmatched | StageStatus::PartiallyMatched => {
                    println!("Near-miss for '{}': stage '{}' — {:?}",
                        pattern.name, stage.anchor, stage.status);
                    for clause in &stage.clauses {
                        println!("  clause: matched={}, reason={:?}",
                            clause.matched, clause.reason);
                    }
                }
            }
        }
    }
}
```

A rule that reaches stage 2 of 3 before failing is a near-miss worth investigating — the system was one event away from a violation.

## The pattern across all three examples

| Pattern | What makes it a violation | Stages | Key mechanism |
|---------|--------------------------|--------|---------------|
| Unauthorized access | Access after revocation without re-auth | 2 + negation | Variable join on user AND resource |
| Four-eyes | Same person in both roles | 2, no negation | Match = violation (conceptual inversion) |
| Unapproved export | Export without matching approval | 1 + negation | `unless after` checks for missing approval |

## Where to go next

- [Getting Started](/docs/getting-started) — Build and evaluate patterns in Rust.
- [Debugging Patterns](/docs/guides/debugging-patterns) — Systematic troubleshooting with gap analysis.
- [Incremental Integration](/docs/guides/incremental-integration) — Real-time compliance monitoring.
- [Scoring Reference](/docs/reference/scoring) — Rank violations by severity using surprise scoring.
