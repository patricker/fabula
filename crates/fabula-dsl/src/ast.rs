//! Abstract syntax tree types for the fabula DSL.

/// Top-level document — can contain patterns and/or graph declarations.
#[derive(Debug, Clone)]
pub struct Document {
    pub patterns: Vec<PatternAst>,
    pub graphs: Vec<GraphAst>,
}

/// A pattern declaration.
#[derive(Debug, Clone)]
pub struct PatternAst {
    pub name: String,
    pub stages: Vec<StageAst>,
    pub negations: Vec<NegationAst>,
    pub temporals: Vec<TemporalAst>,
}

/// A stage within a pattern.
#[derive(Debug, Clone)]
pub struct StageAst {
    pub anchor: String,
    pub clauses: Vec<ClauseAst>,
}

/// Whether a clause source is a variable reference or a literal node name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// `?var` — references a previously bound variable or the stage anchor.
    Var,
    /// `name` — a literal graph node name (or stage anchor, or negation scan root).
    Literal,
}

/// A single clause in a stage or negation body.
#[derive(Debug, Clone)]
pub struct ClauseAst {
    /// The source node identifier (without `?` prefix).
    pub source: String,
    /// Whether the source is a `?var` reference or a literal node name.
    pub source_kind: SourceKind,
    /// The edge label.
    pub label: String,
    /// What the clause matches against.
    pub target: ClauseTarget,
    /// Whether this clause is negated (prefixed with `!`).
    pub negated: bool,
}

/// The right-hand side of a clause.
#[derive(Debug, Clone)]
pub enum ClauseTarget {
    /// A literal string value: `= "foo"`
    LiteralStr(String),
    /// A literal number: `= 42` or `< 0.5`
    LiteralNum(f64),
    /// A literal boolean: `= true`
    LiteralBool(bool),
    /// Bind to a variable: `-> ?var`
    Bind(String),
    /// A node reference: `-> nodeName`
    NodeRef(String),
    /// A value constraint: `< 0.5`, `> 10`, `<= 100`, `>= 0`
    Constraint(ConstraintOp, ConstraintValue),
}

/// Constraint operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintOp {
    Lt,
    Gt,
    Lte,
    Gte,
}

/// Value in a constraint.
#[derive(Debug, Clone)]
pub enum ConstraintValue {
    Num(f64),
    Str(String),
}

/// A negation block.
#[derive(Debug, Clone)]
pub struct NegationAst {
    pub kind: NegationKind,
    pub clauses: Vec<ClauseAst>,
}

/// The type of negation.
#[derive(Debug, Clone)]
pub enum NegationKind {
    /// `unless between start end { ... }`
    Between(String, String),
    /// `unless after start { ... }`
    After(String),
    /// `unless { ... }` (global)
    Global,
}

/// An explicit temporal constraint.
#[derive(Debug, Clone)]
pub struct TemporalAst {
    pub left: String,
    pub relation: String,
    pub right: String,
}

/// A graph declaration.
#[derive(Debug, Clone)]
pub struct GraphAst {
    pub edges: Vec<EdgeAst>,
    pub now: Option<i64>,
}

/// A single edge in a graph declaration.
#[derive(Debug, Clone)]
pub struct EdgeAst {
    pub time_start: i64,
    /// If Some, this is a bounded interval [start, end).
    pub time_end: Option<i64>,
    pub source: String,
    pub label: String,
    pub target: EdgeTarget,
}

/// The target of an edge in a graph.
#[derive(Debug, Clone)]
pub enum EdgeTarget {
    /// String literal: `= "foo"`
    Str(String),
    /// Number literal: `= 42`
    Num(f64),
    /// Boolean: `= true`
    Bool(bool),
    /// Node reference: `-> alice`
    NodeRef(String),
}
