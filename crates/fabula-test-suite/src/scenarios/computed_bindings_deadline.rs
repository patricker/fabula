//! A pattern using `let deadline = ?ts + 5` to assert that a follow-up event
//! occurs at exactly the deadline pulse. Runs across all in-tree adapters.

use crate::TestGraph;
use fabula::builder::PatternBuilder;
use fabula::engine::{DefaultLetEvaluator, SiftEngine, SiftEngineFor, SiftEvent};
use fabula::expr::{BinOp, Expr};
use fabula::interval::Interval;
use fabula::pattern::Pattern;

/// Build a pattern equivalent to:
///
/// ```text
/// pattern deadline_match {
///     stage e1 {
///         e1.type = "world"
///         e1.pulse_count -> ?ts
///     }
///     let deadline = ?ts + 5
///     stage e2 {
///         e2.type = "world"
///         e2.pulse_count = ?deadline
///     }
/// }
/// ```
///
/// Built via `PatternBuilder` since the test-suite is generic over `G::V` and
/// can't go through the DSL (which is fixed to `MemValue` at the public API).
fn deadline_pattern<G: TestGraph>() -> Pattern<String, G::V> {
    PatternBuilder::<String, G::V>::new("deadline_match")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), G::str_val("world"))
                .edge_bind("e1", "pulse_count".into(), "ts")
                .let_binding(
                    "deadline",
                    Expr::bin(BinOp::Add, Expr::var("ts"), Expr::lit(G::num_val(5.0))),
                )
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), G::str_val("world"))
                .edge_eq_var("e2", "pulse_count".into(), "deadline")
        })
        .build()
}

/// Batch evaluation: a pattern with `let deadline = ?ts + 5` matches when the
/// follow-up event's `pulse_count` equals `?ts + 5`.
pub fn batch_computed_bindings_deadline<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("e1", "type", "world", 1);
    g.add_num_edge("e1", "pulse_count", 3.0, 1);
    g.add_str_edge("e2", "type", "world", 5);
    g.add_num_edge("e2", "pulse_count", 8.0, 5);
    g.set_current_time(10);

    let pattern = deadline_pattern::<G>();
    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);

    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        1,
        "{}: expected one batch match",
        std::any::type_name::<G>()
    );
}

/// Incremental evaluation: edges arrive in chronological order with secondary
/// clauses BEFORE the trigger (so `edges_from` lookups succeed at trigger time);
/// the engine emits a single Completed event after stage 2's trigger arrives.
pub fn incremental_computed_bindings_deadline<G: TestGraph>() {
    let mut g = G::new_graph();
    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(deadline_pattern::<G>());

    // Stage 1 edges. Add the secondary first so it's visible at trigger time.
    g.add_num_edge("e1", "pulse_count", 3.0, 1);
    g.set_current_time(1);
    g.add_str_edge("e1", "type", "world", 1);
    engine.on_edge_added(
        &g,
        &"e1".to_string(),
        &"type".to_string(),
        &G::str_val("world"),
        &Interval::open(1),
    );

    // Stage 2 edges. Same pattern: secondary first, trigger second.
    g.add_num_edge("e2", "pulse_count", 8.0, 5);
    g.set_current_time(5);
    g.add_str_edge("e2", "type", "world", 5);
    let evs = engine.on_edge_added(
        &g,
        &"e2".to_string(),
        &"type".to_string(),
        &G::str_val("world"),
        &Interval::open(5),
    );

    let completed = evs
        .iter()
        .filter(|e| matches!(e, SiftEvent::Completed { .. }))
        .count();
    assert_eq!(
        completed,
        1,
        "{}: expected one Completed event",
        std::any::type_name::<G>()
    );
}

/// Negative case: when `pulse_count = 99` (not 8 = ?ts + 5), no match emerges.
pub fn batch_computed_bindings_deadline_no_match<G: TestGraph>() {
    let mut g = G::new_graph();
    g.add_str_edge("e1", "type", "world", 1);
    g.add_num_edge("e1", "pulse_count", 3.0, 1);
    g.add_str_edge("e2", "type", "world", 5);
    g.add_num_edge("e2", "pulse_count", 99.0, 5); // wrong value
    g.set_current_time(10);

    let mut engine: SiftEngineFor<G> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(deadline_pattern::<G>());

    let matches = engine.evaluate(&g);
    assert_eq!(
        matches.len(),
        0,
        "{}: expected no match (deadline mismatch)",
        std::any::type_name::<G>()
    );
}
