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

**Performance:** Each BFS node triggers one `DataSource::scan_any_time` call per causal label, then filters by target. Total cost is `O(|edges_with_causal_labels| × |causal_labels| × |nodes_visited|)`. For graphs with many edges or tight hot paths (e.g., MCTS inner loops), consider caching the result or building a reverse adjacency index over causal labels once and reusing it. A future `DataSource::predecessors()` extension point would allow adapters to provide an indexed lookup; until then, plan for the scan cost.

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

## Related

- [How-to: Trace an effect back to its causes](/guides/tracing-causal-chains)
- [Engine reference](/reference/engine) -- for non-causal gap analysis (`why_not`)
