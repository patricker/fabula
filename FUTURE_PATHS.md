# Future Paths — Cross-Domain Applications for Fabula

**Status**: Research document
**Date**: 2026-04-03

Fabula was designed for story sifting — finding narratively compelling event
sequences in simulation output. But the core engine is a **general-purpose
incremental streaming pattern matcher over labeled temporal multigraphs**.
This document captures research into alternative domains where fabula's
primitives provide genuine novel value, plus the platform investments needed
to unlock them.

---

## Core Primitives (Domain-Agnostic Framing)

Stripping the narrative terminology, fabula provides:

| Primitive | Abstract Capability |
|-----------|-------------------|
| Multi-stage patterns | Ordered subgraph template matching with variable joins |
| Allen interval algebra | 13 temporal relations between interval-typed edges + metric gap bounds |
| Negation windows | Bounded temporal absence detection ("X must NOT occur between A and B") |
| Composition | Sequence, exclusive choice, bounded repeat with variable sharing |
| Staleness detection | Flag partial matches stuck for N ticks |
| Setup/resolution pairing | Track unresolved "plants" awaiting "payoffs" |
| Surprise scoring | Statistical anomaly detection at pattern and property level |
| Gap analysis (`why_not`) | Clause-by-clause diagnosis of incomplete matches |
| Speculative evaluation | Clone engine, inject hypotheticals, score, commit/discard |
| Pluggable graph backend | `DataSource` trait — any store, zero-dep core, WASM target |

### What no existing production system combines

Research consistently identified three capabilities that are genuinely novel
across all domains studied:

1. **Graph + Allen algebra + negation in one engine.** CEP systems (Flink,
   Esper) handle flat event streams with sequence patterns. Graph databases
   handle topology but not incremental temporal matching. No production
   system combines all three. The closest academic work is GrapeL (Ehmes
   et al., 2020) which requires two separate engines (eMoflon + Apama).

2. **`why_not()` gap analysis.** No production CEP, SIEM, process mining,
   or monitoring tool explains WHY a pattern didn't complete at the clause
   level. Maps to regulatory audit trails, clinical decision support,
   debugging, and scenario extraction.

3. **Speculative evaluation (clone-speculate-discard).** Unique to fabula's
   decoupled `SiftEngine` architecture. Enables what-if analysis, procedural
   content generation, treatment planning, compliance impact assessment,
   and predictive alerting — without building a separate simulation.

---

## Domain Analysis

### Tier 1 — Strong Novel Value

#### Distributed Tracing / Root Cause Analysis

Traces ARE temporal graphs — spans with parent-child edges, labels, and
durations. Current tools have significant expressiveness gaps:

- **TraceQL** (Grafana Tempo): Cannot express temporal ordering between
  non-parent-child spans. Cannot match across traces. Negation operators
  are experimental and produce false positives.
- **Jaeger**: Query by trace ID or attribute filters only. No structural
  pattern matching.
- **Honeycomb**: BubbleUp for statistical anomaly detection on individual
  spans, not multi-step temporal sequences.
- **Dynatrace Davis AI**: Black-box fault-tree analysis with 5/15-minute
  sliding windows. Users cannot define custom failure cascade patterns.

**Example pattern — DB cascade failure:**
```
stage db_err {
  ?db_span.service = "db-primary"
  ?db_span.status = "error"
  ?db_span.caller -> ?caller
}
stage retry {
  ?retry_span.service = "db-primary"
  ?retry_span.operation = "retry"
  ?retry_span.caller -> ?caller       // same caller (variable join)
}
stage cascade {
  ?upstream.status = "error"
  ?upstream.caller -> ?caller2
}
temporal db_err before retry gap 0..2000     // retry within 2s
temporal retry before cascade gap 0..10000   // cascade within 10s
unless between db_err cascade {
  ?recovery.service = "db-primary"
  ?recovery.status = "ok"                    // no DB recovery in between
}
```

