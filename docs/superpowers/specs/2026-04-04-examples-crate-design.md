# Design: fabula-examples Crate, remark-code-region Plugin, and Naive Reader Updates

**Date**: 2026-04-04
**Status**: Draft

Every code sample in the documentation is extracted from a compiled, tested
source file. If the code changes, the docs update. If the code breaks, the
build breaks. Stale examples are structurally impossible.

---

## Part 1: fabula-examples Crate

### Purpose

A workspace crate containing every code sample used in the docs. Each file
compiles and runs as a test. The docs extract regions from these files via
a remark plugin. CI runs `cargo test -p fabula-examples` alongside the doc
build — if any example fails to compile, CI fails.

### Crate Structure

```
crates/fabula-examples/
├── Cargo.toml
├── src/
│   └── lib.rs                          # Shared test helpers (graph builders, etc.)
├── dsl/
│   ├── getting_started/
│   │   └── suspicious_login.fabula
│   ├── build/
│   │   ├── insider_trading.fabula
│   │   ├── repeated_manipulation.fabula
│   │   └── flash_crash.fabula
│   ├── cookbook/
│   │   ├── two_betrayals.fabula
│   │   ├── broken_promise.fabula
│   │   ├── low_loyalty.fabula
│   │   ├── sortie_during_siege.fabula
│   │   ├── unfulfilled_promise.fabula
│   │   ├── kept_promise.fabula
│   │   ├── escalating_price.fabula
│   │   ├── brute_force.fabula
│   │   └── multi_signal_shutdown.fabula
│   ├── reference/
│   │   └── violation_of_hospitality.fabula
│   └── use_cases/
│       ├── broken_promise.fabula
│       ├── cascade_timeout.fabula
│       ├── four_eyes_violation.fabula
│       ├── lateral_movement.fabula
│       ├── stuck_order.fabula
│       ├── resource_hoarding.fabula
│       └── ...
├── tests/
│   ├── getting_started.rs
│   ├── build_ch01.rs
│   ├── build_ch02.rs
│   ├── build_ch03.rs
│   ├── build_ch04.rs
│   ├── build_ch05.rs
│   ├── build_ch06.rs
│   ├── guides_cookbook.rs
│   ├── guides_composing.rs
│   ├── guides_dsl.rs
│   ├── guides_forking.rs
│   ├── guides_incremental.rs
│   ├── guides_custom_adapter.rs
│   ├── guides_debugging.rs
│   ├── guides_scoring.rs
│   ├── guides_performance.rs          # New: performance page examples
│   ├── reference_patterns.rs
│   ├── reference_scoring.rs
│   ├── reference_engine.rs
│   ├── reference_narratives.rs
│   ├── reference_interval.rs
│   ├── reference_dsl.rs
│   ├── concepts_overview.rs
│   ├── concepts_composition.rs
│   ├── concepts_scoring.rs
│   ├── concepts_narrative.rs
│   ├── use_cases_narrative.rs
│   ├── use_cases_observability.rs
│   ├── use_cases_compliance.rs
│   ├── use_cases_cybersecurity.rs
│   ├── use_cases_process_mining.rs
│   ├── use_cases_simulation.rs
│   └── validate_dsl.rs               # Glob-based DSL file validation
```

### Cargo.toml

```toml
[package]
name = "fabula-examples"
version = "0.1.0"
edition = "2021"
publish = false

[dev-dependencies]
fabula = { path = "../fabula" }
fabula-memory = { path = "../fabula-memory" }
fabula-petgraph = { path = "../fabula-petgraph" }
fabula-dsl = { path = "../fabula-dsl" }
fabula-narratives = { path = "../fabula-narratives" }
glob = "0.3"
```

No regular dependencies. Test-only crate. `src/lib.rs` exports shared
helper functions used across test files (e.g., graph builders that appear
in multiple examples).

### Region Marker Syntax

Rust files use `// #region name` and `// #endregion` comment markers:

```rust
use fabula::prelude::*;
use fabula_memory::*;

#[test]
fn suspicious_login_pattern() {
    let mut graph = MemGraph::new();
    // ... setup ...

    // #region build_pattern
    let pattern = PatternBuilder::<String, MemValue>::new("suspicious_login")
        .stage("e1", |s| s
            .edge("e1", "type".into(), MemValue::Str("login".into()))
            .edge_bind("e1", "user".into(), "user")
            .edge_bind("e1", "location".into(), "loc_a"))
        .stage("e2", |s| s
            .edge("e2", "type".into(), MemValue::Str("login".into()))
            .edge_bind("e2", "user".into(), "user")
            .edge_bind("e2", "location".into(), "loc_b"))
        .build();
    // #endregion

    // ... evaluate and assert ...
}
```

The doc shows only the region content. The surrounding boilerplate (imports,
test fn, setup, assertions) lives in the file but is invisible to readers.

Regions can nest. Regions in the same file can overlap if needed (a "full"
region containing smaller "step" regions for progressive disclosure).

### DSL Files

Plain text files with `.fabula` extension. One snippet per file. No region
markers needed — the entire file content is the snippet.

```
pattern suspicious_login {
  stage e1 {
    e1.type = "login"
    e1.user -> ?user
    e1.location -> ?loc_a
  }
  stage e2 {
    e2.type = "login"
    e2.user -> ?user
    e2.location -> ?loc_b
  }
}
```

If a DSL file needs multiple extractable sections (rare), it can use
`// #region` / `// #endregion` since `//` is a valid comment in the
fabula DSL.

### DSL Validation Test

A single test that globs all `.fabula` files and asserts each one parses
and compiles:

```rust
use fabula_dsl::compile;
use std::fs;

#[test]
fn all_dsl_examples_compile() {
    let pattern = format!(
        "{}/dsl/**/*.fabula",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut count = 0;
    for entry in glob::glob(&pattern).unwrap() {
        let path = entry.unwrap();
        let content = fs::read_to_string(&path).unwrap();
        compile(&content).unwrap_or_else(|e| {
            panic!("{}: {e}", path.display());
        });
        count += 1;
    }
    assert!(count > 0, "no .fabula files found");
}
```

### Handling Fragment Code Blocks

Many doc blocks are fragments — a PatternBuilder call without imports or
`fn main()`. The example file contains the full compilable context; the
region captures only the interesting part. The test proves it compiles.

For reference pages that show individual method signatures or return types,
these stay as inline code blocks in the docs. They're type-level
documentation, not runnable examples. The line between "extract" and
"leave inline" is:

- **Extract**: Any block that could be copy-pasted into a Rust file and
  (with imports) would compile. Builder calls, engine usage, scoring,
  graph setup, evaluation loops.
- **Leave inline**: Type definitions shown for reference (`pub struct
  Pattern<L, V>`), method signatures, trait bounds, pseudocode, shell
  commands, toml fragments, grammar notation.

Estimated split: ~180 extractable Rust blocks, ~40 DSL files, ~60 inline
(signatures, config, shell, grammar).

### PatternPlayground Components

~17 `<PatternPlayground>` instances across learn/, guides/, and playground/
pages contain DSL patterns and graph literals as JSX props. These are
validated at runtime by the WASM engine when the playground loads.

**Out of scope for this design.** Extracting JSX props to external files
requires a custom MDX transform that's significantly more complex than
code fence extraction. The WASM validation provides a safety net. This
can be addressed in a follow-up if drift becomes a problem.

---

## Part 2: remark-code-region Plugin

### Syntax

Code blocks with `reference` in the meta string are processed:

````md
```rust reference file=tests/getting_started.rs#build_pattern
```

```fabula reference file=dsl/cookbook/broken_promise.fabula
```
````

The plugin:
1. Finds code nodes with `reference` in meta
2. Parses `file=<path>[#<region>]` from meta
3. Reads the file from `crates/fabula-examples/`
4. If `#region` specified: extracts content between markers
5. If no region: uses entire file content
6. Replaces the code block's value with the extracted content
7. Strips `reference file=...` from meta (preserves other meta like
   `title="..."` or line highlighting)

### Implementation

TypeScript remark plugin at `docs/plugins/remark-code-region.ts`:

