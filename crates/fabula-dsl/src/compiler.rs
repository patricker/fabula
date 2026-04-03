//! Compiler: AST → fabula Pattern and MemGraph.
//!
//! The compiler is generic over a [`TypeMapper`] trait that bridges DSL
//! literal types (strings, numbers, booleans, node references) to the
//! target pattern type system. The default [`MemMapper`] produces
//! `Pattern<String, MemValue>` for testing and in-memory evaluation.

use crate::ast::*;
use crate::error::ParseError;
use fabula::builder::{NegationBuilder, PatternBuilder, StageBuilder};
use fabula::compose;
use fabula::datasource::ValueConstraint;
use fabula::interval::AllenRelation;
use fabula::pattern::Pattern;
use fabula_memory::MemValue;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

// ---------------------------------------------------------------------------
// TypeMapper trait
// ---------------------------------------------------------------------------

/// Maps DSL literal types to concrete pattern types.
///
/// The DSL parses labels as strings and values as strings/numbers/bools/node-refs.
/// This trait bridges from those parsed representations to the target type system.
///
/// All methods return `Result` to support fallible mappings (e.g., looking up
/// a string label in a predicate registry that may not contain the label).
///
/// # Example
///
/// ```rust,ignore
/// struct WkMapper { labels: HashMap<String, u32> }
///
/// impl TypeMapper for WkMapper {
///     type L = u32;
///     type V = paracausality::Value;
///
///     fn label(&self, s: &str) -> Result<u32, String> {
///         self.labels.get(s).copied()
///             .ok_or_else(|| format!("unknown predicate '{}'", s))
///     }
///     // ...
/// }
/// ```
pub trait TypeMapper {
    /// The label type for patterns (e.g., `String`, `u32`).
    type L: Clone + Debug;
    /// The value type for patterns (e.g., `MemValue`, `Value`).
    type V: Clone + Debug;

    /// Convert a label string to the target label type.
    fn label(&self, s: &str) -> Result<Self::L, String>;
    /// Convert a string literal to a value.
    fn string_value(&self, s: &str) -> Result<Self::V, String>;
    /// Convert a numeric literal to a value.
    fn num_value(&self, n: f64) -> Result<Self::V, String>;
    /// Convert a boolean literal to a value.
    fn bool_value(&self, b: bool) -> Result<Self::V, String>;
    /// Convert a node reference to a value.
    fn node_ref(&self, name: &str) -> Result<Self::V, String>;
}

/// Default mapper that produces `Pattern<String, MemValue>`.
pub struct MemMapper;

impl TypeMapper for MemMapper {
    type L = String;
    type V = MemValue;

    fn label(&self, s: &str) -> Result<String, String> { Ok(s.to_string()) }
    fn string_value(&self, s: &str) -> Result<MemValue, String> { Ok(MemValue::Str(s.to_string())) }
    fn num_value(&self, n: f64) -> Result<MemValue, String> { Ok(MemValue::Num(n)) }
    fn bool_value(&self, b: bool) -> Result<MemValue, String> { Ok(MemValue::Bool(b)) }
    fn node_ref(&self, name: &str) -> Result<MemValue, String> { Ok(MemValue::Node(name.to_string())) }
}

// ---------------------------------------------------------------------------
// Pattern compilation
// ---------------------------------------------------------------------------

/// Compile a pattern AST into a `Pattern<String, MemValue>` using the default mapper.
pub fn compile_pattern(ast: &PatternAst) -> Result<Pattern<String, MemValue>, ParseError> {
    compile_pattern_with(ast, &MemMapper)
}

/// Compile a [`PatternBody`] (from [`crate::parser::Parser::parse_pattern_body()`])
/// into a `Pattern<String, MemValue>` using the default mapper.
///
/// The `name` is assigned to the resulting pattern since the body doesn't
/// include the `pattern name { }` header.
pub fn compile_pattern_body(
    name: &str,
    body: &PatternBody,
) -> Result<Pattern<String, MemValue>, ParseError> {
    compile_pattern_body_with(name, body, &MemMapper)
}

