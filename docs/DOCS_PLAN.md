# Fabula Documentation Plan

Refined IA based on Diataxis audit, user journey mapping, and API surface analysis.

## Current Problems

1. **"Concepts" folder is actually reference.** 4 of 5 concept pages are API catalogs with signatures, not explanations.
2. **No reference section.** Types and methods scattered across 6+ pages. A user looking up "what fields does PartialMatch have" has to guess which page.
3. **No how-to guides.** Zero task-oriented pages for common problems (defining complex patterns, wiring into a simulation, debugging failures).
4. **Tutorial doesn't teach.** Shows a code block with no scaffolding, no expected output, no narration of steps.
5. **Content duplication.** Hospitality example 4x, DataSource trait 3x.

## Target IA

```
docs/
├── getting-started.md              # TUTORIAL — rewrite from scratch
├── concepts/                       # EXPLANATION only
│   ├── overview.md                 # What is fabula, when to use it, core concepts map
│   ├── how-the-engine-works.md     # 4-phase incremental pipeline, forking, batch vs incremental
│   ├── temporal-model.md           # Why Allen algebra, implicit ordering, open-ended intervals
│   └── design-decisions.md         # Why no Datalog, why generic DataSource, why zero deps
├── guides/                         # HOW-TO only
│   ├── pattern-cookbook.md          # Worked pattern recipes for common scenarios
│   ├── incremental-integration.md  # Wire fabula into a simulation/event loop
│   ├── debugging-patterns.md       # Step-by-step troubleshooting for unmatched patterns
│   ├── custom-adapter.md           # Implement DataSource for your graph store
│   └── golden-tests.md            # Add scenarios to the test suite
├── reference/                      # REFERENCE only
│   ├── interval.md                 # Interval<T>, AllenRelation (all 13)
│   ├── datasource.md              # DataSource trait, Edge, ValueConstraint
│   ├── patterns.md                 # Pattern, Stage, Clause, Target, Var, Negation, builders
│   ├── engine.md                   # SiftEngine, Match, BoundValue, PartialMatch, SiftEvent, GapAnalysis
│   └── adapters/
│       ├── memory.md               # MemGraph, MemValue
│       ├── petgraph.md             # PetTemporalGraph, PetValue, TemporalEdge
│       └── grafeo.md               # GrafeoGraph, GrafeoValue
└── research.md                     # EXPLANATION — research lineage (keep as-is)
```

**19 pages total.** Down from 21 proposed by the journey agent — merged "What is Fabula" into concepts/overview, merged "Architecture" into design-decisions, merged Contributing guidance into golden-tests how-to.

## Page-by-Page Spec

### TUTORIAL (1 page)

**`getting-started.md`** — Rewrite. Current page is a code dump.

Type: Tutorial
Learning objective: "Build and evaluate a temporal graph pattern using fabula in under 10 minutes."
Template: TUTORIAL TEMPLATE from doc-writer skill.

Structure:
1. What you'll build (one sentence + what they'll see)
2. Prerequisites (Rust, cargo)
3. Step 1: Create project (`cargo new`, add deps)
4. Step 2: Build a graph (explain each `add_str`/`add_ref` call)
5. Step 3: Define a pattern (explain stages, variables, negation)
6. Step 4: Evaluate in batch (show output)
7. Step 5: Evaluate incrementally (show SiftEvent output)
8. Complete example
9. What you learned
10. Next steps → concepts/overview, guides/pattern-cookbook

Use a NON-hospitality example for variety (the hospitality example is in 4 other places). Use something like "detect login → access → logout with no unauthorized access between."

### EXPLANATION (4 pages)

**`concepts/overview.md`** — New.

What fabula is in plain English. When to use it (story sifting, simulation monitoring, process mining, compliance). When NOT to use it (OLAP, non-temporal graphs). The 5 core concepts in brief: edge, pattern/stage/clause, variable/join, interval, negation. A visual showing graph → pattern → match. Links to deeper pages.

**`concepts/how-the-engine-works.md`** — New (content extracted from current incremental.md and getting-started.md).

The 4-phase incremental algorithm. Forking behavior (why original PMs survive). Negation priority (Phase 1 before Phase 3). Batch evaluation cascade. Memory lifecycle of partial matches. When to use batch vs incremental. NO struct definitions, NO method signatures — those go to reference/engine.

**`concepts/temporal-model.md`** — Rewrite of current temporal.md.

Why Allen algebra instead of entity-ID ordering. The 13 relations as a conceptual model (not a reference table — the table goes to reference/interval). Implicit stage ordering. Open-ended intervals and their implications. When you'd use explicit `.temporal()` constraints (rare — say so). NO code signatures.

**`concepts/design-decisions.md`** — Rewrite of current design.md stub.

Expand the 5 one-sentence decisions into full explanations with tradeoffs:
- Sifting only, no action system (why, what you lose, how to add it yourself)
- Generic DataSource, not DataScript (why, performance implications)
- Allen intervals, not entity IDs (what you gain, what breaks — same-timestamp events)
- Direct graph traversal, not Datalog (what you lose — recursive rules)
- Zero-dep core (why, how adapters work)
Include: crate layout diagram, module overview for contributors.

### HOW-TO (5 pages)

**`guides/pattern-cookbook.md`** — New. Highest-impact new page.

