# Pattern Discovery Framework — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pluggable generate-evaluate framework for discovering sifting patterns from simulation trace data, with one concrete generator (MINERful-adapted) as proof of concept.

**Architecture:** A `fabula-discovery` workspace crate providing traits (`CandidateGenerator`, `PatternEvaluator`, `PatternFilter`), a `TraceCorpus` abstraction over edge logs, a `DiscoverySession` orchestrator that runs the generate-evaluate loop, and a `pattern_to_dsl()` reverse compiler. Ships with a MINERful-adapted generator that discovers Allen-relation constraints from pairwise edge co-occurrence.

**Tech Stack:** Rust, fabula core (Pattern, evaluate_pattern, gap_analysis), fabula-narratives (scorer), fabula-memory (MemGraph for tests), fabula-dsl (DSL emission)

---

## File Structure

```
crates/fabula-discovery/
├── Cargo.toml
├── src/
│   ├── lib.rs              — module declarations, top-level re-exports
│   ├── corpus.rs           — TraceCorpus: edge log with indexing
│   ├── traits.rs           — CandidateGenerator, PatternEvaluator, PatternFilter traits
│   ├── score.rs            — PatternScore, ScoredPattern types
│   ├── session.rs          — DiscoverySession orchestrator
│   ├── emit.rs             — pattern_to_dsl() reverse compiler
│   ├── evaluators/
│   │   ├── mod.rs          — re-exports
│   │   ├── surprise.rs     — statistical surprise evaluator (interest factor)
│   │   └── narrative.rs    — narrative quality evaluator (via fabula-narratives)
│   └── generators/
│       ├── mod.rs          — re-exports
│       └── minerful.rs     — MINERful-adapted Allen-relation constraint miner
└── tests/
    ├── corpus_tests.rs     — TraceCorpus construction and indexing
    ├── emit_tests.rs       — pattern_to_dsl round-trip tests
    ├── surprise_tests.rs   — statistical surprise evaluator
    ├── minerful_tests.rs   — MINERful generator end-to-end
    └── session_tests.rs    — DiscoverySession integration
```

Each file has one clear responsibility. The `evaluators/` and `generators/` directories mirror the trait structure — new strategies are new files, not modifications to existing ones.

---

### Task 1: Crate Scaffold

**Files:**
- Create: `crates/fabula-discovery/Cargo.toml`
- Create: `crates/fabula-discovery/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "fabula-discovery"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Automated sifting pattern discovery for fabula"
publish = false

[dependencies]
fabula = { path = "../fabula" }
fabula-narratives = { path = "../fabula-narratives" }
fabula-dsl = { path = "../fabula-dsl" }

[dev-dependencies]
fabula-memory = { path = "../fabula-memory" }
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! Automated sifting pattern discovery.
//!
//! Provides a pluggable generate-evaluate framework for discovering
//! narratively interesting patterns from simulation trace data.
//!
//! # Architecture
//!
//! The system works as an iterative loop:
//! 1. A [`CandidateGenerator`] proposes candidate patterns from a [`TraceCorpus`]
//! 2. [`PatternEvaluator`]s score each candidate
//! 3. Scored results feed back to the generator for the next round
//! 4. A [`PatternFilter`] decides which patterns to keep
//!
//! The [`DiscoverySession`] orchestrates this loop with configurable budgets.

mod corpus;
mod emit;
mod score;
mod session;
mod traits;

pub mod evaluators;
pub mod generators;

pub use corpus::TraceCorpus;
pub use emit::pattern_to_dsl;
pub use score::{PatternScore, ScoredPattern};
pub use session::{DiscoverySession, SessionConfig, SessionHistory};
pub use traits::{CandidateGenerator, PatternEvaluator, PatternFilter};
```

- [ ] **Step 3: Add to workspace members**

In root `Cargo.toml`, add `"crates/fabula-discovery"` to the `members` list after `"crates/fabula-examples"`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p fabula-discovery`

Expected: Build succeeds (with warnings about unused imports — that's fine, the modules don't exist yet). Actually it will fail because the modules don't exist. Create empty placeholder files first:

```bash
mkdir -p crates/fabula-discovery/src/evaluators crates/fabula-discovery/src/generators
touch crates/fabula-discovery/src/{corpus,emit,score,session,traits}.rs
touch crates/fabula-discovery/src/evaluators/mod.rs
touch crates/fabula-discovery/src/generators/mod.rs
```

Run: `cargo build -p fabula-discovery`

Expected: Compiles with warnings about unused imports in lib.rs. That's expected — the re-exports reference types that don't exist yet. Comment out the `pub use` lines for now; each task will uncomment them as types are defined.

- [ ] **Step 5: Commit**

```bash
git add crates/fabula-discovery/ Cargo.toml
git commit -m "feat(discovery): scaffold fabula-discovery crate"
```

---

### Task 2: TraceCorpus

**Files:**
- Create: `crates/fabula-discovery/src/corpus.rs`
- Create: `crates/fabula-discovery/tests/corpus_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/fabula-discovery/tests/corpus_tests.rs`:

```rust
use fabula::datasource::Edge;
use fabula::interval::Interval;
use fabula_discovery::TraceCorpus;

#[test]
fn corpus_from_edges() {
    let edges = vec![
        ("alice", "trusts", "bob", 1i64, Some(5i64)),
        ("bob", "betrays", "alice", 3, None),
        ("alice", "trusts", "carol", 2, Some(4)),
    ];

    let trace_edges: Vec<_> = edges
        .into_iter()
        .map(|(src, lbl, tgt, start, end)| {
            (
                src.to_string(),
                lbl.to_string(),
                tgt.to_string(),
                Interval {
                    start,
                    end,
                },
            )
        })
        .collect();

    let corpus = TraceCorpus::new(trace_edges);
    assert_eq!(corpus.len(), 3);
    assert_eq!(corpus.labels().len(), 2); // "trusts", "betrays"
    assert_eq!(corpus.edges_with_label("trusts").len(), 2);
    assert_eq!(corpus.edges_with_label("betrays").len(), 1);
    assert_eq!(corpus.edges_with_label("unknown").len(), 0);
    assert_eq!(corpus.time_range(), (1, 5));
    assert_eq!(corpus.nodes().len(), 3); // alice, bob, carol
}

