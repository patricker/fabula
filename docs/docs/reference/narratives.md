---
sidebar_position: 6
title: Narrative Scoring
---

# Narrative Scoring

`fabula_narratives` -- narrative scoring and thread management for MCTS evaluation. Provides the GM's quality function: scoring signals that tell the search whether a candidate action improves the narrative.

Four modules, each backed by specific research:

| Module | Measures | Research |
|--------|----------|----------|
| `thread` | Thread lifecycle, FILO nesting | Kowal MICE Quotient |
| `tension` | Numeric trajectory (rising/falling/plateau/peak/valley) | Booth 2009 (L4D AI Director), Ely/Frankel/Kamenica 2015 |
| `pivot` | Event distribution shift (JSD) | Schulz et al. 2024 (Narrative Information Theory) |
| `scorer` | Composite quality function | Nelson & Mateas 2005 (Search-Based Drama Management) |

---

## Thread Tracking

### `ThreadTracker`

Tracks narrative thread lifecycles (MICE-style open/close pairs). Register threads, then query status after each tick.

```rust
use fabula_narratives::thread::ThreadTracker;

let mut tracker = ThreadTracker::new();
tracker.register("investigation", open_idx, close_idx);

// After each tick:
let status = tracker.status(|idx| engine.pattern_metrics(idx));
let violations = tracker.check_filo();
```

#### `ThreadTracker::new`

```rust
pub fn new() -> Self
```

---

#### `register`

Register a narrative thread with its open and close pattern indices. If using `observe_delta`, pattern names must follow the convention `{name}_open` and `{name}_close`.

```rust
pub fn register(&mut self, name: impl Into<String>, open_pattern_idx: usize, close_pattern_idx: usize)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | `impl Into<String>` | Thread name (e.g., `"investigation"`). |
| `open_pattern_idx` | `usize` | Pattern index for the opening event. |
| `close_pattern_idx` | `usize` | Pattern index for the closing event. |

---

#### `record_open`

Record that a thread opened. Deduplicates by name.

```rust
pub fn record_open(&mut self, thread_name: &str)
```

---

#### `record_close`

Record that a thread closed.

```rust
pub fn record_close(&mut self, thread_name: &str)
```

---

#### `observe_delta`

Update from a `TickDelta` -- automatically records opens (from `delta.advanced`) and closes (from `delta.completed`) matching the `{name}_open` / `{name}_close` convention. Ignores `delta.negated`, `delta.expired`, and `delta.stalled`.

```rust
pub fn observe_delta(&mut self, delta: &TickDelta)
```

---

#### `status`

Status of all registered threads. Accepts a closure returning `PatternMetrics` for a pattern index. Decoupled from `SiftEngine` for use during MCTS rollouts.

```rust
pub fn status(&self, metrics_fn: impl Fn(usize) -> Option<PatternMetrics>) -> Vec<ThreadStatus>
```

---

#### `unresolved_thread_count`

Count of threads with opens but no corresponding closes.

```rust
pub fn unresolved_thread_count(&self, metrics_fn: impl Fn(usize) -> Option<PatternMetrics>) -> usize
```

---

#### `check_filo`

Check FILO nesting: threads should close in reverse order of opening. Returns violations.

```rust
pub fn check_filo(&self) -> Vec<FiloViolation>
```

---

#### `reset`

Reset tracking state (keeps thread registrations).

```rust
pub fn reset(&mut self)
```

---

### `ThreadStatus`

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Thread name. |
| `open_count` | `usize` | Active open instances. |
| `close_count` | `u64` | Times the close pattern completed. |
| `unresolved` | `bool` | Has opens without corresponding closes. |

---

### `FiloViolation`

| Field | Type | Description |
|-------|------|-------------|
| `closed_thread` | `String` | Thread that closed out of order. |
| `blocking_thread` | `String` | Thread that should have closed first (opened later, still open). |

---

## Tension Tracking

### `TensionTracker`

Tracks a numeric value over a sliding window and classifies the trajectory. The caller provides samples (e.g., character stress, faction hostility) -- the tracker is DataSource-agnostic.

```rust
use fabula_narratives::tension::{TensionTracker, Trajectory};

