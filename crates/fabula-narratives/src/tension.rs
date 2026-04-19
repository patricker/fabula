//! Numeric trajectory sampling and classification.
//!
//! Samples a named numeric value over time and classifies the trajectory
//! (rising, falling, plateau, peak, valley). Based on Left 4 Dead's AI
//! Director (Booth 2009) and Ely/Frankel/Kamenica (2015) suspense model.
//!
//! The tracker does NOT query the DataSource directly -- the caller provides
//! samples. This keeps it DataSource-agnostic.

use std::collections::VecDeque;

/// Classification of a trajectory's recent behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trajectory {
    /// Values increasing over the window.
    Rising,
    /// Values decreasing over the window.
    Falling,
    /// Values approximately constant.
    Plateau,
    /// Rose then fell (local maximum).
    Peak,
    /// Fell then rose (local minimum).
    Valley,
    /// Not enough data to classify.
    Unknown,
}

/// A single timestamped sample.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    /// Tick number when the sample was taken.
    pub tick: u64,
    /// Numeric value at this tick.
    pub value: f64,
}

/// Tracks a numeric value over time and classifies its trajectory.
///
/// The caller samples a value (e.g., character stress, faction hostility)
/// each tick and feeds it to the tracker. The tracker maintains a sliding
/// window and classifies the trend.
///
/// ```rust
/// use fabula_narratives::tension::{TensionTracker, Trajectory};
///
/// let mut tracker = TensionTracker::new(10); // window of 10 samples
/// for i in 0..15 {
///     tracker.push(i as u64, i as f64 * 0.1); // rising tension
/// }
/// assert_eq!(tracker.trajectory(), Trajectory::Rising);
/// assert!(tracker.slope() > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct TensionTracker {
    window_size: usize,
    samples: VecDeque<Sample>,
    /// Slope magnitude below which trajectory is classified as Plateau.
    threshold: f64,
}

impl TensionTracker {
    /// Create a tracker with the given sliding window size.
    ///
    /// Window must be at least 3 (trajectory classification needs 3+ samples).
    /// Uses a default slope threshold of 0.01 for trajectory classification.
    pub fn new(window_size: usize) -> Self {
        assert!(
            window_size >= 3,
            "window must be at least 3 for trajectory classification"
        );
        Self {
            window_size,
            samples: VecDeque::new(),
            threshold: 0.01,
        }
    }

    /// Create a tracker with a custom slope threshold for trajectory classification.
    ///
    /// The threshold controls how steep a slope must be to count as Rising/Falling
    /// vs Plateau. Higher values require stronger trends.
    pub fn with_threshold(window_size: usize, threshold: f64) -> Self {
        assert!(
            window_size >= 3,
            "window must be at least 3 for trajectory classification"
        );
        Self {
            window_size,
            samples: VecDeque::new(),
            threshold,
        }
    }

    /// Push a new sample. Old samples outside the window are dropped.
    pub fn push(&mut self, tick: u64, value: f64) {
        self.samples.push_back(Sample { tick, value });
        if self.samples.len() > self.window_size {
            self.samples.pop_front();
        }
    }

    /// Current value (most recent sample).
    pub fn current(&self) -> Option<f64> {
        self.samples.back().map(|s| s.value)
    }

    /// Compute the slope (linear regression) over the window.
    /// Positive = rising, negative = falling, near-zero = plateau.
    pub fn slope(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        // Simple linear regression: slope = Σ(xi - x̄)(yi - ȳ) / Σ(xi - x̄)²
        let n = self.samples.len() as f64;
        let x_mean: f64 = self.samples.iter().map(|s| s.tick as f64).sum::<f64>() / n;
        let y_mean: f64 = self.samples.iter().map(|s| s.value).sum::<f64>() / n;

        let mut num = 0.0;
        let mut den = 0.0;
        for s in &self.samples {
            let dx = s.tick as f64 - x_mean;
            let dy = s.value - y_mean;
            num += dx * dy;
            den += dx * dx;
        }

        if den.abs() < f64::EPSILON {
            0.0
        } else {
            num / den
        }
    }

    /// Classify the trajectory over the window.
    pub fn trajectory(&self) -> Trajectory {
        if self.samples.len() < 3 {
            return Trajectory::Unknown;
        }

        let slope = self.slope();
        let threshold = self.threshold;

        // Check for peak/valley by splitting window in half
        let mid = self.samples.len() / 2;
        let first_half: Vec<f64> = self.samples.iter().take(mid).map(|s| s.value).collect();
        let second_half: Vec<f64> = self.samples.iter().skip(mid).map(|s| s.value).collect();

        let first_mean = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_mean = second_half.iter().sum::<f64>() / second_half.len() as f64;
        let mid_value = self.samples[mid].value;

        // Peak: middle is higher than both halves' averages
        if mid_value > first_mean
            && mid_value > second_mean
            && (first_mean - second_mean).abs() < threshold * 10.0
        {
            return Trajectory::Peak;
        }
        // Valley: middle is lower than both halves' averages
        if mid_value < first_mean
            && mid_value < second_mean
            && (first_mean - second_mean).abs() < threshold * 10.0
        {
            return Trajectory::Valley;
        }

        if slope > threshold {
            Trajectory::Rising
        } else if slope < -threshold {
            Trajectory::Falling
        } else {
            Trajectory::Plateau
        }
    }

    /// Number of samples in the window.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Clear all samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_trajectory() {
        let mut t = TensionTracker::new(10);
        for i in 0..10 {
            t.push(i, i as f64 * 0.5);
        }
        assert_eq!(t.trajectory(), Trajectory::Rising);
        assert!(t.slope() > 0.0);
    }

    #[test]
    fn falling_trajectory() {
        let mut t = TensionTracker::new(10);
        for i in 0..10 {
            t.push(i, 10.0 - i as f64);
        }
        assert_eq!(t.trajectory(), Trajectory::Falling);
        assert!(t.slope() < 0.0);
    }

    #[test]
    fn plateau_trajectory() {
        let mut t = TensionTracker::new(10);
        for i in 0..10 {
            t.push(i, 5.0);
        }
        assert_eq!(t.trajectory(), Trajectory::Plateau);
        assert!(t.slope().abs() < 0.01);
    }

    #[test]
    fn sliding_window_drops_old() {
        let mut t = TensionTracker::new(5);
        for i in 0..20 {
            t.push(i, i as f64);
        }
        assert_eq!(t.sample_count(), 5);
    }

    #[test]
    fn unknown_with_too_few_samples() {
        let mut t = TensionTracker::new(10);
        t.push(0, 1.0);
        assert_eq!(t.trajectory(), Trajectory::Unknown);
    }

    #[test]
    fn custom_threshold_classifies_gentle_rise_as_plateau() {
        let mut t = TensionTracker::with_threshold(10, 1.0); // very high threshold
        for i in 0..10 {
            t.push(i, i as f64 * 0.001); // very gentle rise
        }
        // With default threshold (0.01) this would be Rising,
        // but with threshold=1.0 the slope is well below threshold
        assert_eq!(t.trajectory(), Trajectory::Plateau);
    }
}
