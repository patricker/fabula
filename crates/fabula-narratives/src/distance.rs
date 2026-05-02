//! Pluggable distance metrics for distribution comparison.
//!
//! [`PivotDetector`](crate::pivot::PivotDetector) measures the shift between
//! consecutive event-type distributions. The shift is computed by a
//! [`DistanceMetric`] — by default Jensen-Shannon divergence (the Schulz et al.
//! 2024 "Pivot" measure), but callable with any metric that satisfies the
//! trait contract.
//!
//! Built-ins:
//! - [`JensenShannon`] — symmetric, bounded in [0, 1] (log base 2). The default.
//! - [`KullbackLeibler`] — asymmetric `KL(p || q)`. Captures "how surprised was
//!   the audience expecting `q` to see `p`?"
//! - [`Hellinger`] — symmetric, bounded in [0, 1]. Less sensitive to tails than JSD.
//!
//! All built-ins return `0.0` for identical distributions and handle
//! sparse/zero entries safely. Custom metrics should preserve those properties.

use std::collections::{HashMap, HashSet};

/// A distance metric between two categorical distributions over `String` keys.
///
/// Implementations should:
/// - Return `0.0` when `p == q` (identical distributions).
/// - Treat missing keys in either distribution as having probability `0`.
/// - Never return `NaN` or negative values for finite probability inputs.
pub trait DistanceMetric {
    fn distance(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64;
}

/// Jensen-Shannon divergence (the Schulz "Pivot" measure).
///
/// `JSD(P || Q) = 0.5 * KL(P || M) + 0.5 * KL(Q || M)`, where `M = (P + Q) / 2`.
/// Symmetric. Uses log base 2, so result is in `[0, 1]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct JensenShannon;

impl DistanceMetric for JensenShannon {
    fn distance(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
        let all_keys: HashSet<&String> = p.keys().chain(q.keys()).collect();
        let mut jsd = 0.0;
        for key in all_keys {
            let p_val = p.get(key).copied().unwrap_or(0.0);
            let q_val = q.get(key).copied().unwrap_or(0.0);
            let m_val = (p_val + q_val) / 2.0;
            if m_val > 0.0 {
                if p_val > 0.0 {
                    jsd += 0.5 * p_val * (p_val / m_val).log2();
                }
                if q_val > 0.0 {
                    jsd += 0.5 * q_val * (q_val / m_val).log2();
                }
            }
        }
        // Floating-point rounding can produce tiny negative values.
        jsd.max(0.0)
    }
}

/// Kullback-Leibler divergence: `KL(P || Q) = sum_i P(i) * log2(P(i) / Q(i))`.
///
/// Asymmetric. Returns `f64::INFINITY` if `Q` has zero mass on any key where
/// `P` is positive (the standard convention for "the audience could not have
/// expected this"). Use `JensenShannon` when you want a symmetric, bounded
/// score; use `KullbackLeibler` when you want directional surprise.
#[derive(Debug, Clone, Copy, Default)]
pub struct KullbackLeibler;

impl DistanceMetric for KullbackLeibler {
    fn distance(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
        let mut kl = 0.0;
        for (key, &p_val) in p.iter() {
            if p_val <= 0.0 {
                continue;
            }
            let q_val = q.get(key).copied().unwrap_or(0.0);
            if q_val <= 0.0 {
                return f64::INFINITY;
            }
            kl += p_val * (p_val / q_val).log2();
        }
        kl.max(0.0)
    }
}

/// Hellinger distance: `H(P, Q) = (1 / sqrt(2)) * sqrt(sum_i (sqrt(P(i)) - sqrt(Q(i)))^2)`.
///
/// Symmetric, bounded in `[0, 1]`. Less sensitive to distribution tails than
/// JSD: a small probability mass appearing in `q` but not `p` contributes
/// less than under JSD. Useful when you want "how much did the world change"
/// without overweighting rare events.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hellinger;

impl DistanceMetric for Hellinger {
    fn distance(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
        let all_keys: HashSet<&String> = p.keys().chain(q.keys()).collect();
        let mut sum_sq_diffs = 0.0;
        for key in all_keys {
            let p_val = p.get(key).copied().unwrap_or(0.0);
            let q_val = q.get(key).copied().unwrap_or(0.0);
            let diff = p_val.sqrt() - q_val.sqrt();
            sum_sq_diffs += diff * diff;
        }
        (sum_sq_diffs / 2.0).sqrt().clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    #[test]
    fn jsd_identical_is_zero() {
        let p = dist(&[("a", 0.5), ("b", 0.5)]);
        assert!(JensenShannon.distance(&p, &p).abs() < 1e-12);
    }

    #[test]
    fn jsd_disjoint_is_one() {
        let p = dist(&[("a", 1.0)]);
        let q = dist(&[("b", 1.0)]);
        let d = JensenShannon.distance(&p, &q);
        assert!((d - 1.0).abs() < 1e-9, "expected ~1.0, got {}", d);
    }

    #[test]
    fn jsd_is_symmetric() {
        let p = dist(&[("a", 0.7), ("b", 0.3)]);
        let q = dist(&[("a", 0.4), ("b", 0.6)]);
        let pq = JensenShannon.distance(&p, &q);
        let qp = JensenShannon.distance(&q, &p);
        assert!((pq - qp).abs() < 1e-12);
    }

    #[test]
    fn kl_identical_is_zero() {
        let p = dist(&[("a", 0.5), ("b", 0.5)]);
        assert!(KullbackLeibler.distance(&p, &p).abs() < 1e-12);
    }

    #[test]
    fn kl_returns_infinity_when_q_misses_p_support() {
        let p = dist(&[("a", 0.5), ("b", 0.5)]);
        let q = dist(&[("a", 1.0)]);
        assert!(KullbackLeibler.distance(&p, &q).is_infinite());
    }

    #[test]
    fn kl_is_asymmetric() {
        let p = dist(&[("a", 0.7), ("b", 0.3)]);
        let q = dist(&[("a", 0.4), ("b", 0.6)]);
        let pq = KullbackLeibler.distance(&p, &q);
        let qp = KullbackLeibler.distance(&q, &p);
        assert!((pq - qp).abs() > 1e-6, "KL must be asymmetric");
    }

    #[test]
    fn hellinger_identical_is_zero() {
        let p = dist(&[("a", 0.5), ("b", 0.5)]);
        assert!(Hellinger.distance(&p, &p).abs() < 1e-12);
    }

    #[test]
    fn hellinger_disjoint_is_one() {
        let p = dist(&[("a", 1.0)]);
        let q = dist(&[("b", 1.0)]);
        let d = Hellinger.distance(&p, &q);
        assert!((d - 1.0).abs() < 1e-9);
    }

    #[test]
    fn hellinger_is_symmetric() {
        let p = dist(&[("a", 0.7), ("b", 0.3)]);
        let q = dist(&[("a", 0.4), ("b", 0.6)]);
        let pq = Hellinger.distance(&p, &q);
        let qp = Hellinger.distance(&q, &p);
        assert!((pq - qp).abs() < 1e-12);
    }
}
