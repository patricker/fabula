---
sidebar_position: 10
title: Performance
---

# Performance and Benchmarks

Every non-narrative reader eventually asks: "How many events per second can fabula handle?" This page answers that question with concrete numbers from the benchmark suite, explains what they mean for your workload, and shows you how to measure your own.

## Throughput

The headline number: **~28 microseconds per `on_edge_added` call** on the PetGraph adapter with a GM-scale workload. That translates to roughly **35,000 edges per second** on a single thread with no parallelism.

This is the incremental path -- feeding edges one at a time via `on_edge_added`. Batch evaluation (`engine.evaluate(&graph)`) scans the entire graph on every call and is significantly slower. Use batch mode for one-shot queries; use incremental mode for real-time workloads.

The 28us figure was achieved after the Phase 2.3 fingerprint optimization, which replaced string-based deduplication with XOR hashing for a **5.8x speedup** (down from 164us per edge).

## What "GM-scale" means

The benchmark workload models a game master (GM) overseeing a narrative simulation. The default `WorkloadConfig` is:

| Parameter | Value | Rationale |
|---|---|---|
| `pattern_count` | 30 | Mix of multi-stage, single-stage, negation-heavy, many-binding, and never-matching |
| `stages_per_pattern` | 3 | Average across all pattern categories |
| `negation_fraction` | 0.5 | Half of all patterns have negation windows |
| `pre_existing_edges` | 5,000 | Accumulated graph state from prior simulation ticks |
| `edges_per_tick` | 10 | Each character performs 1-2 actions per tick, 10 characters |
| `character_count` | 10 | Produces realistic variable join fan-out |

The 30 patterns span five categories that stress different engine paths:

- **Multi-stage with negation** (6 patterns) -- 2-4 stages, negation between first and last stage
- **High-fanout single-stage** (6 patterns) -- match common events, producing many partial matches
- **Negation-heavy** (6 patterns) -- rare event triggers with common-event negation bodies and dual negation windows (`unless_between` + `unless_after`)
- **Many-binding** (6 patterns) -- 4+ variable bindings per match to stress HashMap cloning
- **Never-matching** (6 patterns) -- phantom event types that never appear, exercising the "check everything, match nothing" path

This is a realistic workload for a narrative simulation or game server. If your workload has fewer patterns or fewer edges per tick, performance will be better. If you have hundreds of patterns or thousands of edges per tick, see the scaling section below.

## Scaling behavior

The benchmark suite sweeps individual dimensions while holding others at baseline. Key findings:

### Pattern count

| Patterns | Adapter | Behavior |
|---|---|---|
| 1 | PetGraph | Below noise floor |
| 10 | PetGraph | Baseline |
| 30 | PetGraph | ~28us/edge (GM-scale) |
| 100 | PetGraph | Linear increase -- each `on_edge_added` tries stage 0 of every pattern |

Pattern count scales linearly because the engine's initiation phase (Phase 2 in the 4-phase algorithm) tests the incoming edge against stage 0 of every registered pattern. Use `set_pattern_enabled(false)` to deactivate patterns that are not relevant to the current game state.

### Pre-existing edge count

| Edges | Adapter | Notes |
|---|---|---|
| 100 | PetGraph | Fast -- small graph, few secondary clause lookups |
| 1,000 | PetGraph | Baseline |
| 5,000 | PetGraph | Stable -- PetGraph has real graph indexing |
| 100 | MemGraph | Comparable to PetGraph |
| 1,000+ | MemGraph | **O(E^2)** in batch mode -- MemGraph scans linearly |

MemGraph is a Vec-backed store designed for testing simplicity, not production throughput. For any workload with more than a few hundred edges, use PetGraph or a custom `DataSource` adapter with proper indexing.

### Stage depth

| Stages | Behavior |
|---|---|
| 1 | Single-stage patterns complete immediately -- no partial match tracking overhead |
| 3 | Baseline (GM-scale) |
| 5 | More active partial matches to advance per incoming edge, modest increase |

Stage depth affects the advancement phase (Phase 3). More stages mean more partial matches in flight and more secondary clause lookups per advancement attempt. The cost is proportional to `active_partial_matches * clauses_per_stage`.

### Negation fraction

| Fraction | Behavior |
|---|---|
| 0% | No negation checks -- fastest |
| 50% | Baseline (GM-scale) |
| 100% | Every pattern has negation windows -- measurable but not dramatic increase |

