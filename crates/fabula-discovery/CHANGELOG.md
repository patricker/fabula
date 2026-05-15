# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/patricker/fabula/compare/fabula-discovery-v0.1.0...fabula-discovery-v0.2.0) - 2026-05-15

### Added

- *(engine)* [**breaking**] replace V: ArithmeticValue with pluggable LetEvaluator
- *(fabula)* add Stage::let_bindings field
- *(fabula)* add Pattern.advance_in_place field and builder method
- *(fabula)* per-PM inactivity pruning with configurable threshold
- *(fabula-dsl)* add `importance N` directive for pattern weighting
- *(fabula)* add ValueConstraint::OneOf for value disjunction

### Fixed

- add advance_in_place to Pattern literal in discovery tests

### Other

- *(fabula)* replace em-dashes with -- in Rust sources for ASCII consistency
