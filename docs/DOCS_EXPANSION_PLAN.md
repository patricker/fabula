# Documentation Expansion Plan v2

A documentation site that feels like a textbook you can run.

Making fabula THE learning place for sifting — not just narrative sifting, but temporal pattern matching across all domains. Every concept backed by running code. Every code sample pulled from a tested source file. Every non-trivial idea with an interactive demo you can manipulate in your browser.

Informed by four parallel audits:
- **Code-vs-docs gap audit**: 45 specific gaps (undocumented APIs, stale signatures, missing features)
- **Research content mining**: Insights from DESIGN.md, ROADMAP.md, FUTURE_PATHS.md, GOLDEN_TEST_SPEC.md
- **Competitive research**: Patterns from tokio.rs, react.dev, Stripe, Flink CEP, learn.svelte.dev, Regex101
- **User journey analysis**: Link graph, 4 persona walkthroughs, dead ends, orphan pages

---

## Site Design Principles

### 1. Code is the source of truth

No prose-only code blocks. Every snippet is extracted from a compiled, tested Rust file via `remark-code-region`. If the code changes, the docs update. If the code breaks, the build breaks. Stale examples are structurally impossible.

This requires a companion crate — `fabula-examples/` — containing every code sample used in the docs. Each example file compiles and runs as a test. The docs extract regions via remark plugin:

```rust
// examples/getting_started.rs
// #region suspicious_login_pattern
let pattern = PatternBuilder::<String, MemValue>::new("suspicious_login")
    .stage("login_a", |s| s
        .edge("login_a", "type".into(), MemValue::Str("login".into()))
        .edge_bind("login_a", "user".into(), "user")
        .edge_bind("login_a", "location".into(), "loc_a"))
    // ...full code, no ellipsis...
    .build();
// #endregion
```

```mdx
<!-- docs/getting-started.md -->
import CodeRegion from '@site/src/components/CodeRegion';

<CodeRegion file="examples/getting_started.rs" region="suspicious_login_pattern" />
```

**Build integration**: `cargo test -p fabula-examples` runs in CI alongside the doc build. If any example fails to compile, CI fails. Zero stale code is a structural guarantee, not a policy.

### 2. Interaction before explanation

When a concept can be demonstrated, demonstrate it first. Show the playground. Let the reader manipulate. *Then* explain what they just saw. Sliders and buttons teach faster than paragraphs.

On concept pages, the structure is:

1. One-sentence framing
2. **Interactive demo** (embedded compact playground or visualization)
3. Explanation of what the reader just saw
4. Deeper detail
5. Links to reference

This is the opposite of most technical docs (wall of text → code at the end). The demo comes first because building intuition through interaction is faster than reading.

### 3. Progressive disclosure

The site serves two audiences — people learning sifting for the first time and experienced practitioners evaluating the library. The structure goes shallow-to-deep: tutorials assume nothing, concepts assume curiosity, reference assumes competence. A reader never hits a wall of jargon without a link to the page that explains it.

### 4. Minimal, dense, precise

No filler. No "in this section we will discuss." Every sentence either teaches something or shows something. White space is generous. Text is tight. Code blocks are complete — never a `// ...` that hides the part you actually needed.

### 5. Workshop, not lecture hall

The site feels like a well-lit workbench — clean, purposeful, slightly technical. Not playful (no mascots, no emoji headers). Not corporate (no gradients, no stock illustrations). Think: a good O'Reilly book if it could run in your browser.

- Light mode: white background, near-black text, muted blue accents
- Dark mode: true dark (not grey), same accent palette
- Demos use color functionally: green for matched edges, red for negated, yellow for partial matches, blue for active stages. Color carries meaning — never decoration.
- Monospace for code (Prism-highlighted Rust). Clean sans-serif for prose (system font stack or Inter).
- Content column: 720px max. Demos are full-width within the column — they need room to breathe.
- Controls are simple: sliders, buttons, dropdowns. No custom widgets.

### 6. Demos are not decorations

Each interactive element exists because the concept it teaches is genuinely harder to understand without interaction. Dragging interval bars to see Allen relations change teaches more in 10 seconds than a table of 13 definitions. Stepping through edge arrivals and watching partial matches fork teaches the 4-phase algorithm viscerally.

The playground supports the docs, not the other way around. Reading path is primary. Demos are inline where they teach; the standalone playground pages are a bonus.

### 7. What this site is not

- Not a paper. We cite Kreminski, Nelson, Mateas, and Allen, but we don't reproduce proofs.
- Not a blog. No dates, no "updates," no opinions section. Content is evergreen or it doesn't exist.
- Not a marketing page. No testimonials, no "trusted by" logos. The code speaks.
- Not a playground-first app. You learn by reading and doing, not by clicking around.

### Success criteria

A reader who completes the tutorial sequence can use any feature of the library. A reader who visits a single concept page leaves understanding one thing deeply. A reader who opens the playground learns something without reading a word.

---

## Current State Assessment

### What's Working

- Clean Diataxis separation across 24 pages (executed from DOCS_PLAN.md)
- Interactive WASM playgrounds (pattern editor, step-through, Allen visualizer) — most libraries never get this far
- Well-scaffolded getting-started tutorial with expected output at every step
- Excellent research lineage page with correct paper citations
- Thorough design decisions with trade-off analysis
- Comprehensive DSL reference with compose syntax, TypeMapper, composable parser API
- 12 playground presets covering narrative arcs, negation, temporal constraints

### Five Big Gaps

**Gap 1: The site teaches "how to use fabula" but not "what is sifting."**

Someone who discovers this project doesn't know what sifting is. The overview jumps to "Fabula is a Rust library for..." — that's a product page, not a learning resource. THE learning place teaches the *idea* before the *tool*. FUTURE_PATHS.md has the perfect reframing: *"Stripping the narrative terminology, fabula provides: ordered subgraph template matching with variable joins."* That domain-agnostic framing should lead.

**Gap 2: It's narrative-sifting-centric. Non-narrative use cases are one-paragraph mentions.**

The overview lists 4 use cases but every tutorial, cookbook recipe, and playground preset is narrative-themed. FUTURE_PATHS.md identifies **distributed tracing/observability** as the strongest non-narrative fit ("practical value: HIGH"), yet the expansion plan v1 didn't even include it. Process mining engineers and security engineers see "betrayals and hospitality" and bounce.

