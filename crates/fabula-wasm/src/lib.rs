//! WebAssembly bindings for fabula pattern matching engine.
//!
//! Exposes the DSL parser, batch/incremental evaluation, gap analysis,
//! and Allen interval relation computation to JavaScript.

use wasm_bindgen::prelude::*;

use fabula::engine::{MatchState, SiftEngine, StageStatus};
use fabula::interval::Interval;
use fabula_dsl::ast::GraphAst;
use fabula_memory::{MemGraph, MemValue};
use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// JSON response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OkResult<T: Serialize> {
    ok: bool,
    #[serde(flatten)]
    data: T,
}

#[derive(Serialize)]
struct ErrResult {
    ok: bool,
    error: ErrorInfo,
}

#[derive(Serialize)]
struct ErrorInfo {
    line: usize,
    column: usize,
    span: (usize, usize),
    message: String,
}

fn ok_json<T: Serialize>(data: T) -> JsValue {
    let result = OkResult { ok: true, data };
    JsValue::from_str(&serde_json::to_string(&result).unwrap())
}

fn err_json(e: fabula_dsl::error::ParseError) -> JsValue {
    let result = ErrResult {
        ok: false,
        error: ErrorInfo {
            line: e.line,
            column: e.column,
            span: e.span,
            message: e.message,
        },
    };
    JsValue::from_str(&serde_json::to_string(&result).unwrap())
}

// ---------------------------------------------------------------------------
// Pattern validation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PatternInfo {
    pattern_name: String,
    stage_count: usize,
    negation_count: usize,
    vars: Vec<String>,
}

/// Parse and validate a pattern DSL string.
/// Returns JSON: `{ ok: true, pattern_name, stage_count, negation_count, vars }` or `{ ok: false, error }`.
#[wasm_bindgen]
pub fn parse_and_validate_pattern(dsl: &str) -> JsValue {
    match fabula_dsl::parse_pattern(dsl) {
        Ok(pattern) => {
            let vars: Vec<String> = pattern.all_vars().into_iter().map(|v| v.0.clone()).collect();
            ok_json(PatternInfo {
                pattern_name: pattern.name.clone(),
                stage_count: pattern.stages.len(),
                negation_count: pattern.negations.len(),
                vars,
            })
        }
        Err(e) => err_json(e),
    }
}

// ---------------------------------------------------------------------------
// Graph validation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct GraphInfo {
    edge_count: usize,
    nodes: Vec<String>,
}

/// Parse and validate a graph DSL string.
/// Returns JSON: `{ ok: true, edge_count, nodes }` or `{ ok: false, error }`.
#[wasm_bindgen]
pub fn parse_and_validate_graph(dsl: &str) -> JsValue {
    match parse_graph_with_ast(dsl) {
        Ok((graph, ast)) => {
            let mut nodes: Vec<String> = ast.edges.iter().map(|e| e.source.clone()).collect();
            nodes.sort();
            nodes.dedup();
            ok_json(GraphInfo {
                edge_count: graph.edge_count(),
                nodes,
            })
        }
        Err(e) => err_json(e),
    }
}

fn parse_graph_with_ast(
    dsl: &str,
) -> Result<(MemGraph, GraphAst), fabula_dsl::error::ParseError> {
    let tokens = fabula_dsl::lexer::Lexer::new(dsl).tokenize()?;
    let ast = fabula_dsl::parser::Parser::new(tokens).parse_graph_only()?;
    let graph = fabula_dsl::compiler::compile_graph(&ast);
    Ok((graph, ast))
}

// ---------------------------------------------------------------------------
// Batch evaluation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct BatchResult {
    matches: Vec<MatchJson>,
}

#[derive(Serialize)]
struct MatchJson {
    pattern: String,
    bindings: HashMap<String, String>,
}

/// Evaluate a pattern against a graph in batch mode.
/// Returns JSON: `{ ok: true, matches: [...] }` or `{ ok: false, error }`.
#[wasm_bindgen]
pub fn evaluate_batch(pattern_dsl: &str, graph_dsl: &str) -> JsValue {
    let pattern = match fabula_dsl::parse_pattern(pattern_dsl) {
        Ok(p) => p,
        Err(e) => return err_json(e),
    };
    let graph = match fabula_dsl::parse_graph(graph_dsl) {
        Ok(g) => g,
        Err(e) => return err_json(e),
    };

    let mut engine = SiftEngine::new();
    engine.register(pattern);
    let matches = engine.evaluate(&graph);

    let matches_json: Vec<MatchJson> = matches
        .iter()
        .map(|m| MatchJson {
            pattern: m.pattern.clone(),
            bindings: m
                .bindings
                .iter()
                .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                .collect(),
        })
        .collect();

    ok_json(BatchResult { matches: matches_json })
}

