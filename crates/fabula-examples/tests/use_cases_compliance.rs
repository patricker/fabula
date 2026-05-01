use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};

#[test]
fn gap_analysis_auditing() {
    // #region gap_analysis_auditing
    let violation_pattern = PatternBuilder::<String, MemValue>::new("unauthorized_access")
        .stage("e1", |s| {
            s.edge("e1", "type".into(), MemValue::Str("revoke".into()))
                .edge_bind("e1", "user".into(), "user")
                .edge_bind("e1", "resource".into(), "resource")
        })
        .stage("e2", |s| {
            s.edge("e2", "type".into(), MemValue::Str("access".into()))
                .edge_bind("e2", "user".into(), "user")
                .edge_bind("e2", "resource".into(), "resource")
        })
        .unless_between("e1", "e2", |neg| {
            neg.edge("mid", "type".into(), MemValue::Str("reauthorize".into()))
                .edge_bind("mid", "user".into(), "user")
                .edge_bind("mid", "resource".into(), "resource")
        })
        .build();

    // Build a compliant graph -- revoke then reauthorize then access.
    let mut graph = MemGraph::new();
    graph.add_str("e1", "type", "revoke", 1);
    graph.add_ref("e1", "user", "alice", 1);
    graph.add_ref("e1", "resource", "db_prod", 1);
    graph.add_str("mid", "type", "reauthorize", 2);
    graph.add_ref("mid", "user", "alice", 2);
    graph.add_ref("mid", "resource", "db_prod", 2);
    graph.add_str("e2", "type", "access", 3);
    graph.add_ref("e2", "user", "alice", 3);
    graph.add_ref("e2", "resource", "db_prod", 3);
    graph.set_time(10);

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(violation_pattern);

    let matches = engine.evaluate(&graph);
    if matches.is_empty() {
        // System is compliant. Check near-misses for each pattern:
        for pattern in engine.patterns() {
            let gap = gap_analysis(&graph, pattern);
            for stage in &gap.stages {
                match stage.status {
                    StageStatus::Matched => {}
                    StageStatus::Unmatched | StageStatus::PartiallyMatched { .. } => {
                        println!(
                            "Near-miss for '{}': stage '{}' -- {:?}",
                            pattern.name, stage.anchor, stage.status
                        );
                        for clause in &stage.clauses {
                            println!(
                                "  clause: matched={}, reason={:?}",
                                clause.matched, clause.reason
                            );
                        }
                    }
                }
            }
        }
    }
    // #endregion

    // The graph is compliant (reauthorized between revoke and access).
    assert!(matches.is_empty());
}