/// Compile a [`PatternBody`] using a custom [`TypeMapper`].
pub fn compile_pattern_body_with<M: TypeMapper>(
    name: &str,
    body: &PatternBody,
    mapper: &M,
) -> Result<Pattern<M::L, M::V>, ParseError> {
    let ast = PatternAst {
        name: name.to_string(),
        stages: body.stages.clone(),
        negations: body.negations.clone(),
        temporals: body.temporals.clone(),
    };
    compile_pattern_with(&ast, mapper)
}

/// Compile a pattern AST using a custom [`TypeMapper`].
///
/// Validates variable scoping: `?var` sources must reference a variable
/// that was bound by `-> ?var` in a prior clause or is the current stage anchor.
pub fn compile_pattern_with<M: TypeMapper>(
    ast: &PatternAst,
    mapper: &M,
) -> Result<Pattern<M::L, M::V>, ParseError> {
    let mut builder = PatternBuilder::<M::L, M::V>::new(&ast.name);

    // Track variables bound across stages
    let mut bound_vars: HashSet<String> = HashSet::new();

    for stage in &ast.stages {
        let anchor = stage.anchor.clone();

        // Stage anchor is implicitly in scope within its clauses
        let mut stage_scope = bound_vars.clone();
        stage_scope.insert(anchor.clone());

        // Validate ?var sources are bound
        validate_clause_sources(&stage.clauses, &stage_scope)?;

        // Collect new bindings from this stage's clause targets
        for clause in &stage.clauses {
            if let ClauseTarget::Bind(ref var) = clause.target {
                if var == &anchor {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!(
                            "binding '-> ?{}' collides with stage anchor '{}'. \
                             This silently constrains ?{} to self-loops only. \
                             Use a different variable name.",
                            var, anchor, var
                        ),
                    });
                }
                bound_vars.insert(var.clone());
            }
        }
        // Stage anchor is bound for subsequent stages
        bound_vars.insert(anchor.clone());

        let clauses = stage.clauses.clone();
        builder = builder.stage(&anchor, |s| build_stage(s, &clauses, mapper));
    }

    for neg in &ast.negations {
        // Negation clauses can reference bound vars from completed stages
        validate_clause_sources(&neg.clauses, &bound_vars)?;

        let clauses = neg.clauses.clone();
        builder = match &neg.kind {
            NegationKind::Between(start, end) => {
                builder.unless_between(start, end, |n| build_negation(n, &clauses, mapper))
            }
            NegationKind::After(start) => {
                builder.unless_after(start, |n| build_negation(n, &clauses, mapper))
            }
            NegationKind::Global => {
                builder.unless_global(|n| build_negation(n, &clauses, mapper))
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
        if temp.gap_min.is_some() || temp.gap_max.is_some() {
            builder = builder.temporal_with_gap(
                &temp.left,
                relation,
                &temp.right,
                fabula::pattern::MetricGap {
                    min: temp.gap_min,
                    max: temp.gap_max,
                },
            );
        } else {
            builder = builder.temporal(&temp.left, relation, &temp.right);
        }
    }

    Ok(builder.build())
}

/// Validate that all `?var` sources in clauses reference bound variables.
/// Accumulates bindings from `-> ?var` targets clause-by-clause within the list.
fn validate_clause_sources(
    clauses: &[ClauseAst],
    initial_scope: &HashSet<String>,
) -> Result<(), ParseError> {
    let mut scope = initial_scope.clone();
    for clause in clauses {
        if clause.source_kind == SourceKind::Var && !scope.contains(&clause.source) {
            return Err(ParseError {
                line: 0,
                column: 0,
                span: (0, 0),
                message: format!(
                    "variable '?{}' used as source but not yet bound. \
                     Bind it with '-> ?{}' in a prior clause, or use '{}' \
                     (without ?) for a literal node name.",
                    clause.source, clause.source, clause.source
                ),
            });
        }
        // Negation (!) is only valid with literal values and node references.
        // Constraints and bindings cannot be negated.
        if clause.negated {
            match &clause.target {
                ClauseTarget::Constraint(..) => {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!(
                            "negated constraints ('! {}.{} < value') are not supported. \
                             Rewrite as the inverse constraint \
                             (e.g., '! x.v < 0.5' becomes 'x.v >= 0.5').",
                            clause.source, clause.label
                        ),
                    });
                }
                ClauseTarget::Bind(var) => {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!(
                            "negated bindings ('! {}.{} -> ?{}') are not supported.",
                            clause.source, clause.label, var
                        ),
                    });
                }
                _ => {} // Literals and NodeRefs can be negated
            }
        }
        // Bind target for subsequent clauses
        if let ClauseTarget::Bind(ref var) = clause.target {
            scope.insert(var.clone());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage and clause compilation (generic over TypeMapper)
// ---------------------------------------------------------------------------

fn build_stage<M: TypeMapper>(
    mut s: StageBuilder<M::L, M::V>,
    clauses: &[ClauseAst],
    mapper: &M,
) -> StageBuilder<M::L, M::V> {
    for clause in clauses {
        s = add_clause_to_stage(s, clause, mapper);
    }
    s
}

fn add_clause_to_stage<M: TypeMapper>(
    s: StageBuilder<M::L, M::V>,
    clause: &ClauseAst,
    mapper: &M,
) -> StageBuilder<M::L, M::V> {
    let source = &clause.source;
    // Unwrap mapper results — validation errors in mapper are propagated
    // at a higher level; within the builder callback we cannot return Result.
    let label = mapper.label(&clause.label).expect("label mapping failed in stage builder");

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            let v = mapper.string_value(val).expect("string_value mapping failed");
            if clause.negated { s.not_edge(source, label, v) } else { s.edge(source, label, v) }
        }
        ClauseTarget::LiteralNum(val) => {
            let v = mapper.num_value(*val).expect("num_value mapping failed");
            if clause.negated { s.not_edge(source, label, v) } else { s.edge(source, label, v) }
        }
        ClauseTarget::LiteralBool(val) => {
            let v = mapper.bool_value(*val).expect("bool_value mapping failed");
            if clause.negated { s.not_edge(source, label, v) } else { s.edge(source, label, v) }
        }
        ClauseTarget::Bind(var) => {
            s.edge_bind(source, label, var)
        }
        ClauseTarget::NodeRef(node) => {
            let v = mapper.node_ref(node).expect("node_ref mapping failed");
            if clause.negated { s.not_edge(source, label, v) } else { s.edge(source, label, v) }
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint_with(mapper, *op, val);
            s.edge_constrained(source, label, constraint)
        }
    }
}