**Gap 3: Recent features are undocumented. 45 specific code-vs-docs gaps.**

| Feature | Gaps | Severity |
|---------|------|----------|
| `SequentialScorer` (entire API) | 8 items | HIGH |
| `StuAggregation` enum + builders | 5 items | HIGH |
| `concurrent { }` DSL syntax | 5 items | HIGH |
| Unordered groups (Pattern/Builder/Engine) | 12 items | HIGH |
| `sequential_surprise` in narratives | 4 items | HIGH (broken `assemble_signals` signature) |
| Stale signatures (missing `T` parameter) | 4 items | MEDIUM |
| Cold-start attenuation, PMI correction | 2 behaviors | MEDIUM |
| `map_types()`, `temporal_with_gap()`, `MetricGap` | 8 items | MEDIUM |

**Gap 4: The playground is siloed, not woven into the learning flow.**

Out of 21 content pages, only 1 links to the playground (`reference/dsl.md`). The step-through debugger and Allen visualizer are orphans — reachable only via sidebar. react.dev and learn.svelte.dev prove that interactivity *embedded in learning content* is what makes documentation transformative. Fabula has extraordinary interactive tooling that nobody discovers.

**Gap 5: Broken links, dead ends, orphaned pages.**

- 7 placeholder URLs (`your-org`, `your-repo`) across config and content — GitHub buttons are 404s
- 9 orphaned reference pages (only reachable via sidebar)
- Pattern cookbook is a dead end (no outbound links after 8 recipes)
- research.md orphaned from content pages
- `guides/incremental-integration.md` has an incomplete code snippet (uses `tracker` variable never defined)
- Leftover Docusaurus scaffold files (`markdown-page.md`, dead `HomepageFeatures/index.tsx`)

---

## Strategic Insights from Competitive Research

### What separates "good library docs" from "THE resource"

| Pattern | Inspiration | Fabula Application |
|---------|-------------|-------------------|
| Multi-chapter project tutorial | Tokio mini-redis | "Build a Simulation Monitor" — 6 chapters building one real system |
| Embed playgrounds in learning content | react.dev (600+ sandboxes) | Inline `<PatternPlayground compact />` in concept/guide pages |
| Auto-generated pattern explanation | Regex101 | "Explain" panel that narrates each stage/variable/negation |
| Guided exercises with start/solved states | learn.svelte.dev | "Add an unless_between clause to prevent..." with Show Solution |
| Playground as primary landing CTA | Bevy's multiple entry points | "Try it now" → playground, "Build something" → tutorial, "Understand" → concepts |
| Combinator catalog | Flink CEP | Every builder method: description / code / matching graph / non-matching graph |
| State machine diagram | Esper | PartialMatch lifecycle: Active → Complete | Negated | Expired |
| Data model bridge | Serde | "Your simulation events → fabula graph edges" modeling patterns |
| Outcome-focused navigation | Stripe | "Detect a sequence" not "Pattern Cookbook", Rust/DSL side-by-side |
| Learning paths with time estimates | Microsoft Learn | Two tracks: "Narrative AI Developer" (~2hr), "Engine Integrator" (~90min) |
| Pattern vs. sequence distinction | Siddhi CEP | Callout: "Stages are patterns, not sequences — other events can occur between them" |

### The single biggest structural change

The playground should be **woven into the learning flow**, not siloed on dedicated pages. Every concept page should have an embedded compact playground demonstrating that concept. Every cookbook recipe should have "Try it" → pre-loaded playground. This is what react.dev and learn.svelte.dev do, and it's what makes people stay.

---

## Target Information Architecture

```
docs/
├── getting-started.md                    # TUTORIAL — keep (update links)
│
├── learn/                                # NEW — conceptual on-ramp + guided exercises
│   ├── what-is-sifting.md                # EXPLANATION — the idea itself, domain-agnostic
│   ├── sifting-by-example.md             # TUTORIAL — 4 domains, same concept, with playgrounds
│   ├── patterns-from-first-principles.md # EXPLANATION — stages, clauses, variables, joins
│   ├── thinking-in-time.md              # EXPLANATION — why intervals matter
│   └── interactive-tutorial.mdx          # TUTORIAL — guided exercises (svelte-style)
│
├── build/                                # NEW — multi-chapter project tutorial (tokio-style)
│   ├── overview.md                       # What you'll build, prerequisites
│   ├── 01-simulation-loop.md             # Set up event-producing simulation
│   ├── 02-define-patterns.md             # Define patterns with the builder + DSL
│   ├── 03-incremental-matching.md        # Wire engine into the loop
│   ├── 04-react-to-events.md             # Handle SiftEvents, drain matches
│   ├── 05-score-and-rank.md              # Surprise scoring, StU, sequential
│   └── 06-speculate-with-mcts.md         # Fork engine, speculate, score, select
│
├── use-cases/                            # NEW — domain-specific tutorials
│   ├── narrative-sifting.md              # Game/simulation narrative detection
│   ├── observability.md                  # Distributed tracing, root cause analysis
│   ├── process-mining.md                 # Business process compliance
│   ├── compliance-checking.md            # Forbidden sequence detection
│   ├── simulation-monitoring.md          # Emergent behavior in ABMs
│   └── cybersecurity.md                  # Threat detection, MITRE ATT&CK patterns
│
├── concepts/                             # EXPLANATION — fabula-specific
│   ├── overview.md                       # Update: new features, data model bridge
│   ├── how-the-engine-works.md           # Update: unordered groups, state machine diagram
│   ├── temporal-model.md                 # Update: embed Allen visualizer
│   ├── design-decisions.md              # Update: scoring section, fix dead links
│   ├── composition.md                    # NEW — why/how to compose patterns
│   ├── scoring-and-surprise.md           # NEW — information theory for mortals
│   └── narrative-quality.md              # NEW — threads, tension, pivots, MCTS
│
├── playground/                           # Keep existing 3 + add new
│   ├── pattern-playground.mdx            # Keep
│   ├── step-through.mdx                 # Update: add outbound links
│   ├── allen-visualizer.mdx             # Update: add outbound links
│   └── scoring-explorer.mdx              # NEW — experiment with surprise scoring
│
├── guides/                               # HOW-TO — keep existing + add
│   ├── pattern-cookbook.md               # Update: add concurrent/compose recipes, playground links, next steps
│   ├── incremental-integration.md       # Update: fix incomplete code snippet
│   ├── debugging-patterns.md            # Update: link to step-through playground, add next steps
│   ├── custom-adapter.md               # Keep
│   ├── golden-tests.md                  # Keep
│   ├── scoring-matches.md               # NEW — full observe → score → rank workflow
│   ├── composing-patterns.md            # NEW — sequence, choice, repeat recipes
│   ├── forking-for-mcts.md             # NEW — clone engine, speculate, discard
│   └── dsl-in-rust.md                   # NEW — parse → compile → evaluate workflow
│
├── reference/                            # Update stale pages + fix orphans
│   ├── interval.md                       # Update: link to Allen visualizer
│   ├── datasource.md                    # Update: add intro paragraph linking to custom-adapter
│   ├── patterns.md                      # UPDATE: unordered_groups, map_types, MetricGap
│   ├── engine.md                        # UPDATE: matched_stages, unordered behavior
│   ├── scoring.md                       # UPDATE: StuAggregation, PMI, confidence, SequentialScorer
│   ├── narratives.md                    # UPDATE: sequential_surprise, assemble_signals (8 params)
│   ├── dsl.md                           # UPDATE: concurrent { }, Concurrent token
│   └── adapters/                        # Update: add intro paragraphs
│       ├── memory.md
│       ├── petgraph.md
│       └── grafeo.md
│
├── glossary.md                           # NEW — sifting terminology reference
├── research.md                           # Keep (add outbound links to concepts)
└── learning-paths.md                     # NEW — two tracks with time estimates
```