// ---------------------------------------------------------------------------
// Incremental evaluation (step-by-step replay)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct IncrementalResult {
    steps: Vec<StepJson>,
}

#[derive(Serialize)]
struct StepJson {
    timestamp: i64,
    edges_added: Vec<EdgeAddedJson>,
    events: Vec<EventJson>,
    partial_matches: Vec<PartialMatchJson>,
}

#[derive(Serialize)]
struct EdgeAddedJson {
    source: String,
    label: String,
    target: String,
}

#[derive(Serialize)]
struct EventJson {
    #[serde(rename = "type")]
    event_type: String,
    pattern: String,
    match_id: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bindings: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clause_label: Option<String>,
}

#[derive(Serialize)]
struct PartialMatchJson {
    pattern: String,
    match_id: usize,
    next_stage: usize,
    state: String,
    bindings: HashMap<String, String>,
    created_at: i64,
}

/// Evaluate a pattern incrementally, replaying graph edges by timestamp.
/// Returns JSON: `{ ok: true, steps: [...] }` or `{ ok: false, error }`.
#[wasm_bindgen]
pub fn evaluate_incremental(pattern_dsl: &str, graph_dsl: &str) -> JsValue {
    let pattern = match fabula_dsl::parse_pattern(pattern_dsl) {
        Ok(p) => p,
        Err(e) => return err_json(e),
    };

    // Parse graph AST to get edges grouped by timestamp
    let graph_ast_result = {
        let tokens = match fabula_dsl::lexer::Lexer::new(graph_dsl).tokenize() {
            Ok(t) => t,
            Err(e) => return err_json(e),
        };
        fabula_dsl::parser::Parser::new(tokens).parse_graph_only()
    };
    let graph_ast = match graph_ast_result {
        Ok(ast) => ast,
        Err(e) => return err_json(e),
    };

    // Group edges by timestamp and sort
    let mut time_groups: std::collections::BTreeMap<i64, Vec<&fabula_dsl::ast::EdgeAst>> =
        std::collections::BTreeMap::new();
    for edge in &graph_ast.edges {
        time_groups.entry(edge.time_start).or_default().push(edge);
    }

    let now_time = graph_ast.now.unwrap_or_else(|| {
        graph_ast.edges.iter().map(|e| e.time_start).max().unwrap_or(0) + 1
    });

    let mut graph = MemGraph::new();
    let mut engine = SiftEngine::new();
    let pattern_name = pattern.name.clone();
    engine.register(pattern);

    let mut steps = Vec::new();

    for (timestamp, edges) in &time_groups {
        graph.set_time(now_time);
        let mut edges_added = Vec::new();
        let mut all_events = Vec::new();

        // Collect edge info and add ALL edges to graph first,
        // so secondary clauses are available when the engine evaluates.
        struct EdgeInfo {
            source: String,
            label: String,
            target_val: MemValue,
            target_str: String,
            interval: Interval<i64>,
        }
        let mut edge_infos = Vec::new();

        for edge_ast in edges {
            let (target_val, target_str) = match &edge_ast.target {
                fabula_dsl::ast::EdgeTarget::Str(s) => {
                    (MemValue::Str(s.clone()), format!("\"{}\"", s))
                }
                fabula_dsl::ast::EdgeTarget::Num(n) => {
                    (MemValue::Num(*n), format!("{}", n))
                }
                fabula_dsl::ast::EdgeTarget::Bool(b) => {
                    (MemValue::Bool(*b), format!("{}", b))
                }
                fabula_dsl::ast::EdgeTarget::NodeRef(n) => {
                    (MemValue::Node(n.clone()), format!("@{}", n))
                }
            };

            let interval = if let Some(end) = edge_ast.time_end {
                graph.add_edge_bounded(
                    &edge_ast.source,
                    &edge_ast.label,
                    target_val.clone(),
                    *timestamp,
                    end,
                );
                Interval::new(*timestamp, end)
            } else {
                graph.add_edge(
                    &edge_ast.source,
                    &edge_ast.label,
                    target_val.clone(),
                    *timestamp,
                );
                Interval::open(*timestamp)
            };

            edge_infos.push(EdgeInfo {
                source: edge_ast.source.clone(),
                label: edge_ast.label.clone(),
                target_val,
                target_str,
                interval,
            });
        }

        // Now notify the engine about each edge (graph already has all of them)
        for info in &edge_infos {
            edges_added.push(EdgeAddedJson {
                source: info.source.clone(),
                label: info.label.clone(),
                target: info.target_str.clone(),
            });

            let events = engine.on_edge_added(
                &graph,
                &info.source,
                &info.label,
                &info.target_val,
                &info.interval,
            );

            for event in events {
                all_events.push(convert_event(&event, &pattern_name));
            }
        }

        let partial_matches: Vec<PartialMatchJson> = engine
            .partial_matches()
            .iter()
            .map(|pm| PartialMatchJson {
                pattern: pattern_name.clone(),
                match_id: pm.id,
                next_stage: pm.next_stage,
                state: match pm.state {
                    MatchState::Active => "active".to_string(),
                    MatchState::Complete => "complete".to_string(),
                    MatchState::Dead => "dead".to_string(),
                },
                bindings: pm
                    .bindings
                    .iter()
                    .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                    .collect(),
                created_at: pm.created_at,
            })
            .collect();

        steps.push(StepJson {
            timestamp: *timestamp,
            edges_added,
            events: all_events,
            partial_matches,
        });
    }

    ok_json(IncrementalResult { steps })
}

