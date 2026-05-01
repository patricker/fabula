use fabula_dsl::parse_document;
use fabula_dsl::parse_pattern;

#[test]
fn parse_pattern_usage() {
    // #region parse_pattern
    let pattern = parse_pattern(
        r#"
        pattern betrayal {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
            }
            stage e2 {
                e2.eventType = "betray"
                e2.actor -> ?char
            }
            unless {
                mid.eventType = "reconcile"
                mid.actor -> ?char
            }
        }
    "#,
    )
    .expect("pattern should parse");

    assert_eq!(pattern.name, "betrayal");
    assert_eq!(pattern.stages.len(), 2);
    assert_eq!(pattern.negations.len(), 1);
    // #endregion
}

#[test]
fn parse_document_usage() {
    // #region parse_document
    let doc = parse_document(
        r#"
        pattern setup {
            stage e1 {
                e1.eventType = "promise"
                e1.actor -> ?char
            }
        }
        pattern payoff {
            stage e2 {
                e2.eventType = "fulfill"
                e2.actor -> ?char
            }
        }
        compose promise_kept = setup >> payoff sharing(char)

        graph {
            @1 e1.eventType = "promise"
            @1 e1.actor -> alice
            @3 e2.eventType = "fulfill"
            @3 e2.actor -> alice
            now = 10
        }
    "#,
    )
    .expect("document should parse");

    assert_eq!(doc.patterns.len(), 3); // setup, payoff, composed
    assert_eq!(doc.graphs.len(), 1);
    // #endregion
}

#[test]
fn dsl_compile_and_evaluate() {
    use fabula::prelude::*;
    use fabula_memory::MemGraph;

    // #region dsl_eval
    let pattern = parse_pattern(
        r#"
        pattern suspicious_login {
            stage e1 {
                e1.type = "login"
                e1.user -> ?user
                e1.location -> ?loc_a
            }
            stage e2 {
                e2.type = "login"
                e2.user -> ?user
                e2.location -> ?loc_b
            }
            unless between e1 e2 {
                mid.type = "logout"
                mid.user -> ?user
            }
        }
    "#,
    )
    .expect("pattern should parse");

    let mut engine: SiftEngineFor<MemGraph> = SiftEngine::new(DefaultLetEvaluator);
    engine.register(pattern);

    let mut graph = MemGraph::new();
    graph.add_str("login1", "type", "login", 1);
    graph.add_ref("login1", "user", "alice", 1);
    graph.add_str("login1", "location", "new_york", 1);
    graph.add_str("login2", "type", "login", 3);
    graph.add_ref("login2", "user", "alice", 3);
    graph.add_str("login2", "location", "tokyo", 3);
    graph.set_time(10);

    let matches = engine.evaluate(&graph);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern, "suspicious_login");
    // #endregion
}

// Verify that embedded DSL examples also parse
#[test]
fn dsl_metadata_example() {
    // #region dsl_metadata
    let pattern = parse_pattern(
        r#"
        pattern betrayal {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
            }
            meta("thread_type", "conflict")
            meta("priority", "high")
        }
    "#,
    )
    .expect("pattern with metadata should parse");

    assert_eq!(pattern.metadata.get("thread_type").unwrap(), "conflict");
    assert_eq!(pattern.metadata.get("priority").unwrap(), "high");
    // #endregion
}

#[test]
fn dsl_deadline_example() {
    // #region dsl_deadline
    let pattern = parse_pattern(
        r#"
        pattern time_sensitive {
            stage e1 { e1.type = "offer" }
            stage e2 { e2.type = "accept" }
            deadline 10
        }
    "#,
    )
    .expect("pattern with deadline should parse");

    assert_eq!(pattern.deadline_ticks, Some(10));
    // #endregion
}

#[test]
fn dsl_compose_sequence() {
    // #region dsl_compose_sequence
    let doc = parse_document(
        r#"
        pattern setup {
            stage e1 { e1.eventType = "promise"  e1.actor -> ?char }
        }
        pattern payoff {
            stage e2 { e2.eventType = "fulfill"  e2.actor -> ?char }
        }

        compose promise_kept = setup >> payoff sharing(char)
    "#,
    )
    .expect("compose should parse");

    // The composed pattern "promise_kept" is the third pattern
    let composed = &doc.patterns[2];
    assert_eq!(composed.name, "promise_kept");
    assert_eq!(composed.stages.len(), 2);
    // #endregion
}

#[test]
fn dsl_compose_choice() {
    // #region dsl_compose_choice
    let doc = parse_document(
        r#"
        pattern war { stage e1 { e1.type = "war" } }
        pattern famine { stage e1 { e1.type = "famine" } }
        pattern plague { stage e1 { e1.type = "plague" } }

        compose crisis = war | famine | plague
    "#,
    )
    .expect("choice compose should parse");

    // Choice creates 3 separate patterns with a shared group
    let crisis_patterns: Vec<_> = doc.patterns.iter().filter(|p| p.group.is_some()).collect();
    assert_eq!(crisis_patterns.len(), 3);
    // #endregion
}

#[test]
fn dsl_compose_repeat() {
    // #region dsl_compose_repeat
    let doc = parse_document(
        r#"
        pattern offense {
            stage e1 {
                e1.type = "offense"
                e1.offender -> ?offender
            }
        }

        compose three_strikes = offense * 3 sharing(offender)
    "#,
    )
    .expect("repeat compose should parse");

    let composed = doc.patterns.iter().find(|p| p.name == "three_strikes");
    assert!(composed.is_some());
    // Exact repeat unrolls to 3 copies = 3 stages
    assert_eq!(composed.unwrap().stages.len(), 3);
    // #endregion
}

#[test]
fn dsl_concurrent_group() {
    // #region dsl_concurrent
    let pattern = parse_pattern(
        r#"
        pattern multi_signal_alert {
            stage e1 {
                e1.type = "anomaly_detected"
                e1.sensor -> ?sensor
            }
            concurrent {
                stage e2 {
                    e2.type = "temperature_spike"
                    e2.sensor -> ?sensor
                }
                stage e3 {
                    e3.type = "pressure_drop"
                    e3.sensor -> ?sensor
                }
            }
            stage e4 {
                e4.type = "shutdown"
                e4.sensor -> ?sensor
            }
        }
    "#,
    )
    .expect("concurrent pattern should parse");

    assert_eq!(pattern.stages.len(), 4);
    assert_eq!(pattern.unordered_groups.len(), 1);
    // #endregion
}
