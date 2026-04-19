//! Reverse compiler: `Pattern<String, String>` to fabula DSL text.
//!
//! Produces valid DSL that can be parsed by [`fabula_dsl::parse_document()`],
//! enabling discovered patterns to be serialised, shared, and edited.

use fabula::datasource::ValueConstraint;
use fabula::interval::AllenRelation;
use fabula::pattern::{Clause, MetricGap, Negation, Pattern, Stage, Target, TemporalConstraint};

/// Convert a `Pattern<String, String>` to fabula DSL text.
///
/// The output is valid fabula DSL that can be parsed by `fabula_dsl::parse_document()`.
/// This enables round-tripping: build a pattern programmatically, emit DSL, then
/// parse it back.
///
/// # Limitations
///
/// - Only handles `Pattern<String, String>` (not `MemValue` or custom types).
///   String values are emitted as quoted string literals.
/// - Composition metadata (group, repeat_range) is not emitted -- compose
///   directives are a higher-level concept not representable from a single pattern.
/// - Unordered groups are emitted as `concurrent { }` blocks.
/// - `ValueConstraint::Between(lo, hi)` is emitted as `>= lo`, dropping the
///   upper bound. The DSL has no range constraint syntax. Round-tripped patterns
///   will match a wider set of values than the original.
pub fn pattern_to_dsl(pattern: &Pattern<String, String>) -> String {
    let mut out = String::new();

    if pattern.private {
        out.push_str("private ");
    }

    out.push_str(&format!("pattern {} {{\n", pattern.name));

    // Emit stages, wrapping unordered groups in concurrent blocks
    let mut emitted: Vec<bool> = vec![false; pattern.stages.len()];

    for (idx, stage) in pattern.stages.iter().enumerate() {
        if emitted[idx] {
            continue;
        }

        // Check if this stage is the start of an unordered group
        if let Some(group) = pattern
            .unordered_groups
            .iter()
            .find(|g| g.first() == Some(&idx))
        {
            out.push_str("  concurrent {\n");
            for &gi in group {
                if let Some(s) = pattern.stages.get(gi) {
                    emit_stage(&mut out, s, "    ");
                    emitted[gi] = true;
                }
            }
            out.push_str("  }\n");
        } else {
            emit_stage(&mut out, stage, "  ");
            emitted[idx] = true;
        }
    }

    for negation in &pattern.negations {
        emit_negation(&mut out, negation);
    }

    for temporal in &pattern.temporal {
        emit_temporal(&mut out, temporal);
    }

    if let Some(ticks) = pattern.deadline_ticks {
        out.push_str(&format!("  deadline {}\n", ticks));
    }

    // Emit metadata in sorted order for deterministic output
    let mut meta_pairs: Vec<(&String, &String)> = pattern.metadata.iter().collect();
    meta_pairs.sort_by_key(|(k, _)| *k);
    for (key, value) in meta_pairs {
        out.push_str(&format!(
            "  meta({}, {})\n",
            quote_string(key),
            quote_string(value)
        ));
    }

    out.push_str("}\n");
    out
}

fn emit_stage(out: &mut String, stage: &Stage<String, String>, indent: &str) {
    out.push_str(&format!("{}stage {} {{\n", indent, stage.anchor.0));
    for clause in &stage.clauses {
        emit_clause(out, clause, indent);
    }
    out.push_str(&format!("{}}}\n", indent));
}

fn emit_clause(out: &mut String, clause: &Clause<String, String>, indent: &str) {
    let negation_prefix = if clause.negated { "! " } else { "" };

    let (target_op, target_str) = match &clause.target {
        Target::Bind(var) => ("-> ", format!("?{}", var.0)),
        Target::Literal(val) => ("= ", quote_string(val)),
        Target::Constraint(c) => emit_constraint(c),
    };

    let quoted_label = quote_string(&clause.label);

    out.push_str(&format!(
        "{}  {}{}.{} {}{}\n",
        indent, negation_prefix, clause.source.0, quoted_label, target_op, target_str
    ));
}

