//! PUCT vs Gumbel MCTS comparison with narrative scoring evaluation.
//!
//! Answers the Gumbel question: at what character count does the scoring pipeline
//! become the MCTS bottleneck, and does Gumbel's fewer-simulations advantage
//! matter in practice?
//!
//! The benchmark wraps the narrative scoring pipeline as an MCTS evaluation function
//! and compares:
//! - **PUCT** (standard AlphaGo-style MCTS): needs more simulations for convergence
//! - **Gumbel** (Sequential Halving with Gumbel noise): needs fewer simulations
//!
//! If scoring is cheap (small character count), tree overhead dominates and Gumbel's
//! advantage is marginal. If scoring is expensive (large character count), Gumbel's
//! ability to match PUCT quality with fewer evaluations saves real wall-clock time.

use std::sync::Arc;

use divan::Bencher;
use fabula_bench::narrative_workload::{generate_trace, NarrativeTrace, NarrativeTraceConfig};
use fabula_narratives::distance::JensenShannon;
use fabula_narratives::pivot::PivotDetector;
use fabula_narratives::scorer::{assemble_signals, score, NarrativeWeights};
use fabula_narratives::tension::TensionTracker;
use fabula_narratives::thread::ThreadTracker;
use treant::tree_policy::AlphaGoPolicy;
use treant::{CycleBehaviour, GameState, MCTSManager, MCTS};
use treant_gumbel::{GumbelConfig, GumbelEvaluator, GumbelSearch};

fn main() {
    divan::main();
}

// ---------------------------------------------------------------------------
// Narrative "game" for MCTS -- single-player optimization over intervention choices
// ---------------------------------------------------------------------------

const BRANCHING_FACTOR: usize = 5;
const SEARCH_DEPTH: usize = 4;

/// A narrative game state that wraps the scoring pipeline.
/// Each "move" selects one of BRANCHING_FACTOR candidate interventions per tick.
/// The evaluation function runs the full narrative scoring pipeline.
#[derive(Clone)]
struct NarrativeGame {
    tick_index: usize,
    depth: usize,
    thread_tracker: ThreadTracker,
    tension_tracker: TensionTracker,
    pivot_detector: PivotDetector<JensenShannon>,
    trace: Arc<NarrativeTrace>,
}

impl NarrativeGame {
    fn new(trace: Arc<NarrativeTrace>) -> Self {
        let mut thread_tracker = ThreadTracker::new();
        for (name, open_idx, close_idx) in &trace.thread_registrations {
            thread_tracker.register(name, *open_idx, *close_idx);
        }
        Self {
            tick_index: 0,
            depth: 0,
            thread_tracker,
            tension_tracker: TensionTracker::new(20),
            pivot_detector: PivotDetector::<JensenShannon>::new(),
            trace,
        }
    }

    fn current_tick_index(&self) -> usize {
        self.tick_index.min(self.trace.ticks.len() - 1)
    }
}

impl GameState for NarrativeGame {
    type Move = usize;
    type Player = ();
    type MoveList = Vec<usize>;

    fn current_player(&self) {}

    fn available_moves(&self) -> Vec<usize> {
        if self.depth >= SEARCH_DEPTH || self.tick_index >= self.trace.ticks.len() {
            vec![]
        } else {
            (0..BRANCHING_FACTOR).collect()
        }
    }

    fn make_move(&mut self, _mov: &usize) {
        // Each move advances the narrative by one tick.
        // Different moves would represent different interventions in a real GM;
        // here all consume the same tick data to isolate evaluation cost.
        let idx = self.current_tick_index();
        let tick = &self.trace.ticks[idx];

        self.thread_tracker.observe_delta(&tick.delta);
        self.tension_tracker.push(tick.tick, tick.tension_value);
        for et in &tick.event_types {
            self.pivot_detector.push(et);
        }
        self.pivot_detector.end_tick();

        self.tick_index += 1;
        self.depth += 1;
    }
}

// ---------------------------------------------------------------------------
// PUCT evaluator (standard MCTS path)
// ---------------------------------------------------------------------------

struct NarrativePUCTEval {
    weights: NarrativeWeights,
}

impl treant::Evaluator<NarrativePUCTSpec> for NarrativePUCTEval {
    type StateEvaluation = i64;

    fn evaluate_new_state(
        &self,
        state: &NarrativeGame,
        moves: &Vec<usize>,
        _handle: Option<treant::SearchHandle<NarrativePUCTSpec>>,
    ) -> (Vec<f64>, i64) {
        let value = self.evaluate_state(state);
        let n = moves.len();
        let priors = if n > 0 {
            vec![1.0 / n as f64; n]
        } else {
            vec![]
        };
        (priors, value)
    }

    fn evaluate_existing_state(
        &self,
        state: &NarrativeGame,
        _existing: &i64,
        _handle: treant::SearchHandle<NarrativePUCTSpec>,
    ) -> i64 {
        self.evaluate_state(state)
    }

    fn interpret_evaluation_for_player(&self, evaln: &i64, _player: &()) -> i64 {
        *evaln
    }
}

