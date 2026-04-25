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

    fn label(&self, s: &str) -> Result<String, String> {
        Ok(s.to_string())
    }
    fn string_value(&self, s: &str) -> Result<MemValue, String> {
        Ok(MemValue::Str(s.to_string()))
    }
    fn num_value(&self, n: f64) -> Result<MemValue, String> {
        Ok(MemValue::Num(n))
    }
    fn bool_value(&self, b: bool) -> Result<MemValue, String> {
        Ok(MemValue::Bool(b))
    }
    fn node_ref(&self, name: &str) -> Result<MemValue, String> {
        Ok(MemValue::Node(name.to_string()))
    }
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
        metadata: body.metadata.clone(),
        deadline: body.deadline,
        unordered_groups: body.unordered_groups.clone(),
        private: body.private,
        importance: body.importance,
        advance_in_place: body.advance_in_place,
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

    // Pre-collect bindings from concurrent group siblings so scoping is
    // symmetric within a group (order-independent).
    let mut group_prebound: HashMap<usize, HashSet<String>> = HashMap::new();
    for group in &ast.unordered_groups {
        let mut group_vars = HashSet::new();
        for &si in group {
            if let Some(stage) = ast.stages.get(si) {
                group_vars.insert(stage.anchor.clone());
                for clause in &stage.clauses {
                    if let ClauseTarget::Bind(ref var) = clause.target {
                        group_vars.insert(var.clone());
                    }
                }
            }
        }
        for &si in group {
            group_prebound.insert(si, group_vars.clone());
        }
    }

    for (stage_idx, stage) in ast.stages.iter().enumerate() {
        let anchor = stage.anchor.clone();

        // Stage anchor is implicitly in scope within its clauses.
        // For concurrent group members, sibling bindings are also in scope.
        let mut stage_scope = bound_vars.clone();
        stage_scope.insert(anchor.clone());
        if let Some(sibling_vars) = group_prebound.get(&stage_idx) {
            stage_scope.extend(sibling_vars.iter().cloned());
        }

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

        // Validate and compile this stage's let bindings.
        // Lets see clause-bound vars from this stage and all prior bindings.
        let mut compiled_lets: Vec<fabula::expr::ComputedBinding<M::V>> = Vec::new();
        for la in &stage.let_bindings {
            // Reject shadowing of any already-bound var (clause vars, anchor,
            // earlier lets, or vars from prior stages).
            if bound_vars.contains(&la.name) {
                return Err(ParseError {
                    line: 0,
                    column: 0,
                    span: (0, 0),
                    message: format!(
                        "let '{}' shadows an already-bound variable",
                        la.name
                    ),
                });
            }
            let expr = compile_expr(&la.expr, mapper)?;
            // Validate every Var reference in the expression is bound.
            for v in expr.vars() {
                if !bound_vars.contains(v) {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!(
                            "let '{}' references unbound variable ?{}",
                            la.name, v
                        ),
                    });
                }
            }
            // The let's name becomes available to subsequent lets and stages.
            bound_vars.insert(la.name.clone());
            compiled_lets.push(fabula::expr::ComputedBinding::new(la.name.clone(), expr));
        }

        let clauses = stage.clauses.clone();
        let lets = compiled_lets;
        builder = builder.stage(&anchor, move |mut s| {
            for clause in &clauses {
                s = add_clause_to_stage(s, clause, mapper);
            }
            for cb in &lets {
                s = s.let_binding(cb.name.clone(), cb.expr.clone());
            }
            s
        });
    }

    for neg in &ast.negations {
        // Negation clauses can reference bound vars from completed stages
        validate_clause_sources(&neg.clauses, &bound_vars)?;

        // Reject unless_between where both anchors are in the same concurrent group
        if let NegationKind::Between(start, end) = &neg.kind {
            for group in &ast.unordered_groups {
                let start_in = group
                    .iter()
                    .any(|&i| ast.stages.get(i).is_some_and(|s| s.anchor == *start));
                let end_in = group
                    .iter()
                    .any(|&i| ast.stages.get(i).is_some_and(|s| s.anchor == *end));
                if start_in && end_in {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!(
                            "unless_between anchors '{}' and '{}' are in the same concurrent group. \
                             Temporal ordering between concurrent stages is undefined.",
                            start, end
                        ),
                    });
                }
            }
        }

        let clauses = neg.clauses.clone();
        builder = match &neg.kind {
            NegationKind::Between(start, end) => {
                builder.unless_between(start, end, |n| build_negation(n, &clauses, mapper))
            }
            NegationKind::After(start) => {
                builder.unless_after(start, |n| build_negation(n, &clauses, mapper))
            }
            NegationKind::Global => builder.unless_global(|n| build_negation(n, &clauses, mapper)),
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

    // Convert ordered metadata pairs to HashMap (last-write-wins for duplicates)
    for (key, value) in &ast.metadata {
        builder = builder.metadata(key, value);
    }

    if let Some(deadline) = ast.deadline {
        if deadline < 1.0 {
            return Err(ParseError {
                line: 0,
                column: 0,
                span: (0, 0),
                message: format!("deadline must be a positive integer, got {}", deadline),
            });
        }
        builder = builder.deadline(deadline as u64);
    }

    let mut pattern = builder.importance(ast.importance).build();
    pattern.unordered_groups = ast.unordered_groups.clone();
    pattern.private = ast.private;
    pattern.advance_in_place = ast.advance_in_place;
    Ok(pattern)
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
        // Validate ConstraintVar references are in scope
        if let ClauseTarget::ConstraintVar(_, ref var) = clause.target {
            if !scope.contains(var) {
                return Err(ParseError {
                    line: 0,
                    column: 0,
                    span: (0, 0),
                    message: format!(
                        "variable '?{}' used in constraint but not yet bound. \
                         Bind it with '-> ?{}' in a prior clause or stage.",
                        var, var
                    ),
                });
            }
        }
        // Negation (!) is only valid with literal values and node references.
        // Constraints and bindings cannot be negated.
        if clause.negated {
            match &clause.target {
                ClauseTarget::Constraint(..) | ClauseTarget::ConstraintVar(..) => {
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

fn add_clause_to_stage<M: TypeMapper>(
    s: StageBuilder<M::L, M::V>,
    clause: &ClauseAst,
    mapper: &M,
) -> StageBuilder<M::L, M::V> {
    let source = &clause.source;
    // Unwrap mapper results -- validation errors in mapper are propagated
    // at a higher level; within the builder callback we cannot return Result.
    let label = mapper
        .label(&clause.label)
        .expect("label mapping failed in stage builder");

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            let v = mapper
                .string_value(val)
                .expect("string_value mapping failed");
            if clause.negated {
                s.not_edge(source, label, v)
            } else {
                s.edge(source, label, v)
            }
        }
        ClauseTarget::LiteralNum(val) => {
            let v = mapper.num_value(*val).expect("num_value mapping failed");
            if clause.negated {
                s.not_edge(source, label, v)
            } else {
                s.edge(source, label, v)
            }
        }
        ClauseTarget::LiteralBool(val) => {
            let v = mapper.bool_value(*val).expect("bool_value mapping failed");
            if clause.negated {
                s.not_edge(source, label, v)
            } else {
                s.edge(source, label, v)
            }
        }
        ClauseTarget::Bind(var) => s.edge_bind(source, label, var),
        ClauseTarget::NodeRef(node) => {
            let v = mapper.node_ref(node).expect("node_ref mapping failed");
            if clause.negated {
                s.not_edge(source, label, v)
            } else {
                s.edge(source, label, v)
            }
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint_with(mapper, *op, val);
            s.edge_constrained(source, label, constraint)
        }
        ClauseTarget::ConstraintVar(op, var) => {
            let constraint = make_var_constraint(*op, var);
            s.edge_constrained(source, label, constraint)
        }
        ClauseTarget::OneOf(values) => {
            let mapped: Vec<M::V> = values
                .iter()
                .map(|v| match v {
                    ConstraintValue::Str(s_val) => mapper
                        .string_value(s_val)
                        .expect("string_value mapping failed in OneOf"),
                    ConstraintValue::Num(n) => mapper
                        .num_value(*n)
                        .expect("num_value mapping failed in OneOf"),
                })
                .collect();
            if clause.negated {
                s.not_edge_one_of(source, label, mapped)
            } else {
                s.edge_one_of(source, label, mapped)
            }
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
    let label = mapper
        .label(&clause.label)
        .expect("label mapping failed in negation builder");

    match &clause.target {
        ClauseTarget::LiteralStr(val) => {
            let v = mapper
                .string_value(val)
                .expect("string_value mapping failed");
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
        ClauseTarget::Bind(var) => n.edge_bind(source, label, var),
        ClauseTarget::NodeRef(node) => {
            let v = mapper.node_ref(node).expect("node_ref mapping failed");
            n.edge(source, label, v)
        }
        ClauseTarget::Constraint(op, val) => {
            let constraint = make_constraint_with(mapper, *op, val);
            n.edge_constrained(source, label, constraint)
        }
        ClauseTarget::ConstraintVar(op, var) => {
            let constraint = make_var_constraint(*op, var);
            n.edge_constrained(source, label, constraint)
        }
        ClauseTarget::OneOf(values) => {
            let mapped: Vec<M::V> = values
                .iter()
                .map(|v| match v {
                    ConstraintValue::Str(s_val) => mapper
                        .string_value(s_val)
                        .expect("string_value mapping failed in OneOf"),
                    ConstraintValue::Num(n_val) => mapper
                        .num_value(*n_val)
                        .expect("num_value mapping failed in OneOf"),
                })
                .collect();
            n.edge_constrained(source, label, ValueConstraint::OneOf(mapped))
        }
    }
}

fn compile_expr<M: TypeMapper>(
    ast: &ExprAst,
    mapper: &M,
) -> Result<fabula::expr::Expr<M::V>, ParseError> {
    use fabula::expr::{BinOp, Expr};
    match ast {
        ExprAst::Literal(ConstraintValue::Num(n)) => mapper
            .num_value(*n)
            .map(Expr::Literal)
            .map_err(|m| ParseError {
                line: 0,
                column: 0,
                span: (0, 0),
                message: m,
            }),
        ExprAst::Literal(ConstraintValue::Str(s)) => mapper
            .string_value(s)
            .map(Expr::Literal)
            .map_err(|m| ParseError {
                line: 0,
                column: 0,
                span: (0, 0),
                message: m,
            }),
        ExprAst::Var(name) => Ok(Expr::Var(name.clone())),
        ExprAst::BinOp(op, l, r) => {
            let lo = compile_expr(l, mapper)?;
            let ro = compile_expr(r, mapper)?;
            let bop = match op {
                ExprBinOp::Add => BinOp::Add,
                ExprBinOp::Sub => BinOp::Sub,
                ExprBinOp::Mul => BinOp::Mul,
                ExprBinOp::Div => BinOp::Div,
            };
            Ok(Expr::BinOp(bop, Box::new(lo), Box::new(ro)))
        }
    }
}

fn make_constraint_with<M: TypeMapper>(
    mapper: &M,
    op: ConstraintOp,
    val: &ConstraintValue,
) -> ValueConstraint<M::V> {
    let v = match val {
        ConstraintValue::Num(n) => mapper
            .num_value(*n)
            .expect("num_value mapping failed in constraint"),
        ConstraintValue::Str(s) => mapper
            .string_value(s)
            .expect("string_value mapping failed in constraint"),
    };
    match op {
        ConstraintOp::Eq => ValueConstraint::Eq(v),
        ConstraintOp::Lt => ValueConstraint::Lt(v),
        ConstraintOp::Gt => ValueConstraint::Gt(v),
        ConstraintOp::Lte => ValueConstraint::Lte(v),
        ConstraintOp::Gte => ValueConstraint::Gte(v),
    }
}

fn make_var_constraint<V>(op: ConstraintOp, var: &str) -> ValueConstraint<V> {
    match op {
        ConstraintOp::Eq => ValueConstraint::EqVar(var.to_string()),
        ConstraintOp::Lt => ValueConstraint::LtVar(var.to_string()),
        ConstraintOp::Gt => ValueConstraint::GtVar(var.to_string()),
        ConstraintOp::Lte => ValueConstraint::LteVar(var.to_string()),
        ConstraintOp::Gte => ValueConstraint::GteVar(var.to_string()),
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
        ComposeBody::Sequence {
            left,
            right,
            shared,
        } => {
            let a = resolve(left)?;
            let b = resolve(right)?;
            let shared_refs: Vec<&str> = shared.iter().map(|s| s.as_str()).collect();
            Ok(vec![compose::sequence(&ast.name, a, b, &shared_refs)])
        }
        ComposeBody::Choice {
            alternatives,
            exclusive,
        } => {
            let pats = alternatives
                .iter()
                .map(|name| resolve(name))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(compose::choice(&ast.name, &pats, *exclusive))
        }
        ComposeBody::Repeat {
            pattern,
            min,
            max,
            shared,
        } => {
            let p = resolve(pattern)?;
            let shared_refs: Vec<&str> = shared.iter().map(|s| s.as_str()).collect();
            if *min < 1 {
                return Err(ParseError {
                    line: 0,
                    column: 0,
                    span: (0, 0),
                    message: "repeat count must be at least 1".to_string(),
                });
            }
            if let Some(max_val) = max {
                if *max_val < *min {
                    return Err(ParseError {
                        line: 0,
                        column: 0,
                        span: (0, 0),
                        message: format!("repeat max ({}) must be >= min ({})", max_val, min),
                    });
                }
            }
            // Exact repeat (min == max): use original unrolled repeat for backward compat
            if *max == Some(*min) {
                Ok(vec![compose::repeat(&ast.name, p, *min, &shared_refs)])
            } else {
                Ok(vec![compose::repeat_range(
                    &ast.name,
                    p,
                    *min,
                    *max,
                    &shared_refs,
                )])
            }
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
// Graph compilation (always MemGraph -- test-only)
// ---------------------------------------------------------------------------

/// Compile a graph AST into a `MemGraph`.
pub fn compile_graph(ast: &GraphAst) -> fabula_memory::MemGraph {
    let mut graph = fabula_memory::MemGraph::new();

    for edge in &ast.edges {
        match &edge.target {
            EdgeTarget::Str(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(
                        &edge.source,
                        &edge.label,
                        MemValue::Str(val.clone()),
                        edge.time_start,
                        end,
                    );
                } else {
                    graph.add_str(&edge.source, &edge.label, val, edge.time_start);
                }
            }
            EdgeTarget::Num(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(
                        &edge.source,
                        &edge.label,
                        MemValue::Num(*val),
                        edge.time_start,
                        end,
                    );
                } else {
                    graph.add_num(&edge.source, &edge.label, *val, edge.time_start);
                }
            }
            EdgeTarget::Bool(val) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(
                        &edge.source,
                        &edge.label,
                        MemValue::Bool(*val),
                        edge.time_start,
                        end,
                    );
                } else {
                    graph.add_edge(
                        &edge.source,
                        &edge.label,
                        MemValue::Bool(*val),
                        edge.time_start,
                    );
                }
            }
            EdgeTarget::NodeRef(node) => {
                if let Some(end) = edge.time_end {
                    graph.add_edge_bounded(
                        &edge.source,
                        &edge.label,
                        MemValue::Node(node.clone()),
                        edge.time_start,
                        end,
                    );
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
    use crate::lexer::Lexer;
    use crate::parser::Parser;

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
        fn string_value(&self, s: &str) -> Result<String, String> {
            Ok(s.to_string())
        }
        fn num_value(&self, n: f64) -> Result<String, String> {
            Ok(n.to_string())
        }
        fn bool_value(&self, b: bool) -> Result<String, String> {
            Ok(b.to_string())
        }
        fn node_ref(&self, name: &str) -> Result<String, String> {
            Ok(name.to_string())
        }
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

    #[test]
    fn metadata_parsed_and_compiled() {
        let input = r#"pattern my_rule {
            meta("severity", "high")
            meta("mitre", "T1078")
            stage e1 {
                e1.eventType = "betray"
            }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.metadata.len(), 2);
        assert_eq!(
            ast.metadata[0],
            ("severity".to_string(), "high".to_string())
        );
        assert_eq!(ast.metadata[1], ("mitre".to_string(), "T1078".to_string()));

        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.metadata.get("severity").unwrap(), "high");
        assert_eq!(pattern.metadata.get("mitre").unwrap(), "T1078");
    }

    #[test]
    fn metadata_after_stages() {
        let input = r#"pattern test {
            stage e1 { e1.type = "x" }
            meta("key", "val")
        }"#;
        let ast = parse_ast(input);
        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.metadata.get("key").unwrap(), "val");
        assert_eq!(pattern.stages.len(), 1);
    }

    #[test]
    fn metadata_duplicate_key_last_wins() {
        let input = r#"pattern test {
            meta("key", "first")
            meta("key", "second")
            stage e1 { e1.type = "x" }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.metadata.len(), 2); // AST preserves both

        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.metadata.get("key").unwrap(), "second"); // last wins
        assert_eq!(pattern.metadata.len(), 1);
    }

    #[test]
    fn compile_pattern_body_with_metadata() {
        let input = r#"pattern wrapper {
            meta("source", "test")
            stage e1 { e1.type = "x" }
        }"#;
        let tokens = Lexer::new(input).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.expect(crate::lexer::TokenKind::Pattern).unwrap();
        let _name = parser.expect_ident().unwrap();
        parser.expect(crate::lexer::TokenKind::LBrace).unwrap();
        let body = parser.parse_pattern_body().unwrap();

        assert_eq!(body.metadata.len(), 1);

        let pattern = compile_pattern_body("renamed", &body).unwrap();
        assert_eq!(pattern.name, "renamed");
        assert_eq!(pattern.metadata.get("source").unwrap(), "test");
    }

    #[test]
    fn deadline_parsed_and_compiled() {
        let input = r#"pattern sla {
            deadline 2880
            stage e1 { e1.type = "submit" }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.deadline, Some(2880.0));

        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.deadline_ticks, Some(2880));
    }

    #[test]
    fn no_deadline_is_none() {
        let input = r#"pattern test {
            stage e1 { e1.type = "x" }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.deadline, None);

        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.deadline_ticks, None);
    }

    #[test]
    fn deadline_with_metadata() {
        let input = r#"pattern sla {
            meta("severity", "high")
            deadline 100
            stage e1 { e1.type = "x" }
        }"#;
        let pattern = compile_pattern(&parse_ast(input)).unwrap();
        assert_eq!(pattern.deadline_ticks, Some(100));
        assert_eq!(pattern.metadata.get("severity").unwrap(), "high");
    }

    #[test]
    fn deadline_zero_rejected() {
        let input = r#"pattern bad {
            deadline 0
            stage e1 { e1.type = "x" }
        }"#;
        let result = compile_pattern(&parse_ast(input));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("positive integer"));
    }

    // -----------------------------------------------------------------------
    // Cross-stage value comparison (ConstraintVar)
    // -----------------------------------------------------------------------

    #[test]
    fn constraint_var_gt_parsed_and_compiled() {
        let input = r#"pattern escalation {
            stage e1 {
                e1.type = "order"
                e1.price -> ?base_price
            }
            stage e2 {
                e2.type = "order"
                e2.price > ?base_price
            }
        }"#;
        let ast = parse_ast(input);
        assert!(matches!(
            &ast.stages[1].clauses[1].target,
            ClauseTarget::ConstraintVar(ConstraintOp::Gt, var) if var == "base_price"
        ));

        let pattern = compile_pattern(&ast).unwrap();
        match &pattern.stages[1].clauses[1].target {
            fabula::pattern::Target::Constraint(ValueConstraint::GtVar(v)) => {
                assert_eq!(v, "base_price");
            }
            other => panic!("expected GtVar, got {:?}", other),
        }
    }

    #[test]
    fn constraint_var_all_operators() {
        for (op_str, expected_op) in [
            ("<", ConstraintOp::Lt),
            (">", ConstraintOp::Gt),
            ("<=", ConstraintOp::Lte),
            (">=", ConstraintOp::Gte),
            ("=", ConstraintOp::Eq),
        ] {
            let input = format!(
                r#"pattern test {{
                    stage e1 {{ e1.val -> ?v }}
                    stage e2 {{ e2.val {} ?v }}
                }}"#,
                op_str
            );
            let ast = parse_ast(&input);
            assert!(
                matches!(
                    &ast.stages[1].clauses[0].target,
                    ClauseTarget::ConstraintVar(op, var) if *op == expected_op && var == "v"
                ),
                "failed for operator {}",
                op_str
            );
        }
    }

    #[test]
    fn constraint_var_unbound_rejected() {
        let input = r#"pattern bad {
            stage e1 {
                e1.type = "x"
                e1.score > ?unbound
            }
        }"#;
        let result = compile_pattern(&parse_ast(input));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not yet bound"));
    }

    #[test]
    fn constraint_var_negated_rejected() {
        let input = r#"pattern bad {
            stage e1 { e1.val -> ?v }
            stage e2 { ! e2.val > ?v }
        }"#;
        let result = compile_pattern(&parse_ast(input));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("negated constraints"));
    }

    // -----------------------------------------------------------------------
    // Concurrent (unordered) groups
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_parsed_and_compiled() {
        let input = r#"pattern test {
            stage setup { setup.type = "start" }
            concurrent {
                stage a { a.type = "alpha" }
                stage b { b.type = "beta" }
            }
            stage end { end.type = "finish" }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.stages.len(), 4); // setup, a, b, end
        assert_eq!(ast.unordered_groups.len(), 1);
        assert_eq!(ast.unordered_groups[0], vec![1, 2]); // a and b

        let pattern = compile_pattern(&ast).unwrap();
        assert_eq!(pattern.stages.len(), 4);
        assert_eq!(pattern.unordered_groups.len(), 1);
        assert_eq!(pattern.unordered_groups[0], vec![1, 2]);
    }

    #[test]
    fn concurrent_only_group() {
        let input = r#"pattern test {
            concurrent {
                stage a { a.type = "alpha" }
                stage b { b.type = "beta" }
            }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.stages.len(), 2);
        assert_eq!(ast.unordered_groups, vec![vec![0, 1]]);
    }

    #[test]
    fn concurrent_multiple_groups() {
        let input = r#"pattern test {
            concurrent {
                stage a { a.type = "alpha" }
                stage b { b.type = "beta" }
            }
            stage mid { mid.type = "mid" }
            concurrent {
                stage c { c.type = "gamma" }
                stage d { d.type = "delta" }
            }
        }"#;
        let ast = parse_ast(input);
        assert_eq!(ast.stages.len(), 5);
        assert_eq!(ast.unordered_groups.len(), 2);
        assert_eq!(ast.unordered_groups[0], vec![0, 1]);
        assert_eq!(ast.unordered_groups[1], vec![3, 4]);
    }

    #[test]
    fn concurrent_unless_between_same_group_rejected() {
        let input = r#"pattern bad {
            concurrent {
                stage a { a.type = "alpha" }
                stage b { b.type = "beta" }
            }
            unless between a b {
                mid.type = "block"
            }
        }"#;
        let result = compile_pattern(&parse_ast(input));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("same concurrent group"));
    }

    #[test]
    fn concurrent_unless_between_different_groups_ok() {
        let input = r#"pattern ok {
            stage setup { setup.type = "start" }
            concurrent {
                stage a { a.type = "alpha" }
                stage b { b.type = "beta" }
            }
            unless between setup a {
                mid.type = "block"
            }
        }"#;
        let result = compile_pattern(&parse_ast(input));
        assert!(result.is_ok());
    }

    #[test]
    fn concurrent_dsl_evaluate() {
        let doc = crate::parse_document(
            r#"
            pattern test {
                concurrent {
                    stage a { a.type = "alpha" }
                    stage b { b.type = "beta" }
                }
            }

            graph {
                @1 ev1.type = "beta"
                @2 ev2.type = "alpha"
                now = 10
            }
            "#,
        )
        .unwrap();

        assert_eq!(doc.patterns[0].unordered_groups, vec![vec![0, 1]]);

        let mut engine = fabula::engine::SiftEngine::<String, String, MemValue, i64>::new();
        engine.register(doc.patterns[0].clone());
        let matches = engine.evaluate(&doc.graphs[0]);
        assert_eq!(matches.len(), 1);
    }
}