fn convert_event(
    event: &fabula::engine::SiftEvent<String, MemValue>,
    _pattern_name: &str,
) -> EventJson {
    match event {
        fabula::engine::SiftEvent::Advanced {
            pattern,
            match_id,
            stage_index,
        } => EventJson {
            event_type: "advanced".to_string(),
            pattern: pattern.clone(),
            match_id: *match_id,
            stage_index: Some(*stage_index),
            bindings: None,
            clause_label: None,
        },
        fabula::engine::SiftEvent::Completed {
            pattern,
            match_id,
            bindings,
        } => EventJson {
            event_type: "completed".to_string(),
            pattern: pattern.clone(),
            match_id: *match_id,
            stage_index: None,
            bindings: Some(
                bindings
                    .iter()
                    .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                    .collect(),
            ),
            clause_label: None,
        },
        fabula::engine::SiftEvent::Negated {
            pattern,
            match_id,
            clause_label,
            ..
        } => EventJson {
            event_type: "negated".to_string(),
            pattern: pattern.clone(),
            match_id: *match_id,
            stage_index: None,
            bindings: None,
            clause_label: Some(clause_label.clone()),
        },
    }
}

// ---------------------------------------------------------------------------
// Gap analysis (why_not)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct GapResult {
    analysis: GapAnalysisJson,
}

#[derive(Serialize)]
struct GapAnalysisJson {
    pattern: String,
    stages: Vec<StageAnalysisJson>,
}

#[derive(Serialize)]
struct StageAnalysisJson {
    anchor: String,
    status: String,
    clauses: Vec<ClauseAnalysisJson>,
}

#[derive(Serialize)]
struct ClauseAnalysisJson {
    description: String,
    matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// Run gap analysis (why_not) on a pattern against a graph.
/// Returns JSON: `{ ok: true, analysis: {...} }` or `{ ok: false, error }`.
#[wasm_bindgen]
pub fn why_not(pattern_dsl: &str, graph_dsl: &str) -> JsValue {
    let pattern = match fabula_dsl::parse_pattern(pattern_dsl) {
        Ok(p) => p,
        Err(e) => return err_json(e),
    };
    let graph = match fabula_dsl::parse_graph(graph_dsl) {
        Ok(g) => g,
        Err(e) => return err_json(e),
    };

    let pattern_name = pattern.name.clone();
    let mut engine = SiftEngine::new();
    engine.register(pattern);

    match engine.why_not(&graph, &pattern_name) {
        Some(analysis) => {
            let stages: Vec<StageAnalysisJson> = analysis
                .stages
                .iter()
                .map(|s| StageAnalysisJson {
                    anchor: s.anchor.clone(),
                    status: match &s.status {
                        StageStatus::Matched => "matched".to_string(),
                        StageStatus::PartiallyMatched { matched, total } => {
                            format!("partial ({}/{})", matched, total)
                        }
                        StageStatus::Unmatched => "unmatched".to_string(),
                    },
                    clauses: s
                        .clauses
                        .iter()
                        .map(|c| ClauseAnalysisJson {
                            description: c.description.clone(),
                            matched: c.matched,
                            reason: c.reason.clone(),
                        })
                        .collect(),
                })
                .collect();

            ok_json(GapResult {
                analysis: GapAnalysisJson {
                    pattern: analysis.pattern.clone(),
                    stages,
                },
            })
        }
        None => {
            let e = fabula_dsl::error::ParseError {
                line: 0,
                column: 0,
                span: (0, 0),
                message: format!("pattern '{}' not found", pattern_name),
            };
            err_json(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Allen relation computation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AllenResult {
    relation: Option<String>,
}

/// Compute the Allen interval relation between two bounded intervals.
/// Returns JSON: `{ ok: true, relation: "Before" | ... | null }`.
#[wasm_bindgen]
pub fn allen_relation(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> JsValue {
    let a = Interval::new(a_start as i64, a_end as i64);
    let b = Interval::new(b_start as i64, b_end as i64);
    let relation = a.relation(&b).map(|r| format!("{:?}", r));
    ok_json(AllenResult { relation })
}
