# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/patricker/fabula/compare/fabula-narratives-v0.1.0...fabula-narratives-v0.2.0) - 2026-05-17

### Added

- *(narratives)* assemble_signals_with_significance aggregator
- *(narratives)* score() consumes weighted FILO and resolution signals
- *(narratives)* SignificanceMap + weighted FILO/resolution fields
- *(narratives)* time_scale multiplier on NarrativeWeights
- *(narratives)* DistanceMetric trait + JSD/KL/Hellinger built-ins
- *(fabula-narratives)* weight advancements/completions by pattern importance

### Other

- *(narratives)* post-review polish for Tier 1 work
- *(narratives)* example for assemble_signals_with_significance
- *(narratives)* time_scale usage example
- *(narratives)* PivotDetector generic over DistanceMetric
- *(fabula)* replace em-dashes with -- in Rust sources for ASCII consistency
