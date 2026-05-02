//! Narrative thread lifecycle management (MICE-style).
//!
//! Tracks open/close thread pairs and validates FILO (First In, Last Out)
//! nesting. Based on Mary Robinette Kowal's MICE Quotient model:
//! Milieu, Inquiry, Character, Event threads open and close; well-nested
//! stories close threads in reverse order of opening.
//!
//! The tracker does NOT define what constitutes an "open" or "close" event --
//! the caller registers pattern indices for each thread. The tracker monitors
//! engine state to report which threads are open, stale, or mis-nested.

/// A registered narrative thread (open/close pair).
#[derive(Debug, Clone)]
pub struct ThreadDef {
    /// Human-readable name (e.g., "investigation", "milieu_forest").
    pub name: String,
    /// Pattern index for the opening event.
    pub open_pattern_idx: usize,
    /// Pattern index for the closing event.
    pub close_pattern_idx: usize,
}

/// Current status of a single thread.
#[derive(Debug, Clone)]
pub struct ThreadStatus {
    pub name: String,
    /// Number of open instances (PMs that advanced past stage 0 but haven't completed).
    pub open_count: usize,
    /// Number of times the close pattern has completed.
    pub close_count: u64,
    /// Whether this thread has opens without corresponding closes.
    pub unresolved: bool,
}

/// A FILO nesting violation -- thread A opened before thread B but B hasn't
/// closed yet while A is closing.
#[derive(Debug, Clone, Default)]
pub struct FiloViolation {
    /// Thread that closed out of order.
    pub closed_thread: String,
    /// Thread that should have closed first (opened later but still open).
    pub blocking_thread: String,
}

/// Tracks narrative thread lifecycles.
///
/// Register threads (open/close pattern pairs), then query status after
/// each tick. The tracker reads engine state -- it does not modify it.
///
/// ```rust,ignore
/// let mut tracker = ThreadTracker::new();
/// tracker.register("investigation", open_idx, close_idx);
/// // After each tick, observe the engine's tick delta:
/// tracker.observe_delta(&delta);
/// let status = tracker.status(|idx| engine.is_pattern_enabled(idx));
/// let violations = tracker.check_filo();
/// ```
#[derive(Debug, Clone, Default)]
pub struct ThreadTracker {
    threads: Vec<ThreadDef>,
    /// Order in which threads were first opened (for FILO checking).
    open_order: Vec<String>,
    /// Order in which threads were closed.
    close_order: Vec<String>,
}

