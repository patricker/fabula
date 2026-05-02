use fabula_narratives::distance::JensenShannon;
use fabula_narratives::pivot::PivotDetector;
use fabula_narratives::scorer::{score, NarrativeSignals, NarrativeWeights};
use fabula_narratives::tension::{TensionTracker, Trajectory};
use fabula_narratives::thread::ThreadTracker;

#[test]
fn thread_tracker_usage() {
    // #region thread_tracker
    let mut tracker = ThreadTracker::new();
    tracker.register("investigation", 0, 1);

    // Simulate thread lifecycle
    tracker.record_open("investigation");
    let violations = tracker.check_filo();
    // #endregion

    assert!(violations.is_empty());
}

#[test]
fn tension_tracker_usage() {
    // #region tension_tracker
    let mut tracker = TensionTracker::new(10); // window of 10 samples
    for i in 0..15 {
        tracker.push(i as u64, i as f64 * 0.1);
    }
    assert_eq!(tracker.trajectory(), Trajectory::Rising);
    assert!(tracker.slope() > 0.0);
    // #endregion
}

#[test]
fn pivot_detector_usage() {
    // #region pivot_detector
    let mut pivot = PivotDetector::<JensenShannon>::new();

    // Tick 1: peaceful events
    pivot.push("trade");
    pivot.push("trade");
    pivot.push("talk");
    let _ = pivot.end_tick(); // first tick: 0 (no previous)

    // Tick 2: sudden violence
    pivot.push("attack");
    pivot.push("attack");
    pivot.push("harm");
    let jsd = pivot.end_tick();
    assert!(jsd > 0.5); // dramatic shift
                        // #endregion
}

#[test]
fn composite_scorer_usage() {
    // #region composite_scorer
    let signals = NarrativeSignals {
        advancements: 3,
        completions: 1,
        resolutions: 1,
        pivot_magnitude: 0.4,
        ..Default::default()
    };
    let result = score(&signals, &NarrativeWeights::default());
    assert!(result.total > 0.0);
    println!("Breakdown: {:?}", result.breakdown);
    // #endregion
}

#[test]
fn custom_weights() {
    // #region custom_weights
    let horror_weights = NarrativeWeights {
        tension_fit: 5.0,       // pacing matters a lot in horror
        resolution_reward: 1.0, // keep things unresolved for dread
        pivot_reward: 3.0,      // dramatic turns are key
        ..Default::default()
    };

    let signals = NarrativeSignals {
        advancements: 2,
        completions: 0,
        tension_fit: 1.0, // tension is rising as desired
        pivot_magnitude: 0.8,
        ..Default::default()
    };
    let result = score(&signals, &horror_weights);
    // High score: tension is matching desired trajectory and pivot is high
    assert!(result.breakdown.tension > 0.0);
    assert!(result.breakdown.pivot > 0.0);
    // #endregion
}
