use fabula::prelude::*;
use fabula_memory::{MemGraph, MemValue};
use std::collections::HashMap;

#[test]
fn step1_surprise_scorer() {
    // #region step1_surprise
    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new();

    let alliance_idx = engine.register(
        PatternBuilder::new("alliance")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("ally".into()))
                    .edge_bind("e1", "actor".into(), "a")
                    .edge_bind("e1", "target".into(), "b")
            })
            .build(),
    );

    let betrayal_idx = engine.register(
        PatternBuilder::new("betrayal")
            .stage("e1", |s| {
                s.edge("e1", "eventType".into(), MemValue::Str("betray".into()))
                    .edge_bind("e1", "actor".into(), "a")
                    .edge_bind("e1", "target".into(), "b")
            })
            .build(),
    );

    let mut scorer = SurpriseScorer::new();
    scorer.set_baseline(alliance_idx, 0.5);
    scorer.set_baseline(betrayal_idx, 0.1);

    let mut graph = MemGraph::new();

    for round in 1..=10 {
        graph.add_str(&format!("ev_ally_{round}"), "eventType", "ally", round);
        graph.add_ref(&format!("ev_ally_{round}"), "actor", "alice", round);
        graph.add_ref(&format!("ev_ally_{round}"), "target", "bob", round);
        if round == 7 {
            graph.add_str("ev_betray_1", "eventType", "betray", round);
            graph.add_ref("ev_betray_1", "actor", "alice", round);
            graph.add_ref("ev_betray_1", "target", "bob", round);
        }
        graph.set_time(round);

        let matches = engine.evaluate(&graph);
        scorer.observe(&matches, engine.patterns());
    }

    let final_matches = engine.evaluate(&graph);
    let scored = scorer.score(&final_matches, engine.patterns());

    for sm in &scored {
        println!("{}: {:.2} bits", sm.pattern, sm.surprise);
    }
    // #endregion
}

#[test]
fn step2_stu_scorer() {
    // #region step2_stu
    let mut stu = StuScorer::new();

    stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=king"]);
    stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);
    stu.observe_one("betrayal", &["actor_trait=ambitious", "target_role=merchant"]);
    stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);
    stu.observe_one("betrayal", &["actor_trait=loyal", "target_role=merchant"]);

    let mut bindings_rare: HashMap<String, BoundValue<String, MemValue>> = HashMap::new();
    bindings_rare.insert(
        "trait".into(),
        BoundValue::Value(MemValue::Str("ambitious".into())),
    );
    bindings_rare.insert(
        "role".into(),
        BoundValue::Value(MemValue::Str("king".into())),
    );

    let match_rare = Match {
        pattern: "betrayal".into(),
        pattern_idx: Some(0),
        bindings: bindings_rare,
        intervals: HashMap::new(),
        metadata: HashMap::new(),
    };

    fn extract_properties(m: &Match<String, MemValue, i64>) -> Vec<String> {
        let mut props = Vec::new();
        if let Some(BoundValue::Value(MemValue::Str(t))) = m.bindings.get("trait") {
            props.push(format!("actor_trait={t}"));
        }
        if let Some(BoundValue::Value(MemValue::Str(r))) = m.bindings.get("role") {
            props.push(format!("target_role={r}"));
        }
        props
    }

    let props_rare = extract_properties(&match_rare);
    let scored = stu.score(&[(match_rare, props_rare)]);
    println!("betrayal stu_score: {:.4}", scored[0].stu_score);
    for (prop, freq) in &scored[0].property_frequencies {
        println!("  {prop}: {freq:.3}");
    }
    // #endregion
}

#[test]
fn step3_aggregation_modes() {
    // #region step3_aggregation
    let _stu_tfidf = StuScorer::new().with_aggregation(StuAggregation::TfIdf);
    let _stu_min = StuScorer::new().with_aggregation(StuAggregation::Min);
    let _stu_geo = StuScorer::new().with_aggregation(StuAggregation::GeometricMean);
    // #endregion
}