impl NarrativePUCTEval {
    fn evaluate_state(&self, state: &NarrativeGame) -> i64 {
        let idx = state.current_tick_index();
        let tick = &state.trace.ticks[idx];

        let filo_violations = state.thread_tracker.check_filo().len();
        let trajectory = state.tension_tracker.trajectory();
        let pivot_magnitude = state.pivot_detector.last_pivot();

        let signals = assemble_signals(
            &tick.delta,
            &tick.plant_statuses,
            filo_violations,
            trajectory,
            tick.desired_trajectory,
            pivot_magnitude,
            tick.surprise,
            tick.sequential_surprise,
        );
        let result = score(&signals, &self.weights);
        (result.total * 100.0) as i64
    }
}

// ---------------------------------------------------------------------------
// PUCT MCTS spec
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NarrativePUCTSpec;

impl MCTS for NarrativePUCTSpec {
    type State = NarrativeGame;
    type Eval = NarrativePUCTEval;
    type NodeData = ();
    type ExtraThreadData = ();
    type TreePolicy = AlphaGoPolicy;
    type TranspositionTable = ();

    fn cycle_behaviour(&self) -> CycleBehaviour<Self> {
        CycleBehaviour::UseCurrentEvalWhenCycleDetected
    }
    fn fpu_value(&self) -> f64 {
        0.0
    }
    fn max_playout_depth(&self) -> usize {
        SEARCH_DEPTH + 2
    }
}

// ---------------------------------------------------------------------------
// Gumbel evaluator
// ---------------------------------------------------------------------------

struct NarrativeGumbelEval {
    weights: NarrativeWeights,
}

impl GumbelEvaluator<NarrativeGame> for NarrativeGumbelEval {
    fn evaluate(&self, state: &NarrativeGame, _moves: &[usize]) -> (Vec<f64>, f64) {
        let idx = state.current_tick_index();
        let tick = &state.trace.ticks[idx];

        let filo_violations = state.thread_tracker.check_filo().len();
        let trajectory = state.tension_tracker.trajectory();
        let pivot_magnitude = state.pivot_detector.last_pivot();

        let signals = assemble_signals(
            &tick.delta,
            &tick.plant_statuses,
            filo_violations,
            trajectory,
            tick.desired_trajectory,
            pivot_magnitude,
            tick.surprise,
            tick.sequential_surprise,
        );
        let result = score(&signals, &self.weights);

        // Uniform logits (no learned policy), value normalized to [-1, 1]
        let logits = vec![0.0; _moves.len()];
        let value = (result.total / 50.0).clamp(-1.0, 1.0);
        (logits, value)
    }
}

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

fn make_trace(character_count: usize) -> Arc<NarrativeTrace> {
    Arc::new(generate_trace(&NarrativeTraceConfig {
        character_count,
        tick_count: 200,
        ..NarrativeTraceConfig::default()
    }))
}

fn run_puct(trace: &Arc<NarrativeTrace>, simulations: u64) {
    let game = NarrativeGame::new(Arc::clone(trace));
    let eval = NarrativePUCTEval {
        weights: NarrativeWeights::default(),
    };
    let mut manager = MCTSManager::new(game, NarrativePUCTSpec, eval, AlphaGoPolicy::new(1.5), ());
    manager.playout_n(simulations);
    std::hint::black_box(manager.best_move());
}

fn run_gumbel(trace: &Arc<NarrativeTrace>, simulations: u32) {
    let game = NarrativeGame::new(Arc::clone(trace));
    let eval = NarrativeGumbelEval {
        weights: NarrativeWeights::default(),
    };
    let config = GumbelConfig {
        m_actions: BRANCHING_FACTOR.min(16),
        c_puct: 1.25,
        max_depth: SEARCH_DEPTH + 2,
        value_scale: 50.0,
        seed: 42,
    };
    let mut search = GumbelSearch::new(eval, config);
    let result = search.search(&game, simulations);
    std::hint::black_box(result.best_move);
}

// ===========================================================================
// Benchmarks: PUCT vs Gumbel at various character counts x simulation budgets
// ===========================================================================

/// Compare PUCT and Gumbel at various simulation budgets with 50 characters.
/// Gumbel should match PUCT quality with fewer simulations.
mod puct_vs_gumbel {
    use super::*;

    #[divan::bench(args = [32, 64, 128, 256, 512])]
    fn puct_simulations(bencher: Bencher, sims: u64) {
        let trace = make_trace(50);
        bencher.bench(|| run_puct(&trace, sims));
    }

    #[divan::bench(args = [16, 32, 64, 128, 256])]
    fn gumbel_simulations(bencher: Bencher, sims: u32) {
        let trace = make_trace(50);
        bencher.bench(|| run_gumbel(&trace, sims));
    }
}

/// Character count scaling -- answers the Gumbel question directly.
/// At what character count does PUCT-1000 exceed 16ms, making Gumbel's
/// fewer-simulation advantage relevant?
mod character_scaling {
    use super::*;

    const PUCT_SIMS: u64 = 200;
    const GUMBEL_SIMS: u32 = 64;

    #[divan::bench(args = [2, 10, 50, 200])]
    fn puct(bencher: Bencher, characters: usize) {
        let trace = make_trace(characters);
        bencher.bench(|| run_puct(&trace, PUCT_SIMS));
    }

    #[divan::bench(args = [2, 10, 50, 200])]
    fn gumbel(bencher: Bencher, characters: usize) {
        let trace = make_trace(characters);
        bencher.bench(|| run_gumbel(&trace, GUMBEL_SIMS));
    }
}