fn build_negation<M: TypeMapper>(
    mut n: NegationBuilder<M::L, M::V>,
    clauses: &[ClauseAst],
    mapper: &M,
) -> NegationBuilder<M::L, M::V> {
    for clause in clauses {
        n = add_clause_to_negation(n, clause, mapper);
    }
    n
}

fn add_clause_to_negation<M: TypeMapper>(
    n: NegationBuilder<M::L, M::V>,
    clause: &ClauseAst,
    mapper: &M,
) -> NegationBuilder<M::L, M::V> {
    let source = &clause.source;
    let label = mapper.label(&clause.label).expect("label mapping failed in negation builder");

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            let v = mapper.string_value(val).expect("string_value mapping failed");
            n.edge(source, label, v)
        }
        ClauseTarget::LiteralNum(val) => {
            let v = mapper.num_value(*val).expect("num_value mapping failed");
            n.edge(source, label, v)
        }
        ClauseTarget::LiteralBool(val) => {
            let v = mapper.bool_value(*val).expect("bool_value mapping failed");
            n.edge(source, label, v)
        }
        ClauseTarget::Bind(var) => {
            n.edge_bind(source, label, var)
        }
        ClauseTarget::NodeRef(node) => {
            let v = mapper.node_ref(node).expect("node_ref mapping failed");
            n.edge(source, label, v)
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint_with(mapper, *op, val);
            n.edge_constrained(source, label, constraint)
        }
    }
}

