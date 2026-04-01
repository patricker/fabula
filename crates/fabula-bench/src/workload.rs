//! Workload generators for benchmarking and profiling.
//!
//! Two entry points:
//! - [`build_isolated_workload`] — pre-built graph + engine + pending edges for divan
//! - [`build_gm_workload`] — 200-tick realistic GM simulation for profiling

use fabula::prelude::*;
use fabula_test_suite::TestGraph;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// An edge waiting to be added to the graph and notified to the engine.
#[derive(Clone, Debug)]
pub struct PendingEdge {
    pub source: String,
    pub label: String,
    pub value_str: Option<String>,
    pub value_ref: Option<String>,
    pub value_num: Option<f64>,
    pub time: i64,
}

impl PendingEdge {
    /// Create a string-valued edge.
    pub fn new_str(source: &str, label: &str, value: &str, time: i64) -> Self {
        Self {
            source: source.to_string(),
            label: label.to_string(),
            value_str: Some(value.to_string()),
            value_ref: None,
            value_num: None,
            time,
        }
    }
    /// Create a node-reference edge.
    pub fn new_ref(source: &str, label: &str, to: &str, time: i64) -> Self {
        Self {
            source: source.to_string(),
            label: label.to_string(),
            value_str: None,
            value_ref: Some(to.to_string()),
            value_num: None,
            time,
        }
    }

    /// Insert this edge into the graph (without notifying the engine).
    pub fn insert<G: TestGraph>(&self, graph: &mut G) {
        if let Some(ref v) = self.value_str {
            graph.add_str_edge(&self.source, &self.label, v, self.time);
        } else if let Some(ref r) = self.value_ref {
            graph.add_ref_edge(&self.source, &self.label, r, self.time);
        } else if let Some(n) = self.value_num {
            graph.add_num_edge(&self.source, &self.label, n, self.time);
        }
    }

