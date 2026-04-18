---
sidebar_position: 9
title: Causality
---

# Causal Pathfinding

`fabula::causality` -- retrospective tracing of causal chains through explicit causal edges in a temporal graph.

## Mental model

Causality is represented as **explicit edges** in your graph. An edge labeled `"causes"` from event A to event B means "A caused B." Callers mark which labels are causal and with what weight; edges not in the causal labels map are ignored during traversal.

This is a **retrospective** tool: given an effect, find the chain of causes that led to it. Forward projection ("what might this cause?") is out of scope.

---

## `causal_paths`

```rust
pub fn causal_paths<DS: DataSource>(
    ds: &DS,
    effect: &DS::N,
    max_hops: usize,
    causal_labels: &HashMap<DS::L, f64>,
) -> Vec<CausalPath<DS::N, DS::V, DS::T>>
where
    DS::T: NumericTime,
```

Walks backward from `effect` through causal edges, returning all paths of length up to `max_hops`, sorted by cleanliness descending.

| Parameter | Type | Description |
|---|---|---|
| `ds` | `&impl DataSource` | The graph to search. |
| `effect` | `&DS::N` | The node whose causes you want to trace. |
| `max_hops` | `usize` | Maximum number of edges per returned path. |
| `causal_labels` | `&HashMap<DS::L, f64>` | Which edge labels count as causal, and their weights in `(0.0, 1.0]`. |

**Temporal validation:** An edge is only followed if its time is strictly less than the next hop's time. Out-of-order causal edges are skipped.

**Cycle guard:** Each path tracks its own visited set; a node cannot appear twice in the same path.

**Emission:** Every depth along every branch is returned — a chain `a → b → c → d` queried at `d` yields three paths (`{c,d}`, `{b,c,d}`, `{a,b,c,d}`), each scored independently. Short paths typically outrank long ones because `gap_penalty` grows and `divergence_factor` only shrinks with depth.

**Performance:** Each BFS node triggers one `DataSource::predecessors(node, label)` call per causal label. The trait's default implementation scans all edges with that label and filters by target — `O(|edges_with_label|)` per call. For graphs with many edges or tight hot paths (e.g., MCTS inner loops), adapters can override `predecessors` with an indexed reverse-adjacency lookup for `O(1)` access. The in-tree adapters (`MemGraph`, `PetGraph`, `GrafeoGraph`) currently use the scanning default; contribute an override if you hit this in profile.

---

## `CausalPath`

| Field | Type | Description |
|---|---|---|
| `nodes` | `Vec<N>` | Path nodes ordered root-cause → effect. `len() == edges.len() + 1`. |
| `edges` | `Vec<CausalEdge<V, T>>` | Edges between consecutive nodes. |
| `cleanliness` | `f64` | Quality score in `[0.0, 1.0]`. Higher is better. |
| `confidence` | `f64` | Weakest-link confidence — the minimum edge weight along the path. Captures "a chain is only as strong as its least-certain edge," and provides a distinct signal from `cleanliness` (which averages weights rather than taking the minimum). |

### Cleanliness formula

```
cleanliness = mean(edge_weights) × (1 − gap_penalty) × divergence_factor
```

- `mean(edge_weights)` — arithmetic mean of each edge's weight from the causal labels map.
- `gap_penalty` — saturating at `0.5`, derived from total time span: `0.5 × (1 − exp(−total_gap / 50.0))`.
- `divergence_factor = 1.0 / (1.0 + divergent_branches)` — `divergent_branches` is the total count of sibling causes along the path (every node with more than one causal predecessor contributes). Penalizes paths that pass through highly-branched nodes.

The scorer is also exposed as a standalone function for explainability and re-scoring with altered inputs:

```rust
pub fn cleanliness_score(
    weights: &[f64],
    total_gap: f64,
    divergent_branches: usize,
) -> f64
```

Pass it the edge weights, summed absolute time gaps between consecutive edges, and the total branches-skipped count to reproduce a path's score or explore "what if this weight were higher?" without re-traversing the graph.

---

## `CausalEdge`

| Field | Type | Description |
|---|---|---|
| `value` | `V` | The edge's target value as it appeared in the graph. |
| `time` | `T` | The edge's start time. |
| `weight` | `f64` | The weight contributed by the edge's label. |

---

## `event_causal_surprise`

```rust
pub fn event_causal_surprise<DS: DataSource>(
    ds: &DS,
    event: &DS::N,
    max_hops: usize,
    causal_labels: &HashMap<DS::L, f64>,
) -> f64
where
    DS::T: NumericTime,
```

Score an event's *contextual* surprise — how predictable was this event given the causal graph leading to it? Returns a value in `[0.0, 1.0]`.

| Parameter | Type | Description |
|---|---|---|
| `ds` | `&impl DataSource` | The graph to query. |
| `event` | `&DS::N` | The event node being scored. |
| `max_hops` | `usize` | Max depth for the backward search. Short windows (3–5) are typical. |
| `causal_labels` | `&HashMap<DS::L, f64>` | Same map you pass to `causal_paths`. |

### Formula

```
surprise = 1.0 − best_path_cleanliness
```

where `best_path_cleanliness` is the highest `cleanliness` among paths returned by [`causal_paths`]. When no paths exist, `surprise = 1.0`.

### Interpretation

| Score | Meaning |
|---|---|
| `0.0` | Event is fully explained by a clean, short causal chain. Expected. |
| `~0.5` | Either multiple candidate causes (divergent predecessors share a path) or a moderately weak causal weight. Partially explained. |
| `1.0` | No causal explanation — the event "came out of nowhere." |

Note that **the proximate cause dominates**: because `causal_paths` emits every depth and sorts cleanliness-descending, a short clean chain will always outrank a longer one. Temporal gap only affects paths with two or more edges, so a distant upstream history does *not* raise surprise as long as a direct immediate cause exists. If you want "long gap" to count against the score, you'd need to post-process `causal_paths` yourself and pick a different path-selection rule.

### Contextual vs statistical surprise

This is **contextual** surprise — "given what just happened, was this predictable?" It is orthogonal to the **statistical** surprise scorers in [`fabula::scoring`](/reference/scoring), which measure how unusual an event is relative to a baseline frequency.

A statistically common event (e.g. "another betrayal this tick") can still be contextually surprising if nothing in the causal graph led to it. A statistically rare event can be contextually unsurprising if a clean chain explains it. Compose both signals in your downstream scoring.

---

## `event_causal_surprise_batch`

```rust
pub fn event_causal_surprise_batch<DS: DataSource>(
    ds: &DS,
    events: &[DS::N],
    max_hops: usize,
    causal_labels: &HashMap<DS::L, f64>,
) -> Vec<f64>
where
    DS::T: NumericTime,
```

Compute [`event_causal_surprise`] for each input event. Returns a `Vec<f64>` the same length as `events`, in the same order.

Useful when you want to score every completion from a tick at once and feed the results into a narrative signal pipeline. Computation is independent per event; cost scales linearly with input length.

---

## Related

- [How-to: Trace an effect back to its causes](/guides/tracing-causal-chains)
- [Engine reference](/reference/engine) -- for non-causal gap analysis (`why_not`)