fn make_constraint_with<M: TypeMapper>(
    mapper: &M,
    op: ConstraintOp,
    val: &ConstraintValue,
) -> ValueConstraint<M::V> {
    let v = match val {
        ConstraintValue::Num(n) => mapper.num_value(*n).expect("num_value mapping failed in constraint"),
        ConstraintValue::Str(s) => mapper.string_value(s).expect("string_value mapping failed in constraint"),
    };
    match op {
        ConstraintOp::Lt => ValueConstraint::Lt(v),
        ConstraintOp::Gt => ValueConstraint::Gt(v),
        ConstraintOp::Lte => ValueConstraint::Lte(v),
        ConstraintOp::Gte => ValueConstraint::Gte(v),
    }
}

// ---------------------------------------------------------------------------
// Compose compilation
// ---------------------------------------------------------------------------

/// Compile a compose directive using the default mapper.
pub fn compile_compose(
    ast: &ComposeAst,
    known: &HashMap<String, Pattern<String, MemValue>>,
) -> Result<Vec<Pattern<String, MemValue>>, ParseError> {
    compile_compose_with(ast, known, &MemMapper)
}

/// Compile a compose directive using a custom [`TypeMapper`].
///
/// Resolves pattern names against already-compiled patterns in `known`.
/// Returns one or more patterns (choice returns multiple).
#[allow(clippy::type_complexity)]
pub fn compile_compose_with<M: TypeMapper>(
    ast: &ComposeAst,
    known: &HashMap<String, Pattern<M::L, M::V>>,
    _mapper: &M,
) -> Result<Vec<Pattern<M::L, M::V>>, ParseError> {
    let resolve = |name: &str| -> Result<&Pattern<M::L, M::V>, ParseError> {
        known.get(name).ok_or_else(|| ParseError {
            line: 0,
            column: 0,
            span: (0, 0),
            message: format!(
                "compose '{}' references pattern '{}' which has not been defined yet. \
                 Define it before the compose directive.",
                ast.name, name
            ),
        })
    };

    match &ast.body {
        ComposeBody::Sequence { left, right, shared } => {
            let a = resolve(left)?;
            let b = resolve(right)?;
            let shared_refs: Vec<&str> = shared.iter().map(|s| s.as_str()).collect();
            Ok(vec![compose::sequence(&ast.name, a, b, &shared_refs)])
        }
        ComposeBody::Choice { alternatives } => {
            let pats = alternatives.iter()
                .map(|name| resolve(name))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(compose::choice(&ast.name, &pats, true))
        }
        ComposeBody::Repeat { pattern, count, shared } => {
            let p = resolve(pattern)?;
            let shared_refs: Vec<&str> = shared.iter().map(|s| s.as_str()).collect();
            Ok(vec![compose::repeat(&ast.name, p, *count, &shared_refs)])
        }
    }
}

// ---------------------------------------------------------------------------
// Allen relation parsing
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Graph compilation (always MemGraph — test-only)
// ---------------------------------------------------------------------------