#[test]
fn corpus_split() {
    let trace_edges = vec![
        ("a".into(), "x".into(), "b".into(), Interval { start: 1i64, end: Some(2) }),
        ("a".into(), "y".into(), "c".into(), Interval { start: 3, end: Some(4) }),
        ("b".into(), "x".into(), "c".into(), Interval { start: 5, end: Some(6) }),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let (train, test) = corpus.split_at(&3);
    assert_eq!(train.len(), 1); // only edge starting before t=3
    assert_eq!(test.len(), 2);  // edges starting at t=3 and t=5
}

#[test]
fn corpus_label_pairs() {
    let trace_edges = vec![
        ("alice".into(), "trusts".into(), "bob".into(), Interval { start: 1i64, end: Some(5) }),
        ("bob".into(), "betrays".into(), "alice".into(), Interval { start: 3, end: None }),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let pairs = corpus.label_pairs();
    // Should include ("trusts", "betrays") and ("betrays", "trusts")
    assert!(pairs.len() >= 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-discovery --test corpus_tests`

Expected: FAIL — `TraceCorpus` not found.

- [ ] **Step 3: Implement TraceCorpus**

Write `crates/fabula-discovery/src/corpus.rs`:

```rust
use fabula::interval::{AllenRelation, Interval};
use std::collections::{HashMap, HashSet};

/// A single edge in a trace corpus.
#[derive(Debug, Clone)]
pub struct TraceEdge {
    pub source: String,
    pub label: String,
    pub target: String,
    pub interval: Interval<i64>,
}

/// An indexed log of edges for pattern discovery.
///
/// Built from a simulation's edge history. Provides indexed access
/// by label, by node, and by time range for efficient mining.
#[derive(Debug, Clone)]
pub struct TraceCorpus {
    edges: Vec<TraceEdge>,
    by_label: HashMap<String, Vec<usize>>,
    by_source: HashMap<String, Vec<usize>>,
    by_target: HashMap<String, Vec<usize>>,
}

impl TraceCorpus {
    /// Build a corpus from a list of (source, label, target, interval) tuples.
    pub fn new(raw: Vec<(String, String, String, Interval<i64>)>) -> Self {
        let edges: Vec<TraceEdge> = raw
            .into_iter()
            .map(|(source, label, target, interval)| TraceEdge {
                source,
                label,
                target,
                interval,
            })
            .collect();

        let mut by_label: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_source: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_target: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, e) in edges.iter().enumerate() {
            by_label.entry(e.label.clone()).or_default().push(i);
            by_source.entry(e.source.clone()).or_default().push(i);
            by_target.entry(e.target.clone()).or_default().push(i);
        }

        Self {
            edges,
            by_label,
            by_source,
            by_target,
        }
    }

    pub fn len(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn edges(&self) -> &[TraceEdge] {
        &self.edges
    }

    /// All distinct labels in the corpus.
    pub fn labels(&self) -> HashSet<&str> {
        self.by_label.keys().map(|s| s.as_str()).collect()
    }

    /// All distinct nodes (sources and targets) in the corpus.
    pub fn nodes(&self) -> HashSet<&str> {
        let mut nodes: HashSet<&str> = HashSet::new();
        for e in &self.edges {
            nodes.insert(&e.source);
            nodes.insert(&e.target);
        }
        nodes
    }

    /// Edges with a given label.
    pub fn edges_with_label(&self, label: &str) -> Vec<&TraceEdge> {
        self.by_label
            .get(label)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// Edges originating from a given node.
    pub fn edges_from_node(&self, node: &str) -> Vec<&TraceEdge> {
        self.by_source
            .get(node)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// (min_start, max_end) across all edges.
    /// For open-ended intervals, uses start as the end bound.
    pub fn time_range(&self) -> (i64, i64) {
        let min = self.edges.iter().map(|e| e.interval.start).min().unwrap_or(0);
        let max = self
            .edges
            .iter()
            .map(|e| e.interval.end.unwrap_or(e.interval.start))
            .max()
            .unwrap_or(0);
        (min, max)
    }

    /// Split into two corpora at time `t`.
    /// Edges with `start < t` go to the first corpus; the rest to the second.
    pub fn split_at(&self, t: &i64) -> (Self, Self) {
        let (before, after): (Vec<_>, Vec<_>) = self
            .edges
            .iter()
            .cloned()
            .map(|e| (e.source.clone(), e.label.clone(), e.target.clone(), e.interval.clone()))
            .partition(|(_, _, _, iv)| iv.start < *t);

        (Self::new(before), Self::new(after))
    }

    /// All ordered pairs of distinct labels.
    pub fn label_pairs(&self) -> Vec<(&str, &str)> {
        let labels: Vec<&str> = self.labels().into_iter().collect();
        let mut pairs = Vec::new();
        for &a in &labels {
            for &b in &labels {
                if a != b {
                    pairs.push((a, b));
                }
            }
        }
        pairs
    }

    /// For a pair of labels, find all instances where edges share a node
    /// (source of one matches source or target of the other) and compute
    /// the Allen relation between their intervals.
    pub fn pairwise_relations(
        &self,
        label_a: &str,
        label_b: &str,
    ) -> Vec<PairwiseHit> {
        let edges_a = self.edges_with_label(label_a);
        let edges_b = self.edges_with_label(label_b);
        let mut hits = Vec::new();

        for a in &edges_a {
            for b in &edges_b {
                // Check for shared node (source-source, source-target, target-source)
                let shared = if a.source == b.source {
                    Some(SharedNode::Source(a.source.clone()))
                } else if a.source == b.target {
                    Some(SharedNode::SourceTarget(a.source.clone()))
                } else if a.target == b.source {
                    Some(SharedNode::TargetSource(a.target.clone()))
                } else {
                    None
                };

                if let Some(shared_node) = shared {
                    if let Some(relation) = a.interval.relation(&b.interval) {
                        hits.push(PairwiseHit {
                            edge_a_idx: self.edges.iter().position(|e| std::ptr::eq(e, *a)).unwrap_or(0),
                            edge_b_idx: self.edges.iter().position(|e| std::ptr::eq(e, *b)).unwrap_or(0),
                            shared_node,
                            relation,
                        });
                    }
                }
            }
        }

        hits
    }
}

/// How two edges share a node.
#[derive(Debug, Clone, PartialEq)]
pub enum SharedNode {
    /// Both edges have the same source.
    Source(String),
    /// Edge A's source equals edge B's target.
    SourceTarget(String),
    /// Edge A's target equals edge B's source.
    TargetSource(String),
}

/// A co-occurrence of two edges sharing a node with a computed Allen relation.
#[derive(Debug, Clone)]
pub struct PairwiseHit {
    pub edge_a_idx: usize,
    pub edge_b_idx: usize,
    pub shared_node: SharedNode,
    pub relation: AllenRelation,
}
```

- [ ] **Step 4: Uncomment the `TraceCorpus` re-export in lib.rs**

In `crates/fabula-discovery/src/lib.rs`, uncomment:
```rust
pub use corpus::TraceCorpus;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p fabula-discovery --test corpus_tests`

Expected: All 3 tests pass.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p fabula-discovery -- -D warnings`

Expected: No warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/fabula-discovery/src/corpus.rs crates/fabula-discovery/tests/corpus_tests.rs crates/fabula-discovery/src/lib.rs
git commit -m "feat(discovery): TraceCorpus — indexed edge log for pattern mining"
```

---

### Task 3: Core Traits and Score Types

**Files:**
- Create: `crates/fabula-discovery/src/traits.rs`
- Create: `crates/fabula-discovery/src/score.rs`

- [ ] **Step 1: Write score types**

Create `crates/fabula-discovery/src/score.rs`:

```rust
use fabula::pattern::Pattern;
use std::collections::HashMap;

/// Per-evaluator scores for a candidate pattern.
#[derive(Debug, Clone, Default)]
pub struct PatternScore {
    /// Named scores from each evaluator that ran.
    pub scores: HashMap<String, f64>,
    /// Which round of the session produced this.
    pub round: usize,
    /// Which generator produced the candidate.
    pub generator: String,
}

impl PatternScore {
    /// Weighted composite score across all evaluators.
    pub fn composite(&self, weights: &HashMap<String, f64>) -> f64 {
        self.scores
            .iter()
            .map(|(name, &value)| {
                let w = weights.get(name).copied().unwrap_or(1.0);
                w * value
            })
            .sum()
    }
}

/// A candidate pattern paired with its evaluation scores.
#[derive(Debug, Clone)]
pub struct ScoredPattern<L, V> {
    pub pattern: Pattern<L, V>,
    pub score: PatternScore,
}
```

- [ ] **Step 2: Write trait definitions**

Create `crates/fabula-discovery/src/traits.rs`:

```rust
use crate::corpus::TraceCorpus;
use crate::score::{PatternScore, ScoredPattern};
use fabula::pattern::Pattern;

/// Proposes candidate patterns. Receives scored feedback to guide the next round.
///
/// Generators maintain internal state across rounds — a population of
/// high-scoring patterns, frequency tables, or conversation history with an LLM.
pub trait CandidateGenerator {
    /// Propose up to `budget` candidate patterns from the corpus.
    fn generate(
        &mut self,
        corpus: &TraceCorpus,
        budget: usize,
    ) -> Vec<Pattern<String, String>>;

    /// Receive scored results from the previous round.
    /// The generator can use these to guide future proposals.
    fn feedback(&mut self, scored: &[ScoredPattern<String, String>]);

    /// Human-readable name for this generator (used in score metadata).
    fn name(&self) -> &str;
}

/// Scores a candidate pattern against a corpus.
///
/// Multiple evaluators can run on the same candidate. Each returns a named
/// score that is aggregated into a [`PatternScore`].
pub trait PatternEvaluator {
    /// Score a candidate pattern.
    fn evaluate(
        &self,
        pattern: &Pattern<String, String>,
        corpus: &TraceCorpus,
    ) -> f64;

    /// Human-readable name for this evaluator (used as score key).
    fn name(&self) -> &str;
}

/// Decides whether a scored pattern is worth keeping.
pub trait PatternFilter {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool;
}

/// A filter that accepts patterns whose composite score exceeds a threshold.
pub struct ThresholdFilter {
    pub threshold: f64,
    pub weights: std::collections::HashMap<String, f64>,
}

impl PatternFilter for ThresholdFilter {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool {
        scored.score.composite(&self.weights) >= self.threshold
    }
}
```

- [ ] **Step 3: Uncomment re-exports in lib.rs**

In `crates/fabula-discovery/src/lib.rs`, uncomment:
```rust
pub use score::{PatternScore, ScoredPattern};
pub use traits::{CandidateGenerator, PatternEvaluator, PatternFilter};
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p fabula-discovery`

Expected: Compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/fabula-discovery/src/traits.rs crates/fabula-discovery/src/score.rs crates/fabula-discovery/src/lib.rs
git commit -m "feat(discovery): core traits — CandidateGenerator, PatternEvaluator, PatternFilter"
```

---

### Task 4: DiscoverySession Orchestrator

**Files:**
- Create: `crates/fabula-discovery/src/session.rs`
- Create: `crates/fabula-discovery/tests/session_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/fabula-discovery/tests/session_tests.rs`:

```rust
use fabula::interval::Interval;
use fabula::pattern::Pattern;
use fabula_discovery::{
    CandidateGenerator, DiscoverySession, PatternEvaluator, PatternFilter, PatternScore,
    ScoredPattern, SessionConfig, TraceCorpus,
};
use std::collections::HashMap;

/// A dummy generator that emits a fixed pattern each round.
struct FixedGenerator {
    pattern: Pattern<String, String>,
    feedback_count: usize,
}

impl CandidateGenerator for FixedGenerator {
    fn generate(
        &mut self,
        _corpus: &TraceCorpus,
        budget: usize,
    ) -> Vec<Pattern<String, String>> {
        vec![self.pattern.clone(); budget.min(1)]
    }

    fn feedback(&mut self, _scored: &[ScoredPattern<String, String>]) {
        self.feedback_count += 1;
    }

    fn name(&self) -> &str {
        "fixed"
    }
}

/// A dummy evaluator that always returns 0.8.
struct ConstantEvaluator;

impl PatternEvaluator for ConstantEvaluator {
    fn evaluate(
        &self,
        _pattern: &Pattern<String, String>,
        _corpus: &TraceCorpus,
    ) -> f64 {
        0.8
    }

    fn name(&self) -> &str {
        "constant"
    }
}

/// Accept everything.
struct AcceptAll;

impl PatternFilter for AcceptAll {
    fn accept(&self, _scored: &ScoredPattern<String, String>) -> bool {
        true
    }
}

fn make_corpus() -> TraceCorpus {
    TraceCorpus::new(vec![
        ("a".into(), "trusts".into(), "b".into(), Interval { start: 1i64, end: Some(5) }),
        ("b".into(), "betrays".into(), "a".into(), Interval { start: 3, end: None }),
    ])
}

fn make_pattern() -> Pattern<String, String> {
    use fabula::builder::PatternBuilder;
    PatternBuilder::new("test_pattern")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build()
}

#[test]
fn session_runs_configured_rounds() {
    let corpus = make_corpus();
    let generator = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };
    let config = SessionConfig {
        max_rounds: 3,
        candidates_per_round: 2,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(ConstantEvaluator)],
        AcceptAll,
    );

    assert_eq!(result.rounds, 3);
    assert!(!result.accepted.is_empty());
}

#[test]
fn session_history_tracks_all_candidates() {
    let corpus = make_corpus();
    let generator = FixedGenerator {
        pattern: make_pattern(),
        feedback_count: 0,
    };
    let config = SessionConfig {
        max_rounds: 2,
        candidates_per_round: 1,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(ConstantEvaluator)],
        AcceptAll,
    );

    // 2 rounds × 1 candidate = 2 total evaluated
    assert_eq!(result.all_scored.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-discovery --test session_tests`

Expected: FAIL — types not found.

- [ ] **Step 3: Implement DiscoverySession**

Write `crates/fabula-discovery/src/session.rs`:

```rust
use crate::corpus::TraceCorpus;
use crate::score::{PatternScore, ScoredPattern};
use crate::traits::{CandidateGenerator, PatternEvaluator, PatternFilter};

/// Configuration for a discovery session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Maximum number of generate-evaluate rounds.
    pub max_rounds: usize,
    /// How many candidates to request per round.
    pub candidates_per_round: usize,
}

/// The result of a completed discovery session.
#[derive(Debug)]
pub struct SessionHistory {
    /// How many rounds actually ran.
    pub rounds: usize,
    /// All candidates that were evaluated, in order.
    pub all_scored: Vec<ScoredPattern<String, String>>,
    /// Candidates that passed the filter.
    pub accepted: Vec<ScoredPattern<String, String>>,
}

/// Orchestrates the generate-evaluate loop.
pub struct DiscoverySession {
    config: SessionConfig,
}

impl DiscoverySession {
    pub fn new(config: SessionConfig) -> Self {
        Self { config }
    }

    /// Run the full discovery loop.
    ///
    /// 1. Generator proposes candidates
    /// 2. Evaluators score each candidate
    /// 3. Filter decides which to keep
    /// 4. Scored results feed back to the generator
    /// 5. Repeat for `max_rounds`
    pub fn run(
        &mut self,
        corpus: &TraceCorpus,
        mut generator: impl CandidateGenerator,
        evaluators: Vec<Box<dyn PatternEvaluator>>,
        filter: impl PatternFilter,
    ) -> SessionHistory {
        let mut all_scored = Vec::new();
        let mut accepted = Vec::new();

        for round in 0..self.config.max_rounds {
            let candidates = generator.generate(corpus, self.config.candidates_per_round);

            let mut round_scored = Vec::new();
            for pattern in candidates {
                let mut scores = std::collections::HashMap::new();
                for evaluator in &evaluators {
                    let value = evaluator.evaluate(&pattern, corpus);
                    scores.insert(evaluator.name().to_string(), value);
                }

                let scored = ScoredPattern {
                    pattern,
                    score: PatternScore {
                        scores,
                        round,
                        generator: generator.name().to_string(),
                    },
                };

                if filter.accept(&scored) {
                    accepted.push(scored.clone());
                }

                round_scored.push(scored);
            }

            generator.feedback(&round_scored);
            all_scored.extend(round_scored);
        }

        SessionHistory {
            rounds: self.config.max_rounds,
            all_scored,
            accepted,
        }
    }
}
```

- [ ] **Step 4: Uncomment re-exports in lib.rs**

In `crates/fabula-discovery/src/lib.rs`, uncomment:
```rust
pub use session::{DiscoverySession, SessionConfig, SessionHistory};
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p fabula-discovery --test session_tests`

Expected: Both tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/fabula-discovery/src/session.rs crates/fabula-discovery/tests/session_tests.rs crates/fabula-discovery/src/lib.rs
git commit -m "feat(discovery): DiscoverySession — generate-evaluate loop orchestrator"
```

---

### Task 5: Pattern-to-DSL Emitter

**Files:**
- Create: `crates/fabula-discovery/src/emit.rs`
- Create: `crates/fabula-discovery/tests/emit_tests.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/fabula-discovery/tests/emit_tests.rs`:

```rust
use fabula::builder::PatternBuilder;
use fabula::interval::AllenRelation;
use fabula_discovery::pattern_to_dsl;

#[test]
fn emit_simple_two_stage() {
    let pattern = PatternBuilder::<String, String>::new("hospitality")
        .stage("e1", |s| {
            s.edge_bind("e1", "arrives".to_string(), "guest")
        })
        .stage("e2", |s| {
            s.edge_bind("e2", "greets".to_string(), "guest")
        })
        .temporal("e1", AllenRelation::Before, "e2")
        .build();

    let dsl = pattern_to_dsl(&pattern);

    // Should parse back without error
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(parsed.is_ok(), "Round-trip failed: {}\nDSL:\n{}", parsed.unwrap_err(), dsl);
}

#[test]
fn emit_with_negation() {
    let pattern = PatternBuilder::<String, String>::new("trust_unbroken")
        .stage("e1", |s| {
            s.edge_bind("e1", "trusts".to_string(), "target")
        })
        .stage("e2", |s| {
            s.edge_bind("e2", "helps".to_string(), "target")
        })
        .temporal("e1", AllenRelation::Before, "e2")
        .unless_between("e1", "e2", |n| {
            n.edge_bind("e1", "betrays".to_string(), "target")
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(parsed.is_ok(), "Round-trip failed: {}\nDSL:\n{}", parsed.unwrap_err(), dsl);
}

#[test]
fn emit_with_literal_value() {
    let pattern = PatternBuilder::<String, String>::new("specific_value")
        .stage("e1", |s| {
            s.edge("e1", "status".to_string(), "active".to_string())
        })
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(dsl.contains("\"active\""), "DSL should contain quoted literal: {}", dsl);
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(parsed.is_ok(), "Round-trip failed: {}\nDSL:\n{}", parsed.unwrap_err(), dsl);
}

#[test]
fn emit_private_pattern() {
    let pattern = PatternBuilder::<String, String>::new("hidden")
        .stage("e1", |s| {
            s.edge_bind("e1", "event".to_string(), "target")
        })
        .private()
        .build();

    let dsl = pattern_to_dsl(&pattern);
    assert!(dsl.contains("private"), "DSL should contain 'private': {}", dsl);
    let parsed = fabula_dsl::parse_document(&dsl);
    assert!(parsed.is_ok(), "Round-trip failed: {}\nDSL:\n{}", parsed.unwrap_err(), dsl);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-discovery --test emit_tests`

Expected: FAIL — `pattern_to_dsl` not found.

- [ ] **Step 3: Implement pattern_to_dsl**

Write `crates/fabula-discovery/src/emit.rs`:

```rust
use fabula::interval::AllenRelation;
use fabula::pattern::{Negation, Pattern, Stage, Target, TemporalConstraint};

/// Convert a `Pattern<String, String>` to fabula DSL text.
///
/// The output is valid fabula DSL that can be parsed by `fabula_dsl::parse_document()`.
pub fn pattern_to_dsl(pattern: &Pattern<String, String>) -> String {
    let mut out = String::new();

    if pattern.private {
        out.push_str("private ");
    }

    out.push_str(&format!("pattern {} {{\n", pattern.name));

    for stage in &pattern.stages {
        emit_stage(&mut out, stage);
    }

    for temporal in &pattern.temporal {
        emit_temporal(&mut out, temporal);
    }

    for negation in &pattern.negations {
        emit_negation(&mut out, negation);
    }

    if let Some(ticks) = pattern.deadline_ticks {
        out.push_str(&format!("  deadline {}\n", ticks));
    }

    for (key, value) in &pattern.metadata {
        out.push_str(&format!("  meta \"{}\" \"{}\"\n", key, value));
    }

    out.push_str("}\n");
    out
}

fn emit_stage(out: &mut String, stage: &Stage<String, String>) {
    out.push_str(&format!("  stage {} {{\n", stage.anchor.0));
    for clause in &stage.clauses {
        let prefix = if clause.negated { "!" } else { "" };
        let target_str = match &clause.target {
            Target::Bind(var) => format!("?{}", var.0),
            Target::Literal(val) => format!("\"{}\"", val),
            Target::Constraint(c) => emit_constraint(c),
        };
        out.push_str(&format!(
            "    {}?{} .{} {}\n",
            prefix, clause.source.0, clause.label, target_str
        ));
    }
    out.push_str("  }\n");
}

fn emit_constraint(c: &fabula::datasource::ValueConstraint<String>) -> String {
    use fabula::datasource::ValueConstraint;
    match c {
        ValueConstraint::Eq(v) => format!("\"{}\"", v),
        ValueConstraint::Lt(v) => format!("< \"{}\"", v),
        ValueConstraint::Gt(v) => format!("> \"{}\"", v),
        ValueConstraint::Lte(v) => format!("<= \"{}\"", v),
        ValueConstraint::Gte(v) => format!(">= \"{}\"", v),
        ValueConstraint::Between(lo, hi) => format!("\"{}\"..\"{}\"", lo, hi),
        ValueConstraint::Any => "_".to_string(),
        ValueConstraint::EqVar(v) => format!("?{}", v),
        ValueConstraint::LtVar(v) => format!("< ?{}", v),
        ValueConstraint::GtVar(v) => format!("> ?{}", v),
        ValueConstraint::LteVar(v) => format!("<= ?{}", v),
        ValueConstraint::GteVar(v) => format!(">= ?{}", v),
    }
}

fn emit_temporal(out: &mut String, tc: &TemporalConstraint) {
    let relation_str = match tc.relation {
        AllenRelation::Before => "before",
        AllenRelation::After => "after",
        AllenRelation::Meets => "meets",
        AllenRelation::MetBy => "met_by",
        AllenRelation::Overlaps => "overlaps",
        AllenRelation::OverlappedBy => "overlapped_by",
        AllenRelation::Starts => "starts",
        AllenRelation::StartedBy => "started_by",
        AllenRelation::During => "during",
        AllenRelation::Contains => "contains",
        AllenRelation::Finishes => "finishes",
        AllenRelation::FinishedBy => "finished_by",
        AllenRelation::Equals => "equals",
    };

    out.push_str(&format!(
        "  {} {} {}\n",
        tc.left.0, relation_str, tc.right.0
    ));

    if let Some(ref gap) = tc.gap {
        if let (Some(min), Some(max)) = (gap.min, gap.max) {
            out.push_str(&format!("  gap {} {} {}..{}\n", tc.left.0, tc.right.0, min, max));
        }
    }
}

fn emit_negation(out: &mut String, neg: &Negation<String, String>) {
    if neg.is_global {
        out.push_str("  unless_global {\n");
    } else if let Some(ref end) = neg.between_end {
        out.push_str(&format!(
            "  unless_between {} {} {{\n",
            neg.between_start.0, end.0
        ));
    } else {
        out.push_str(&format!("  unless_after {} {{\n", neg.between_start.0));
    }

    for clause in &neg.clauses {
        let target_str = match &clause.target {
            Target::Bind(var) => format!("?{}", var.0),
            Target::Literal(val) => format!("\"{}\"", val),
            Target::Constraint(c) => emit_constraint(c),
        };
        out.push_str(&format!(
            "    ?{} .{} {}\n",
            clause.source.0, clause.label, target_str
        ));
    }

    out.push_str("  }\n");
}
```

- [ ] **Step 4: Uncomment re-export in lib.rs**

In `crates/fabula-discovery/src/lib.rs`, uncomment:
```rust
pub use emit::pattern_to_dsl;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p fabula-discovery --test emit_tests`

Expected: All 4 tests pass. If any fail on round-trip parsing, examine the emitted DSL and fix the formatting to match what `fabula_dsl::parse_document()` expects. The DSL parser is the ground truth — adjust the emitter until it produces parseable output.

- [ ] **Step 6: Commit**

```bash
git add crates/fabula-discovery/src/emit.rs crates/fabula-discovery/tests/emit_tests.rs crates/fabula-discovery/src/lib.rs
git commit -m "feat(discovery): pattern_to_dsl — reverse compiler for discovered patterns"
```

---

### Task 6: Statistical Surprise Evaluator

**Files:**
- Create: `crates/fabula-discovery/src/evaluators/mod.rs`
- Create: `crates/fabula-discovery/src/evaluators/surprise.rs`
- Create: `crates/fabula-discovery/tests/surprise_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/fabula-discovery/tests/surprise_tests.rs`:

```rust
use fabula::builder::PatternBuilder;
use fabula::interval::{AllenRelation, Interval};
use fabula_discovery::evaluators::SurpriseEvaluator;
use fabula_discovery::{PatternEvaluator, TraceCorpus};

fn make_corpus() -> TraceCorpus {
    // 10 "trusts" edges, 2 "betrays" edges, 1 "trusts then betrays" co-occurrence
    // "betrays" is rare, so a pattern matching it should score higher
    let mut edges = Vec::new();
    for i in 0..10 {
        edges.push((
            format!("a{}", i),
            "trusts".into(),
            "b".into(),
            Interval { start: i * 10i64, end: Some(i * 10 + 5) },
        ));
    }
    edges.push((
        "x".into(),
        "betrays".into(),
        "b".into(),
        Interval { start: 50, end: Some(55) },
    ));
    edges.push((
        "y".into(),
        "betrays".into(),
        "b".into(),
        Interval { start: 70, end: Some(75) },
    ));
    TraceCorpus::new(edges)
}

#[test]
fn rare_label_scores_higher() {
    let corpus = make_corpus();
    let eval = SurpriseEvaluator;

    let common_pattern = PatternBuilder::<String, String>::new("common")
        .stage("e1", |s| s.edge_bind("e1", "trusts".to_string(), "target"))
        .build();

    let rare_pattern = PatternBuilder::<String, String>::new("rare")
        .stage("e1", |s| s.edge_bind("e1", "betrays".to_string(), "target"))
        .build();

    let common_score = eval.evaluate(&common_pattern, &corpus);
    let rare_score = eval.evaluate(&rare_pattern, &corpus);

    assert!(
        rare_score > common_score,
        "rare pattern ({}) should score higher than common pattern ({})",
        rare_score,
        common_score
    );
}

#[test]
fn empty_match_scores_zero() {
    let corpus = make_corpus();
    let eval = SurpriseEvaluator;

    let no_match = PatternBuilder::<String, String>::new("nonexistent")
        .stage("e1", |s| s.edge_bind("e1", "unknown_label".to_string(), "target"))
        .build();

    let score = eval.evaluate(&no_match, &corpus);
    assert_eq!(score, 0.0, "no-match pattern should score 0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-discovery --test surprise_tests`

Expected: FAIL — `SurpriseEvaluator` not found.

- [ ] **Step 3: Implement SurpriseEvaluator**

Write `crates/fabula-discovery/src/evaluators/mod.rs`:

```rust
mod surprise;
mod narrative;

pub use surprise::SurpriseEvaluator;
pub use narrative::NarrativeEvaluator;
```

Write `crates/fabula-discovery/src/evaluators/surprise.rs`:

```rust
use crate::corpus::TraceCorpus;
use crate::traits::PatternEvaluator;
use fabula::pattern::Pattern;

/// Scores patterns by statistical surprise.
///
/// Uses an interest factor inspired by MINERful:
/// `match_count / expected_count_under_independence`.
///
/// A pattern that matches more often than random label co-occurrence
/// would predict gets a high score. A pattern that matches less than
/// expected (or not at all) gets a low score.
pub struct SurpriseEvaluator;

impl PatternEvaluator for SurpriseEvaluator {
    fn evaluate(
        &self,
        pattern: &Pattern<String, String>,
        corpus: &TraceCorpus,
    ) -> f64 {
        if corpus.is_empty() || pattern.stages.is_empty() {
            return 0.0;
        }

        // Count how many edges in the corpus match each stage's first clause label
        let total = corpus.len() as f64;
        let mut label_freqs: Vec<f64> = Vec::new();

        for stage in &pattern.stages {
            if let Some(clause) = stage.clauses.first() {
                let count = corpus.edges_with_label(&clause.label).len() as f64;
                label_freqs.push(count / total);
            }
        }

        if label_freqs.is_empty() {
            return 0.0;
        }

        // Expected co-occurrence under independence: product of individual frequencies
        let expected_freq: f64 = label_freqs.iter().product();
        if expected_freq == 0.0 {
            return 0.0;
        }

        // Actual match count — use a simple heuristic based on shared-node co-occurrence
        // For single-stage patterns, observed = label frequency
        // For multi-stage, count pairwise co-occurrences sharing a node
        let observed_freq = if pattern.stages.len() == 1 {
            label_freqs[0]
        } else {
            // Count instances where all stage labels co-occur on a shared node
            let first_label = &pattern.stages[0].clauses[0].label;
            let mut co_occurrence = 0usize;

            for edge_a in corpus.edges_with_label(first_label) {
                let mut matches_all = true;
                for stage in pattern.stages.iter().skip(1) {
                    if let Some(clause) = stage.clauses.first() {
                        let has_match = corpus
                            .edges_with_label(&clause.label)
                            .iter()
                            .any(|e| e.source == edge_a.source || e.target == edge_a.source);
                        if !has_match {
                            matches_all = false;
                            break;
                        }
                    }
                }
                if matches_all {
                    co_occurrence += 1;
                }
            }

            co_occurrence as f64 / total
        };

        if observed_freq == 0.0 {
            return 0.0;
        }

        // Interest factor: observed / expected
        // Values > 1.0 mean "more frequent than chance"
        // Take log to compress the scale, add 1 to avoid negative scores for rare-but-present
        let interest = observed_freq / expected_freq;

        // Score: combine interest with rarity
        // Rare labels that co-occur = high surprise
        // Common labels that co-occur = low surprise
        let rarity = -label_freqs.iter().map(|f| f.ln()).sum::<f64>() / label_freqs.len() as f64;

        interest * rarity
    }

    fn name(&self) -> &str {
        "surprise"
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fabula-discovery --test surprise_tests`

Expected: Both tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fabula-discovery/src/evaluators/
git commit -m "feat(discovery): SurpriseEvaluator — statistical interest factor scoring"
```

---

### Task 7: Narrative Quality Evaluator

**Files:**
- Create: `crates/fabula-discovery/src/evaluators/narrative.rs`

- [ ] **Step 1: Implement NarrativeEvaluator**

Write `crates/fabula-discovery/src/evaluators/narrative.rs`:

```rust
use crate::corpus::TraceCorpus;
use crate::traits::PatternEvaluator;
use fabula::engine::free::evaluate_pattern;
use fabula::pattern::Pattern;
use fabula_memory::MemGraph;

/// Scores patterns by narrative quality.
///
/// Builds a MemGraph from the corpus, runs `evaluate_pattern` against it,
/// and scores based on match count and specificity (stages × constraints).
/// A pattern with moderate match count and high specificity scores best.
pub struct NarrativeEvaluator;

impl PatternEvaluator for NarrativeEvaluator {
    fn evaluate(
        &self,
        pattern: &Pattern<String, String>,
        corpus: &TraceCorpus,
    ) -> f64 {
        let graph = corpus_to_memgraph(corpus);
        let matches = evaluate_pattern(&graph, pattern);

        let match_count = matches.len();
        if match_count == 0 {
            return 0.0;
        }

        // Specificity: more stages and more clauses = more specific pattern
        let total_clauses: usize = pattern.stages.iter().map(|s| s.clauses.len()).sum();
        let specificity = (pattern.stages.len() as f64) + (total_clauses as f64 * 0.5);

        // Sweet spot scoring: penalize both too few and too many matches
        // Peak at ~5-20 matches for a typical corpus
        let corpus_size = corpus.len() as f64;
        let match_ratio = match_count as f64 / corpus_size;
        let match_quality = if match_ratio > 0.5 {
            // Too general — matches more than half the corpus
            0.5 / match_ratio
        } else {
            // Good — specific enough to be interesting
            (match_count as f64).ln().max(0.0)
        };

        match_quality * specificity
    }

    fn name(&self) -> &str {
        "narrative"
    }
}

/// Convert a TraceCorpus to a MemGraph for pattern evaluation.
fn corpus_to_memgraph(corpus: &TraceCorpus) -> MemGraph {
    use fabula_memory::MemValue;

    let mut graph = MemGraph::new();
    let (_, max_t) = corpus.time_range();
    graph.set_time(max_t);

    for edge in corpus.edges() {
        let value = MemValue::Node(edge.target.clone());
        if let Some(end) = edge.interval.end {
            graph.add_edge_bounded(&edge.source, &edge.label, value, edge.interval.start, end);
        } else {
            graph.add_edge(&edge.source, &edge.label, value, edge.interval.start);
        }
    }

    graph
}
```

- [ ] **Step 2: Move fabula-memory to regular dependencies**

In `crates/fabula-discovery/Cargo.toml`, move `fabula-memory` from `[dev-dependencies]` to `[dependencies]`:

```toml
[dependencies]
fabula = { path = "../fabula" }
fabula-narratives = { path = "../fabula-narratives" }
fabula-dsl = { path = "../fabula-dsl" }
fabula-memory = { path = "../fabula-memory" }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p fabula-discovery`

Expected: Compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/fabula-discovery/src/evaluators/narrative.rs crates/fabula-discovery/Cargo.toml
git commit -m "feat(discovery): NarrativeEvaluator — match quality + specificity scoring"
```

---

### Task 8: MINERful-Adapted Generator

**Files:**
- Create: `crates/fabula-discovery/src/generators/mod.rs`
- Create: `crates/fabula-discovery/src/generators/minerful.rs`
- Create: `crates/fabula-discovery/tests/minerful_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/fabula-discovery/tests/minerful_tests.rs`:

```rust
use fabula::interval::Interval;
use fabula_discovery::generators::MinerfulGenerator;
use fabula_discovery::{CandidateGenerator, TraceCorpus};

fn make_corpus_with_clear_pattern() -> TraceCorpus {
    // Create a corpus where "trusts" is consistently followed by "betrays"
    // for the same pair of entities. This should be discoverable.
    let mut edges = Vec::new();

    // 5 instances of trusts-then-betrays for the same entity pair
    for i in 0..5 {
        let t = i * 20i64;
        edges.push((
            "alice".into(),
            "trusts".into(),
            "bob".into(),
            Interval { start: t, end: Some(t + 5) },
        ));
        edges.push((
            "alice".into(),
            "betrays".into(),
            "bob".into(),
            Interval { start: t + 10, end: Some(t + 15) },
        ));
    }

    // Some noise: unrelated edges
    for i in 0..3 {
        edges.push((
            "carol".into(),
            "helps".into(),
            "dave".into(),
            Interval { start: (i * 30) as i64, end: Some((i * 30 + 5) as i64) },
        ));
    }

    TraceCorpus::new(edges)
}

#[test]
fn discovers_pairwise_constraints() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.3,
        min_confidence: 0.5,
    });

    let candidates = gen.generate(&corpus, 10);

    // Should discover at least one pattern involving "trusts" and "betrays"
    let has_trust_betray = candidates.iter().any(|p| {
        let labels: Vec<&str> = p
            .stages
            .iter()
            .flat_map(|s| s.clauses.iter().map(|c| c.label.as_str()))
            .collect();
        labels.contains(&"trusts") && labels.contains(&"betrays")
    });

    assert!(
        has_trust_betray,
        "Should discover trusts-betrays pattern. Found {} candidates: {:?}",
        candidates.len(),
        candidates.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[test]
fn respects_budget() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.1,
    });

    let candidates = gen.generate(&corpus, 3);
    assert!(candidates.len() <= 3, "Should respect budget of 3, got {}", candidates.len());
}

#[test]
fn generated_patterns_have_temporal_constraints() {
    let corpus = make_corpus_with_clear_pattern();
    let mut gen = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.3,
        min_confidence: 0.5,
    });

    let candidates = gen.generate(&corpus, 10);

    for pattern in &candidates {
        if pattern.stages.len() > 1 {
            assert!(
                !pattern.temporal.is_empty(),
                "Multi-stage pattern '{}' should have temporal constraints",
                pattern.name
            );
        }
    }
}

use fabula_discovery::generators::MinerfulConfig;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fabula-discovery --test minerful_tests`

Expected: FAIL — `MinerfulGenerator` not found.

- [ ] **Step 3: Implement MinerfulGenerator**

Write `crates/fabula-discovery/src/generators/mod.rs`:

```rust
mod minerful;

pub use minerful::{MinerfulConfig, MinerfulGenerator};
```

Write `crates/fabula-discovery/src/generators/minerful.rs`:

```rust
use crate::corpus::{PairwiseHit, SharedNode, TraceCorpus};
use crate::score::ScoredPattern;
use crate::traits::CandidateGenerator;
use fabula::interval::AllenRelation;
use fabula::pattern::{
    Clause, Pattern, Stage, Target, TemporalConstraint, Var,
};
use std::collections::HashMap;

/// Configuration for the MINERful-adapted generator.
#[derive(Debug, Clone)]
pub struct MinerfulConfig {
    /// Minimum fraction of corpus edges that must participate in a pattern.
    pub min_support: f64,
    /// Minimum fraction of co-occurrences where the Allen relation holds.
    pub min_confidence: f64,
}

/// MINERful-adapted constraint miner.
///
/// Discovers two-stage patterns by:
/// 1. Finding all label pairs that share nodes in the corpus
/// 2. Computing Allen relation distributions for each pair
/// 3. Emitting patterns for pairs exceeding support/confidence thresholds
///
/// Based on Di Ciccio & Mecella's MINERful (2015), adapted for
/// Allen interval algebra over temporal graphs.
pub struct MinerfulGenerator {
    config: MinerfulConfig,
    round: usize,
}

impl MinerfulGenerator {
    pub fn new(config: MinerfulConfig) -> Self {
        Self { config, round: 0 }
    }
}

/// Statistics for a label pair + Allen relation combination.
#[derive(Debug)]
struct PairStats {
    label_a: String,
    label_b: String,
    relation: AllenRelation,
    /// How many instances of this (label_a, label_b, relation) triple exist.
    count: usize,
    /// Total co-occurrences of (label_a, label_b) regardless of relation.
    total_pair: usize,
    /// Most common shared node type for this pair.
    shared_node_example: SharedNode,
}

impl CandidateGenerator for MinerfulGenerator {
    fn generate(
        &mut self,
        corpus: &TraceCorpus,
        budget: usize,
    ) -> Vec<Pattern<String, String>> {
        self.round += 1;

        let total_edges = corpus.len() as f64;
        if total_edges == 0.0 {
            return Vec::new();
        }

        // Phase 1: Compute pairwise Allen relation statistics
        let mut pair_stats: Vec<PairStats> = Vec::new();

        for (label_a, label_b) in corpus.label_pairs() {
            let hits = corpus.pairwise_relations(label_a, label_b);
            if hits.is_empty() {
                continue;
            }

            // Count by Allen relation
            let mut by_relation: HashMap<AllenRelation, (usize, SharedNode)> = HashMap::new();
            for hit in &hits {
                let entry = by_relation
                    .entry(hit.relation)
                    .or_insert((0, hit.shared_node.clone()));
                entry.0 += 1;
            }

            let total_pair = hits.len();
            let support = total_pair as f64 / total_edges;

            if support < self.config.min_support {
                continue;
            }

            for (relation, (count, shared_node)) in by_relation {
                let confidence = count as f64 / total_pair as f64;
                if confidence >= self.config.min_confidence {
                    pair_stats.push(PairStats {
                        label_a: label_a.to_string(),
                        label_b: label_b.to_string(),
                        relation,
                        count,
                        total_pair,
                        shared_node_example: shared_node,
                    });
                }
            }
        }

        // Phase 2: Sort by interest factor (count * confidence) and take top budget
        pair_stats.sort_by(|a, b| {
            let interest_a = a.count as f64 * (a.count as f64 / a.total_pair as f64);
            let interest_b = b.count as f64 * (b.count as f64 / b.total_pair as f64);
            interest_b
                .partial_cmp(&interest_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        pair_stats.truncate(budget);

        // Phase 3: Convert to Pattern objects
        pair_stats
            .into_iter()
            .enumerate()
            .map(|(i, stats)| build_pattern(i, self.round, &stats))
            .collect()
    }

    fn feedback(&mut self, _scored: &[ScoredPattern<String, String>]) {
        // MINERful is a single-pass miner — feedback doesn't change its behavior.
        // Future: could adjust support/confidence thresholds based on acceptance rates.
    }

    fn name(&self) -> &str {
        "minerful"
    }
}

fn build_pattern(idx: usize, round: usize, stats: &PairStats) -> Pattern<String, String> {
    let name = format!(
        "discovered_r{}_{}_{}_{}",
        round,
        idx,
        stats.label_a.replace(' ', "_"),
        stats.label_b.replace(' ', "_"),
    );

    // Determine variable bindings from shared node type
    let (source_a, target_a, source_b, target_b) = match &stats.shared_node_example {
        SharedNode::Source(node) => {
            // Both edges share the same source
            ("actor", "target_a", "actor", "target_b")
        }
        SharedNode::SourceTarget(_) => {
            // Edge A's source = Edge B's target
            ("actor", "target_a", "source_b", "actor")
        }
        SharedNode::TargetSource(_) => {
            // Edge A's target = Edge B's source
            ("source_a", "actor", "actor", "target_b")
        }
    };

    let stage_a = Stage {
        anchor: Var::new("e1"),
        clauses: vec![Clause {
            source: Var::new(source_a),
            label: stats.label_a.clone(),
            target: Target::Bind(Var::new(target_a)),
            negated: false,
        }],
    };

    let stage_b = Stage {
        anchor: Var::new("e2"),
        clauses: vec![Clause {
            source: Var::new(source_b),
            label: stats.label_b.clone(),
            target: Target::Bind(Var::new(target_b)),
            negated: false,
        }],
    };

    Pattern {
        name,
        stages: vec![stage_a, stage_b],
        temporal: vec![TemporalConstraint {
            left: Var::new("e1"),
            relation: stats.relation,
            right: Var::new("e2"),
            gap: None,
        }],
        negations: Vec::new(),
        group: None,
        metadata: HashMap::new(),
        deadline_ticks: None,
        repeat_range: None,
        unordered_groups: Vec::new(),
        private: false,
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fabula-discovery --test minerful_tests`

Expected: All 3 tests pass. If `discovers_pairwise_constraints` fails, check that the support/confidence thresholds in the test match the corpus data. The corpus has 5 trusts + 5 betrays + 3 helps = 13 edges. 5 co-occurrences of trusts-betrays gives support ≈ 5/13 ≈ 0.38, which exceeds 0.3.

- [ ] **Step 5: Commit**

```bash
git add crates/fabula-discovery/src/generators/ crates/fabula-discovery/tests/minerful_tests.rs
git commit -m "feat(discovery): MINERful-adapted generator — Allen-relation constraint mining"
```

---

### Task 9: End-to-End Integration Test

**Files:**
- Create: `crates/fabula-discovery/tests/integration.rs`

- [ ] **Step 1: Write integration test**

Create `crates/fabula-discovery/tests/integration.rs`:

```rust
//! End-to-end: build corpus → run discovery session → emit DSL → parse DSL

use fabula::interval::Interval;
use fabula_discovery::evaluators::{NarrativeEvaluator, SurpriseEvaluator};
use fabula_discovery::generators::{MinerfulConfig, MinerfulGenerator};
use fabula_discovery::{
    pattern_to_dsl, DiscoverySession, PatternFilter, ScoredPattern, SessionConfig, TraceCorpus,
};

/// Accept patterns with any positive composite score.
struct AcceptPositive;
impl PatternFilter for AcceptPositive {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool {
        scored.score.scores.values().any(|&v| v > 0.0)
    }
}

fn make_rich_corpus() -> TraceCorpus {
    let mut edges = Vec::new();

    // Repeated pattern: trust → betray (5 instances, same entities)
    for i in 0..5 {
        let t = i * 20i64;
        edges.push(("alice".into(), "trusts".into(), "bob".into(), Interval { start: t, end: Some(t + 5) }));
        edges.push(("alice".into(), "betrays".into(), "bob".into(), Interval { start: t + 10, end: Some(t + 15) }));
    }

    // Repeated pattern: meet → ally (3 instances, different entities)
    for i in 0..3 {
        let t = 100 + i * 20i64;
        let src = format!("char_{}", i);
        edges.push((src.clone(), "meets".into(), "hero".into(), Interval { start: t, end: Some(t + 3) }));
        edges.push((src, "allies_with".into(), "hero".into(), Interval { start: t + 5, end: Some(t + 8) }));
    }

    // Noise
    for i in 0..8 {
        edges.push((
            format!("npc_{}", i),
            "wanders".into(),
            "town".into(),
            Interval { start: (i * 5) as i64, end: Some((i * 5 + 2) as i64) },
        ));
    }

    TraceCorpus::new(edges)
}

#[test]
fn end_to_end_discovery_and_emission() {
    let corpus = make_rich_corpus();

    let generator = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.3,
    });

    let config = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 10,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![
            Box::new(SurpriseEvaluator),
            Box::new(NarrativeEvaluator),
        ],
        AcceptPositive,
    );

    assert!(!result.accepted.is_empty(), "Should discover at least one pattern");

    // Every accepted pattern should emit valid DSL
    for scored in &result.accepted {
        let dsl = pattern_to_dsl(&scored.pattern);
        let parsed = fabula_dsl::parse_document(&dsl);
        assert!(
            parsed.is_ok(),
            "Pattern '{}' emitted invalid DSL:\n{}\nError: {}",
            scored.pattern.name,
            dsl,
            parsed.unwrap_err()
        );
    }

    // Session history should be complete
    assert!(result.all_scored.len() >= result.accepted.len());
    assert!(result.rounds == 1);
}

#[test]
fn discovered_patterns_match_corpus() {
    let corpus = make_rich_corpus();

    let generator = MinerfulGenerator::new(MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.3,
    });

    let config = SessionConfig {
        max_rounds: 1,
        candidates_per_round: 10,
    };

    let mut session = DiscoverySession::new(config);
    let result = session.run(
        &corpus,
        generator,
        vec![Box::new(SurpriseEvaluator)],
        AcceptPositive,
    );

    // At least one discovered pattern should actually produce matches
    // when evaluated against a MemGraph built from the same corpus
    let graph = {
        use fabula_memory::{MemGraph, MemValue};
        let mut g = MemGraph::new();
        let (_, max_t) = corpus.time_range();
        g.set_time(max_t);
        for edge in corpus.edges() {
            let value = MemValue::Node(edge.target.clone());
            if let Some(end) = edge.interval.end {
                g.add_edge_bounded(&edge.source, &edge.label, value, edge.interval.start, end);
            } else {
                g.add_edge(&edge.source, &edge.label, value, edge.interval.start);
            }
        }
        g
    };

    let any_matches = result.accepted.iter().any(|scored| {
        let matches = fabula::engine::free::evaluate_pattern(&graph, &scored.pattern);
        !matches.is_empty()
    });

    assert!(any_matches, "At least one discovered pattern should match the corpus");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p fabula-discovery --test integration`

Expected: Both tests pass. If they fail, debug by examining which patterns the MINERful generator produces and whether the DSL emitter handles them correctly.

- [ ] **Step 3: Run the full workspace test suite**

Run: `cargo test --workspace --exclude fabula-grafeo --exclude fabula-wasm`

Expected: All tests pass, including the new fabula-discovery tests.

- [ ] **Step 4: Run clippy on the full workspace**

Run: `cargo clippy --workspace --exclude fabula-grafeo --exclude fabula-wasm -- -D warnings`

Expected: No warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/fabula-discovery/tests/integration.rs
git commit -m "test(discovery): end-to-end integration — corpus → discover → emit DSL → parse"
```

---

### Task 10: Update Workspace Documentation

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add fabula-discovery to the workspace layout table**

In `CLAUDE.md`, add a row to the workspace layout table after `fabula-examples`:

```
| `fabula-discovery` | Pattern discovery: generate-evaluate framework, MINERful generator, DSL emission |
```

Update the crate count from "10 crates" to "11 crates".

- [ ] **Step 2: Verify the CLAUDE.md is accurate**

Read the updated CLAUDE.md and confirm the new row matches the crate's actual purpose.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add fabula-discovery to CLAUDE.md workspace layout"
```

---

## Self-Review

**Spec coverage check:**
- [x] Framework traits (CandidateGenerator, PatternEvaluator, PatternFilter) — Task 3
- [x] TraceCorpus abstraction — Task 2
- [x] DSL reverse compiler (pattern_to_dsl) — Task 5
- [x] One concrete generator (MINERful-adapted) — Task 8
- [x] Evaluation harness with narrative-quality and statistical-surprise evaluators — Tasks 6-7
- [x] Serializable session history — Task 4 (SessionHistory struct)
- [x] DiscoverySession orchestrator — Task 4

**Placeholder scan:** No TBD, TODO, or "fill in later" found. All code blocks are complete.

**Type consistency check:**
- `Pattern<String, String>` used consistently throughout (traits, generators, evaluators, emitter)
- `TraceCorpus` type matches between corpus.rs and all consumers
- `ScoredPattern` / `PatternScore` match between score.rs and session.rs
- `CandidateGenerator` / `PatternEvaluator` / `PatternFilter` trait signatures match between traits.rs and all implementations

**Missing from spec (noted but out of scope):** The spec mentions "serializable session history for reproducibility." The current `SessionHistory` is a plain struct — adding `#[derive(Serialize, Deserialize)]` with a serde feature flag would complete this, but Pattern doesn't derive Serialize without the serde feature. This can be deferred to a follow-up task.