```typescript
import { visit } from 'unist-util-visit';
import * as fs from 'fs';
import * as path from 'path';

const EXAMPLES_ROOT = path.resolve(__dirname, '../../crates/fabula-examples');

function extractRegion(content: string, regionName: string): string {
  const startMarker = `// #region ${regionName}`;
  const endMarker = '// #endregion';

  const startIdx = content.indexOf(startMarker);
  if (startIdx === -1) {
    throw new Error(`Region "${regionName}" not found`);
  }

  const contentStart = content.indexOf('\n', startIdx) + 1;
  const endIdx = content.indexOf(endMarker, contentStart);
  if (endIdx === -1) {
    throw new Error(`No matching #endregion for "${regionName}"`);
  }

  const raw = content.slice(contentStart, endIdx);

  // Dedent: find minimum indentation and strip it
  const lines = raw.split('\n');
  const nonEmpty = lines.filter(l => l.trim().length > 0);
  const minIndent = nonEmpty.reduce((min, line) => {
    const indent = line.match(/^(\s*)/)?.[1].length ?? 0;
    return Math.min(min, indent);
  }, Infinity);
  const dedented = lines.map(l => l.slice(minIndent)).join('\n');

  return dedented.trim();
}

function readExample(filePath: string, regionName?: string): string {
  const fullPath = path.join(EXAMPLES_ROOT, filePath);
  if (!fs.existsSync(fullPath)) {
    throw new Error(
      `Example file not found: ${filePath}\n` +
      `  Resolved to: ${fullPath}`
    );
  }
  const content = fs.readFileSync(fullPath, 'utf-8');
  return regionName ? extractRegion(content, regionName) : content.trim();
}

export default function remarkCodeRegion() {
  return (tree: any) => {
    visit(tree, 'code', (node: any) => {
      const meta: string = node.meta || '';
      if (!meta.includes('reference')) return;

      const fileMatch = meta.match(/file=(\S+)/);
      if (!fileMatch) {
        throw new Error(
          `Code block has "reference" but no file= parameter:\n` +
          `  meta: ${meta}`
        );
      }

      const spec = fileMatch[1];
      const hashIdx = spec.indexOf('#');
      const filePath = hashIdx >= 0 ? spec.slice(0, hashIdx) : spec;
      const region = hashIdx >= 0 ? spec.slice(hashIdx + 1) : undefined;

      node.value = readExample(filePath, region);

      // Clean meta: remove "reference" and "file=..."
      node.meta = meta
        .replace(/\breference\b\s*/, '')
        .replace(/file=\S+\s*/, '')
        .trim() || null;
    });
  };
}
```

### Docusaurus Configuration

```typescript
// docusaurus.config.ts
import remarkCodeRegion from './plugins/remark-code-region';