Negation checking (Phase 1) iterates over active partial matches with negation windows. The cost depends on how many active partial matches have negation and how many clauses are in each negation body.

## Frame budget

At 60 fps, one frame is **16.67 ms**. At 28us per `on_edge_added` call, the engine can process roughly **570 edges per frame** before consuming the entire frame budget. In practice you want sifting to take a small fraction of the frame:

| Scenario | Events/sec | Per-frame edges | Budget used |
|---|---|---|---|
| Narrative sim (10 chars, 10 ticks/s) | ~100-200 | ~10-20 | < 1% |
| 64-player server at 64 ticks/s | ~4,000 | ~63 | ~11% |
| Battle royale, 100 players at 128 ticks/s | ~13,000 | ~200 | ~34% |
| IoT sensor network (1K-15K events/s) | ~15,000 | N/A (not frame-based) | 43% of 1-second budget |

All of these workloads fit comfortably within budget on a single thread. For the narrative simulation use case that fabula was designed for, sifting overhead is negligible.

## Memory

Partial matches are the primary memory consumer. Each `PartialMatch` holds a `HashMap<String, BoundValue>` of variable bindings plus metadata (fingerprint, state, stage index, timestamps). In a typical GM-scale workload, you will see 100-300 active partial matches at steady state.

### Controlling memory growth

**`drain_completed()`.** Call this after each tick (or every N ticks) to move completed matches out of the engine. The profiling binary drains every 10 ticks, which is a reasonable default. The returned `Vec<Match>` gives you the results; the engine forgets them.

**Fingerprint dedup.** The engine maintains a `HashSet<u64>` of XOR fingerprints covering Active, Complete, and Dead partial matches. This prevents re-creating a partial match that was just negated, which would otherwise cause unbounded PM accumulation in adversarial workloads.

**`stale_patterns()`.** Returns pattern names that have active partial matches which haven't advanced in N ticks. Use this to identify patterns that are accumulating stale PMs and consider disabling them with `set_pattern_enabled(false)`.

**`deadline` on patterns.** Setting a deadline in ticks causes the engine to automatically expire partial matches that exceed the deadline, emitting `SiftEvent::Expired`. This is the most direct way to prevent PM accumulation from slow-moving patterns.

## Benchmarking your workload

### Running the built-in benchmarks

```bash
cargo bench -p fabula-bench
```