    /// Notify the engine about this edge (graph must already contain it).
    pub fn notify<G: TestGraph>(
        &self,
        graph: &G,
        engine: &mut SiftEngine<G>,
    ) -> Vec<SiftEvent<String, G::V>> {
        let interval = fabula::interval::Interval::open(self.time);
        if let Some(ref v) = self.value_str {
            engine.on_edge_added(graph, &self.source, &self.label, &G::str_val(v), &interval)
        } else if let Some(ref r) = self.value_ref {
            engine.on_edge_added(graph, &self.source, &self.label, &G::node_val(r), &interval)
        } else if let Some(n) = self.value_num {
            engine.on_edge_added(graph, &self.source, &self.label, &G::num_val(n), &interval)
        } else {
            Vec::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Seeded PRNG (minimal xorshift64 — no external dep)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn usize(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
    fn f64(&mut self) -> f64 {
        (self.next_u64() & 0x000F_FFFF_FFFF_FFFF) as f64 / (1u64 << 52) as f64
    }
}

// ---------------------------------------------------------------------------
// Pattern generators
// ---------------------------------------------------------------------------

/// Event types used in workload generation.
const COMMON_EVENTS: &[&str] = &["move", "talk", "trade", "observe", "wait"];
const RARE_EVENTS: &[&str] = &[
    "harm", "betray", "showHospitality", "promise", "breakPromise",
    "forgive", "steal", "gift", "threaten", "flee",
];

fn all_events() -> Vec<&'static str> {
    let mut v: Vec<&str> = COMMON_EVENTS.to_vec();
    v.extend_from_slice(RARE_EVENTS);
    v
}

fn character_name(i: usize) -> String {
    const NAMES: &[&str] = &[
        "alice", "bob", "charlie", "diana", "eve",
        "frank", "grace", "hector", "iris", "jack",
        "kate", "leon", "mira", "nolan", "olive",
        "pedro", "quinn", "rosa", "sam", "tara",
    ];
    if i < NAMES.len() {
        NAMES[i].to_string()
    } else {
        format!("char_{}", i)
    }
}

/// Generate a multi-stage pattern with optional negation.
fn gen_pattern<G: TestGraph>(
    name: &str,
    stages: usize,
    event_types: &[&str],
    with_negation: bool,
    rng: &mut Rng,
) -> Pattern<String, G::V> {
    let mut builder = PatternBuilder::<String, G::V>::new(name);
    let mut stage_anchors = Vec::new();

    for s in 0..stages {
        let anchor = format!("e{}", s);
        let event_type = event_types[rng.usize(event_types.len())];
        stage_anchors.push(anchor.clone());

        builder = builder.stage(&anchor, |sb| {
            let sb = sb.edge(&anchor, "eventType".to_string(), G::str_val(event_type));
            sb.edge_bind(&anchor, "actor".to_string(), "actor")
        });
    }

    if with_negation && stages >= 2 {
        let neg_event = event_types[rng.usize(event_types.len())];
        builder = builder.unless_between(
            &stage_anchors[0],
            stage_anchors.last().unwrap(),
            |neg| {
                neg.edge("neg_ev", "eventType".to_string(), G::str_val(neg_event))
                    .edge_bind("neg_ev", "actor".to_string(), "actor")
            },
        );
    }

    builder.build()
}

/// Generate a single-stage pattern (high fanout — matches many edges).
fn gen_single_stage_pattern<G: TestGraph>(
    name: &str,
    event_type: &str,
) -> Pattern<String, G::V> {
    PatternBuilder::<String, G::V>::new(name)
        .stage("e0", |s| {
            s.edge("e0", "eventType".to_string(), G::str_val(event_type))
                .edge_bind("e0", "actor".to_string(), "actor")
        })
        .build()
}

/// Generate a never-matching pattern (event type doesn't exist in workload).
fn gen_never_match_pattern<G: TestGraph>(name: &str, i: usize) -> Pattern<String, G::V> {
    let phantom = format!("phantom_event_{}", i);
    PatternBuilder::<String, G::V>::new(name)
        .stage("e0", |s| {
            s.edge("e0", "eventType".to_string(), G::str_val(&phantom))
                .edge_bind("e0", "actor".to_string(), "actor")
        })
        .build()
}

/// Generate a many-binding pattern (4+ bindings to stress HashMap cloning).
fn gen_many_binding_pattern<G: TestGraph>(
    name: &str,
    event_type: &str,
) -> Pattern<String, G::V> {
    PatternBuilder::<String, G::V>::new(name)
        .stage("e0", |s| {
            s.edge("e0", "eventType".to_string(), G::str_val(event_type))
                .edge_bind("e0", "actor".to_string(), "actor")
                .edge_bind("e0", "target".to_string(), "target")
                .edge_bind("e0", "location".to_string(), "location")
        })
        .stage("e1", |s| {
            s.edge("e1", "eventType".to_string(), G::str_val(event_type))
                .edge_bind("e1", "actor".to_string(), "actor")
                .edge_bind("e1", "witness".to_string(), "witness")
        })
        .build()
}

// ---------------------------------------------------------------------------
// Isolated workload (for divan benchmarks)
// ---------------------------------------------------------------------------

/// Configuration for isolated benchmark workloads.
#[derive(Clone, Debug)]
pub struct WorkloadConfig {
    pub pattern_count: usize,
    pub stages_per_pattern: usize,
    pub negation_fraction: f64,
    pub pre_existing_edges: usize,
    pub edges_per_tick: usize,
    pub character_count: usize,
    pub seed: u64,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            pattern_count: 30,
            stages_per_pattern: 3,
            negation_fraction: 0.5,
            pre_existing_edges: 5000,
            edges_per_tick: 10,
            character_count: 10,
            seed: 42,
        }
    }
}

/// Pre-built workload ready for benchmarking.
pub struct IsolatedWorkload<G: TestGraph> {
    pub graph: G,
    pub engine: SiftEngine<G>,
    pub pending_edges: Vec<PendingEdge>,
}