const config: Config = {
  presets: [
    ['classic', {
      docs: {
        remarkPlugins: [remarkCodeRegion],
        // ... existing config
      },
    }],
  ],
};
```

### Error Behavior

The plugin throws on:
- Missing `file=` parameter when `reference` is present
- File not found
- Region not found in file

These are build-time errors. A missing or renamed region breaks the
Docusaurus build, which CI catches. This is the structural guarantee.

### Fallback Rendering

If the plugin is not loaded (e.g., someone reads the raw markdown on
GitHub), the code block appears empty. This is acceptable — the raw
markdown is not the primary reading surface.

---

## Part 3: Migration Map

### File-to-Test Mapping

Each test file backs one doc page. Region names match the concept being
demonstrated, not the doc heading.

| Doc Page | Test File | Regions (approx) | DSL Files |
|----------|-----------|-------------------|-----------|
| getting-started.md | getting_started.rs | graph_setup, build_pattern, batch_eval, incremental_eval, full_main | suspicious_login.fabula |
| build/01-simulation-loop.md | build_ch01.rs | trading_sim | — |
| build/02-define-patterns.md | build_ch02.rs | insider_pattern, repeated_pattern, flash_crash, full_code | insider_trading.fabula, repeated_manipulation.fabula, flash_crash.fabula |
| build/03-incremental-matching.md | build_ch03.rs | incremental_loop | — |
| build/04-react-to-events.md | build_ch04.rs | event_handling | — |
| build/05-score-and-rank.md | build_ch05.rs | surprise, stu, sequential, full_scoring | — |
| build/06-speculate-with-mcts.md | build_ch06.rs | fork_speculate | — |
| guides/pattern-cookbook.md | guides_cookbook.rs | recipe_1 through recipe_9 (pattern + graph + why_not per recipe) | 9 .fabula files |
| guides/dsl-in-rust.md | guides_dsl.rs | parse_single, parse_document, eval, typemapper, composable_parser | — |
| guides/forking-for-mcts.md | guides_forking.rs | simple_fork, clone_engine, fork_graph, speculate, score, compare, full_example | — |
| guides/incremental-integration.md | guides_incremental.rs | register, feed_loop, full_integration, narrative_scoring | — |
| guides/custom-adapter.md | guides_custom_adapter.rs | value_type, graph_struct, datasource_impl, smoke_test, testgraph_impl | — |
| guides/debugging-patterns.md | guides_debugging.rs | batch_check, gap_analysis, pm_inspection | — |
| guides/scoring-matches.md | guides_scoring.rs | surprise_scorer, stu_scorer, aggregation, pmi, sequential, combined | — |
| guides/performance.md (NEW) | guides_performance.rs | bench_config, profile_run | — |
| concepts/overview.md | concepts_overview.rs | pattern_builder, negation | — |
| concepts/composition.md | concepts_composition.rs | sequence_example, choice_example, repeat_example, output_bindings | — |
| concepts/scoring-and-surprise.md | concepts_scoring.rs | shannon, stu_example, pmi_example | — |
| concepts/narrative-quality.md | concepts_narrative.rs | weights_example, tuning_example | — |
| reference/patterns.md | reference_patterns.rs | pattern_struct, builder_methods, unordered_group | — |
| reference/scoring.md | reference_scoring.rs | surprise_api, stu_api, sequential_api, aggregation_modes | — |
| reference/engine.md | reference_engine.rs | engine_create, batch_eval, incremental, tick_delta, forking | — |
| reference/narratives.md | reference_narratives.rs | thread_tracker, tension_tracker, pivot, composite | — |
| reference/interval.md | reference_interval.rs | interval_create, allen_relations, gap_constraints | — |
| reference/dsl.md | reference_dsl.rs | compile_pattern, compile_document, typemapper | violation_of_hospitality.fabula |
| use-cases/narrative-sifting.md | use_cases_narrative.rs | hospitality, escalating, broken_promise_scored | 3 .fabula files |
| use-cases/observability.md | use_cases_observability.rs | cascade, retry_storm, sla_breach, incremental_integration | 3 .fabula files |
| use-cases/compliance-checking.md | use_cases_compliance.rs | access_revocation, four_eyes, unapproved_export, gap_analysis | 3 .fabula files |
| use-cases/cybersecurity.md | use_cases_cybersecurity.rs | lateral_movement, impossible_travel, credential_stuffing | 3 .fabula files |
| use-cases/process-mining.md | use_cases_process_mining.rs | skipped_approval, out_of_order, sla_timeout, batch_audit | 3 .fabula files |
| use-cases/simulation-monitoring.md | use_cases_simulation.rs | resource_hoarding, cascade, oscillation, incremental | 3 .fabula files |

### What Stays Inline

- Type/struct definitions in reference pages (shown for API documentation)
- Method signature lines (e.g., `pub fn evaluate(...)`)
- Grammar notation in reference/dsl.md
- Shell commands (`cargo test`, `cargo bench`, etc.)
- TOML fragments (Cargo.toml dependency lines)
- PatternPlayground JSX props (~17 instances)

---

## Part 4: Naive Reader Updates

### High Priority

**1. Performance and Benchmarks Page**

New file: `docs/docs/guides/performance.md`

Contents:
- Throughput: ~28us/edge (petgraph, GM-scale, 30 patterns, 5K edges)
- What "GM-scale" means: 30 patterns, 10 characters, 3 stages average,
  50% with negation, 10 edges/tick
- Scaling behavior: pattern count (1/10/30/100), edge count, stage depth
- Memory: partial match lifecycle, `drain_completed()` for GC
- Frame budget: 16ms at 60fps, budget for ~570 `on_edge_added` calls/frame
- How to benchmark your workload: `fabula-bench` configuration
- When petgraph vs MemGraph matters

Backed by: `guides_performance.rs` with benchmark config examples.

Sidebar position: after golden-tests.md in guides.

**2. DSL Quick Reference**

New file: `docs/docs/learn/dsl-quick-reference.md`

Pure syntax reference, one example per construct:
- `pattern name { }` — pattern declaration
- `stage name { }` — event slot
- `e1.label = "value"` — edge match
- `e1.label -> ?var` — variable binding
- `e1.label > ?var` — cross-stage comparison (5 operators)
- `unless between e1 e2 { }` — negation window (3 scopes)
- `temporal e1 before e2` — Allen constraint
- `temporal e1 before e2 gap 0..100` — metric gap
- `concurrent { }` — unordered group
- `compose name = a >> b` — sequence
- `compose name = a | b` — choice
- `compose name = a * 3` — exact repeat
- `compose name = a * 3..5 sharing(var)` — range repeat
- `meta("key", "value")` — metadata
- `deadline 30` — tick deadline

No semantics, no explanation of when to use what. Just syntax and one
example each. Links to full DSL reference for details.

Sidebar position: after interactive-tutorial in learn section.

Backed by: DSL files (one per construct) + `validate_dsl.rs`.

**3. Schema Mapping Guidance**

Add a "Mapping your data" section to each of the 6 use-case pages.
Each section shows how real-world event schemas map to fabula edges.

| Use Case | Real Schema | Mapping |
|----------|-------------|---------|
| Observability | OpenTelemetry span | span → edge, traceID → source node, service → label, duration → interval |
| Cybersecurity | Windows Event Log | EventID → label, Computer → source, TargetUserName → target, TimeCreated → interval start |
| Compliance | Transaction log | TransactionID → source, action → label, actor → target, timestamp → point interval |
| Process Mining | XES event log | case:concept:name → source, concept:name → label, org:resource → target, time:timestamp → interval |
| Narrative | Simulation event | eventID → source, eventType → label, actor → target, tick → interval |
| Simulation | Agent event | agentID → source, action → label, target → target, step → interval |

Each mapping is 5-10 lines showing the `DataSource` edge structure.
No new test files needed — these are structural guidance, not code.

**4. Build Chapter 4 Schedule Change Callout**

Add a Docusaurus admonition at the top of `build/04-react-to-events.md`,
after the frontmatter and title:

```md
:::caution Different event schedule
This chapter uses a different event schedule from chapters 1-3. The
patterns and graph events below are new — don't expect continuity with
the trading simulation from earlier chapters.
:::
```

### Medium Priority

**5. False Positive Discussion**

Add "Limitations and false positives" section to `cybersecurity.md` and
`observability.md`.

Cybersecurity:
- Lateral movement: credential rotation evades join on `?cred`
- Impossible travel: VPN/proxy creates false positives
- Credential stuffing: distributed attacks from multiple IPs
- Mitigation: combine with statistical baselines, use surprise scoring
  to rank matches by anomaly

Observability:
- Cascade timeout: retries that succeed but slowly can still match
- Retry storm: legitimate retry bursts during deployment
- SLA breach: clock skew between services
- Mitigation: metric gap constraints to tighten time windows, negation
  windows for expected maintenance events

**6. Tool Comparison Sections**

Add "How fabula compares" subsection to each use-case page. Brief, factual,
not adversarial. Focus on what each tool does well and where fabula adds
value.

| Use Case | Compare To |
|----------|------------|
| Observability | Datadog monitors, Jaeger TraceQL, Grafana alerting |
| Cybersecurity | Splunk correlation searches, Elastic EQL, Sigma rules |
| Process Mining | ProM, Disco/Celonis, Declare/LTLf |
| Compliance | SIEM correlation, manual audit scripts |
| Narrative | Felt (Kreminski 2019), Winnow (2021) |
| Simulation | Custom observer code, Flink CEP |

**7. Weight Tuning Guide**

Add "Tuning weights" section to `concepts/narrative-quality.md`:
- Combat-heavy game: increase tension weight, decrease thread balance
- Mystery/detective: increase pivot weight, increase thread completion
- Sandbox/emergent: increase surprise weight, decrease tension
- Before/after example showing how weight changes reorder candidates

Backed by: `concepts_narrative.rs#tuning_example` with code.