**~20 new pages, ~14 updates to existing pages, ~7 infrastructure fixes.**

---

## The Learning Funnel

```
Discover                    "What is this?"
  │
  ├─→ Try it now            → Playground (zero install, immediate gratification)
  ├─→ Learn the idea        → learn/what-is-sifting.md
  └─→ Build something       → getting-started.md
          │
          ▼
Explore                     "Does this apply to my domain?"
  │
  ├─→ use-cases/            → narrative, observability, process mining, security...
  └─→ learn/sifting-by-example → same pattern, 4 domains
          │
          ▼
Build                       "I want to build a real system."
  │
  ├─→ build/ (6 chapters)   → project tutorial: simulation monitor
  └─→ guides/               → task-specific how-tos
          │
          ▼
Master                      "I need to look up / understand deeply."
  │
  ├─→ concepts/             → engine internals, scoring theory, composition
  ├─→ reference/            → complete API documentation
  └─→ research.md           → academic foundations
```

---

## Detailed Page Specifications

### NEW: learn/what-is-sifting.md — EXPLANATION (Keystone Page)

**THE most important new page.** Teaches the concept of sifting independent of fabula.

Opens with an embedded playground — a pre-loaded compliance pattern (access after revocation) that the reader can run immediately. No preamble. They see a pattern, a graph, and a match. Then the text explains what they just saw.

Content outline:
1. **Demo first**: Embedded playground with a 3-stage compliance pattern. The reader clicks "Evaluate" and sees a match. They haven't read a word of explanation yet, but they've seen sifting work.
2. **What just happened**: One paragraph explaining the demo — "You defined a sequence of events with constraints. The engine found every instance in the data. That's sifting."
3. **What sifting is**: Ordered subgraph template matching with variable joins over temporal data. The regex analogy: "A regular expression engine for event sequences in temporal graphs."
4. **Four domains where sifting applies** — same demo pattern reframed:
   - Narrative detection: "Two betrayals by the same character with no reconciliation"
   - Distributed tracing: "DB timeout after service restart with no recovery event"
   - Compliance: "Access after revocation with no re-authorization" (the demo they just ran)
   - Process mining: "Order placed, shipped, never confirmed"
