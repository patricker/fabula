# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/patricker/fabula/compare/fabula-dsl-v0.1.0...fabula-dsl-v0.2.0) - 2026-05-02

### Added

- *(dsl)* parse and expand templates at compile time
- *(dsl)* add TemplateAst, InstantiateAst; extend DocumentAst and PatternAst
- *(dsl)* lex `template` and `instantiate` keywords
- *(engine)* [**breaking**] replace V: ArithmeticValue with pluggable LetEvaluator
- *(fabula-dsl)* compile lets to ComputedBinding with reference validation
- *(fabula-dsl)* parse 'let' statements with arithmetic expression grammar
- *(fabula-dsl)* recognize 'let' keyword and '/' operator
- *(fabula-dsl)* add ExprAst, LetAst, StageAst.let_bindings
- *(fabula-dsl)* parse advance_in_place pattern modifier
- *(fabula-dsl)* add `importance N` directive for pattern weighting
- *(fabula-dsl)* add `in [...]` syntax for value disjunction

### Fixed

- *(dsl)* two correctness bugs in template expansion
- *(docs,dsl)* review follow-ups for advance_in_place

### Other

- workspace-wide cargo fmt
- *(fabula)* replace em-dashes with -- in Rust sources for ASCII consistency