#[test]
fn step4_pmi_correction() {
    // #region step4_pmi
    let mut stu = StuScorer::new().with_pmi_correction();

    for _ in 0..20 {
        stu.observe_one("raid", &["faction=rebels", "location=hideout"]);
    }
    for _ in 0..80 {
        stu.observe_one("raid", &["faction=crown", "location=castle"]);
    }

    let pmi = stu.pmi_for("raid", "faction=rebels", "location=hideout");
    println!("PMI(rebels, hideout): {:.2}", pmi.unwrap_or(0.0));
    // #endregion
}

#[test]
fn step5_sequential_surprise() {
    // #region step5_sequential
    let mut seq = SequentialScorer::new();

    seq.observe_transition("alliance", "trade");
    seq.observe_transition("alliance", "trade");
    seq.observe_transition("alliance", "trade");
    seq.observe_transition("alliance", "betrayal");
    seq.observe_transition("trade", "trade");
    seq.observe_transition("trade", "alliance");

    let surprise = seq.score_transition("alliance", "betrayal");
    let boring = seq.score_transition("alliance", "trade");
    println!("alliance -> betrayal: {surprise:.2} bits");
    println!("alliance -> trade:    {boring:.2} bits");
    // #endregion
}

#[test]
fn step5_sequential_integration() {
    // #region step5_sequential_integration
    let mut seq = SequentialScorer::new();
    let mut last_completed: Option<String> = None;

    // Inside your evaluation loop, after engine.evaluate() or drain_completed():
    let completed_matches: Vec<Match<String, String, i64>> = vec![];
    for m in &completed_matches {
        if let Some(prev) = &last_completed {
            seq.observe_transition(prev, &m.pattern);
        }
        last_completed = Some(m.pattern.clone());
    }
    // #endregion
    let _ = (seq, last_completed);
}

#[test]
fn step6_combine_scores() {
    // #region step6_combine
    struct RankedMatch {
        pattern: String,
        combined_score: f64,
        pattern_surprise: f64,
        stu_score: f64,
        sequential_surprise: f64,
    }

    fn rank_matches(
        pattern_scored: &[ScoredMatch<String, MemValue, i64>],
        stu_scored: &[StuScoredMatch<String, MemValue, i64>],
        seq: &SequentialScorer,
        last_pattern: Option<&str>,
        w_pattern: f64,
        w_stu: f64,
        w_seq: f64,
    ) -> Vec<RankedMatch> {
        let mut ranked: Vec<RankedMatch> = pattern_scored
            .iter()
            .zip(stu_scored.iter())
            .map(|(ps, ss)| {
                let seq_surprise = match last_pattern {
                    Some(prev) => seq.score_transition(prev, &ps.pattern),
                    None => 0.0,
                };
                let stu_inverted = 1.0 - ss.stu_score;
                let combined =
                    w_pattern * ps.surprise + w_stu * stu_inverted + w_seq * seq_surprise;
                RankedMatch {
                    pattern: ps.pattern.clone(),
                    combined_score: combined,
                    pattern_surprise: ps.surprise,
                    stu_score: ss.stu_score,
                    sequential_surprise: seq_surprise,
                }
            })
            .collect();
        ranked.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        ranked
    }
    // #endregion

    // Verify it compiles by calling with empty inputs
    let seq = SequentialScorer::new();
    let ranked = rank_matches(&[], &[], &seq, None, 0.4, 0.3, 0.3);

    // #region step6_print
    for (i, r) in ranked.iter().enumerate() {
        println!(
            "#{}: {} (combined={:.2}, pattern={:.2}, stu={:.4}, seq={:.2})",
            i + 1,
            r.pattern,
            r.combined_score,
            r.pattern_surprise,
            r.stu_score,
            r.sequential_surprise
        );
    }
    // #endregion
}