5. **The key insight about negation**: "Expected event that never happened is THE core detection signal across security, clinical, compliance, and observability."
6. **What no existing system combines** (from FUTURE_PATHS.md):
   - Graph + Allen intervals + negation windows
   - `why_not()` clause-level gap analysis (no CEP, SIEM, or process mining tool has this)
   - Clone-speculate-discard (unique to fabula's decoupled architecture)
7. **Where to go next**: sifting-by-example, getting-started, research

**Learning objective:** Understand what sifting is and identify at least one problem in your own domain where it applies.

### NEW: learn/sifting-by-example.md — TUTORIAL

Four short worked examples showing the same sifting concept across domains. Each example includes an **embedded compact playground**.

1. **Narrative**: "Two betrayals by the same character with no reconciliation"
2. **Observability**: "DB timeout cascade — service A calls B, B calls C, C times out, no recovery"
3. **Compliance**: "Access after credential revocation with no re-authorization"
4. **Process mining**: "Order placed, shipped, but never delivery-confirmed"

Each example: problem (2 sentences) → DSL pattern (5 lines) → graph (5 edges) → embedded playground → result explanation. Link each to the full use-case tutorial.

**Learning objective:** Read and write a basic sifting pattern in the DSL, and apply the pattern concept to a non-narrative domain.

### NEW: learn/interactive-tutorial.mdx — TUTORIAL (Svelte-style)

5 guided exercises with embedded playgrounds, each with a starting state and a "Show Solution" button.

1. **Your First Pattern** — Given a graph, write a 1-stage pattern to find all "login" events
2. **Joining Variables** — Extend to 2 stages, same user logs in twice (variable join)
3. **Negation Windows** — Add `unless between` to exclude cases where the user logged out
4. **Value Constraints** — Add `e.severity > 3` to filter by numeric threshold
5. **Temporal Relations** — Use explicit Allen constraint: find events that happen *during* a siege

Each exercise: instruction text + pre-loaded playground with incomplete pattern + "Show Solution" loads the solved preset.

**Learning objective:** Write patterns with stages, variables, negation, constraints, and temporal relations by completing guided exercises.

### NEW: build/ — Multi-Chapter Project Tutorial (Tokio-style)

**"Build a Simulation Monitor with Fabula"** — THE investment that transforms "good docs" into "THE resource."

Each chapter teaches one major concept through building a real system:

**`build/overview.md`**: What you'll build (a simulation that generates events + a monitor that detects patterns and ranks them by surprise). Prerequisites. Architecture diagram.

**`build/01-simulation-loop.md`**: Create a simple simulation that generates timestamped events (agent actions in a trading simulation). Build the `MemGraph`, generate random events, print them. Expected output at each step.

**`build/02-define-patterns.md`**: Define 3 patterns: insider trading (sequence + negation), market manipulation (repeat-range), flash crash (concurrent group). Use both builder API and DSL. Show the Rust/DSL equivalence side by side.

**`build/03-incremental-matching.md`**: Wire the engine into the simulation loop. `on_edge_added()` for each event, `end_tick()` per round. React to `SiftEvent::Completed` and `SiftEvent::Negated`. Print matches as they occur.

**`build/04-react-to-events.md`**: Build a handler that logs matches, tracks statistics, and uses `why_not()` to explain near-misses. `drain_completed()` for memory management. Deadline expiry for stale partial matches.

**`build/05-score-and-rank.md`**: Add `SurpriseScorer` and `StuScorer`. Extract properties from matches. Observe, score, rank. Show how the same pattern produces different surprise scores based on the rarity of its properties. Demonstrate `StuAggregation` modes and `SequentialScorer`.

**`build/06-speculate-with-mcts.md`**: Clone the engine. Fork the graph. Add hypothetical events. Score with `NarrativeScore`. Compare candidates. Select the best action. Discard the fork. Show how this enables a simple "AI director."

Each chapter ends with: complete code for that chapter, expected output, "What you learned", link to next chapter.

### NEW: use-cases/observability.md — TUTORIAL

**The #1 missing use case** per FUTURE_PATHS.md analysis.

Uses the "DB cascade failure" pattern from FUTURE_PATHS.md:
- Service A calls B, B calls C, C returns timeout
- No recovery event (retry success) within 5 ticks
- Variable join on `caller` traces the call chain

Covers: modeling distributed traces as temporal graphs, detecting cascade failures, using gap constraints for SLA thresholds, `why_not()` for near-miss analysis, incremental mode for real-time monitoring.

**Learning objective:** Model distributed service traces as temporal graph patterns and detect cascade failures in real time using fabula's incremental engine.

### NEW: use-cases/cybersecurity.md — TUTORIAL

Uses patterns from FUTURE_PATHS.md:
- Lateral movement with `repeat` (attacker moves through 3+ hosts)
- C2 beaconing with metric gap constraints (periodic callbacks)
- Impossible travel with negation (login from two locations, no VPN event)

Frame with MITRE ATT&CK terminology where applicable.

**Learning objective:** Define threat detection patterns using temporal graph sifting and identify multi-stage attack sequences that span hosts and time windows.

### NEW: concepts/scoring-and-surprise.md — EXPLANATION

Information theory for people who don't read information theory. Opens with an interactive scoring demo — two matches of the same pattern, one with common properties, one with rare properties. A slider adjusts observation count. The reader sees scores change in real time. Then the text explains what they're seeing.

Content outline:
1. **Demo first**: Embedded scoring explorer showing two "betrayal" matches — one by a common faction, one by a rare faction. Slider for observation count shows cold-start effects.
2. **The ranking problem**: "find all matches" returns too many results. Not all matches are interesting.
3. **Shannon surprise**: `-log2(p)` — "how many bits of information does this carry?" Common events carry few bits; rare events carry many.
3. **Pattern-level scoring** (`SurpriseScorer`): How often does this pattern fire vs. baseline?
4. **Property-level scoring** (`StuScorer`): Two matches of the same pattern can have different surprise if one involves rarer attributes. The StU insight from Kreminski et al. (ICIDS 2022).
5. **Aggregation modes**: Why multiple modes exist and when to use each:
   - ArithmeticMean (default): "average rarity of properties"
   - TfIdf: "total information content" — higher = more surprising (reversed polarity!)
   - GeometricMean: "multiplicative rarity" — sensitive to individual rare properties
   - Min: "bottleneck rarity" — only the rarest property matters
6. **Cold-start problem**: With few observations, all properties are "rare." Confidence weighting: `1 - 1/(total + 1)` lerps toward "unsurprising" until enough data accumulates.
7. **Correlated properties (PMI)**: "Ambitious king" isn't surprising because "ambitious" and "king" are individually rare — it's surprising because they rarely *co-occur*. `PMI(pi, pj) = log2(P(pi,pj) / P(pi)*P(pj))`. When PMI > 1 bit, replace the less-rare member's frequency.
8. **Sequential surprise**: Pattern A completed, then pattern B completed. How unexpected is that transition? Bigram model: `P(B|A)` from observed frequencies, scored as `-log2(P(B|A))`.
9. **Connection to research**: Kreminski et al. (ICIDS 2022), Shannon information theory.

No API reference on this page — link to `reference/scoring.md` for method signatures.

### NEW: concepts/how-the-engine-works.md updates

Add:
1. **PartialMatch lifecycle state machine diagram** (from Esper competitive insight):
   ```
   [Created] → [Active] → [Complete]
                    ↓          
                [Negated]    
                    ↓
                [Expired] (via end_tick + deadline)
   ```
2. **Winnow 7-step walkthrough** (from GOLDEN_TEST_SPEC.md Appendix C) — concrete step-by-step trace showing the pool of partial matches evolving as events arrive
3. **Unordered group evaluation**: How Phase 2 tries all group stages as initiators, how Phase 3 advances any unmatched stage in the group, how temporal ordering is relaxed within groups
4. **Pattern vs. sequence callout** (from Siddhi competitive insight): "Fabula stages are like CEP *patterns*, not *sequences*: other events can occur between matched stages. If you need contiguity, use a negation window."

### NEW: glossary.md — REFERENCE

Alphabetical glossary of sifting terminology. Key terms:

- **Advancement** — when a partial match progresses to its next stage
- **Allen relation** — one of 13 temporal relationships between two intervals
- **Binding** — a variable-to-value assignment within a match
- **Clause** — a single constraint within a stage (edge match, value constraint, or binding)
- **Cold-start** — insufficient observation data for reliable surprise scoring
- **Completion** — when all stages of a pattern have been matched
- **Concurrent group** — stages that can match in any order (unordered)
- **DataSource** — the trait abstracting graph storage backends
- **Edge** — the atomic unit: source node, label, target, time interval
- **Forking** — cloning the engine for speculative evaluation (MCTS)
- **Gap analysis** — clause-by-clause diagnosis of why a pattern didn't match (`why_not`)
- **Join** — when a variable appears in multiple stages, forcing the same entity
- **Match** — a complete set of bindings satisfying all stages and constraints
- **Negation window** — a temporal range where specified events must NOT occur
- **Nugget** — (Felt terminology) a set of variable bindings representing an interesting event sequence
- **Partial match** — an in-progress match with some stages satisfied
- **Pattern** — a named template describing a sequence of events to find
- **Plant/payoff** — Chekhov's gun: a setup event that should eventually resolve
- **PMI** — Pointwise Mutual Information, measures property co-occurrence surprise
- **Sifting** — automatically identifying interesting event sequences in temporal data
- **Stage** — an ordered event slot within a pattern
- **StU** — "Select the Unexpected" — scoring heuristic ranking matches by property rarity
- **Tick** — one logical time step in incremental evaluation
- **Variable** — a named placeholder that binds to nodes or values during matching

### NEW: learning-paths.md — REFERENCE

Two structured tracks with time estimates (Microsoft Learn style):

**Track 1: "Narrative AI Developer"** (~2.5 hours)
1. Getting Started (10 min)
2. What is Sifting (10 min)
3. Patterns from First Principles (15 min)
4. Interactive Tutorial — exercises 1-5 (30 min)
5. Pattern Cookbook (20 min)
6. Scoring and Surprise concepts (15 min)
7. Build chapters 1-4 (45 min)
8. Narrative Quality concepts (15 min)
9. Build chapters 5-6 (20 min)

**Track 2: "Engine Integrator"** (~90 min)
1. Getting Started (10 min)
2. Sifting by Example (10 min)
3. How the Engine Works (15 min)
4. Incremental Integration guide (15 min)
5. DSL in Rust guide (10 min)
6. Debugging Patterns (10 min)
7. Custom Adapter (15 min)
8. Design Decisions (10 min)

---

## Reference Page Updates (Code-vs-Docs Gap Closure)

### reference/scoring.md — 20 items to add

**Add `SequentialScorer` section:**
- `SequentialScorer::new() -> Self`
- `observe_transition(&mut self, prev: &str, current: &str)`
- `transition_probability(&self, prev: &str, current: &str) -> Option<f64>` (Laplace-smoothed)
- `score_transition(&self, prev: &str, current: &str) -> f64` (-log2, 0.0 for unseen)
- `total_transitions_from(&self, prev: &str) -> u64`
- `vocabulary_size(&self, prev: &str) -> usize`
- `reset(&mut self)`

**Add `StuAggregation` section:**
- Enum with 4 variants: `ArithmeticMean` (default), `TfIdf`, `GeometricMean`, `Min`
- Polarity note: ArithmeticMean/GeometricMean/Min = lower is more surprising; TfIdf = higher is more surprising

**Add to `StuScorer` section:**
- `with_aggregation(self, aggregation: StuAggregation) -> Self`
- `with_pmi_correction(self) -> Self`
- `pmi_for(&self, pattern: &str, pi: &str, pj: &str) -> Option<f64>`
- Cold-start confidence weighting behavior description
- PMI correction behavior description

**Fix stale signatures** — add `T` time parameter to:
- `StuScorer::score<N, V, T>(...)` 
- `SurpriseScorer::observe<N, V, T, L, VV>(...)`
- `SurpriseScorer::score<N, V, T, L, VV>(...)`
- `SurpriseScorer::observe_events<N, V, T, L, VV>(...)`

**Fix `StuScoredMatch` description** — polarity depends on aggregation mode, not always "lower = more surprising"

### reference/narratives.md — 4 items to fix

- Add `sequential_surprise: f64` to `NarrativeSignals`
- Add `sequential_surprise_reward: f64` to `NarrativeWeights` (default: 1.0)
- Add `sequential_surprise: f64` to `ScoreBreakdown`
- Fix `assemble_signals` signature: add 8th parameter `sequential_surprise: f64`

### reference/dsl.md — 5 items to add

- `concurrent { stage ... stage ... }` block syntax in pattern grammar
- `Concurrent` token in lexer token reference keywords list
- `unordered_groups: Vec<Vec<usize>>` field on `PatternBody` struct
- `unordered_groups: Vec<Vec<usize>>` field on `PatternAst` struct  
- Compiler validation: `unless_between` rejects both anchors in the same concurrent group
- Example showing concurrent group with shared bindings

### reference/patterns.md — 10 items to add

- `unordered_groups: Vec<Vec<usize>>` field on `Pattern`
- `unordered_group_for(&self, stage_idx: usize) -> Option<&Vec<usize>>`
- `same_unordered_group(&self, a: usize, b: usize) -> bool`
- `condition_count(&self) -> usize`
- `map_types<L2, V2>(&self, label_fn, value_fn) -> Pattern<L2, V2>`
- `Clause::map_types()`, `Stage::map_types()`, `Negation::map_types()`, `Target::map()`
- `PatternBuilder::unordered_group()` method
- `UnorderedGroupBuilder` struct and `stage()` method
- `MetricGap` struct definition (min, max fields)
- `PatternBuilder::temporal_with_gap()` method

### reference/engine.md — 3 items to add

- `matched_stages: u64` bitmask on `PartialMatch`
- Phase 2 unordered group behavior (multi-stage initiation)
- Phase 3 unordered group behavior (any-order advancement, relaxed temporal checks)

---

## Infrastructure Fixes

### Dead links — 7 placeholder URLs to fix

| File | Line | Current | Action |
|------|------|---------|--------|
| `docusaurus.config.ts` | 15 | `https://your-org.github.io` | Replace with actual URL |
| `docusaurus.config.ts` | 17 | `organizationName: 'your-org'` | Replace with actual org |
| `docusaurus.config.ts` | 34 | `https://github.com/your-org/fabula/tree/main/docs/` | Fix edit URL |
| `docusaurus.config.ts` | 68 | `https://github.com/your-org/fabula` | Fix navbar GitHub link |
| `docusaurus.config.ts` | 88 | `https://github.com/your-org/fabula` | Fix footer GitHub link |
| `src/pages/index.tsx` | 26 | `https://github.com/your-org/fabula` | Fix homepage GitHub button |
| `docs/concepts/design-decisions.md` | 140 | `https://github.com/your-repo/fabula/blob/main/DESIGN.md` | Fix (uses "your-repo", not even "your-org") |

### Dead ends — add "Next steps" sections

| Page | Add outbound links to |
|------|----------------------|
| `guides/pattern-cookbook.md` | playground, incremental-integration, scoring-matches |
| `guides/debugging-patterns.md` | playground/step-through, pattern-cookbook |
| `guides/golden-tests.md` | custom-adapter, playground |
| `playground/step-through.mdx` | how-the-engine-works, debugging-patterns |
| `playground/allen-visualizer.mdx` | temporal-model, reference/interval |

### Orphan rescue — add inbound links from content pages

| Orphaned Page | Add links from |
|---------------|---------------|
| `reference/datasource` | concepts/overview, custom-adapter guide |
| `reference/patterns` | getting-started (next steps), pattern-cookbook |
| `reference/engine` | how-the-engine-works, incremental-integration |
| `reference/adapters/*` | custom-adapter guide, design-decisions |
| `research.md` | concepts/overview, design-decisions, what-is-sifting |
| `playground/step-through` | how-the-engine-works, debugging-patterns, getting-started |
| `playground/allen-visualizer` | temporal-model, reference/interval |

### Cleanup

- Delete `src/pages/markdown-page.md` (Docusaurus scaffold leftover)
- Delete or update `src/components/HomepageFeatures/index.tsx` (dead code, references Docusaurus SVGs)
- Fix incomplete code snippet in `guides/incremental-integration.md` lines 265-277 (`tracker` undefined)
- Add `SiftEngineFor` alias explanation to getting-started or concepts/overview (currently only in reference/engine but used in guides)

### Playground integration — embed in learning flow

| Content Page | Playground Integration |
|-------------|----------------------|
| `getting-started.md` | After Step 3: "Try modifying this pattern in the playground" callout |
| `concepts/overview.md` | Embed compact playground with broken_promise pattern |
| `concepts/temporal-model.md` | Link to Allen visualizer + embed "During" pattern playground |
| `concepts/how-the-engine-works.md` | Link to step-through debugger |
| `guides/pattern-cookbook.md` | "Try it" link for each recipe |
| `guides/debugging-patterns.md` | Link to step-through debugger |
| `reference/interval.md` | Link to Allen visualizer |
| `reference/dsl.md` | Already has playground link (keep) |

---

## Homepage Redesign

Current: Title + tagline + one "Get Started" button + 6 feature cards. Placeholder GitHub URL. No playground link.

Target: Workshop aesthetic. Minimal. Three entry points. Code snippet on the fold. No gradients, no stock illustrations.

```
┌─────────────────────────────────────────────────┐
│                                                  │
│  fabula                                          │
│                                                  │
│  Pattern matching over temporal graphs.          │
│                                                  │
│  pattern breach {                                │
│    stage e1 { e1.type = "revoke" ... }           │
│    stage e2 { e2.type = "access" ... }           │
│    unless between e1 e2 { ... "reauth" ... }     │
│  }                                               │
│  // 1 match: access after revocation,            │
│  // no re-authorization between                  │
│                                                  │
│  [Try it]  [Get Started]  [Learn Sifting]        │
│                                                  │
├─────────────────────────────────────────────────┤
│                                                  │
│  Narrative    Observability    Process Mining     │
│  Sifting      & Tracing        & Compliance      │
│                                                  │
│  Cyber-       Simulation       Game AI           │
│  security     Monitoring       & MCTS            │
│                                                  │
├─────────────────────────────────────────────────┤
│                                                  │
│  Zero dependencies. Incremental matching.        │
│  Allen interval algebra. Negation windows.       │
│  Gap analysis. Surprise scoring. Pattern         │
│  composition. DSL with TypeMapper.               │
│  Bring your own graph.                           │
│                                                  │
│  Built on: Felt (ICIDS 2019), Winnow (AIIDE     │
│  2021), StU (ICIDS 2022), Allen (CACM 1983)     │
│                                                  │
│  cargo add fabula fabula-memory                  │
│                                                  │
└─────────────────────────────────────────────────┘
```

Key changes from current:
- **Code on the fold.** A real DSL pattern — not prose about features. The reader sees what fabula looks like before scrolling.
- **Three entry points.** "Try it" (playground), "Get Started" (tutorial), "Learn Sifting" (concepts). Different doors for different people.
- **Use cases as navigation.** Each card links to its use-case tutorial. Proves domain breadth immediately.
- **`cargo add` at the bottom.** The reader who's already convinced can start immediately.
- **No stock illustrations.** No "undraw" SVGs. No Feature component with description paragraphs. Dense, purposeful, minimal.
- **Workshop aesthetic.** Monospace code, muted colors, generous whitespace. The code is the hero.

---

## Sidebar Configuration

```ts
const sidebars: SidebarsConfig = {
  docsSidebar: [
    'getting-started',
    {
      type: 'category',
      label: 'Learn Sifting',
      items: [
        'learn/what-is-sifting',
        'learn/sifting-by-example',
        'learn/patterns-from-first-principles',
        'learn/thinking-in-time',
        'learn/interactive-tutorial',
      ],
    },
    {
      type: 'category',
      label: 'Build a Simulation Monitor',
      items: [
        'build/overview',
        'build/01-simulation-loop',
        'build/02-define-patterns',
        'build/03-incremental-matching',
        'build/04-react-to-events',
        'build/05-score-and-rank',
        'build/06-speculate-with-mcts',
      ],
    },
    {
      type: 'category',
      label: 'Use Cases',
      items: [
        'use-cases/narrative-sifting',
        'use-cases/observability',
        'use-cases/process-mining',
        'use-cases/compliance-checking',
        'use-cases/simulation-monitoring',
        'use-cases/cybersecurity',
      ],
    },
    {
      type: 'category',
      label: 'Playground',
      items: [
        'playground/pattern-playground',
        'playground/step-through',
        'playground/allen-visualizer',
        'playground/scoring-explorer',
      ],
    },
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/overview',
        'concepts/how-the-engine-works',
        'concepts/temporal-model',
        'concepts/design-decisions',
        'concepts/composition',
        'concepts/scoring-and-surprise',
        'concepts/narrative-quality',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/pattern-cookbook',
        'guides/incremental-integration',
        'guides/scoring-matches',
        'guides/composing-patterns',
        'guides/forking-for-mcts',
        'guides/dsl-in-rust',
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
        'reference/scoring',
        'reference/narratives',
        'reference/dsl',
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
    'glossary',
    'learning-paths',
    'research',
  ],
};
```

---

## Build Infrastructure

### `fabula-examples` crate

A new workspace member containing every code sample used in the documentation. Each file is a runnable example that compiles and passes as a test. The docs extract regions via a remark plugin.

```
crates/fabula-examples/
├── Cargo.toml
├── src/
│   └── lib.rs              # empty, this is an examples-only crate
└── examples/
    ├── getting_started.rs   # all code from getting-started.md
    ├── incremental.rs       # all code from incremental-integration guide
    ├── scoring.rs           # all code from scoring-matches guide
    ├── composition.rs       # all code from composing-patterns guide
    ├── forking.rs           # all code from forking-for-mcts guide
    ├── build_01.rs          # project tutorial chapter 1
    ├── build_02.rs          # project tutorial chapter 2
    ├── build_03.rs          # ...
    ├── build_04.rs
    ├── build_05.rs
    ├── build_06.rs
    ├── usecase_observability.rs
    ├── usecase_compliance.rs
    ├── usecase_process.rs
    ├── usecase_security.rs
    ├── usecase_narrative.rs
    └── usecase_simulation.rs
```

Each file uses `// #region name` / `// #endregion` markers. The remark plugin `remark-code-region` extracts named regions into doc pages. CI runs `cargo test -p fabula-examples` — if any example fails to compile, the build fails. Zero stale code is a structural guarantee, not a policy.

### Remark plugin: `remark-code-region`

A Docusaurus remark plugin that replaces `<CodeRegion file="..." region="..." />` with the extracted code block, syntax-highlighted as Rust. This is a build-time transformation — no runtime cost, no client-side fetching.

Fallback: if the remark plugin is too much infrastructure for Phase A, start with a simpler approach — a pre-commit hook that runs `cargo check -p fabula-examples` and warns on changes to example files. The remark plugin can be added later when the example crate is stable.

---

## Execution Order

### Phase 0 — Build Infrastructure

Set up the structural guarantee that code samples stay correct.

0a. Create `fabula-examples` crate with Cargo.toml, add to workspace
0b. Migrate getting-started.md code into `examples/getting_started.rs` with region markers
0c. Add `cargo test -p fabula-examples` to CI
0d. (Optional) Implement `remark-code-region` plugin or simpler pre-commit check

### Phase A — Infrastructure Fixes

Fix broken things that affect current users. No new content.

1. Fix 7 placeholder URLs in docusaurus.config.ts, index.tsx, design-decisions.md
2. Delete scaffold files (markdown-page.md, dead HomepageFeatures)
3. Fix incomplete code snippet in incremental-integration.md
4. Add "Next steps" sections to 5 dead-end pages
5. Add inbound links to 9 orphaned pages
6. Add playground links to 8 content pages

### Phase B — Reference Gap Closure

Close the 45 code-vs-docs gaps. Mechanical work, high value.

7. Update reference/scoring.md (SequentialScorer, StuAggregation, PMI, confidence, fix signatures)
8. Update reference/narratives.md (sequential_surprise, fix assemble_signals)
9. Update reference/dsl.md (concurrent blocks, Concurrent token)
10. Update reference/patterns.md (unordered_groups, map_types, MetricGap, builders)
11. Update reference/engine.md (matched_stages, unordered behavior)
12. Update concepts/design-decisions.md (scoring section, fix dead link)

### Phase C — The On-Ramp

Highest-leverage new content. Demo-first pages that hook visitors.

13. Write learn/what-is-sifting.md (keystone page — opens with embedded playground)
14. Write learn/sifting-by-example.md (4 domains with embedded playgrounds)
15. Write glossary.md
16. Write learning-paths.md
17. Redesign homepage (3 entry points, use cases, research credibility)

### Phase D — Domain Tutorials

Prove breadth. Each tutorial opens with the running code, then explains.

18. Write use-cases/observability.md (distributed tracing — strongest non-narrative fit)
19. Write use-cases/compliance-checking.md (forbidden sequences, why_not for near-misses)
20. Write use-cases/process-mining.md (business process deviation detection)
21. Write use-cases/cybersecurity.md (MITRE ATT&CK patterns, lateral movement, beaconing)
22. Write use-cases/narrative-sifting.md (game AI, narrative detection)
23. Write use-cases/simulation-monitoring.md (emergent behavior in ABMs)

### Phase E — Concept Deep-Dives

Each concept page: demo first, then explain what the reader just saw.

24. Write concepts/composition.md (open with compose demo)
25. Write concepts/scoring-and-surprise.md (open with scoring explorer)
26. Write concepts/narrative-quality.md (open with tension arc visualization)
27. Update concepts/how-the-engine-works.md (state machine diagram, Winnow walkthrough, unordered groups, pattern-vs-sequence callout)
28. Update concepts/temporal-model.md (embed Allen visualizer at the top)
29. Update concepts/overview.md (data model bridge, new features)

### Phase F — How-To Guides

Task-focused. Code extracted from `fabula-examples` crate.

30. Write guides/scoring-matches.md
31. Write guides/composing-patterns.md
32. Write guides/forking-for-mcts.md
33. Write guides/dsl-in-rust.md
34. Update guides/pattern-cookbook.md (concurrent group recipes, compose recipes, playground links)

### Phase G — Project Tutorial

The transformative investment. 6 chapters building one real system. All code lives in `fabula-examples/examples/build_*.rs`, extracted into docs via region markers.

35. Write build/overview.md
36. Write build/01-simulation-loop.md
37. Write build/02-define-patterns.md
38. Write build/03-incremental-matching.md
39. Write build/04-react-to-events.md
40. Write build/05-score-and-rank.md
41. Write build/06-speculate-with-mcts.md

### Phase H — Interactive Learning

Polish the learning experience. Guided exercises with start/solved states.

42. Write learn/patterns-from-first-principles.md (opens with visual: pattern as template with holes)
43. Write learn/thinking-in-time.md (opens with Allen visualizer)
44. Write learn/interactive-tutorial.mdx (5 guided exercises with embedded playgrounds)
45. Write playground/scoring-explorer.mdx
46. Update sidebar configuration
47. Full link audit + `docusaurus build` verification

---

## Content Rules

### From DOCS_PLAN.md (still apply)

- Every page has exactly one Diataxis type
- Active voice, present tense throughout
- Code examples are complete and runnable
- Reference pages: every param typed and described
- How-to pages: prerequisites, numbered steps, expected output
- Hospitality example used sparingly — tutorials use different domains

### From the Site Design Brief (new)

- **Code is the source of truth.** No prose-only code blocks. Every snippet extracted from a compiled, tested file in `fabula-examples`. No `// ...` that hides the part you actually needed. Complete code blocks always.
- **Interaction before explanation.** When a concept can be demonstrated, demonstrate it first. Playground or visualization at the top of concept pages. Text explains what the reader just experienced.
- **Minimal, dense, precise.** No filler. No "in this section we will discuss." Every sentence teaches or shows. White space generous. Text tight.
- **Workshop, not lecture hall.** No mascots, no emoji, no gradients, no stock illustrations. Color carries meaning — never decoration. Good O'Reilly book that runs in the browser.
- **Demos are not decorations.** Each interactive element exists because the concept is genuinely harder to understand without interaction. No gratuitous interactivity.
- **Reading path is primary.** The playground supports the docs, not the other way around. Demos are inline where they teach; standalone playground pages are a bonus.
- **Evergreen content only.** No dates, no "updates," no opinions. Content is correct or it doesn't exist.

### From the expansion audit (new)

- **Non-narrative examples required** — at least 40% of examples across the site use non-narrative domains (compliance, observability, process mining, security).
- **No placeholder URLs** — every link must resolve. Test with `docusaurus build`.
- **No dead ends** — every page has "Next steps" or "Related" section with outbound links.
- **No orphans** — every page has at least one inbound link from content (not just sidebar).
- **Glossary terms linked** on first use in each page.
- **"Learn" pages have no API reference** — concepts only, link to reference for method signatures.
- **"Use case" tutorials are self-contained** — prerequisites, full code, expected output.
- **Builder API and DSL shown side-by-side** where both apply.
- **Time estimates on tutorial pages** — "Time: ~X min" in the header.
- **State diagram in engine concepts** — PartialMatch lifecycle: Active → Complete | Negated | Expired.

---

## Execution Status

Phases A through H are complete. 52 doc pages total (up from 24). Three review passes conducted (code-vs-docs audit, structural/link audit, content quality audit). Five naive reader reviews conducted (learn path, use cases, build tutorial, concepts+guides, reference+homepage).

### Completed

- Phase 0: Deferred (fabula-examples crate + remark-code-region)
- Phase A: Done — 7 placeholder URLs fixed, scaffold deleted, dead ends patched, orphans rescued, playground woven into 8 pages
- Phase B: Done — 45 code-vs-docs gaps closed across 6 reference pages
- Phase C: Done — what-is-sifting, sifting-by-example, glossary, learning-paths, homepage redesign
- Phase D: Done — 6 use-case tutorials (narrative, observability, process-mining, compliance, cybersecurity, simulation-monitoring)
- Phase E: Done — 3 concept pages (composition, scoring-and-surprise, narrative-quality) + engine update (state machine, Winnow walkthrough, unordered groups)
- Phase F: Done — 4 guides (scoring-matches, composing-patterns, forking-for-mcts, dsl-in-rust) + cookbook update (concurrent recipe)
- Phase G: Done — 7 build tutorial pages (overview + 6 chapters)
- Phase H: Done — patterns-from-first-principles, thinking-in-time, interactive-tutorial, scoring-explorer

---

## Future Work (from naive reader feedback)

Findings from 5 Haiku-model naive readers reviewing all 52 pages. Organized by impact.

### High Priority

**1. Performance and benchmarks page.**
Every non-narrative reader asked: "How many events/sec can fabula handle?" The bench crate exists (`fabula-bench`, ~28us/edge on petgraph) but no doc page surfaces this data. Create `concepts/performance.md` or `guides/performance.md` with throughput numbers, memory footprint, and scaling guidance per domain.

**2. DSL syntax primer.**
The learn section uses DSL syntax in playgrounds before formally introducing it. The quick-guide added to sifting-by-example helps, but a dedicated `learn/dsl-quick-reference.md` page (just syntax, no semantics) would eliminate the most common newcomer confusion.

**3. Schema mapping guidance per use case.**
Every use-case page uses simplified data (`e1.type = "login"`). Real systems have complex schemas (Windows Event Logs, XES process logs, OpenTelemetry spans). Each use-case page should have a "Mapping your data" section showing how to model real-world event schemas as fabula graph edges.

**4. Build chapter 4 schedule change callout.**
Chapters 1-3 use one event schedule; chapter 4 switches to a completely different schedule without a prominent warning. Add a bold callout: "This chapter uses a different event schedule from chapters 1-3."

### Medium Priority

**5. False positive discussion for security/observability.**
The cybersecurity and observability use cases were rated weakest by the skeptical backend engineer reader. The patterns are technically correct but lack discussion of false positive rates, evasion scenarios, and comparison to existing tools (Splunk, Datadog, Zeek).

**6. Comparison to existing tools.**
Process mining reader asked: "What does fabula add over ProM/Disco/Celonis?" Observability reader asked: "How does this compare to Datadog/Jaeger?" A brief "Fabula vs. X" section in each use-case page (or a dedicated comparison page) would address this.

**7. Weight tuning guide for narrative scoring.**
The narrative-quality concepts page documents default weights but doesn't explain how to tune them. A guide showing "if your game is combat-heavy, increase these weights" would bridge the gap between understanding and application.

**8. Worked PMI correction example in scoring reference.**
The scoring reference explains PMI and `with_pmi_correction()` but has no worked numerical example showing how correction changes the score. The scoring-explorer page covers other scorers with excellent numerical examples but omits PMI.

### Low Priority

**9. Forking guide: simpler intro example.**
The forking-for-mcts guide jumps to a 143-line example with narrative scoring. A simpler 30-line example (just compare two actions, no narrative weights) before the full example would improve accessibility.

**10. Composition output example.**
The composition concepts page explains how variables are renamed (`a_`, `b_`, `rep0_`) but doesn't show what match output looks like. A worked example showing actual bindings with prefixed variables would make the renaming concrete.

**11. "Temporal graph" definition on the what-is-sifting page.**
The keystone page uses "temporal graph" without defining it. Add one sentence: "A temporal graph is a collection of edges where each edge has a time interval — when the relationship was valid."

**12. Cold-start confidence ceiling.**
The scoring-and-surprise concepts page explains cold-start attenuation but doesn't state that confidence asymptotically approaches 1.0. Add: "Confidence reaches ~0.99 after 100 observations, effectively disabling attenuation."
