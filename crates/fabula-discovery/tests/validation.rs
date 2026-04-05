//! Validation experiments for fabula-discovery.
//!
//! These tests go beyond unit testing to validate that the discovery
//! framework produces meaningful, generalizable patterns from realistic data.
//!
//! Run with detailed output:
//! ```bash
//! cargo test -p fabula-discovery --test validation -- --nocapture
//! ```

use fabula::engine::evaluate_pattern;
use fabula::interval::Interval;
use fabula_discovery::evaluators::{MatchQualityEvaluator, SurpriseEvaluator};
use fabula_discovery::generators::{MinerfulConfig, MinerfulGenerator};
use fabula_discovery::{
    pattern_to_dsl, DiscoverySession, PatternEvaluator, PatternFilter, ScoredPattern,
    SessionConfig, SessionHistory, TraceCorpus,
};
use fabula_memory::{MemGraph, MemValue};
use std::collections::HashMap;

// ─── Helpers ──────────────────────────────────────────────────────────

fn edge(
    src: &str,
    label: &str,
    tgt: &str,
    start: i64,
    end: i64,
) -> (String, String, String, Interval<i64>) {
    (
        src.into(),
        label.into(),
        tgt.into(),
        Interval {
            start,
            end: Some(end),
        },
    )
}

/// Build a MemGraph with open-ended intervals (all edges visible at now()).
/// evaluate_pattern is a snapshot query at ds.now() — bounded edges ending
/// before now() are invisible. Open-ended intervals with start-time ordering
/// preserve temporal constraint checking for Before/Meets relations.
fn corpus_to_open_memgraph(corpus: &TraceCorpus) -> MemGraph {
    let mut graph = MemGraph::new();
    let (_, max_t) = corpus.time_range();
    graph.set_time(max_t + 1);
    for e in corpus.edges() {
        let val = MemValue::Node(e.target.clone());
        graph.add_edge(&e.source, &e.label, val, e.interval.start);
    }
    graph
}

/// Accept any pattern with at least one positive score.
struct AcceptPositive;
impl PatternFilter for AcceptPositive {
    fn accept(&self, scored: &ScoredPattern<String, String>) -> bool {
        scored.score.scores.values().any(|&v| v > 0.0)
    }
}

/// Accept all patterns unconditionally.
struct AcceptAll;
impl PatternFilter for AcceptAll {
    fn accept(&self, _scored: &ScoredPattern<String, String>) -> bool {
        true
    }
}

fn discover(
    corpus: &TraceCorpus,
    config: MinerfulConfig,
    filter: impl PatternFilter,
) -> SessionHistory {
    let generator = MinerfulGenerator::new(config);
    let evaluators: Vec<Box<dyn PatternEvaluator>> =
        vec![Box::new(MatchQualityEvaluator), Box::new(SurpriseEvaluator)];
    let mut session = DiscoverySession::new(SessionConfig::default());
    session.run(corpus, generator, evaluators, filter)
}

/// Extract label pair from a 2-stage scored pattern.
fn label_pair(sp: &ScoredPattern<String, String>) -> (String, String) {
    let labels: Vec<&str> = sp
        .pattern
        .stages
        .iter()
        .flat_map(|s| s.clauses.iter())
        .map(|c| c.label.as_str())
        .collect();
    (
        labels.first().unwrap_or(&"?").to_string(),
        labels.last().unwrap_or(&"?").to_string(),
    )
}

/// Check if any discovered pattern contains both labels (either order).
fn has_label_pair(patterns: &[ScoredPattern<String, String>], a: &str, b: &str) -> bool {
    patterns.iter().any(|sp| {
        let (l1, l2) = label_pair(sp);
        (l1 == a && l2 == b) || (l1 == b && l2 == a)
    })
}

fn print_patterns(patterns: &[ScoredPattern<String, String>]) {
    for (i, sp) in patterns.iter().enumerate() {
        let (l1, l2) = label_pair(sp);
        let mq = sp.score.scores.get("match_quality").copied().unwrap_or(0.0);
        let su = sp.score.scores.get("surprise").copied().unwrap_or(0.0);
        let composite = sp.score.composite(&HashMap::new());
        println!(
            "  [{:2}] {:<50} MQ={:7.3} SU={:7.3} C={:7.3}  ({} -> {})",
            i, sp.pattern.name, mq, su, composite, l1, l2
        );
    }
}