Worked recipes, each with: problem → pattern definition → matching graph → non-matching graph → why_not output.
Recipes:
1. Repeated behavior by same actor (2-stage, variable join)
2. Violation with exception clause (negation between stages)
3. Numeric threshold trigger (edge_constrained with Lt/Gt)
4. Overlapping events (explicit Allen constraint)
5. Absence detection (single stage + unless_after)
6. Multi-clause negation body (negation only fires when ALL clauses match)

**`guides/incremental-integration.md`** — New.

Wire fabula into a simulation loop. Complete example: simulation produces edges → engine.on_edge_added → react to SiftEvent::Completed. Covers:
- When to call drain_completed vs inspect partial_matches
- Handling late-arriving / out-of-order edges
- Memory management (partial match accumulation)
- Performance characteristics

**`guides/debugging-patterns.md`** — New (replaces current gap-analysis.md).

Step-by-step troubleshooting workflow:
1. Call why_not, read the output
2. Common failure table (11 failure modes with causes and fixes)
3. Inspecting partial matches for in-flight debugging
4. Batch vs incremental disagreement diagnosis
5. Negation-specific debugging

**`guides/custom-adapter.md`** — Rewrite of current adapters/custom.md.

Same structure but add:
- Complete minimal working example (not just `todo!()`)
- Pitfalls section (scan returning target instead of source, _any_time returning empty, now() returning 0)
- Verification checklist after golden suite passes
- Remove placeholder links

**`guides/golden-tests.md`** — Rewrite of current testing/golden-suite.md.

Pure how-to: how to add a scenario (3 steps), how to run, how to debug failures. Move the coverage table and architecture description to a brief "About the suite" section at the end. Include contributing guidance (naming conventions, what makes a good scenario).

### REFERENCE (7 pages)

**`reference/interval.md`** — Allen relations table (all 13 with visual descriptions), Interval struct, all methods with signatures/params/returns.

**`reference/datasource.md`** — DataSource trait (4 associated types, 6 methods), Edge struct, ValueConstraint enum (7 variants + matches method). Table showing when each method is called by the engine.

**`reference/patterns.md`** — Pattern/Stage/Clause/Target/Var/TemporalConstraint/Negation structs. PatternBuilder/StageBuilder/NegationBuilder with all methods. One minimal example per builder method.

**`reference/engine.md`** — SiftEngine (all methods), Match, BoundValue, PartialMatch, MatchState, SiftEvent (all variants), GapAnalysis/StageAnalysis/StageStatus/ClauseAnalysis.

**`reference/adapters/memory.md`** — MemGraph struct, MemValue enum, all methods.

**`reference/adapters/petgraph.md`** — PetTemporalGraph, PetValue, TemporalEdge, NodeRef.

**`reference/adapters/grafeo.md`** — GrafeoGraph, GrafeoValue, storage notes.

### EXPLANATION (keep as-is)

**`research.md`** — Clean, single-type. Keep.

## Pages to DELETE

| Current Page | Disposition |
|---|---|
| `concepts/patterns.md` | Content moves to reference/patterns.md + concepts/overview.md |
| `concepts/incremental.md` | Content moves to concepts/how-the-engine-works.md + reference/engine.md |
| `concepts/negation.md` | Content moves to reference/patterns.md (negation API) + guides/pattern-cookbook.md (negation recipes) |
| `concepts/temporal.md` | Content moves to concepts/temporal-model.md (explanation) + reference/interval.md (Allen table) |
| `concepts/gap-analysis.md` | Replaced by guides/debugging-patterns.md |
| `adapters/overview.md` | Content moves to reference/datasource.md |
| `adapters/memory.md` | Moves to reference/adapters/memory.md |
| `adapters/petgraph.md` | Moves to reference/adapters/petgraph.md |
| `adapters/grafeo.md` | Moves to reference/adapters/grafeo.md |
| `adapters/custom.md` | Replaced by guides/custom-adapter.md |
| `testing/golden-suite.md` | Replaced by guides/golden-tests.md |
| `design.md` | Replaced by concepts/design-decisions.md |

## Sidebar

```ts
const sidebars = {
  docsSidebar: [
    'getting-started',
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/overview',
        'concepts/how-the-engine-works',
        'concepts/temporal-model',
        'concepts/design-decisions',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/pattern-cookbook',
        'guides/incremental-integration',
        'guides/debugging-patterns',
        'guides/custom-adapter',
        'guides/golden-tests',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'reference/interval',
        'reference/datasource',
        'reference/patterns',
        'reference/engine',
        {
          type: 'category',
          label: 'Adapters',
          items: [
            'reference/adapters/memory',
            'reference/adapters/petgraph',
            'reference/adapters/grafeo',
          ],
        },
      ],
    },
    'research',
  ],
};
```

## Execution Order

1. Create directory structure (`concepts/`, `guides/`, `reference/`, `reference/adapters/`)
2. Write reference pages first (they're the most mechanical — catalog from source)
3. Write concepts pages (extract from existing + new explanation content)
4. Write guides (new how-to content)
5. Rewrite getting-started tutorial
6. Update sidebar, docusaurus config, landing page
7. Delete old pages
8. Build site, verify no broken links
9. Commit

## Content Rules

- Every page has exactly one Diataxis type
- Active voice, present tense throughout
- Code examples are complete and runnable
- Reference pages: every param typed and described
- How-to pages: prerequisites, numbered steps, expected output
- Tutorial: scaffolding from `cargo new`, expected output at every step
- No placeholder links (`your-org/fabula`)
- Hospitality example used ONCE (in pattern-cookbook). Tutorial uses a different domain.
- DataSource trait shown ONCE (in reference/datasource). Everywhere else cross-references.