let mut tracker = TensionTracker::new(10); // window of 10 samples
for i in 0..15 {
    tracker.push(i as u64, i as f64 * 0.1);
}
assert_eq!(tracker.trajectory(), Trajectory::Rising);
assert!(tracker.slope() > 0.0);
```

#### `TensionTracker::new`

Create a tracker with the given sliding window size (minimum 3).

```rust
pub fn new(window_size: usize) -> Self
```

**Panics** if `window_size < 3`.

---

#### `with_threshold`

Create a tracker with a custom slope threshold for trajectory classification. Higher values require stronger trends to classify as Rising/Falling.

```rust
pub fn with_threshold(window_size: usize, threshold: f64) -> Self
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `window_size` | `usize` | -- | Sliding window size (minimum 3). |
| `threshold` | `f64` | 0.01 | Slope magnitude below which trajectory is Plateau. |

---

#### `push`

Push a new sample. Old samples outside the window are dropped.

```rust
pub fn push(&mut self, tick: u64, value: f64)
```

---

#### `current`

Most recent sample value.

```rust
pub fn current(&self) -> Option<f64>
```

---

#### `slope`

Linear regression slope over the window. Positive = rising, negative = falling, near-zero = plateau.

```rust
pub fn slope(&self) -> f64
```

---

#### `trajectory`

Classify the trajectory over the window.

```rust
pub fn trajectory(&self) -> Trajectory
```

Returns `Unknown` with fewer than 3 samples.

---

#### `sample_count`

Number of samples currently in the window.

```rust
pub fn sample_count(&self) -> usize
```

---

#### `reset`

Clear all samples.

```rust
pub fn reset(&mut self)
```

---

### `Trajectory`

Classification of a trajectory's recent behavior.

| Variant | Description |
|---------|-------------|
| `Rising` | Values increasing over the window. |
| `Falling` | Values decreasing over the window. |
| `Plateau` | Values approximately constant (slope below threshold). |
| `Peak` | Rose then fell (local maximum). |
| `Valley` | Fell then rose (local minimum). |
| `Unknown` | Not enough data to classify. |

Trait implementations: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

---

## Pivot Detection

### `PivotDetector`

Detects narrative pivots via Jensen-Shannon Divergence between consecutive tick event-type distributions. High JSD = dramatic turn; low JSD = continuation.

JSD is symmetric and bounded in [0, 1] (log base 2).

```rust
use fabula_narratives::pivot::PivotDetector;

let mut pivot = PivotDetector::new();

// Tick 1: peaceful events
pivot.push("trade"); pivot.push("trade"); pivot.push("talk");
let _ = pivot.end_tick(); // first tick: 0 (no previous)

// Tick 2: sudden violence
pivot.push("attack"); pivot.push("attack"); pivot.push("harm");
let jsd = pivot.end_tick();
assert!(jsd > 0.5); // dramatic shift
```

#### `PivotDetector::new`

```rust
pub fn new() -> Self
```

---

#### `push`

Record an event type for the current tick.

```rust
pub fn push(&mut self, event_type: &str)
```

---

#### `end_tick`

End the current tick: compute JSD against previous tick's distribution, save current as previous, clear accumulators. Returns JSD in [0, 1]. First tick returns 0.0. Empty ticks return 0.0 and leave the previous distribution unchanged.

```rust
pub fn end_tick(&mut self) -> f64
```

---

#### `last_pivot`

Most recent JSD value.

```rust
pub fn last_pivot(&self) -> f64
```

---

#### `average_pivot`

Average pivot magnitude over the last N ticks. Returns 0.0 if history is empty or window is 0.

```rust
pub fn average_pivot(&self, window: usize) -> f64
```

---

#### `history`

Full history of JSD values.

```rust
pub fn history(&self) -> &[f64]
```

---

#### `reset`

Reset all state.

```rust
pub fn reset(&mut self)
```

---

## Composite Scorer

### `score`

Pure function: signals in, score out. Combines multiple scoring signals into a single `NarrativeScore` using configurable weights.

```rust
use fabula_narratives::scorer::{score, NarrativeSignals, NarrativeWeights};

let signals = NarrativeSignals {
    advancements: 3,
    completions: 1,
    resolutions: 1,
    pivot_magnitude: 0.4,
    ..Default::default()
};
let result = score(&signals, &NarrativeWeights::default());
assert!(result.total > 0.0);
println!("Breakdown: {:?}", result.breakdown);
```

```rust
pub fn score(signals: &NarrativeSignals, weights: &NarrativeWeights) -> NarrativeScore
```

---

### `tension_fit`

