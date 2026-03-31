//! Compiler: AST → fabula Pattern and MemGraph.

use crate::ast::*;
use crate::error::ParseError;
use fabula::builder::{NegationBuilder, PatternBuilder, StageBuilder};
use fabula::datasource::ValueConstraint;
use fabula::interval::AllenRelation;
use fabula::pattern::Pattern;
use fabula_memory::{MemGraph, MemValue};

/// Compile a pattern AST into a fabula `Pattern<String, MemValue>`.
pub fn compile_pattern(ast: &PatternAst) -> Result<Pattern<String, MemValue>, ParseError> {
    let mut builder = PatternBuilder::<String, MemValue>::new(&ast.name);

    for stage in &ast.stages {
        let anchor = stage.anchor.clone();
        let clauses = stage.clauses.clone();
        builder = builder.stage(&anchor, |s| build_stage(s, &clauses));
    }

    for neg in &ast.negations {
        let clauses = neg.clauses.clone();
        builder = match &neg.kind {
            NegationKind::Between(start, end) => {
                builder.unless_between(start, end, |n| build_negation(n, &clauses))
            }
            NegationKind::After(start) => {
                builder.unless_after(start, |n| build_negation(n, &clauses))
            }
            NegationKind::Global => {
                builder.unless_global(|n| build_negation(n, &clauses))
            }
        };
    }

    for temp in &ast.temporals {
        let relation = parse_allen_relation(&temp.relation).map_err(|msg| ParseError {
            line: 0,
            column: 0,
            span: (0, 0),
            message: msg,
        })?;
        builder = builder.temporal(&temp.left, relation, &temp.right);
    }

    Ok(builder.build())
}

fn build_stage(mut s: StageBuilder<String, MemValue>, clauses: &[ClauseAst]) -> StageBuilder<String, MemValue> {
    for clause in clauses {
        s = add_clause_to_stage(s, clause);
    }
    s
}

fn add_clause_to_stage(s: StageBuilder<String, MemValue>, clause: &ClauseAst) -> StageBuilder<String, MemValue> {
    let source = &clause.source;
    let label = clause.label.clone();

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            if clause.negated {
                s.not_edge(source, label, MemValue::Str(val.clone()))
            } else {
                s.edge(source, label, MemValue::Str(val.clone()))
            }
        }
        ClauseTarget::LiteralNum(val) => {
            if clause.negated {
                s.not_edge(source, label, MemValue::Num(*val))
            } else {
                s.edge(source, label, MemValue::Num(*val))
            }
        }
        ClauseTarget::LiteralBool(val) => {
            if clause.negated {
                s.not_edge(source, label, MemValue::Bool(*val))
            } else {
                s.edge(source, label, MemValue::Bool(*val))
            }
        }
        ClauseTarget::Bind(var) => {
            s.edge_bind(source, label, var)
        }
        ClauseTarget::NodeRef(node) => {
            if clause.negated {
                s.not_edge(source, label, MemValue::Node(node.clone()))
            } else {
                s.edge(source, label, MemValue::Node(node.clone()))
            }
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint(*op, val);
            s.edge_constrained(source, label, constraint)
        }
    }
}

fn build_negation(mut n: NegationBuilder<String, MemValue>, clauses: &[ClauseAst]) -> NegationBuilder<String, MemValue> {
    for clause in clauses {
        n = add_clause_to_negation(n, clause);
    }
    n
}

fn add_clause_to_negation(n: NegationBuilder<String, MemValue>, clause: &ClauseAst) -> NegationBuilder<String, MemValue> {
    let source = &clause.source;
    let label = clause.label.clone();

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            n.edge(source, label, MemValue::Str(val.clone()))
        }
        ClauseTarget::LiteralNum(val) => {
            n.edge(source, label, MemValue::Num(*val))
        }
        ClauseTarget::LiteralBool(val) => {
            n.edge(source, label, MemValue::Bool(*val))
        }
        ClauseTarget::Bind(var) => {
            n.edge_bind(source, label, var)
        }
        ClauseTarget::NodeRef(node) => {
            n.edge(source, label, MemValue::Node(node.clone()))
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint(*op, val);
            n.edge_constrained(source, label, constraint)
        }
    }
}

fn make_constraint(op: ConstraintOp, val: &ConstraintValue) -> ValueConstraint<MemValue> {
    let mem_val = match val {
        ConstraintValue::Num(n) => MemValue::Num(*n),
        ConstraintValue::Str(s) => MemValue::Str(s.clone()),
    };
    match op {
        ConstraintOp::Lt => ValueConstraint::Lt(mem_val),
        ConstraintOp::Gt => ValueConstraint::Gt(mem_val),
        ConstraintOp::Lte => ValueConstraint::Lte(mem_val),
        ConstraintOp::Gte => ValueConstraint::Gte(mem_val),
    }
}

fn parse_allen_relation(s: &str) -> Result<AllenRelation, String> {
    match s {
        "before" => Ok(AllenRelation::Before),
        "after" => Ok(AllenRelation::After),
        "meets" => Ok(AllenRelation::Meets),
        "met_by" => Ok(AllenRelation::MetBy),
        "overlaps" => Ok(AllenRelation::Overlaps),
        "overlapped_by" => Ok(AllenRelation::OverlappedBy),
        "during" => Ok(AllenRelation::During),
        "contains" => Ok(AllenRelation::Contains),
        "starts" => Ok(AllenRelation::Starts),
        "started_by" => Ok(AllenRelation::StartedBy),
        "finishes" => Ok(AllenRelation::Finishes),
        "finished_by" => Ok(AllenRelation::FinishedBy),
        "equals" => Ok(AllenRelation::Equals),
        _ => Err(format!("unknown Allen relation '{}'. Expected one of: before, after, meets, met_by, overlaps, overlapped_by, during, contains, starts, started_by, finishes, finished_by, equals", s)),
    }
}

/// Compile a graph AST into a `MemGraph`.
pub fn compile_graph(ast: &GraphAst) -> MemGraph {
    let mut graph = MemGraph::new();

    for edge in &ast.edges {
        match &edge.target {
            EdgeTarget::Str(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(&edge.source, &edge.label, MemValue::Str(val.clone()), edge.time_start, end);
                } else {
                    graph.add_str(&edge.source, &edge.label, val, edge.time_start);
                }
            }
            EdgeTarget::Num(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(&edge.source, &edge.label, MemValue::Num(*val), edge.time_start, end);
                } else {
                    graph.add_num(&edge.source, &edge.label, *val, edge.time_start);
                }
            }
            EdgeTarget::Bool(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(&edge.source, &edge.label, MemValue::Bool(*val), edge.time_start, end);
                } else {
                    graph.add_edge(&edge.source, &edge.label, MemValue::Bool(*val), edge.time_start);
                }
            }
            EdgeTarget::NodeRef(node) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(&edge.source, &edge.label, MemValue::Node(node.clone()), edge.time_start, end);
                } else {
                    graph.add_ref(&edge.source, &edge.label, node, edge.time_start);
                }
            }
        }
    }

    if let Some(t) = ast.now {
        graph.set_time(t);
    }

    graph
}
