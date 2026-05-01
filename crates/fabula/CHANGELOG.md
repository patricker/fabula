# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/patricker/fabula/compare/fabula-v0.1.0...fabula-v0.2.0) - 2026-05-01

### Added

- *(engine)* [**breaking**] replace V: ArithmeticValue with pluggable LetEvaluator
- *(engine)* add LetEvaluator trait + built-in evaluators
- *(fabula)* preserve and rename let_bindings through compose ops
- *(fabula)* evaluate Stage::let_bindings in batch + incremental
- *(fabula)* PatternBuilder.let_binding API
- *(fabula)* add Stage::let_bindings field
- *(fabula)* Expr<V>, BinOp, ArithmeticValue, ComputedBinding
- *(fabula)* consume original PM on strict-forward advance_in_place advancement
- *(fabula)* add Pattern.advance_in_place field and builder method
- *(fabula)* re-export event_causal_surprise from prelude
- *(fabula)* add event_causal_surprise and batch helper for contextual surprise
- *(fabula)* add DataSource::predecessors extension point
- *(fabula)* expose cleanliness_score and document scan cost
- *(fabula)* causal_paths BFS with temporal validation and cycle guard
- *(fabula)* add cleanliness_score for causal path quality
- *(fabula)* scaffold causality module with CausalPath struct
- *(fabula)* per-PM inactivity pruning with configurable threshold
- *(fabula)* add kill_pms_involving for bulk entity invalidation
- *(fabula-dsl)* add `importance N` directive for pattern weighting
- *(fabula)* add edge_one_of and not_edge_one_of to PatternBuilder
- *(fabula)* add ValueConstraint::OneOf for value disjunction

### Fixed

- *(fabula)* clear let names in repeat_range loop_bindings
- *(fabula)* per-site ArithmeticValue bound instead of DataSource supertrait
- *(fabula)* serde build + strengthen let_bindings map_types test
- *(fabula)* preserve advance_in_place through compose ops + add edge-case tests
- *(fabula)* emit causal paths at every depth, not just maximal chains

### Other

- post-refactor cleanup for downstream consumers
- document the LetEvaluator decoupling
- *(engine)* integration test for LetEvaluator over foreign V
- *(engine)* integration test for LetEvaluator over foreign V
- *(engine)* fix stale SiftEngineFor example after LetEvaluator refactor
- *(engine)* plumb evaluator through try_match_stage
- *(engine)* plumb evaluator through find_stage_matches
- *(engine)* route eval_stage_lets through LetEvaluator
- *(engine)* cover unbound-var path in DefaultLetEvaluator
- workspace-wide cargo fmt
- *(fabula)* replace em-dashes with -- in Rust sources for ASCII consistency
- *(fabula)* rename branches_skipped → divergent_branches; confidence uses min weight
