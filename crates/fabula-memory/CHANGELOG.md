# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/patricker/fabula/compare/fabula-memory-v0.1.0...fabula-memory-v0.1.1) - 2026-04-25

### Added

- ArithmeticValue impls for MemValue, PetValue, GrafeoValue
- *(fabula)* add DataSource::predecessors extension point
- *(fabula)* per-PM inactivity pruning with configurable threshold
- *(fabula)* add kill_pms_involving for bulk entity invalidation

### Other

- *(fabula)* replace em-dashes with -- in Rust sources for ASCII consistency
- *(fabula-memory)* event_causal_surprise integration tests
- *(fabula)* rename branches_skipped → divergent_branches; confidence uses min weight
- *(fabula-memory)* causal_paths against MemGraph with temporal + cycle scenarios