impl ThreadTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a narrative thread with its open and close pattern indices.
    ///
    /// If using [`observe_delta`](Self::observe_delta), the engine's pattern names
    /// must follow the convention `{name}_open` and `{name}_close`. For example,
    /// registering `"investigation"` expects patterns named `"investigation_open"`
    /// and `"investigation_close"`. For custom naming, use [`record_open`](Self::record_open)
    /// and [`record_close`](Self::record_close) directly.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        open_pattern_idx: usize,
        close_pattern_idx: usize,
    ) {
        self.threads.push(ThreadDef {
            name: name.into(),
            open_pattern_idx,
            close_pattern_idx,
        });
    }

    /// Record that a thread opened. Call when the open pattern's first stage matches.
    ///
    /// Deduplicates by name: calling this twice with the same name is a no-op.
    /// FILO tracking is per-thread-name, not per-instance -- if the same thread
    /// type opens multiple times, only the first open is tracked for nesting order.
    pub fn record_open(&mut self, thread_name: &str) {
        if !self.open_order.contains(&thread_name.to_string()) {
            self.open_order.push(thread_name.to_string());
        }
    }

    /// Record that a thread closed. Call when the close pattern completes.
    pub fn record_close(&mut self, thread_name: &str) {
        self.close_order.push(thread_name.to_string());
    }

    /// Update from a TickDelta -- automatically records opens and closes.
    ///
    /// Matches pattern names in the delta against `{thread_name}_open` and
    /// `{thread_name}_close` conventions. For custom pattern names, use
    /// `record_open` / `record_close` directly.
    ///
    /// Note the asymmetry: opens are detected from `delta.advanced` (a thread
    /// opens when its open pattern begins matching), while closes are detected
    /// from `delta.completed` (a thread closes when its close pattern fully
    /// resolves).
    pub fn observe_delta(&mut self, delta: &fabula::engine::TickDelta) {
        // Collect matching thread names first to avoid borrow conflict
        let opens: Vec<String> = self
            .threads
            .iter()
            .filter(|t| delta.advanced.contains(&format!("{}_open", t.name)))
            .map(|t| t.name.clone())
            .collect();
        let closes: Vec<String> = self
            .threads
            .iter()
            .filter(|t| delta.completed.contains(&format!("{}_close", t.name)))
            .map(|t| t.name.clone())
            .collect();
        for name in opens {
            self.record_open(&name);
        }
        for name in closes {
            self.record_close(&name);
        }
    }

    /// Status of all registered threads.
    ///
    /// Accepts a closure that returns [`PatternMetrics`] for a given pattern
    /// index. Typical usage: `tracker.status(|idx| engine.pattern_metrics(idx))`.
    ///
    /// Decoupled from `SiftEngine` so callers can provide cached or speculative
    /// metrics during MCTS rollouts without requiring a full engine reference.
    pub fn status(
        &self,
        metrics_fn: impl Fn(usize) -> Option<fabula::engine::PatternMetrics>,
    ) -> Vec<ThreadStatus> {
        self.threads
            .iter()
            .map(|thread| {
                let metrics_open = metrics_fn(thread.open_pattern_idx);
                let metrics_close = metrics_fn(thread.close_pattern_idx);

                let open_count = metrics_open
                    .as_ref()
                    .map(|m| m.active_pm_count)
                    .unwrap_or(0);
                let close_count = metrics_close
                    .as_ref()
                    .map(|m| m.completion_count)
                    .unwrap_or(0);
                let open_completions = metrics_open
                    .as_ref()
                    .map(|m| m.completion_count)
                    .unwrap_or(0);

                ThreadStatus {
                    name: thread.name.clone(),
                    open_count,
                    close_count,
                    unresolved: open_completions > close_count,
                }
            })
            .collect()
    }

    /// Count of currently unresolved (open but not closed) threads.
    pub fn unresolved_thread_count(
        &self,
        metrics_fn: impl Fn(usize) -> Option<fabula::engine::PatternMetrics>,
    ) -> usize {
        self.status(metrics_fn)
            .iter()
            .filter(|s| s.unresolved)
            .count()
    }

    /// Check FILO nesting: threads should close in reverse order of opening.
    /// Returns violations where a thread closed while a later-opened thread
    /// was still open.
    pub fn check_filo(&self) -> Vec<FiloViolation> {
        let mut violations = Vec::new();
        let mut still_open: Vec<&str> = self.open_order.iter().map(|s| s.as_str()).collect();

        for closed in &self.close_order {
            if let Some(pos) = still_open.iter().position(|&s| s == closed.as_str()) {
                // Check: are there threads opened AFTER this one that are still open?
                for &later_open in &still_open[pos + 1..] {
                    // Only a violation if the later thread hasn't been closed yet
                    let later_closed_count = self
                        .close_order
                        .iter()
                        .filter(|c| c.as_str() == later_open)
                        .count();
                    let later_open_count = self
                        .open_order
                        .iter()
                        .filter(|o| o.as_str() == later_open)
                        .count();
                    if later_closed_count < later_open_count {
                        violations.push(FiloViolation {
                            closed_thread: closed.clone(),
                            blocking_thread: later_open.to_string(),
                        });
                    }
                }
                still_open.remove(pos);
            }
        }

        violations
    }

    /// Reset all tracking state (keeps thread registrations).
    pub fn reset(&mut self) {
        self.open_order.clear();
        self.close_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filo_correct_nesting() {
        let mut tracker = ThreadTracker::new();
        tracker.register("milieu", 0, 1);
        tracker.register("inquiry", 2, 3);

        // Open milieu, open inquiry, close inquiry, close milieu (correct FILO)
        tracker.record_open("milieu");
        tracker.record_open("inquiry");
        tracker.record_close("inquiry");
        tracker.record_close("milieu");

        assert!(
            tracker.check_filo().is_empty(),
            "correct nesting should have no violations"
        );
    }

    #[test]
    fn filo_violation_detected() {
        let mut tracker = ThreadTracker::new();
        tracker.register("milieu", 0, 1);
        tracker.register("inquiry", 2, 3);

        // Open milieu, open inquiry, close milieu (violation! inquiry still open)
        tracker.record_open("milieu");
        tracker.record_open("inquiry");
        tracker.record_close("milieu");

        let violations = tracker.check_filo();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].closed_thread, "milieu");
        assert_eq!(violations[0].blocking_thread, "inquiry");
    }

    #[test]
    fn reset_clears_tracking() {
        let mut tracker = ThreadTracker::new();
        tracker.register("test", 0, 1);
        tracker.record_open("test");
        tracker.record_close("test");
        tracker.reset();
        assert!(tracker.check_filo().is_empty());
    }
}