/// Build a deterministic workload for divan benchmarks.
///
/// The graph is pre-populated with `config.pre_existing_edges`. The engine
/// has `config.pattern_count` patterns registered. `pending_edges` contains
/// `config.edges_per_tick` edges ready to be fed via `on_edge_added`.
pub fn build_isolated_workload<G: TestGraph>(config: &WorkloadConfig) -> IsolatedWorkload<G> {
    let mut rng = Rng::new(config.seed);
    let events = all_events();
    let mut graph = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();

    // Register patterns
    let patterns_with_neg =
        (config.pattern_count as f64 * config.negation_fraction).round() as usize;
    for i in 0..config.pattern_count {
        let name = format!("pat_{}", i);
        let with_neg = i < patterns_with_neg;
        let pattern = gen_pattern::<G>(
            &name,
            config.stages_per_pattern,
            &events,
            with_neg,
            &mut rng,
        );
        engine.register(pattern);
    }

    // Pre-populate graph
    let base_time = 1i64;
    for t in 0..config.pre_existing_edges {
        let time = base_time + t as i64;
        let actor = character_name(rng.usize(config.character_count));
        // 60% common events, 40% rare
        let event_type = if rng.f64() < 0.6 {
            COMMON_EVENTS[rng.usize(COMMON_EVENTS.len())]
        } else {
            RARE_EVENTS[rng.usize(RARE_EVENTS.len())]
        };

        let ev_node = format!("ev_{}", t);
        graph.add_str_edge(&ev_node, "eventType", event_type, time);
        graph.add_ref_edge(&ev_node, "actor", &actor, time);

        // Some events have targets
        if rng.f64() < 0.4 {
            let target = character_name(rng.usize(config.character_count));
            graph.add_ref_edge(&ev_node, "target", &target, time);
        }
    }

    let next_time = base_time + config.pre_existing_edges as i64;
    graph.set_current_time(next_time);

    // Generate pending edges for the benchmark tick
    let mut pending_edges = Vec::new();
    for i in 0..config.edges_per_tick {
        let time = next_time + i as i64;
        let actor = character_name(rng.usize(config.character_count));
        let event_type = if rng.f64() < 0.6 {
            COMMON_EVENTS[rng.usize(COMMON_EVENTS.len())]
        } else {
            RARE_EVENTS[rng.usize(RARE_EVENTS.len())]
        };

        let ev_node = format!("bench_ev_{}", i);
        pending_edges.push(PendingEdge::new_str(&ev_node, "eventType", event_type, time));
        pending_edges.push(PendingEdge::new_ref(&ev_node, "actor", &actor, time));

        if rng.f64() < 0.4 {
            let target = character_name(rng.usize(config.character_count));
            pending_edges.push(PendingEdge::new_ref(&ev_node, "target", &target, time));
        }
    }

    IsolatedWorkload {
        graph,
        engine,
        pending_edges,
    }
}

// ---------------------------------------------------------------------------
// GM-profile workload (for profiling binary)
// ---------------------------------------------------------------------------

/// A single simulation tick with its edges.
#[derive(Clone, Debug)]
pub struct Tick {
    pub time: i64,
    pub edges: Vec<PendingEdge>,
}

/// Full GM-profile workload: 200 ticks of incremental evaluation.
pub struct GmWorkload<G: TestGraph> {
    pub graph: G,
    pub engine: SiftEngine<G>,
    pub ticks: Vec<Tick>,
}