**Key differentiators over existing tools:**
- Allen algebra: error `overlaps` retry (not just "before")
- Negation: "no recovery between failure and cascade"
- Cross-trace correlation: spans from different traces share the same graph
- Incremental evaluation: pattern matching as spans stream in via
  OpenTelemetry, not batch queries after the fact
- Gap analysis: "why hasn't this failure cascade pattern completed? We
  saw the DB error but no retry from the same caller yet"

**Practical value: HIGH.** The gap between what operators want to express
and what tools allow is enormous. Possibly the strongest non-narrative fit.

---

#### Cybersecurity — SIEM / Threat Detection

MITRE ATT&CK kill chains are multi-stage temporal patterns. Current tools
treat security events as points, not intervals:

- **Splunk**: Correlation searches fire on thresholds within time windows.
  No structured multi-stage pattern matching.
- **Elastic EQL**: Supports `sequence by [join_field] [with maxspan=5m]`
  but events are instantaneous — no Allen relations, no interval overlap.
  Negation limited to `until` clauses.
- **Sigma rules**: Single-event detection signatures. No temporal
  sequencing at all.

**Example pattern — persistence contains discovery:**
```
stage persist {
  ?persist_event.tactic = "persistence"
  ?persist_event.host -> ?victim
}
stage discover {
  ?disc_event.tactic = "discovery"
  ?disc_event.host -> ?victim           // same host
}
temporal persist contains discover       // discovery DURING persistence
unless between persist discover {
  ?mfa.type = "mfa_challenge"
  ?mfa.user -> ?attacker                // no MFA challenge
}
```

Allen `contains` expresses that the discovery phase is *temporally contained
within* the persistence interval — a relationship that no SIEM can express
because they model events as points, not intervals.

**Key differentiators:**
- Full Allen algebra for events with duration (port scans, C2 sessions,
  persistence intervals)
- Negation windows for absence of expected controls (no MFA, no approval,
  no log entry)
- Incremental matching: real-time vs Splunk's periodic batch correlation
- Staleness detection: "low and slow" attacks advancing one stage per day
- Surprise scoring: "this pattern matched, and the variable bindings are
  statistically unusual (rare source country, unusual service account)"

**Market context:** SIEM market ~$6.4B (2024). "Detection-as-code" movement
(Sigma, YARA-L, EQL) is seeking more expressive pattern languages. DARPA
programs (Transparent Computing, CHASE) fund provenance graph pattern
matching research — fabula's incremental approach addresses their latency
limitations.

**Related patterns:**
- Lateral movement: `repeat` composition across N hops with shared `?dc`
- C2 beaconing: `repeat` with metric gap range (55s..65s) for periodicity
- Data exfiltration: multi-stage with negation for missing DLP approval
- Insider threat: cross-system variable joins (badge-out → VPN → access)

---

#### Clinical Pathway Analysis / Pharmacovigilance

Patient journeys are temporal graphs. Mayo Clinic's Time Event Ontology
(TEO) research validated that >95% of temporal expressions in real EHR data
can be represented with Allen's 13 relations.

**Example pattern — adverse drug event with missing intervention:**
```
stage prescribed {
  ?patient.prescribed -> ?drug
}
stage lab_drop {
  ?patient.lab_result -> ?value
  ?patient.lab_result < 2.0            // below safety threshold
}
stage complication {
  ?patient.diagnosed -> ?condition
}
temporal prescribed before lab_drop gap 0..4320    // within 72 hours
temporal lab_drop before complication gap 0..10080 // within 7 days
unless between lab_drop complication {
  ?patient.intervention = "dose_adjustment"        // no corrective action
}
```

**Key differentiators over existing clinical tools:**
- **vs. Process mining** (ProM, Celonis): Operates on flat event logs,
  not graph structures. Limited temporal constraints (ordering only).
  No native negation windows. No incremental/streaming mode.
- **vs. CEP** (Flink, Esper): Flat event streams, no graph traversal,
  no Allen algebra, limited variable join expressiveness.
- **vs. OHDSI/OMOP TPD**: Single temporal association statistic, not
  multi-stage pattern matching with negation.
