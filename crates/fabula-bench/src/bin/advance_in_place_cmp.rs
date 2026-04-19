//! Compare peak PM count with and without advance_in_place.
//!
//! Runs a synthetic "crowd" workload (200 actors entering and leaving) against
//! a simple two-stage pattern, first without the flag, then with it. Prints
//! peak active PM count for each and the ratio.

use fabula::builder::PatternBuilder;
use fabula::engine::{SiftEngine, SiftEngineFor};
use fabula::interval::Interval;
use fabula::pattern::Pattern;
use fabula_memory::{MemGraph, MemValue};

fn pattern(advance_in_place: bool) -> Pattern<String, MemValue> {
    let mut b = PatternBuilder::<String, MemValue>::new("enter_then_leave")
        .stage("a", |s| {
            s.edge("a", "eventType".to_string(), MemValue::Str("enter".into()))
        })
        .stage("b", |s| {
            s.edge("b", "eventType".to_string(), MemValue::Str("leave".into()))
        });
    if advance_in_place {
        b = b.advance_in_place();
    }
    b.build()
}

use fabula::engine::MatchState;

/// Returns (active_pm_count, total_pm_count) at end of workload.
fn run_workload(advance_in_place: bool) -> (usize, usize) {
    let mut g = MemGraph::new();
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();
    engine.register(pattern(advance_in_place));

    // 200 enters.
    for i in 0..200 {
        let ev = format!("ev_enter_{i}");
        let t = i as i64 * 2;
        g.add_str(&ev, "eventType", "enter", t);
        g.set_time(t);
        engine.on_edge_added(
            &g,
            &ev,
            &"eventType".to_string(),
            &MemValue::Str("enter".into()),
            &Interval::open(t),
        );
    }
    engine.end_tick(0);

    // 200 leaves. Without the flag, each leave edge matches every alive
    // stage-1 PM, producing 200 Complete forks per leave and keeping all
    // 200 stage-1 PMs alive for the next leave -- PM accumulation blows up.
    // With the flag, the first leave consumes all stage-1 PMs; subsequent
    // leaves have nothing to match.
    for i in 0..200 {
        let ev = format!("ev_leave_{i}");
        let t = 500 + i as i64 * 2;
        g.add_str(&ev, "eventType", "leave", t);
        g.set_time(t);
        engine.on_edge_added(
            &g,
            &ev,
            &"eventType".to_string(),
            &MemValue::Str("leave".into()),
            &Interval::open(t),
        );
    }
    engine.end_tick(0);

    let active = engine
        .partial_matches()
        .iter()
        .filter(|pm| pm.state == MatchState::Active)
        .count();
    let total = engine.partial_matches().len();
    (active, total)
}

fn main() {
    let (active_baseline, total_baseline) = run_workload(false);
    let (active_optimized, total_optimized) = run_workload(true);
    println!("metric,without_flag,with_flag");
    println!("active_pms_at_end,{active_baseline},{active_optimized}");
    println!("total_pms_at_end,{total_baseline},{total_optimized}");
    let active_ratio = if active_baseline > 0 {
        active_optimized as f64 / active_baseline as f64
    } else {
        0.0
    };
    let total_ratio = if total_baseline > 0 {
        total_optimized as f64 / total_baseline as f64
    } else {
        0.0
    };
    println!("active_ratio,{active_ratio:.3}");
    println!("total_ratio,{total_ratio:.3}");
}