/// Compile a graph AST into a `MemGraph`.
pub fn compile_graph(ast: &GraphAst) -> fabula_memory::MemGraph {
    let mut graph = fabula_memory::MemGraph::new();

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use crate::lexer::Lexer;

    fn parse_ast(input: &str) -> PatternAst {
        let tokens = Lexer::new(input).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_pattern_only().unwrap()
    }

    #[test]
    fn mem_mapper_matches_existing_behavior() {
        let input = r#"pattern test {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
            }
        }"#;
        let ast = parse_ast(input);
        let direct = compile_pattern(&ast).unwrap();
        let via_mapper = compile_pattern_with(&ast, &MemMapper).unwrap();
        assert_eq!(direct, via_mapper);
    }

    /// A custom mapper that uppercases labels and wraps values.
    #[derive(Debug, Clone)]
    enum UpperValue {
        Text(String),
        Number(f64),
        Flag(bool),
        Ref(String),
    }

    struct UpperMapper;

    impl TypeMapper for UpperMapper {
        type L = String;
        type V = UpperValue;

        fn label(&self, s: &str) -> Result<String, String> {
            Ok(s.to_uppercase())
        }
        fn string_value(&self, s: &str) -> Result<UpperValue, String> {
            Ok(UpperValue::Text(s.to_uppercase()))
        }
        fn num_value(&self, n: f64) -> Result<UpperValue, String> {
            Ok(UpperValue::Number(n))
        }
        fn bool_value(&self, b: bool) -> Result<UpperValue, String> {
            Ok(UpperValue::Flag(b))
        }
        fn node_ref(&self, name: &str) -> Result<UpperValue, String> {
            Ok(UpperValue::Ref(name.to_uppercase()))
        }
    }

    #[test]
    fn custom_mapper_transforms_labels() {
        let input = r#"pattern test {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
            }
        }"#;
        let ast = parse_ast(input);
        let pattern = compile_pattern_with(&ast, &UpperMapper).unwrap();
        assert_eq!(pattern.stages[0].clauses[0].label, "EVENTTYPE");
        assert_eq!(pattern.stages[0].clauses[1].label, "ACTOR");
    }

    #[test]
    fn custom_mapper_transforms_values() {
        let input = r#"pattern test {
            stage e1 {
                e1.eventType = "betray"
                e1.score > 5
            }
        }"#;
        let ast = parse_ast(input);
        let pattern = compile_pattern_with(&ast, &UpperMapper).unwrap();
        // String literal uppercased
        match &pattern.stages[0].clauses[0].target {
            fabula::pattern::Target::Literal(UpperValue::Text(s)) => assert_eq!(s, "BETRAY"),
            other => panic!("expected Text, got {:?}", other),
        }
        // Constraint value mapped through num_value
        match &pattern.stages[0].clauses[1].target {
            fabula::pattern::Target::Constraint(ValueConstraint::Gt(UpperValue::Number(n))) => {
                assert_eq!(*n, 5.0);
            }
            other => panic!("expected Gt(Number), got {:?}", other),
        }
    }

    /// A mapper that rejects unknown labels.
    struct StrictMapper;

    impl TypeMapper for StrictMapper {
        type L = u32;
        type V = String;

        fn label(&self, s: &str) -> Result<u32, String> {
            match s {
                "eventType" => Ok(1),
                "actor" => Ok(2),
                _ => Err(format!("unknown predicate '{}'", s)),
            }
        }
        fn string_value(&self, s: &str) -> Result<String, String> { Ok(s.to_string()) }
        fn num_value(&self, n: f64) -> Result<String, String> { Ok(n.to_string()) }
        fn bool_value(&self, b: bool) -> Result<String, String> { Ok(b.to_string()) }
        fn node_ref(&self, name: &str) -> Result<String, String> { Ok(name.to_string()) }
    }

    #[test]
    fn strict_mapper_succeeds_with_known_labels() {
        let input = r#"pattern test {
            stage e1 {
                e1.eventType = "betray"
                e1.actor -> ?char
            }
        }"#;
        let ast = parse_ast(input);
        let pattern = compile_pattern_with(&ast, &StrictMapper).unwrap();
        assert_eq!(pattern.stages[0].clauses[0].label, 1u32);
        assert_eq!(pattern.stages[0].clauses[1].label, 2u32);
    }

    #[test]
    #[should_panic(expected = "unknown predicate 'badLabel'")]
    fn strict_mapper_panics_on_unknown_label() {
        let input = r#"pattern test {
            stage e1 {
                e1.badLabel = "value"
            }
        }"#;
        let ast = parse_ast(input);
        // The mapper error propagates as a panic from within the builder callback
        let _ = compile_pattern_with(&ast, &StrictMapper);
    }
}