This runs the full [divan](https://github.com/nvzqz/divan) benchmark suite, which includes:

**GM-profile benchmarks** (Tier 1):
- `gm_profile::edges_per_tick_petgraph` -- 1, 10, 50 edges per tick
- `gm_profile::edges_per_tick_memgraph` -- same, MemGraph adapter
- `gm_profile::negation_fraction_petgraph` -- 0%, 50%, 100% negation
- `gm_profile::warm_edges_per_tick_petgraph` -- warm-start with ~100-200 active PMs

**Scaling benchmarks** (Tier 2):
- `scaling::pattern_count_petgraph` -- 1, 10, 30, 100 patterns
- `scaling::edge_count_petgraph` -- 100, 1K, 5K pre-existing edges
- `scaling::stage_count_petgraph` -- 1, 3, 5 stages per pattern
- `scaling::batch_petgraph` -- batch evaluation at 100 and 1K edges

### Configuring a custom workload

The `WorkloadConfig` struct lets you tune every dimension independently:

```rust
use fabula_bench::{build_isolated_workload, WorkloadConfig};
use fabula_test_suite::PetGraph;

let config = WorkloadConfig {
    pattern_count: 50,
    stages_per_pattern: 4,
    negation_fraction: 0.8,
    pre_existing_edges: 10_000,
    edges_per_tick: 20,
    character_count: 20,
    seed: 42,
};

let workload = build_isolated_workload::<PetGraph>(&config);
// workload.graph -- pre-populated graph
// workload.engine -- engine with patterns registered
// workload.pending_edges -- edges to feed during measurement
```

For divan benchmarks, put workload construction inside `with_inputs` (unmeasured setup) and only time the `on_edge_added` / `evaluate` calls.

### Profiling binary

The `fabula-profile` binary runs a full 200-tick GM simulation and emits per-tick CSV to stdout:

```bash
# Per-tick CSV (pipe to file for analysis)
cargo run --release --bin fabula-profile > profile.csv

# Choose adapter
cargo run --release --bin fabula-profile -- --adapter memgraph > profile.csv

# Heap profiling with dhat
cargo run --release --bin fabula-profile --features dhat-heap

# CPU flamegraph with samply
samply record target/release/fabula-profile
```

The CSV columns are:

| Column | Description |
|---|---|
| `tick` | Simulation tick number |
| `edges` | Edges processed this tick |
| `elapsed_us` | Wall-clock microseconds for this tick |
| `active_pms` | Active partial matches after this tick |
| `tick_on_edge` | `on_edge_added` calls this tick |
| `tick_fingerprints` | Fingerprint computations this tick |
| `tick_neg_checks` | Negation checks this tick |
| `peak_pms` | Lifetime high-water mark for active PMs |
| `advanced` | PMs that advanced a stage this tick |
| `completed` | Patterns that completed this tick |
| `negated` | PMs killed by negation this tick |

The summary printed to stderr includes the average microseconds per `on_edge_added` call across the full run.

## Adapter comparison

| Adapter | Best for | Indexing | Incremental | Batch |
|---|---|---|---|---|
| **PetGraph** | Production workloads | Real graph indexing via `petgraph::StableGraph` | Fast (28us/edge GM-scale) | Scales to 5K+ edges |
| **MemGraph** | Testing and prototyping | Vec-backed linear scan | Comparable at small scale | O(E^2) at 1K+ edges |
| **GrafeoGraph** | Persistent storage | Database-backed queries | Depends on storage backend | Depends on storage backend |

**Recommendation:** Use PetGraph for anything performance-sensitive. Use MemGraph for unit tests and golden tests where simplicity matters more than speed. Use GrafeoGraph when you need persistent graph storage across process restarts.

### Why MemGraph is slower at scale

MemGraph stores edges in a `Vec` and answers `edges_from` queries by scanning the entire vec. This is O(E) per query. Since each `on_edge_added` call can trigger multiple `edges_from` queries (one per secondary clause), and batch evaluation scans all edges as potential triggers, MemGraph batch becomes O(E^2) in practice. PetGraph uses `petgraph`'s adjacency list, giving O(degree) lookups instead of O(E).

The MemGraph label indexing optimization was deferred (see ROADMAP Phase 2.4) because the fingerprint optimization (Phase 2.3) closed the incremental performance gap to 28us/edge, and MemGraph is positioned as a testing-only adapter.

## Domain-specific interpretation

The benchmarks above use a narrative simulation workload. Here is how the numbers translate to other domains.

### SIEM / Security

A typical SIEM ingestion pipeline handles 1,000-10,000 events per second per source. Each event maps to 2-5 edges (type, source IP, destination, user, etc.):

| Events/sec | Edges/sec | Single-thread headroom | Verdict |
|------------|-----------|----------------------|---------|
| 1,000 | 3,000 | 35,000 / 3,000 = 11.7x | Comfortable |
| 5,000 | 15,000 | 35,000 / 15,000 = 2.3x | Tight -- consider sharding by source |
| 10,000 | 30,000 | 35,000 / 30,000 = 1.2x | At limit -- shard or batch |

**Key consideration:** SIEM events often arrive out of order from distributed sources. Fabula's incremental engine assumes temporal ordering. Buffer events in a short window (1-5 seconds), sort by timestamp, then feed in order. This adds latency equal to the buffer window.

### Compliance / Audit

Compliance workloads are typically lower volume (hundreds of events/second) but with more patterns (50-200 rules). Pattern count scaling:

| Patterns | Overhead per edge | Notes |
|----------|------------------|-------|
| 30 | 28 us (baseline) | Default benchmark |
| 100 | ~90 us | Near-linear scaling |
| 300 | ~270 us | Still under 1ms per edge |

At 100 patterns and 500 events/second (1,500 edges/second), fabula processes the backlog in ~135ms per second -- well within real-time.

### Observability / Tracing

Distributed tracing generates high-cardinality data (thousands of spans per second). Consider:

- Use **batch evaluation** for post-hoc trace analysis (scan completed traces)
- Use **incremental evaluation** only for real-time alerting on live spans
- Shard by trace ID -- each trace is independent, so engines can run in parallel

### Adapter choice matters

| Adapter | Use case | Scaling |
|---------|----------|---------|
| MemGraph | Testing, small datasets (<10K edges) | O(E) per scan -- linear |
| PetGraph | Production, medium datasets | O(degree) per edges_from -- fast |
| GrafeoGraph | Large persistent graphs | Depends on Grafeo backend |
| Custom | Your production store | You control the indexing |

If you're seeing slow performance, the adapter is almost always the bottleneck, not the engine.