fn emit_constraint(c: &ValueConstraint<String>) -> (&'static str, String) {
    match c {
        ValueConstraint::Eq(v) => ("= ", quote_string(v)),
        ValueConstraint::Lt(v) => ("< ", quote_string(v)),
        ValueConstraint::Gt(v) => ("> ", quote_string(v)),
        ValueConstraint::Lte(v) => ("<= ", quote_string(v)),
        ValueConstraint::Gte(v) => (">= ", quote_string(v)),
        ValueConstraint::Between(lo, _hi) => {
            // Between is not directly representable in DSL syntax; emit >= lo as a
            // lossy but semantically closer approximation (drops the upper bound).
            (">= ", quote_string(lo))
        }
        ValueConstraint::Any => ("-> ", "?_any".to_string()),
        ValueConstraint::EqVar(v) => ("= ", format!("?{}", v)),
        ValueConstraint::LtVar(v) => ("< ", format!("?{}", v)),
        ValueConstraint::GtVar(v) => ("> ", format!("?{}", v)),
        ValueConstraint::LteVar(v) => ("<= ", format!("?{}", v)),
        ValueConstraint::GteVar(v) => (">= ", format!("?{}", v)),
        ValueConstraint::OneOf(vs) => {
            // OneOf is not yet representable in DSL syntax; emit as Eq on the
            // first element as a lossy approximation.
            if let Some(first) = vs.first() {
                ("= ", quote_string(first))
            } else {
                ("= ", quote_string(""))
            }
        }
    }
}

fn emit_temporal(out: &mut String, tc: &TemporalConstraint) {
    let relation_str = match tc.relation {
        AllenRelation::Before => "before",
        AllenRelation::After => "after",
        AllenRelation::Meets => "meets",
        AllenRelation::MetBy => "met_by",
        AllenRelation::Overlaps => "overlaps",
        AllenRelation::OverlappedBy => "overlapped_by",
        AllenRelation::Starts => "starts",
        AllenRelation::StartedBy => "started_by",
        AllenRelation::During => "during",
        AllenRelation::Contains => "contains",
        AllenRelation::Finishes => "finishes",
        AllenRelation::FinishedBy => "finished_by",
        AllenRelation::Equals => "equals",
    };

    out.push_str(&format!(
        "  temporal {} {} {}",
        tc.left.0, relation_str, tc.right.0
    ));

    if let Some(ref gap) = tc.gap {
        emit_gap(out, gap);
    }

    out.push('\n');
}

fn emit_gap(out: &mut String, gap: &MetricGap) {
    match (gap.min, gap.max) {
        (Some(min), Some(max)) => {
            out.push_str(&format!(" gap {}..{}", format_num(min), format_num(max)));
        }
        (Some(min), None) => {
            out.push_str(&format!(" gap {}..", format_num(min)));
        }
        (None, Some(max)) => {
            out.push_str(&format!(" gap ..{}", format_num(max)));
        }
        (None, None) => {
            // No gap constraints -- don't emit anything
        }
    }
}

/// Format a number, using integer format when possible.
fn format_num(n: f64) -> String {
    if n == n.floor() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// Quote a string for DSL emission. If the string contains a double-quote
/// character, use triple-quoted syntax (`"""..."""`) which the DSL lexer supports.
/// If the string contains `"""` (three consecutive double quotes), escape it by
/// inserting a space to break the sequence, since the DSL has no escape mechanism.
/// Otherwise, use simple double-quoted syntax.
fn quote_string(s: &str) -> String {
    if s.contains('"') {
        // Triple-quoted strings cannot contain """ -- the lexer would see it as the
        // closing delimiter. Replace any run of 3+ quotes with pairs separated by
        // a space so the content is slightly altered but the DSL remains valid.
        let safe = s.replace("\"\"\"", "\"\" \"");
        format!("\"\"\"{}\"\"\"", safe)
    } else {
        format!("\"{}\"", s)
    }
}

fn emit_negation(out: &mut String, neg: &Negation<String, String>) {
    if neg.is_global {
        out.push_str("  unless {\n");
    } else if let Some(ref end) = neg.between_end {
        out.push_str(&format!(
            "  unless between {} {} {{\n",
            neg.between_start.0, end.0
        ));
    } else {
        out.push_str(&format!("  unless after {} {{\n", neg.between_start.0));
    }

    for clause in &neg.clauses {
        emit_clause(out, clause, "  ");
    }

    out.push_str("  }\n");
}