**8. Worked PMI Example**

Add PMI section to `playground/scoring-explorer.mdx`:
- Two properties that frequently co-occur (e.g., "warrior" + "aggressive")
- Individual frequencies, joint frequency, expected joint frequency
- PMI calculation: `log2(P(A,B) / P(A)P(B))`
- Before-correction StU score vs after-correction score
- Show that correlated properties get downweighted

### Low Priority

**9. Simpler Forking Intro Example**

Add a ~30-line example before the full example in `forking-for-mcts.md`:
- 1 pattern (simple betrayal)
- 2 candidate actions (betray vs reconcile)
- Fork, add edge, evaluate, compare match counts
- No narrative weights, no NarrativeScorer

Backed by: `guides_forking.rs#simple_fork` region.

**10. Composition Output Example**

Add to `concepts/composition.md` after the variable renaming explanation:
- Show a `sequence(a, b, sharing "char")` pattern
- Show the resulting match bindings: `a_e1`, `a_e2`, `b_e1`, `b_e2`, `char`
- Explain: prefixed names are scoped to each sub-pattern, shared names
  are unprefixed

Backed by: `concepts_composition.rs#output_bindings` region.

**11. Temporal Graph Definition**

Add one sentence to `learn/what-is-sifting.md`, after the first use of
"temporal graphs" (around line 64):