/// Build a realistic GM-profile workload.
///
/// - 30 patterns across 5 categories (multi-stage, high-fanout, negation-heavy,
///   many-binding, never-matching)
/// - 10 characters, 15 event types
/// - 200 ticks, ~10-15 events/tick (~3K total edges)
/// - Characters perform 1-2 actions per tick
pub fn build_gm_workload<G: TestGraph>() -> GmWorkload<G> {
    let mut rng = Rng::new(42);
    let events = all_events();
    let graph = G::new_graph();
    let mut engine: SiftEngine<G> = SiftEngine::new();
    let character_count = 10;

    // -- Category 1: Multi-stage with negation (6 patterns) --
    for i in 0..6 {
        let stages = 2 + (i % 3); // 2, 3, 4, 2, 3, 4
        let pattern = gen_pattern::<G>(
            &format!("multistage_{}", i),
            stages,
            &events,
            true,
            &mut rng,
        );
        engine.register(pattern);
    }

    // -- Category 2: High-fanout single-stage (6 patterns) --
    for (i, &evt) in COMMON_EVENTS.iter().enumerate().take(5) {
        engine.register(gen_single_stage_pattern::<G>(
            &format!("fanout_{}", i),
            evt,
        ));
    }
    engine.register(gen_single_stage_pattern::<G>("fanout_5", "harm"));

    // -- Category 3: Negation-heavy (6 patterns) --
    for i in 0..6 {
        let name = format!("neg_heavy_{}", i);
        // 3-stage with negation, using rare events to trigger more negation checks
        let mut builder = PatternBuilder::<String, G::V>::new(&name);
        let evt_a = RARE_EVENTS[rng.usize(RARE_EVENTS.len())];
        let evt_b = RARE_EVENTS[rng.usize(RARE_EVENTS.len())];
        let neg_evt = COMMON_EVENTS[rng.usize(COMMON_EVENTS.len())];

        builder = builder
            .stage("e0", |s| {
                s.edge("e0", "eventType".to_string(), G::str_val(evt_a))
                    .edge_bind("e0", "actor".to_string(), "actor")
            })
            .stage("e1", |s| {
                s.edge("e1", "eventType".to_string(), G::str_val(evt_b))
                    .edge_bind("e1", "actor".to_string(), "actor")
            })
            .unless_between("e0", "e1", |neg| {
                neg.edge("n0", "eventType".to_string(), G::str_val(neg_evt))
                    .edge_bind("n0", "actor".to_string(), "actor")
            });

        // Add a second negation window for extra negation checking pressure
        let neg_evt2 = COMMON_EVENTS[rng.usize(COMMON_EVENTS.len())];
        builder = builder.unless_after("e0", |neg| {
            neg.edge("n1", "eventType".to_string(), G::str_val(neg_evt2))
                .edge_bind("n1", "actor".to_string(), "actor")
        });

        engine.register(builder.build());
    }

    // -- Category 4: Many-binding patterns (6 patterns) --
    for i in 0..6 {
        let evt = events[rng.usize(events.len())];
        engine.register(gen_many_binding_pattern::<G>(
            &format!("many_bind_{}", i),
            evt,
        ));
    }

    // -- Category 5: Never-matching patterns (6 patterns) --
    for i in 0..6 {
        engine.register(gen_never_match_pattern::<G>(&format!("phantom_{}", i), i));
    }

    // -- Generate 200 ticks of simulation --
    let mut ticks = Vec::with_capacity(200);
    let mut event_counter = 0u64;

    for tick in 0..200i64 {
        let time = tick + 1;
        let mut edges = Vec::new();

        // Each character has a chance to act (10 chars, ~60% act per tick = ~6 events)
        // Plus some events have 2-3 edges each = ~15-20 edges per tick
        for c in 0..character_count {
            if rng.f64() > 0.6 {
                continue; // character doesn't act this tick
            }

            let actor = character_name(c);
            let event_type = if rng.f64() < 0.6 {
                COMMON_EVENTS[rng.usize(COMMON_EVENTS.len())]
            } else {
                RARE_EVENTS[rng.usize(RARE_EVENTS.len())]
            };

            let ev_node = format!("gm_ev_{}", event_counter);
            event_counter += 1;

            edges.push(PendingEdge::new_str(&ev_node, "eventType", event_type, time));
            edges.push(PendingEdge::new_ref(&ev_node, "actor", &actor, time));

            // 40% of events have a target (another character)
            if rng.f64() < 0.4 {
                let mut target_idx = rng.usize(character_count);
                if target_idx == c {
                    target_idx = (c + 1) % character_count;
                }
                let target = character_name(target_idx);
                edges.push(PendingEdge::new_ref(&ev_node, "target", &target, time));
            }

            // 20% of events have a location
            if rng.f64() < 0.2 {
                let loc = format!("loc_{}", rng.usize(5));
                edges.push(PendingEdge::new_ref(&ev_node, "location", &loc, time));
            }
        }

        ticks.push(Tick { time, edges });
    }

    GmWorkload {
        graph,
        engine,
        ticks,
    }
}