// ─── Corpus Builders ─────────────────────────────────────────────────

/// Medieval court with 3 planted patterns + noise.
///
/// P1: befriend Before betray (shared source = the betrayer)  — 3 instances
/// P2: arrive Before depart (shared source = the traveler)    — 3 instances
/// P3: accuse Before exile (accuse.target = exile.source)     — 3 instances
fn medieval_court_corpus() -> TraceCorpus {
    TraceCorpus::new(vec![
        // P1: befriend -> betray (shared source)
        edge("alice", "befriend", "bob", 1, 3),
        edge("alice", "betray", "charlie", 5, 7),
        edge("dave", "befriend", "eve", 10, 12),
        edge("dave", "betray", "frank", 15, 17),
        edge("gwen", "befriend", "hank", 20, 22),
        edge("gwen", "betray", "iris", 25, 27),
        // P2: arrive -> depart (shared source, different destinations)
        edge("alice", "arrive", "north_gate", 30, 32),
        edge("alice", "depart", "south_gate", 35, 37),
        edge("bob", "arrive", "east_gate", 40, 42),
        edge("bob", "depart", "west_gate", 45, 47),
        edge("charlie", "arrive", "main_gate", 50, 52),
        edge("charlie", "depart", "back_gate", 55, 57),
        // P3: accuse -> exile (TargetSource: accuse target = exile source)
        edge("king", "accuse", "alice", 60, 62),
        edge("alice", "exile", "village", 65, 67),
        edge("queen", "accuse", "bob", 70, 72),
        edge("bob", "exile", "forest", 75, 77),
        edge("duke", "accuse", "charlie", 80, 82),
        edge("charlie", "exile", "island", 85, 87),
        // Noise: 12 edges with varied labels/nodes, minimal shared nodes
        edge("iris", "observe", "sunset", 2, 4),
        edge("frank", "speak", "hank_house", 8, 10),
        edge("eve", "feast", "hall", 14, 16),
        edge("hank", "pray", "temple", 22, 24),
        edge("iris", "wander", "meadow", 28, 30),
        edge("king", "decree", "law_1", 34, 36),
        edge("queen", "decree", "law_2", 42, 44),
        edge("duke_manor", "observe", "sunrise", 48, 50),
        edge("village_sq", "trade", "market", 56, 58),
        edge("deep_forest", "grow", "oak", 64, 66),
        edge("far_island", "erode", "cliff", 72, 74),
        edge("old_temple", "ring", "bells", 82, 84),
    ])
}

/// Consistent patterns in two time windows for holdout testing.
/// Window 1 (t=0..100): 5x request->complete + noise
/// Window 2 (t=100..200): 5x request->complete + noise
fn temporal_corpus() -> TraceCorpus {
    let mut edges = Vec::new();
    // Window 1: t=0..100 — 5 request/complete pairs
    for i in 0..5 {
        let t = i * 20i64;
        let src = format!("agent_{}", i);
        let task = format!("task_{}", i);
        edges.push(edge(&src, "request", &task, t, t + 3));
        edges.push(edge(&src, "complete", &task, t + 8, t + 12));
    }
    // Window 2: t=100..200 — 5 request/complete pairs (different agents)
    for i in 0..5 {
        let t = 100 + i * 20i64;
        let src = format!("agent_{}", i + 10);
        let task = format!("task_{}", i + 10);
        edges.push(edge(&src, "request", &task, t, t + 3));
        edges.push(edge(&src, "complete", &task, t + 8, t + 12));
    }
    // Noise in both windows
    for i in 0..6 {
        let t = i * 30i64;
        edges.push(edge(
            &format!("bg_{}", i),
            "idle",
            &format!("loc_{}", i),
            t,
            t + 5,
        ));
    }
    TraceCorpus::new(edges)
}