Compute tension fit from a trajectory and desired direction. Returns 1.0 (match), -1.0 (opposite), or 0.0 (neutral/unknown).

```rust
pub fn tension_fit(actual: Trajectory, desired: Trajectory) -> f64
```

---

### `assemble_signals`

Convenience function: assemble `NarrativeSignals` from tracker outputs and engine data.

```rust
pub fn assemble_signals(
    delta: &TickDelta,
    plant_statuses: &[PlantStatus],
    filo_violations: usize,
    tension_trajectory: Trajectory,
    desired_trajectory: Trajectory,
    pivot_magnitude: f64,
    surprise: f64,
    sequential_surprise: f64,
) -> NarrativeSignals
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `delta` | `&TickDelta` | This tick's delta from the engine. |
| `plant_statuses` | `&[PlantStatus]` | From `engine.plant_status()`. |
| `filo_violations` | `usize` | From `tracker.check_filo().len()`. |
| `tension_trajectory` | `Trajectory` | From `tension.trajectory()`. |
| `desired_trajectory` | `Trajectory` | What the GM wants tension to do. |
| `pivot_magnitude` | `f64` | From `pivot.last_pivot()`. |
| `surprise` | `f64` | From `scorer.surprise_for()` or similar. |
| `sequential_surprise` | `f64` | From `SequentialScorer::score_transition()`. |

---

### `NarrativeWeights`

Configurable weights for each scoring signal. All have sensible defaults.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `progress` | `f64` | 1.0 | Reward per pattern advancement. |
| `completion` | `f64` | 3.0 | Reward per pattern completion. |
| `stall_penalty` | `f64` | -2.0 | Penalty per stalled pattern. |
| `unresolved_penalty` | `f64` | -0.5 | Penalty per unresolved plant. |
| `resolution_reward` | `f64` | 5.0 | Reward per resolved plant/payoff. |
| `filo_violation_penalty` | `f64` | -3.0 | Penalty per FILO nesting violation. |
| `tension_fit` | `f64` | 2.0 | Reward when tension matches desired trajectory. |
| `pivot_reward` | `f64` | 1.5 | Reward scaled by pivot magnitude. |
| `surprise_reward` | `f64` | 1.0 | Reward scaled by surprise score. |
| `sequential_surprise_reward` | `f64` | 1.0 | Reward scaled by sequential surprise score. |

---

### `NarrativeSignals`

Input signals for the scorer. Assemble manually or use `assemble_signals()`.

| Field | Type | Description |
|-------|------|-------------|
| `advancements` | `usize` | Patterns that advanced this tick. |
| `completions` | `usize` | Patterns that completed this tick. |
| `stalled` | `usize` | Stalled patterns. |
| `unresolved_plants` | `usize` | Unresolved plant setups. |
| `resolutions` | `usize` | Plant/payoff pairs resolved this tick. |
| `filo_violations` | `usize` | Thread nesting violations. |
| `tension_fit` | `f64` | 1.0 (match), -1.0 (opposite), 0.0 (neutral). |
| `pivot_magnitude` | `f64` | JSD from PivotDetector (0-1). |
| `surprise` | `f64` | Pattern-level surprise. |
| `sequential_surprise` | `f64` | Sequential transition surprise (from `SequentialScorer`). |

Trait implementations: `Debug`, `Clone`, `Default`.

---

### `NarrativeScore`

Composite score with explainable breakdown.

| Field | Type | Description |
|-------|------|-------------|
| `total` | `f64` | Overall quality score (higher = better). |
| `breakdown` | `ScoreBreakdown` | Per-signal contributions. |

---

### `ScoreBreakdown`

Per-signal contribution to the total score.

| Field | Type | Description |
|-------|------|-------------|
| `progress` | `f64` | From advancements * weight. |
| `completion` | `f64` | From completions * weight. |
| `stall_penalty` | `f64` | From stalled * weight (negative). |
| `unresolved_penalty` | `f64` | From unresolved plants * weight (negative). |
| `resolution` | `f64` | From resolutions * weight. |
| `filo_penalty` | `f64` | From violations * weight (negative). |
| `tension` | `f64` | From tension_fit * weight. |
| `pivot` | `f64` | From pivot_magnitude * weight. |
| `surprise` | `f64` | From surprise * weight. |
| `sequential_surprise` | `f64` | From sequential_surprise * weight. |

Trait implementations: `Debug`, `Clone`, `Default`.