> A temporal graph is a set of edges where each edge has a time interval
> saying when the relationship held.

**12. Cold-Start Confidence Ceiling**

Add one sentence to `concepts/scoring-and-surprise.md`, in the cold-start
section (after the confidence table):

> Confidence asymptotically approaches 1.0 — after 100 observations it
> reaches ~0.99, effectively disabling attenuation.

---

## Part 5: CI Integration

### Workspace Cargo.toml

Add `crates/fabula-examples` to the workspace members list. The existing
`cargo test --workspace` command will automatically include it.

Grafeo is excluded from workspace builds (needs rustc 1.91.1). The
examples crate does not depend on grafeo.

### Build Order

1. `cargo test -p fabula-examples` — all Rust examples compile, all DSL
   files parse
2. `cd docs && npm run build` — Docusaurus build with remark-code-region
   plugin. Any missing file or region breaks the build.

Both gates run in CI. A stale example is caught by either (1) the Rust
test failing if the API changed, or (2) the doc build failing if the
region was renamed.

### Plugin Dependencies

The remark plugin uses only Node.js built-ins (`fs`, `path`) plus
`unist-util-visit` (already a transitive dependency of Docusaurus).
No new npm packages needed.

---

## Part 6: Execution Order

### Phase 1 — Infrastructure

1. Create `crates/fabula-examples/` with Cargo.toml, empty `src/lib.rs`
2. Add to workspace Cargo.toml
3. Create `docs/plugins/remark-code-region.ts`
4. Wire plugin into `docusaurus.config.ts`
5. Verify: `cargo test -p fabula-examples` passes (empty), `npm run build`
   passes with plugin loaded

### Phase 2 — Seed Examples (Prove the Pattern)

6. Migrate `getting-started.md` — create test file + DSL file, replace
   inline blocks with `reference` blocks
7. Migrate one build chapter (ch02 — has both Rust and DSL)
8. Migrate one guide (pattern-cookbook — highest block count)
9. Verify: both `cargo test` and `npm run build` pass, site renders
   correctly

### Phase 3 — Full Migration

10. Migrate remaining build chapters (ch01, ch03-ch06)
11. Migrate remaining guides (7 files)
12. Migrate reference pages (6 files, extractable blocks only)
13. Migrate concept pages (4 files)
14. Migrate use-case pages (6 files)
15. Create all DSL files for cookbook, use-cases, reference

### Phase 4 — Naive Reader Updates

16. Items 11, 12 — one-line fixes (temporal graph def, cold-start ceiling)
17. Item 4 — build ch4 callout
18. Items 9, 10 — simpler forking intro, composition output example
19. Item 2 — DSL quick reference page
20. Item 1 — performance page
21. Item 8 — worked PMI example in scoring-explorer
22. Item 7 — weight tuning section
23. Items 5, 6 — false positive discussion + tool comparisons (6 pages)
24. Item 3 — schema mapping sections (6 pages)

### Phase 5 — Verify

25. `cargo test --workspace` (excluding grafeo)
26. `cargo clippy --workspace -- -D warnings`
27. `cd docs && npm run build` — full Docusaurus build
28. Manual review: spot-check 5-10 pages for correct rendering
