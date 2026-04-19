//! Abstract syntax tree types for the fabula DSL.

/// Top-level document -- contains patterns, graphs, and compose directives
/// in declaration order (needed for name resolution).
#[derive(Debug, Clone)]
pub struct Document {
    pub items: Vec<DocumentItem>,
}

impl Document {
    /// All pattern ASTs in declaration order.
    pub fn patterns(&self) -> Vec<&PatternAst> {
        self.items
            .iter()
            .filter_map(|i| match i {
                DocumentItem::Pattern(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    /// All graph ASTs in declaration order.
    pub fn graphs(&self) -> Vec<&GraphAst> {
        self.items
            .iter()
            .filter_map(|i| match i {
                DocumentItem::Graph(g) => Some(g),
                _ => None,
            })
            .collect()
    }
}

/// A single item in a document.
#[derive(Debug, Clone)]
pub enum DocumentItem {
    Pattern(PatternAst),
    Graph(GraphAst),
    Compose(ComposeAst),
}

/// A compose directive -- builds a pattern from named sub-patterns.
#[derive(Debug, Clone)]
pub struct ComposeAst {
    pub name: String,
    pub body: ComposeBody,
}

/// The body of a compose directive.
#[derive(Debug, Clone)]
pub enum ComposeBody {
    /// `A >> B sharing(x, y)`
    Sequence {
        left: String,
        right: String,
        shared: Vec<String>,
    },
    /// `A | B | C` (exclusive choice by default, `nonexclusive` keyword opts out)
    Choice {
        alternatives: Vec<String>,
        exclusive: bool,
    },
    /// `A * 3 sharing(x, y)` (exact) or `A * 3..5 sharing(x, y)` (range)
    Repeat {
        pattern: String,
        min: usize,
        max: Option<usize>,
        shared: Vec<String>,
    },
}

/// A pattern declaration.
#[derive(Debug, Clone)]
pub struct PatternAst {
    pub name: String,
    pub stages: Vec<StageAst>,
    pub negations: Vec<NegationAst>,
    pub temporals: Vec<TemporalAst>,
    pub metadata: Vec<(String, String)>,
    pub deadline: Option<f64>,
    /// Unordered stage groups (concurrent blocks). Each entry contains the
    /// stage indices (into `stages`) that form a concurrent group.
    pub unordered_groups: Vec<Vec<usize>>,
    /// If true, this pattern was declared with `private` keyword.
    pub private: bool,
    /// Importance weight for narrative scoring. Defaults to 1.0.
    pub importance: f64,
    /// If true, this pattern was declared with `advance_in_place` keyword --
    /// the engine will consume the original PM after strict-forward advancement.
    pub advance_in_place: bool,
}

/// The interior of a pattern -- stages, negations, and temporal constraints,
/// without the `pattern name { }` wrapper.
///
/// Used by [`crate::parser::Parser::parse_pattern_body()`] for composable
/// parsing -- downstream DSLs can parse a pattern body embedded in their own
/// block syntax.
#[derive(Debug, Clone)]
pub struct PatternBody {
    pub stages: Vec<StageAst>,
    pub negations: Vec<NegationAst>,
    pub temporals: Vec<TemporalAst>,
    pub metadata: Vec<(String, String)>,
    pub deadline: Option<f64>,
    pub unordered_groups: Vec<Vec<usize>>,
    pub private: bool,
    pub importance: f64,
    pub advance_in_place: bool,
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
    /// `?var` -- references a previously bound variable or the stage anchor.
    Var,
    /// `name` -- a literal graph node name (or stage anchor, or negation scan root).
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
    /// A constraint comparing against a bound variable: `> ?var`, `= ?var`
    ConstraintVar(ConstraintOp, String),
    /// A value disjunction: `in ["attack", "betray"]`
    OneOf(Vec<ConstraintValue>),
}

/// Constraint operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintOp {
    Eq,
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
    /// Optional metric gap lower bound.
    pub gap_min: Option<f64>,
    /// Optional metric gap upper bound.
    pub gap_max: Option<f64>,
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
