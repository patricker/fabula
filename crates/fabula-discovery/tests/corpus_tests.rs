use fabula::interval::Interval;
use fabula_discovery::{SharedNode, TraceCorpus};

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
                Interval { start, end },
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
        (
            "a".into(),
            "x".into(),
            "b".into(),
            Interval {
                start: 1i64,
                end: Some(2),
            },
        ),
        (
            "a".into(),
            "y".into(),
            "c".into(),
            Interval {
                start: 3,
                end: Some(4),
            },
        ),
        (
            "b".into(),
            "x".into(),
            "c".into(),
            Interval {
                start: 5,
                end: Some(6),
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let (train, test) = corpus.split_at(&3);
    assert_eq!(train.len(), 1); // only edge starting before t=3
    assert_eq!(test.len(), 2); // edges starting at t=3 and t=5
}

#[test]
fn corpus_label_pairs() {
    let trace_edges = vec![
        (
            "alice".into(),
            "trusts".into(),
            "bob".into(),
            Interval {
                start: 1i64,
                end: Some(5),
            },
        ),
        (
            "bob".into(),
            "betrays".into(),
            "alice".into(),
            Interval {
                start: 3,
                end: None,
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let pairs = corpus.label_pairs();
    // Should include ("trusts", "betrays") and ("betrays", "trusts")
    assert!(pairs.len() >= 2);
}

#[test]
fn pairwise_relations_all_shared_node_variants() {
    // Build a corpus that exercises all four SharedNode variants:
    // Source: both edges share the same source
    // SourceTarget: edge A's source == edge B's target
    // TargetSource: edge A's target == edge B's source
    // Target: both edges share the same target
    let trace_edges = vec![
        // Source variant: alice is source of both "likes" and "trusts"
        (
            "alice".into(),
            "likes".into(),
            "bob".into(),
            Interval {
                start: 1i64,
                end: Some(5),
            },
        ),
        (
            "alice".into(),
            "trusts".into(),
            "carol".into(),
            Interval {
                start: 2,
                end: Some(6),
            },
        ),
        // SourceTarget variant: dave is source of "helps" and target of "praises"
        (
            "dave".into(),
            "helps".into(),
            "eve".into(),
            Interval {
                start: 10,
                end: Some(15),
            },
        ),
        (
            "frank".into(),
            "praises".into(),
            "dave".into(),
            Interval {
                start: 11,
                end: Some(16),
            },
        ),
        // TargetSource variant: grace is target of "meets" and source of "follows"
        (
            "henry".into(),
            "meets".into(),
            "grace".into(),
            Interval {
                start: 20,
                end: Some(25),
            },
        ),
        (
            "grace".into(),
            "follows".into(),
            "irene".into(),
            Interval {
                start: 21,
                end: Some(26),
            },
        ),
        // Target variant: jack is target of both "admires" and "respects"
        (
            "kate".into(),
            "admires".into(),
            "jack".into(),
            Interval {
                start: 30,
                end: Some(35),
            },
        ),
        (
            "leo".into(),
            "respects".into(),
            "jack".into(),
            Interval {
                start: 31,
                end: Some(36),
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);

    // Source: likes(alice,bob) and trusts(alice,carol) share source "alice"
    let source_hits = corpus.pairwise_relations("likes", "trusts");
    assert!(
        !source_hits.is_empty(),
        "Should find Source shared-node hit for likes/trusts"
    );
    assert!(
        source_hits
            .iter()
            .any(|h| matches!(&h.shared_node, SharedNode::Source(n) if n == "alice")),
        "Should have Source(alice) hit"
    );

    // SourceTarget: helps(dave,eve) and praises(frank,dave) share node "dave"
    let st_hits = corpus.pairwise_relations("helps", "praises");
    assert!(
        !st_hits.is_empty(),
        "Should find SourceTarget shared-node hit for helps/praises"
    );
    assert!(
        st_hits
            .iter()
            .any(|h| matches!(&h.shared_node, SharedNode::SourceTarget(n) if n == "dave")),
        "Should have SourceTarget(dave) hit"
    );

    // TargetSource: meets(henry,grace) and follows(grace,irene) share node "grace"
    let ts_hits = corpus.pairwise_relations("meets", "follows");
    assert!(
        !ts_hits.is_empty(),
        "Should find TargetSource shared-node hit for meets/follows"
    );
    assert!(
        ts_hits
            .iter()
            .any(|h| matches!(&h.shared_node, SharedNode::TargetSource(n) if n == "grace")),
        "Should have TargetSource(grace) hit"
    );

    // Target: admires(kate,jack) and respects(leo,jack) share target "jack"
    let target_hits = corpus.pairwise_relations("admires", "respects");
    assert!(
        !target_hits.is_empty(),
        "Should find Target shared-node hit for admires/respects"
    );
    assert!(
        target_hits
            .iter()
            .any(|h| matches!(&h.shared_node, SharedNode::Target(n) if n == "jack")),
        "Should have Target(jack) hit"
    );
}

#[test]
fn pairwise_relations_open_ended_skipped() {
    // Two open-ended intervals sharing a node -- Allen relation is undefined (None),
    // so pairwise_relations should return no hits.
    let trace_edges = vec![
        (
            "alice".into(),
            "trusts".into(),
            "bob".into(),
            Interval {
                start: 1i64,
                end: None,
            },
        ),
        (
            "alice".into(),
            "betrays".into(),
            "bob".into(),
            Interval {
                start: 5,
                end: None,
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let hits = corpus.pairwise_relations("trusts", "betrays");
    assert!(
        hits.is_empty(),
        "Open-ended intervals should yield no pairwise hits (Allen relation undefined), got {} hits",
        hits.len()
    );
}

#[test]
fn split_at_boundary_values() {
    let trace_edges = vec![
        (
            "a".into(),
            "x".into(),
            "b".into(),
            Interval {
                start: 5i64,
                end: Some(10),
            },
        ),
        (
            "c".into(),
            "y".into(),
            "d".into(),
            Interval {
                start: 15,
                end: Some(20),
            },
        ),
        (
            "e".into(),
            "z".into(),
            "f".into(),
            Interval {
                start: 25,
                end: Some(30),
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);

    // Split at min_start (t = 5): no edges start before 5, so "before" is empty
    let (before, after) = corpus.split_at(&5);
    assert_eq!(
        before.len(),
        0,
        "Splitting at min_start should yield empty 'before'"
    );
    assert_eq!(
        after.len(),
        3,
        "Splitting at min_start should put all edges in 'after'"
    );

    // Split at t > max_start (t = 30): all edges start before 30, so "after" is empty
    let (before, after) = corpus.split_at(&30);
    assert_eq!(
        before.len(),
        3,
        "Splitting at t > max_start should put all edges in 'before'"
    );
    assert_eq!(
        after.len(),
        0,
        "Splitting at t > max_start should yield empty 'after'"
    );
}

#[test]
fn empty_corpus() {
    let corpus = TraceCorpus::new(vec![]);
    assert_eq!(corpus.len(), 0);
    assert!(corpus.is_empty());
    assert!(corpus.labels().is_empty());
    assert!(corpus.nodes().is_empty());
    assert_eq!(corpus.time_range(), (0, 0));
}

#[test]
fn time_range_all_open_ended() {
    let trace_edges = vec![
        (
            "a".into(),
            "x".into(),
            "b".into(),
            Interval {
                start: 10i64,
                end: None,
            },
        ),
        (
            "c".into(),
            "y".into(),
            "d".into(),
            Interval {
                start: 20,
                end: None,
            },
        ),
        (
            "e".into(),
            "z".into(),
            "f".into(),
            Interval {
                start: 5,
                end: None,
            },
        ),
    ];

    let corpus = TraceCorpus::new(trace_edges);
    let (min, max) = corpus.time_range();
    // min should be 5 (smallest start)
    assert_eq!(min, 5, "min should be the smallest start value");
    // max should use start values as fallback for open-ended: max(10, 20, 5) = 20
    assert_eq!(
        max, 20,
        "max should use start values as fallback for open-ended intervals"
    );
}