- **vs. Sequential pattern mining** (PrefixSpan): Finds frequent
  subsequences but lacks graph structure, interval algebra, and negation.

**Allen algebra is essential here:**
- "Drug A administered DURING hospitalization" (Allen `during`)
- "Drug A course OVERLAPS drug B course" (polypharmacy overlap window)
- "Lab test performed DURING treatment interval" (not just before/after)

**Negation windows are the core clinical value:**
- Missing interventions ("no dose adjustment after abnormal lab")
- Protocol deviations ("sepsis diagnosed but antibiotics never given")
- "Never events" in patient safety

**Additional clinical applications:**
- Pharmacovigilance: multi-drug interaction cascades with surprise scoring
  for above-baseline adverse event rates
- Clinical trial monitoring: protocol compliance as pattern matching,
  staleness for lost-to-follow-up detection
- EHR mining: gap analysis reveals population care gaps ("12,000 diabetics
  matched stages 1-2 but 3,400 never reached annual eye exam")
- Epidemiology: contact tracing with Allen `overlaps` for infectious
  period overlap, `repeat` for super-spreader detection

---

#### Process Mining / Compliance Monitoring

Current conformance checking (token replay, trace alignment) uses Petri
nets with limited temporal reasoning. Declare/LTL cannot combine negation
with variable bindings across stages (requires second-order quantification).

**Example pattern — four-eyes principle violation:**
```
stage check {
  ?check_event.type = "ApprovalCheck"
  ?check_event.performed_by -> ?person
  ?check_event.case_ref -> ?case
}
stage approve {
  ?approve_event.type = "Approval"
  ?approve_event.performed_by -> ?person   // SAME person = violation
  ?approve_event.case_ref -> ?case
}
temporal check before approve
```

When this pattern completes, the four-eyes principle was violated — the
same `?person` performed both check and approval.

**Example — compliance SLA with deadline:**
```
stage submission {
  ?sub.type = "Submission"
  ?sub.case_ref -> ?case
}
stage review {
  ?rev.type = "Review"
  ?rev.case_ref -> ?case
}
temporal submission before review gap 0..2880  // within 48 hours
```

Staleness detection provides proactive warning before the SLA deadline.

**Key differentiators over existing tools:**
- `why_not()` provides clause-by-clause audit trails: "Document #4521
  failed rule CR-17 because Stage 2 required review within 48h, earliest
  review was 72h"
- Negation with variable joins is native (LTLf cannot express this)
- Speculative evaluation: "if we tighten the SLA from 48h to 24h, how
  many historical violations?" without rebuilding a simulation
- Incremental matching for real-time compliance monitoring, not just
  retrospective conformance checking

**Additional applications:**
- Customer journey analysis: churn patterns with temporal constraints
  and negation ("engagement gap" = no re-engagement within N days)
- Supply chain: exception cascade detection with variable joins across
  organizational boundaries
- RPA: detecting automatable copy-paste-transform cycles, temporal
  signatures distinguishing cognitive pauses from mechanical delays

---

### Tier 2 — Clear Value Add

#### Fraud Detection (AML, Card Fraud)

Money laundering layering is a multi-stage temporal pattern across
account-node graphs. Negation windows detect missing legitimate
justifications. Allen algebra detects simultaneous suspicious sessions
(`overlaps`) and transactions during reduced-oversight periods (`during`).

**Market context:** ~$35B fraud detection market. EU AI Act classifies
fraud detection as "high-risk AI" requiring explainable decisions —
regulatory tailwind for pattern-based approaches over black-box ML.

**Key patterns:**
- Structuring/smurfing: `repeat` with threshold (multiple sub-threshold
  deposits), total exceeding reporting limit
- Layering: multi-hop transfer chain with variable joins tracing money flow
- Card fraud testing: small transaction → large purchase within window
- Impossible travel: negation for missing travel booking between two
  geographically distant card uses

---

#### IoT / Predictive Maintenance

Sensor readings have duration (intervals, not points). Multi-sensor
degradation patterns use Allen `during` for concurrent conditions.
WASM build enables edge gateway deployment without cloud round-trips.

**Performance fit:** At 28us/edge, handles ~35K edges/s. Typical
industrial sensor networks produce 1K-15K events/s per zone — fits
comfortably. WasmEdge runtimes on ARM Cortex-M are proven viable.

**Key patterns:**
- "Temperature elevated DURING entire pressure drop" (Allen `during`)
- "Valve opened but flow never reached threshold" (negation + staleness)
- Multi-stage degradation: bearing wear → lubrication low → temp anomaly

---

#### Gaming (Non-Narrative)

- **Achievement systems**: Multi-stage patterns with constraints replace
  brittle observer-pattern code scattered through the codebase
- **Anti-cheat**: Temporal impossibility patterns ("fire event DURING
  reload animation") with zero false positives on known sequences.
  Complements ML-based cheat detection.
- **Procedural quest generation**: Speculative evaluation — clone engine,
  inject hypothetical events, score by pattern interest, commit or discard
- **Player behavior**: Surprise scoring flags deviations from personal
  behavioral baselines (smurf/cheat/churn detection)

**Performance fit:** 64-player server at 64 ticks = ~4K events/s.
Battle royale with 100 players at 128 ticks = ~13K events/s. Trivially
within budget.

---

#### Deployment Safety / Canary Analysis

Multi-signal temporal correlation reduces both false positive and false
negative rollbacks. Current tools (Argo Rollouts, Flagger) evaluate
metrics independently.

**Key pattern — rollback-worthy canary failure:**
"Error rate doubles on canary pod, latency simultaneously increases
(Allen `overlaps`), errors spread to second pod within 5 minutes (variable
join with different pod), no self-healing (negation window)."

---

### Tier 3 — Niche Value

#### Financial Market Surveillance

Spoofing, layering, wash trading are multi-stage temporal patterns over
order book graphs. Gap analysis provides regulatory audit trails. But
HFT-scale data rates exceed single-engine throughput — requires per-
instrument sharding. Regulatory surveillance (seconds-scale) fits.

#### Autonomous Vehicle Safety Validation

Near-miss detection, specification monitoring, scenario extraction from
driving logs. Gap analysis finds "almost-matched" patterns — near-miss
scenarios valuable for testing. Competes with established STL/MTL monitor
ecosystems (VERIFAI, sTaliro).

#### Epidemiology

Contact tracing graphs with Allen algebra distinguishing "contact DURING
infectious period" from "contact BEFORE infectious period." Super-spreader
detection via `repeat` composition. Quarantine compliance via negation.

---

## Platform Investments to Enable Cross-Domain Use

### Current Gaps (Verified Against Codebase 2026-04-03)

1. **No cross-variable value comparison** — `ValueConstraint` only takes
   literal `V`, not variable references. Can't express "value_B > value_A".
2. **No range support for `repeat()`** — Only exact N, not `min..max`.
3. **No parallel/unordered stages** — Strictly sequential left-to-right.
4. **No staleness/timeout events** — `stale_patterns()` is query-only,
   no `SiftEvent::Expired` variant.
5. **No metadata/tags on patterns** — Only name, stages, temporal,
   negations, group.
6. **No windowed aggregation** — No count/sum/avg within sliding window
   as a stage constraint.

### Hit List (Ranked by Domains Unlocked x Feasibility)

#### Tier 1 — High Impact, Low-Medium Effort

**#1: Pattern Metadata / Tags**
Add `pub metadata: HashMap<String, String>` to `Pattern`. Propagate to
`SiftEvent` and `Match`.

Every non-narrative domain needs domain context on patterns: MITRE ATT&CK
technique IDs, compliance regulation codes, clinical protocol codes,
severity levels, runbook URLs, service owners.

- Scope: ~30-50 LoC across pattern.rs, builder.rs, types.rs
- DSL: `meta("severity", "high")`
- Effort: **Trivial**
- Unlocks: All domains (ergonomics)
- Dependencies: None

**#2: `SiftEvent::Expired` — Timeout-Based Absence Detection**
Add optional `deadline_ticks: Option<u64>` to `Pattern` or `PartialMatch`.
In `end_tick()`, scan active PMs for exceeded deadlines, emit
`SiftEvent::Expired { pattern, match_id, bindings, ticks_elapsed }`,
mark as Dead.

"Expected event that never happened" is THE core detection signal across
security, clinical, compliance, and observability.

- Scope: ~100-150 LoC in engine + builder + types
- DSL: `timeout 30` on pattern level
- Effort: **Small-medium**
- Unlocks: Security, Clinical, Compliance, Observability, IoT
- Dependencies: None

**#3: Cross-Stage Value Comparison (`ValueConstraint::BoundVar`)**
New `ValueConstraint` variants: `EqVar(String)`, `GtVar(String)`,
`LtVar(String)`, `GteVar(String)`, `LteVar(String)`. In engine eval,
resolve variable from `pm.bindings` before comparison. Requires
`V: PartialOrd` (already a trait bound).

Many patterns require relating values between stages: deteriorating lab
values, escalating prices, impossible movement speeds, amount mismatches.

- Scope: ~80-120 LoC across pattern.rs, eval.rs, builder.rs
- DSL: `e2.price > ?prev_price`
- Effort: **Small-medium**
- Unlocks: Anti-cheat, Finance, Clinical, IoT, Process Mining
- Dependencies: None

**#4: Repeat with Range (`min..max`)**
Extend `compose::repeat()` to accept `min: usize, max: usize`. Complete
when `min` repetitions match; stop accepting at `max`.

Threshold patterns are ubiquitous: brute force (5+ attempts), structuring
(3+ deposits), sensor anomaly (3+ readings), engagement drop (3+ deaths).

- Scope: ~60-100 LoC in compose.rs + engine completion logic
- DSL: `compose strikes = offense * 3..5 sharing(target)`
- Effort: **Small-medium**
- Unlocks: Security, Finance, IoT, Gaming, Epidemiology
- Dependencies: None

#### Tier 2 — High Impact, Medium Effort

**#5: Unordered / Concurrent Stage Groups**
Allow a group of stages to match in any order (all must match, order
irrelevant). Add `ordering: Ordered | Unordered` to stage groups. Phase 3
(advancement) tries ALL unordered stages against each incoming edge.

Multi-signal correlation patterns are central to observability and clinical
monitoring: "error rate spike AND latency increase co-occur" (order doesn't
matter).

- Scope: ~200-300 LoC across pattern.rs, eval.rs, builder.rs
- DSL: `concurrent { stage a { ... } stage b { ... } }`
- Effort: **Medium** (most architecturally invasive on this list)
- Unlocks: Observability, Security, Clinical, IoT, Canary
- Dependencies: None

**#6: Windowed Aggregation Constraints**
Stage-level constraint: "count of edges matching this clause in the last N
ticks >= threshold." Not a full CEP windowing system — just count/sum/
min/max over a sliding window.

Bridges the gap with CEP systems where windowed aggregation is the primary
primitive.

- Scope: ~200-350 LoC including ring buffer state management
- DSL: `stage s { window(10) count(e.type = "error") >= 5 }`
- Effort: **Medium**
- Unlocks: Security, Finance, IoT, Observability
- Dependencies: None
- Note: Consider deferring in favor of #4 (repeat with range) which
  handles the counting case more simply

#### Tier 3 — Defer Until Tier 1-2 Prove Insufficient

**#7: Computed Value Expressions**
Expression AST with arithmetic (+, -, *, /) and variable references:
`value > ?x + 10`, `distance(?pos_a, ?pos_b) > threshold`. Anti-cheat
(speed = distance/time), finance (spread), clinical (GFR formulas).

- Scope: ~200-300 LoC
- Effort: **Medium-hard** (expression evaluator design, zero-dep constraint)
- Recommend deferring until #3 (BoundVar) proves insufficient

**#8: DataSource Adapter Helpers**
Pre-built adapters or adapter-building utilities for common data formats:
OpenTelemetry spans, JSON event streams, syslog/CEF. Lowers the barrier
for non-narrative domains.

- Scope: ~200-300 LoC per adapter
- Effort: **Small per adapter**
- Recommend building ONE generic event-stream adapter first, then
  domain-specific ones as demand appears

### Recommended Execution Order

**Sprint 1** (quick wins, unblock everything):
  #1 (metadata) → #2 (timeout/absence) → #3 (cross-stage comparison)
  Independent, no architectural changes, together unlock the core
  "temporal absence + value comparison" expressiveness every non-narrative
  domain needs. Metadata makes the platform feel multi-domain-ready.

**Sprint 2** (threshold patterns):
  #4 (repeat with range)
  Extends existing composition infrastructure. Handles the most common
  counting use case without windowed aggregation complexity.

**Sprint 3** (concurrent matching):
  #5 (unordered stages)
  Most architecturally involved change, but unlocks multi-signal
  correlation central to observability and clinical monitoring.

**Defer:** #6 until repeat-with-range proves insufficient, #7 until
cross-stage comparison proves insufficient, #8 until a specific domain
integration is prioritized.

---

## Architectural Fit Assessment

| Property | Implication |
|----------|-------------|
| Zero-dep Rust core | Embeddable: SIEM vendor, medical device, game server, edge gateway |
| WASM target | Browser tools, IoT edge, client-side game logic |
| `DataSource` trait | Plug into Neo4j, OMOP databases, OTel stores, custom backends |
| 28us/edge | Sufficient for most real-time domains except raw HFT data |
| DSL + TypeMapper | Domain experts write patterns without Rust knowledge |

### Competitive Positioning

Fabula's strongest positioning is as an **embeddable detection engine
library** (not an end-user product) for:

1. **SIEM vendors** seeking temporal pattern matching beyond sequence-with-
   maxspan capabilities
2. **Fraud detection platforms** needing explainable pattern matching to
   complement ML scoring (EU AI Act regulatory driver)
3. **Observability platforms** seeking user-defined failure cascade
   detection beyond statistical anomaly alerting
4. **Clinical informatics** systems needing protocol compliance monitoring
   with Allen algebra and negation
5. **Government/defense contractors** building provenance-based threat
   detection (DARPA program alignment)
6. **Detection engineering tooling** — a runtime for a "temporal Sigma"
   compiling human-authored patterns into incremental graph matchers

---

## References

### Existing Tools Compared

- **CEP/Stream Processing**: Apache Flink CEP, Esper, Siddhi — flat event
  streams, sequence patterns with time windows, no graph topology, no Allen
  algebra, no negation windows with variable bindings
- **SIEM**: Splunk SPL, Elastic EQL, Sigma rules — point events, limited
  temporal sequencing, negation limited or absent
- **Tracing**: Grafana TraceQL, Jaeger, Honeycomb — single-trace queries,
  no cross-trace correlation, limited/experimental negation
- **Process Mining**: ProM, Celonis, Disco — Petri net conformance, no
  Allen algebra, no incremental streaming, no negation windows
- **Compliance**: Declare/LTLf — boolean satisfaction, no graduated
  matching, no variable-bound negation
- **Clinical**: OHDSI/OMOP TPD, process mining on EHR — single temporal
  association statistics, not multi-stage graph pattern matching

### Academic Work

- GrapeL (Ehmes et al., 2020) — incremental graph pattern matching + CEP
  via two separate engines (eMoflon + Apama). Fabula unifies both.
- SLEUTH, HOLMES, MORSE (Hossain et al., Milajerdi et al.) — batch graph
  pattern matching for provenance-based intrusion detection. Fabula's
  incremental approach addresses their latency limitation.
- TEO (Mayo Clinic, PMC7647306) — validated Allen's 13 relations capture
  >95% of temporal expressions in real EHR data.
- Schulz et al. (2024) "Narrative Information Theory" — JSD-based pivot
  detection. Already implemented in fabula-narratives.