/// Calibration corpus: boring (common), interesting (rare), medium patterns.
fn calibration_corpus() -> TraceCorpus {
    let mut edges = Vec::new();

    // Boring: 10 instances of greet->greet_back (very common)
    for i in 0..10 {
        let t = i * 10i64;
        let src = format!("person_{}", i);
        let tgt = format!("person_{}", (i + 5) % 10);
        edges.push(edge(&src, "greet", &tgt, t, t + 2));
        edges.push(edge(&tgt, "greet_back", &src, t + 3, t + 5));
    }

    // Interesting: 2 instances of conspire->attack (rare, surprising)
    edges.push(edge("villain_a", "conspire", "target_x", 200, 205));
    edges.push(edge("villain_a", "attack", "target_x", 210, 215));
    edges.push(edge("villain_b", "conspire", "target_y", 220, 225));
    edges.push(edge("villain_b", "attack", "target_y", 230, 235));

    // Medium: 3 instances of observe->report (moderate)
    edges.push(edge("spy_a", "observe_enemy", "camp_1", 250, 255));
    edges.push(edge("spy_a", "report", "commander", 260, 265));
    edges.push(edge("spy_b", "observe_enemy", "camp_2", 270, 275));
    edges.push(edge("spy_b", "report", "commander", 280, 285));
    edges.push(edge("spy_c", "observe_enemy", "camp_3", 290, 295));
    edges.push(edge("spy_c", "report", "commander", 300, 305));

    TraceCorpus::new(edges)
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 1: Planted Pattern Recovery
// ═══════════════════════════════════════════════════════════════════════

/// Build a corpus with 3 known planted patterns in noise.
/// Run MINERful discovery. Check if planted patterns are recovered.
/// This is the gold standard validation in process mining.
#[test]
fn exp1_planted_pattern_recovery() {
    println!("\n========================================");
    println!("EXP 1: Planted Pattern Recovery");
    println!("========================================\n");

    let corpus = medieval_court_corpus();
    println!(
        "Corpus: {} edges, {} labels, {} nodes",
        corpus.len(),
        corpus.labels().len(),
        corpus.nodes().len()
    );
    println!("Labels: {:?}\n", {
        let mut l: Vec<_> = corpus.labels().into_iter().collect();
        l.sort();
        l
    });

    let config = MinerfulConfig {
        min_support: 0.05,
        min_confidence: 0.3,
    };
    let history = discover(&corpus, config, AcceptAll);

    println!("Discovered {} patterns:", history.all_scored.len());
    print_patterns(&history.all_scored);

    // Check planted pattern recovery
    let found_p1 = has_label_pair(&history.all_scored, "befriend", "betray");
    let found_p2 = has_label_pair(&history.all_scored, "arrive", "depart");
    let found_p3 = has_label_pair(&history.all_scored, "accuse", "exile");

    println!("\nPlanted pattern recovery:");
    println!(
        "  P1 befriend->betray: {}",
        if found_p1 { "RECOVERED" } else { "MISSED" }
    );
    println!(
        "  P2 arrive->depart:   {}",
        if found_p2 { "RECOVERED" } else { "MISSED" }
    );
    println!(
        "  P3 accuse->exile:    {}",
        if found_p3 { "RECOVERED" } else { "MISSED" }
    );

    let recovered = [found_p1, found_p2, found_p3]
        .iter()
        .filter(|&&x| x)
        .count();
    println!("  Recovery rate: {}/3", recovered);

    // Count spurious patterns (involving noise labels)
    let planted_labels = ["befriend", "betray", "arrive", "depart", "accuse", "exile"];
    let spurious: Vec<_> = history
        .all_scored
        .iter()
        .filter(|sp| {
            let (l1, l2) = label_pair(sp);
            !planted_labels.contains(&l1.as_str()) || !planted_labels.contains(&l2.as_str())
        })
        .collect();

    println!(
        "\nSpurious patterns: {}/{}",
        spurious.len(),
        history.all_scored.len()
    );
    for sp in &spurious {
        let (l1, l2) = label_pair(sp);
        println!("  SPURIOUS: {} -> {}", l1, l2);
    }

    // Also report: planted patterns that appear in both directions
    let p1_fwd = has_label_pair(&history.all_scored, "befriend", "betray");
    let p1_rev = history.all_scored.iter().any(|sp| {
        let (l1, l2) = label_pair(sp);
        l1 == "betray" && l2 == "befriend"
    });
    if p1_fwd && p1_rev {
        println!("\nNote: befriend->betray found in BOTH directions (Before and After)");
    }

    assert!(
        recovered >= 2,
        "Should recover at least 2 of 3 planted patterns, got {}/3",
        recovered
    );
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 2: Holdout Generalization
// ═══════════════════════════════════════════════════════════════════════

/// Discover patterns on the first temporal half, evaluate on the second.
/// Patterns that only exist in training are overfit; patterns that
/// generalize to test data represent real structural regularities.
#[test]
fn exp2_holdout_generalization() {
    println!("\n========================================");
    println!("EXP 2: Holdout Generalization");
    println!("========================================\n");

    let corpus = temporal_corpus();
    let (train, test) = corpus.split_at(&100);

    println!("Full corpus:    {} edges", corpus.len());
    println!("Training (t<100): {} edges", train.len());
    println!("Test (t>=100):    {} edges", test.len());
    println!("Train labels: {:?}", {
        let mut l: Vec<_> = train.labels().into_iter().collect();
        l.sort();
        l
    });
    println!("Test labels:  {:?}", {
        let mut l: Vec<_> = test.labels().into_iter().collect();
        l.sort();
        l
    });

    let config = MinerfulConfig {
        min_support: 0.1,
        min_confidence: 0.3,
    };
    let history = discover(&train, config, AcceptAll);

    println!("\nDiscovered from training ({}):", history.all_scored.len());
    print_patterns(&history.all_scored);

    // Evaluate discovered patterns against test set
    let test_graph = corpus_to_open_memgraph(&test);

    println!("\nGeneralization to test set:");
    let mut generalized = 0;
    let mut total_test_matches = 0;
    for sp in &history.all_scored {
        let mem_pattern = sp
            .pattern
            .map_types(|l| l.clone(), |v| MemValue::Node(v.clone()));
        let matches = evaluate_pattern(&test_graph, &mem_pattern);
        let status = if matches.is_empty() {
            "NO MATCHES"
        } else {
            generalized += 1;
            "GENERALIZES"
        };
        total_test_matches += matches.len();
        println!(
            "  {} -> {} test matches ({})",
            sp.pattern.name,
            matches.len(),
            status
        );
    }

    // Also check training matches for comparison
    let train_graph = corpus_to_open_memgraph(&train);
    let mut total_train_matches = 0;
    for sp in &history.all_scored {
        let mem_pattern = sp
            .pattern
            .map_types(|l| l.clone(), |v| MemValue::Node(v.clone()));
        let matches = evaluate_pattern(&train_graph, &mem_pattern);
        total_train_matches += matches.len();
    }

    println!(
        "\nSummary: {}/{} patterns generalize",
        generalized,
        history.all_scored.len()
    );
    println!(
        "  Train matches: {}, Test matches: {}",
        total_train_matches, total_test_matches
    );
    if total_train_matches > 0 {
        let ratio = total_test_matches as f64 / total_train_matches as f64;
        println!("  Test/Train ratio: {:.2}", ratio);
        println!(
            "  {}",
            if ratio > 0.5 {
                "Good generalization"
            } else if ratio > 0.1 {
                "Moderate generalization"
            } else {
                "Poor generalization"
            }
        );
    }

    assert!(
        generalized > 0,
        "At least one pattern should generalize to test set"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 3: Noise Tolerance
// ═══════════════════════════════════════════════════════════════════════

/// Same planted patterns with increasing noise.
/// At what noise level do planted patterns disappear?
#[test]
fn exp3_noise_tolerance() {
    println!("\n========================================");
    println!("EXP 3: Noise Tolerance");
    println!("========================================\n");

    // Fixed planted pattern: befriend->betray (3 instances, shared source)
    let planted = vec![
        edge("alice", "befriend", "bob_p", 1, 3),
        edge("alice", "betray", "charlie_p", 5, 7),
        edge("dave", "befriend", "eve_p", 10, 12),
        edge("dave", "betray", "frank_p", 15, 17),
        edge("gwen", "befriend", "hank_p", 20, 22),
        edge("gwen", "betray", "iris_p", 25, 27),
    ];

    println!("Planted: 6 edges (3x befriend->betray, shared source)");
    println!(
        "\n{:<8} {:>6} {:>12} {:>10} {:>10} {:>8}",
        "Noise", "Total", "Discovered", "Found?", "Spurious", "Ratio"
    );
    println!("{}", "-".repeat(62));

    for noise_count in [0, 6, 12, 24, 48, 96, 192, 384] {
        let mut edges = planted.clone();
        // Generate diverse noise with unique node names to avoid sharing
        for i in 0..noise_count {
            let t = 100 + (i as i64) * 3;
            let src = format!("npc_{}", i);
            let label = match i % 8 {
                0 => "observe",
                1 => "speak",
                2 => "feast",
                3 => "wander",
                4 => "pray",
                5 => "trade",
                6 => "craft",
                _ => "idle",
            };
            let tgt = format!("dest_{}", i);
            edges.push(edge(&src, label, &tgt, t, t + 2));
        }

        let corpus = TraceCorpus::new(edges);
        let config = MinerfulConfig {
            min_support: 0.02, // Very low to see what emerges at high noise
            min_confidence: 0.2,
        };
        let history = discover(&corpus, config, AcceptAll);

        let found = has_label_pair(&history.all_scored, "befriend", "betray");
        let spurious = history
            .all_scored
            .iter()
            .filter(|sp| {
                let (l1, l2) = label_pair(sp);
                !(["befriend", "betray"].contains(&l1.as_str())
                    && ["befriend", "betray"].contains(&l2.as_str()))
            })
            .count();

        let signal_ratio = if history.all_scored.is_empty() {
            0.0
        } else {
            let planted_count = history.all_scored.len() - spurious;
            planted_count as f64 / history.all_scored.len() as f64
        };

        println!(
            "{:<8} {:>6} {:>12} {:>10} {:>10} {:>7.1}%",
            noise_count,
            corpus.len(),
            history.all_scored.len(),
            if found { "YES" } else { "NO" },
            spurious,
            signal_ratio * 100.0
        );
    }

    println!("\nNote: 'Ratio' = planted patterns / total discovered");
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 4: Evaluator Calibration
// ═══════════════════════════════════════════════════════════════════════

/// Build a corpus with boring (common) and interesting (rare) patterns.
/// Check if the evaluators rank them correctly.
#[test]
fn exp4_evaluator_calibration() {
    println!("\n========================================");
    println!("EXP 4: Evaluator Calibration");
    println!("========================================\n");

    let corpus = calibration_corpus();
    println!("Corpus: {} edges", corpus.len());
    println!("  greet/greet_back:     20 edges (10 instances, COMMON)");
    println!("  conspire/attack:       4 edges (2 instances, RARE)");
    println!("  observe_enemy/report:  6 edges (3 instances, MEDIUM)\n");

    let config = MinerfulConfig {
        min_support: 0.02,
        min_confidence: 0.2,
    };
    let history = discover(&corpus, config, AcceptAll);

    println!("All discovered ({}):", history.all_scored.len());
    print_patterns(&history.all_scored);

    // Extract scores for specific patterns
    println!("\nEvaluator comparison:");
    println!(
        "  {:<30} {:>10} {:>10} {:>10}",
        "Pattern", "Surprise", "MatchQual", "Composite"
    );
    println!("  {}", "-".repeat(65));

    let categories = [
        ("greet", "greet_back", "COMMON"),
        ("conspire", "attack", "RARE"),
        ("observe_enemy", "report", "MEDIUM"),
    ];

    let mut surprise_scores: HashMap<&str, f64> = HashMap::new();

    for (l1, l2, label) in &categories {
        // Find patterns matching this label pair (either direction)
        let matching: Vec<_> = history
            .all_scored
            .iter()
            .filter(|sp| {
                let (a, b) = label_pair(sp);
                (a == *l1 && b == *l2) || (a == *l2 && b == *l1)
            })
            .collect();

        if matching.is_empty() {
            println!(
                "  {:<30} NOT DISCOVERED",
                format!("{}->{} ({})", l1, l2, label)
            );
        } else {
            for sp in &matching {
                let su = sp.score.scores.get("surprise").copied().unwrap_or(0.0);
                let mq = sp.score.scores.get("match_quality").copied().unwrap_or(0.0);
                let comp = sp.score.composite(&HashMap::new());
                let (a, b) = label_pair(sp);
                println!(
                    "  {:<30} {:>10.3} {:>10.3} {:>10.3}",
                    format!("{}->{} ({})", a, b, label),
                    su,
                    mq,
                    comp,
                );
                surprise_scores.insert(label, su);
            }
        }
    }

    // Calibration checks
    println!("\nCalibration results:");
    let rare_su = surprise_scores.get("RARE").copied();
    let common_su = surprise_scores.get("COMMON").copied();
    let medium_su = surprise_scores.get("MEDIUM").copied();

    if let (Some(r), Some(c)) = (rare_su, common_su) {
        let pass = r > c;
        println!(
            "  Rare > Common surprise: {} (rare={:.3}, common={:.3})",
            if pass { "PASS" } else { "FAIL" },
            r,
            c
        );
    } else {
        println!("  Rare vs Common: CANNOT COMPARE (one not discovered)");
    }

    if let (Some(r), Some(m)) = (rare_su, medium_su) {
        let pass = r > m;
        println!(
            "  Rare > Medium surprise: {} (rare={:.3}, medium={:.3})",
            if pass { "PASS" } else { "FAIL" },
            r,
            m
        );
    }

    if let (Some(m), Some(c)) = (medium_su, common_su) {
        let pass = m > c;
        println!(
            "  Medium > Common surprise: {} (medium={:.3}, common={:.3})",
            if pass { "PASS" } else { "FAIL" },
            m,
            c
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 5: Round-Trip Semantic Fidelity
// ═══════════════════════════════════════════════════════════════════════

/// Discover -> emit DSL -> parse back -> evaluate -> compare match counts.
/// Tests whether the full round-trip preserves matching semantics.
#[test]
fn exp5_roundtrip_semantic_fidelity() {
    println!("\n========================================");
    println!("EXP 5: Round-Trip Semantic Fidelity");
    println!("========================================\n");

    let corpus = medieval_court_corpus();
    let config = MinerfulConfig {
        min_support: 0.05,
        min_confidence: 0.3,
    };
    let history = discover(&corpus, config, AcceptPositive);

    // Use open-ended intervals for evaluation (ensures all edges visible)
    let graph = corpus_to_open_memgraph(&corpus);

    println!(
        "Testing {} patterns through: discover -> emit -> parse -> evaluate\n",
        history.all_scored.len()
    );

    let mut perfect = 0;
    let mut mismatched = 0;
    let mut parse_failed = 0;

    for sp in &history.all_scored {
        // Original: convert to MemValue and evaluate
        let orig_mem = sp
            .pattern
            .map_types(|l| l.clone(), |v| MemValue::Node(v.clone()));
        let orig_matches = evaluate_pattern(&graph, &orig_mem);

        // Round-trip: emit DSL, parse back
        let dsl = pattern_to_dsl(&sp.pattern);
        let parsed = match fabula_dsl::parse_document(&dsl) {
            Ok(doc) => doc,
            Err(e) => {
                println!("  {} PARSE FAILED: {}", sp.pattern.name, e);
                println!("    DSL:\n{}", indent_text(&dsl, "      "));
                parse_failed += 1;
                continue;
            }
        };

        if parsed.patterns.is_empty() {
            println!("  {} NO PATTERNS IN PARSED DOCUMENT", sp.pattern.name);
            parse_failed += 1;
            continue;
        }

        // Evaluate the round-tripped pattern
        let parsed_pattern = &parsed.patterns[0];
        let parsed_matches = evaluate_pattern(&graph, parsed_pattern);

        let (l1, l2) = label_pair(sp);
        if orig_matches.len() == parsed_matches.len() {
            println!(
                "  {} ({}->{}) matches: {} == {} OK",
                sp.pattern.name,
                l1,
                l2,
                orig_matches.len(),
                parsed_matches.len()
            );
            perfect += 1;
        } else {
            println!(
                "  {} ({}->{}) matches: {} != {} MISMATCH",
                sp.pattern.name,
                l1,
                l2,
                orig_matches.len(),
                parsed_matches.len()
            );
            println!("    DSL:\n{}", indent_text(&dsl, "      "));
            mismatched += 1;
        }
    }

    println!(
        "\nSummary: {} perfect, {} mismatched, {} parse failures (of {})",
        perfect,
        mismatched,
        parse_failed,
        history.all_scored.len()
    );

    let fidelity = if history.all_scored.is_empty() {
        0.0
    } else {
        perfect as f64 / history.all_scored.len() as f64 * 100.0
    };
    println!("Round-trip fidelity: {:.0}%", fidelity);

    assert!(
        parse_failed == 0,
        "All patterns should parse after round-trip"
    );
}

fn indent_text(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|l| format!("{}{}", prefix, l))
        .collect::<Vec<_>>()
        .join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 6: Threshold Sensitivity Sweep
// ═══════════════════════════════════════════════════════════════════════

/// Sweep min_support and observe how discovery results change.
/// The "elbow" in the curve indicates a natural operating point.
#[test]
fn exp6_threshold_sweep() {
    println!("\n========================================");
    println!("EXP 6: Threshold Sensitivity Sweep");
    println!("========================================\n");

    let corpus = medieval_court_corpus();
    println!("Corpus: {} edges\n", corpus.len());

    println!(
        "{:<12} {:>10} {:>10} {:>6} {:>6} {:>6}",
        "min_support", "Discovered", "w/Score>0", "P1?", "P2?", "P3?"
    );
    println!("{}", "-".repeat(58));

    for &support in &[0.01, 0.03, 0.05, 0.08, 0.10, 0.12, 0.15, 0.20, 0.30, 0.50] {
        let config = MinerfulConfig {
            min_support: support,
            min_confidence: 0.3,
        };
        let history = discover(&corpus, config, AcceptAll);

        let with_score: Vec<_> = history
            .all_scored
            .iter()
            .filter(|sp| sp.score.composite(&HashMap::new()) > 0.0)
            .collect();

        let p1 = has_label_pair(&history.all_scored, "befriend", "betray");
        let p2 = has_label_pair(&history.all_scored, "arrive", "depart");
        let p3 = has_label_pair(&history.all_scored, "accuse", "exile");

        println!(
            "{:<12.2} {:>10} {:>10} {:>6} {:>6} {:>6}",
            support,
            history.all_scored.len(),
            with_score.len(),
            if p1 { "Y" } else { "-" },
            if p2 { "Y" } else { "-" },
            if p3 { "Y" } else { "-" },
        );
    }

    println!("\nP1=befriend->betray, P2=arrive->depart, P3=accuse->exile");

    // Also sweep min_confidence with fixed support
    println!("\n--- Confidence sweep (min_support=0.05) ---\n");
    println!(
        "{:<15} {:>10} {:>10}",
        "min_confidence", "Discovered", "w/Score>0"
    );
    println!("{}", "-".repeat(40));

    for &conf in &[0.1, 0.2, 0.3, 0.5, 0.7, 0.9] {
        let config = MinerfulConfig {
            min_support: 0.05,
            min_confidence: conf,
        };
        let history = discover(&corpus, config, AcceptAll);
        let with_score: Vec<_> = history
            .all_scored
            .iter()
            .filter(|sp| sp.score.composite(&HashMap::new()) > 0.0)
            .collect();
        println!(
            "{:<15.1} {:>10} {:>10}",
            conf,
            history.all_scored.len(),
            with_score.len()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EXPERIMENT 7: Narrative Arc Discovery
// ═══════════════════════════════════════════════════════════════════════

/// Build a corpus from a Shakespearean-style tragedy with a clear 3-act structure.
/// Can MINERful discover the narrative arc without being told?
///
/// Act 1 (Setup): Characters meet, form alliances, express trust
/// Act 2 (Complication): Jealousy, suspicion, manipulation
/// Act 3 (Resolution): Betrayal, confrontation, death
///
/// The interesting question: does discovery find the act-spanning patterns
/// (trust->betray, ally->duel) that represent character arcs?
#[test]
fn exp7_narrative_arc_discovery() {
    println!("\n========================================");
    println!("EXP 7: Narrative Arc Discovery");
    println!("========================================\n");

    let corpus = TraceCorpus::new(vec![
        // === ACT 1: Setup (t=0..30) ===
        // Romeo and Juliet meet, Tybalt is hostile
        edge("romeo", "attend", "capulet_ball", 0, 3),
        edge("juliet", "attend", "capulet_ball", 1, 4),
        edge("romeo", "love", "juliet", 5, 8),
        edge("juliet", "love", "romeo", 5, 8),
        edge("mercutio", "ally", "romeo", 2, 5),
        edge("benvolio", "ally", "romeo", 2, 5),
        edge("tybalt", "hate", "romeo", 3, 6),
        edge("nurse", "trust", "juliet", 0, 10),
        edge("friar", "trust", "romeo", 4, 12),
        edge("friar", "trust", "juliet", 4, 12),
        // === ACT 2: Complication (t=30..60) ===
        // Tybalt provokes, Romeo tries peace, things escalate
        edge("tybalt", "challenge", "romeo", 30, 33),
        edge("romeo", "refuse", "tybalt", 33, 35),
        edge("tybalt", "attack", "mercutio", 35, 38),
        edge("mercutio", "die", "street", 38, 40),
        edge("romeo", "rage", "tybalt", 40, 42),
        edge("romeo", "kill", "tybalt", 42, 45),
        edge("prince", "banish", "romeo", 46, 50),
        // === ACT 3: Resolution (t=60..90) ===
        // The plan fails, everyone dies
        edge("friar", "plan", "juliet", 60, 63),
        edge("juliet", "fake_death", "tomb", 65, 70),
        edge("romeo", "believe_dead", "juliet", 72, 75),
        edge("romeo", "poison", "romeo", 76, 78),
        edge("romeo", "die", "tomb", 78, 80),
        edge("juliet", "discover", "romeo", 81, 83),
        edge("juliet", "die", "tomb", 84, 86),
        edge("prince", "mourn", "families", 87, 90),
        // Background: servant chatter (noise)
        edge("servant_a", "gossip", "servant_b", 10, 12),
        edge("servant_b", "gossip", "servant_c", 25, 27),
        edge("servant_a", "gossip", "servant_c", 55, 57),
        edge("guard", "patrol", "gate", 15, 18),
        edge("guard", "patrol", "gate", 45, 48),
        edge("guard", "patrol", "gate", 75, 78),
    ]);

    println!("Corpus: {} edges", corpus.len());
    println!("  Act 1 (Setup):        10 edges — love, trust, alliance");
    println!("  Act 2 (Complication):  7 edges — challenge, attack, kill, banish");
    println!("  Act 3 (Resolution):    8 edges — plan, fake_death, poison, die");
    println!("  Noise:                 6 edges — gossip, patrol\n");

    let config = MinerfulConfig {
        min_support: 0.03,
        min_confidence: 0.2,
    };
    let history = discover(&corpus, config, AcceptAll);

    // Sort by composite score (highest first)
    let mut sorted: Vec<_> = history.all_scored.iter().collect();
    sorted.sort_by(|a, b| {
        let ca = b.score.composite(&HashMap::new());
        let cb = a.score.composite(&HashMap::new());
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("Top 15 discovered patterns (by composite score):");
    for (i, sp) in sorted.iter().take(15).enumerate() {
        let (l1, l2) = label_pair(sp);
        let mq = sp.score.scores.get("match_quality").copied().unwrap_or(0.0);
        let su = sp.score.scores.get("surprise").copied().unwrap_or(0.0);
        let composite = sp.score.composite(&HashMap::new());
        println!(
            "  {:2}. {:<25} MQ={:6.2} SU={:6.2} C={:6.2}",
            i + 1,
            format!("{} -> {}", l1, l2),
            mq,
            su,
            composite
        );
    }

    println!("\nTotal discovered: {}", history.all_scored.len());

    // Check for key narrative arcs
    let arcs = [
        ("love", "die", "Star-crossed lovers"),
        ("trust", "die", "Betrayed trust"),
        ("ally", "kill", "Ally becomes killer"),
        ("hate", "attack", "Hatred leads to violence"),
        ("love", "kill", "Love and death"),
        ("trust", "plan", "Trust enables scheming"),
        ("challenge", "kill", "Challenge escalates to death"),
        ("attend", "die", "Meeting leads to doom"),
        ("ally", "die", "Alliance ends in death"),
        ("banish", "die", "Banishment leads to death"),
    ];

    println!("\nNarrative arc recovery:");
    let mut found_arcs = 0;
    for (l1, l2, desc) in &arcs {
        let found = has_label_pair(&sorted.iter().copied().cloned().collect::<Vec<_>>(), l1, l2);
        if found {
            // Find the score
            let sp = sorted.iter().find(|sp| {
                let (a, b) = label_pair(sp);
                (a == *l1 && b == *l2) || (a == *l2 && b == *l1)
            });
            let composite = sp
                .map(|s| s.score.composite(&HashMap::new()))
                .unwrap_or(0.0);
            println!("  {:<35} FOUND (C={:.2})", desc, composite);
            found_arcs += 1;
        } else {
            println!("  {:<35} not found", desc);
        }
    }
    println!("\nNarrative arcs found: {}/{}", found_arcs, arcs.len());

    // Emit the top 3 patterns as DSL for human inspection
    println!("\n--- Top 3 patterns as fabula DSL ---\n");
    for sp in sorted.iter().take(3) {
        let dsl = pattern_to_dsl(&sp.pattern);
        println!("{}", dsl);
    }
}
