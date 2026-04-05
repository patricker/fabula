salience-mcts: Search-Based Drama Management

  The Core Idea

  You have a simulation running — characters forming relationships, factions scheming, settlements growing. fabula watches and detects narrative patterns as they emerge. But
  sometimes emergence alone isn't enough. The story stalls — the succession crisis is building but nobody dies, the betrayal arc has all the ingredients but the characters never
  interact.

  salience-mcts is a GM that can think ahead. It asks: "If I nudge the world this way, what happens to narrative quality 10-50 ticks downstream?" It tries thousands of possible
  interventions, simulates each forward, scores the results using fabula-narratives' composite quality function, and picks the best one.

  Why Salience, Not Fabula

  The boundary between fabula and Salience is detection vs. decision:

  - fabula answers: "what patterns exist or are forming?"
  - Salience answers: "which of these matters right now, and what should we do about it?"

  MCTS planning — "if I nudge the world this way, what happens to narrative quality?" — is fundamentally a selection and action problem. It chooses interventions based on what's
  salient. The evaluation function comes from fabula-narratives, but the decision about what to do with that score belongs to Salience.

  Planning is selection with lookahead. You're asking Salience "which intervention?" and answering it with tree search instead of heuristics.

  fabula's design constraint is "sifting only, no action system." MCTS doesn't violate that constraint — it recommends interventions rather than applying them — but recommendation
  is still a decision, and decisions belong in the selection layer. fabula's job ends at detection and scoring. The progression across the stack is:

    detect (fabula) → score (fabula-narratives) → plan (salience-mcts) → select (Salience) → apply (simulation)

  salience-mcts consumes fabula's public API (gap_analysis, TickDelta, NarrativeScore) across the crate boundary. That's what public APIs are for.

  How It Works

  Current tick: fabula detects a stalled succession_crisis pattern
      │
      ▼
  GM notices: "This pattern advanced to stage 2/3 but hasn't
    completed in 30 ticks. Intervention candidate."
      │
      ▼
  salience-mcts generates candidate interventions:
    - Introduce an assassination event
    - Have a rival faction defect
    - Trigger a famine that weakens the ruler
    - Do nothing (always a candidate)
      │
      ▼
  For each candidate:
    1. Fork the world state (clone the DataSource)
    2. Clone the SiftEngine (fabula already has .clone())
    3. Apply the intervention to the forked state
    4. Simulate forward N ticks (social sim cascade + rules)
    5. Run fabula on each tick → TickDeltas accumulate
    6. Score via fabula-narratives composite scorer:
       - Thread advancement/completion (did patterns fire?)
       - Tension trajectory (rising action? climax? denouement?)
       - Pivot surprise (was this unexpected?)
       - Plant/payoff resolution (did setups pay off?)
       - Stall count (did anything get WORSE?)
    7. Backpropagate score through the MCTS tree
      │
      ▼
  After N playouts: pick the intervention with highest
    narrative quality score. Recommend it to Salience.

  What Makes It Novel

  The research lineage is clear (Weyhrauch 1997 → Nelson & Mateas 2005 → Kartal 2014 → Narrative Studio 2025), and every system hits the same wall: the evaluation function.
  Everyone either:

  - Hand-authors a quality function (Nelson/Mateas — works but domain-specific)
  - Uses a single metric (Kartal — Bayesian believability)
  - Delegates to an LLM (Narrative Studio — 7 dimensions, expensive, non-deterministic)

  We have a multi-signal, deterministic, research-backed scorer that already exists in fabula-narratives (thread lifecycle + tension tracking + pivot detection + surprise +
  plant/payoff). No published work combines story sifting with MCTS. The evaluation function is the solved piece; the search is the missing connector.

  What It Looks Like Technically

  Given that we already have the mcts library at ~/code/mcts/:

  // The GameState: a forked world + cloned SiftEngine
  struct NarrativeState<DS: DataSource + Clone> {
      world: DS,                   // any DataSource — MemGraph for dev, snapshots for prod
      engine: SiftEngineFor<DS>,   // fabula clone
      tension: TensionTracker,     // narrative state
      chronon: i64,
  }

  // The moves: GM interventions
  enum Intervention {
      InjectEvent { event_type: String, subject: EntityId },
      ModifyRelationship { a: EntityId, b: EntityId, delta: f64 },
      TriggerPractice { practice: String, participants: Vec<EntityId> },
      DoNothing,
  }

  // GameState trait: simulate forward, report score
  impl<DS: DataSource + Clone> GameState for NarrativeState<DS> {
      type Move = Intervention;

      fn available_moves(&self) -> Vec<Intervention> {
          // Gap analysis: what patterns are partially matched?
          // What interventions would advance them?
          generate_candidate_interventions(&self.world, &self.engine)
      }

      fn make_move(&mut self, intervention: Intervention) {
          // Apply intervention → mutations → edges → engine
          let mutations = intervention.to_mutations(self.chronon);
          apply_and_observe(&mut self.world, &mut self.engine, &mutations);
          self.chronon += 1;
      }
  }

  // The Evaluator: fabula-narratives composite score
  impl<DS: DataSource + Clone> Evaluator<NarrativeSpec> for NarrativeEvaluator {
      fn evaluate_new_state(&self, state: &NarrativeState<DS>, ...) -> ... {
          // Score using the multi-signal quality function
          let signals = assemble_signals(&state.last_delta, ...);
          let score = scorer::score(&signals, &self.weights);
          score.total
      }
  }

  Paracausality Is an Optimization, Not a Requirement

  The original design assumed Paracausality's immutable EAV SnapshotStore for "free forking." But what MCTS actually needs from the world state is:

  1. Fork — create an independent copy to speculate on
  2. Apply mutations — inject edges
  3. Query — so the SiftEngine can match

  Any DataSource impl satisfies #2 and #3. And for #1 — MemGraph is Clone. It's not free the way an immutable EAV snapshot is, but it works. For a graph with a few thousand edges,
  cloning a Vec-backed store per playout is cheap enough to prototype and benchmark.

  The actual dependency chain is:

  - fabula (SiftEngine, gap_analysis, evaluate_pattern_at) — consumed via public API
  - fabula-narratives (scorer, TensionTracker, ThreadTracker, PivotDetector) — consumed via public API
  - fabula-memory or any Clone + DataSource impl — for prototyping and dev
  - ~/code/mcts/ (search tree) — the MCTS framework

  Paracausality becomes relevant at scale — large-world simulations where cloning the full state per playout is too expensive. At that point, swap MemGraph for a SnapshotStore-backed
  DataSource adapter. The NarrativeState struct is generic over DS: DataSource + Clone, so this is a type parameter change, not a rewrite.

  The Three Key Architectural Advantages

  1. Cheap forking: MemGraph clone for development; Paracausality snapshots at scale. The DataSource trait abstracts this — salience-mcts doesn't care which.
  2. Deterministic rollouts: With SimContext and seeded RNG, forward simulation is deterministic given a seed. Same intervention + same seed = same outcome. This means MCTS tree
  nodes are stable and reusable.
  3. Backward chaining from gaps: fabula's gap_analysis() tells you exactly what a partially-matched pattern is missing. Instead of generating random interventions, the GM can ask:
  "succession_crisis needs a ruler_death event — what interventions could cause one?" This dramatically prunes the search space.

  Where It Sits In The Full Stack

  Social Sim (state) → fabula (detection + scoring) → Salience (selection)
                                                            │
                                                     salience-mcts (planning via lookahead)
                                                            │
                                                            ▼
                                                     Effects → back to state

  salience-mcts is a strategy within the Salience layer. It doesn't replace Salience's heuristic selection — it augments it with tree search for cases where lookahead matters
  (stalled arcs, competing plot threads, high-stakes narrative moments). Simple cases still use Salience's fast path; MCTS is the heavy artillery.

  Open Design Questions

  1. Rollout horizon: How many ticks to simulate forward per playout? Too short and you miss payoffs. Too long and playouts are expensive. Likely needs to be configurable
     per-pattern or per-arc-stage.
  2. Tracker state forking: TensionTracker, ThreadTracker, and PivotDetector all carry state. All three need to be cloned per playout — verify they derive Clone.
  3. Intervention vocabulary: The Intervention enum shown above is illustrative. The real vocabulary depends on what the simulation exposes as valid mutations. This is
     domain-specific and likely needs a trait.
  4. When to engage MCTS: Not every tick needs tree search. Triggers might include: stall detection (pattern stuck for N ticks), tension plateau, or explicit GM request.

  Estimated Size

  ~500-800 LOC of connector code in ~/code/salience/. All the hard pieces exist:
  - MCTS search: ~/code/mcts/ (111 tests, parallel, lock-free)
  - Evaluation function: fabula-narratives (scorer, tension, pivots, plant/payoff)
  - Forkable state: any Clone + DataSource (MemGraph for dev, Paracausality at scale)
  - Pattern detection: fabula SiftEngine
  - Gap analysis: fabula gap_analysis() / evaluate_pattern_at()

  The actual work is defining Intervention types, wiring GameState/Evaluator traits, and deciding the rollout horizon.
