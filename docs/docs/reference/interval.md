---
sidebar_position: 1
title: Interval & Allen Relations
---

# Interval & Allen Relations

`fabula::interval` -- temporal intervals with Allen's 13-relation algebra.

## `Interval<T>`

A half-open time interval `[start, end)`. If `end` is `None`, the interval is open-ended (ongoing).

```rust reference file=tests/reference_interval.rs#interval_creation
```

### Fields

| Name | Type | Description |
|------|------|-------------|
| `start` | `T` | Inclusive start of the interval. |
| `end` | `Option<T>` | Exclusive end, or `None` if open-ended. |

### Trait bounds

All methods require `T: Ord + Clone`.

### Methods

#### `Interval::new`

Creates a bounded interval `[start, end)`.

```rust
pub fn new(start: T, end: T) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `start` | `T` | yes | -- | Inclusive start. |
| `end` | `T` | yes | -- | Exclusive end. |

**Returns:** `Interval<T>`

---

#### `Interval::open`

Creates an open-ended interval `[start, inf)`.

```rust
pub fn open(start: T) -> Self
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `start` | `T` | yes | -- | Inclusive start. |

**Returns:** `Interval<T>`

---

#### `covers`

Tests whether time `t` falls within this interval.

```rust
pub fn covers(&self, t: &T) -> bool
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `t` | `&T` | yes | -- | The time point to test. |

**Returns:** `bool` -- `true` if `start <= t` and (`end` is `None` or `t < end`).

---

#### `is_bounded`

Tests whether this interval has an end.

```rust
pub fn is_bounded(&self) -> bool
```

**Returns:** `bool` -- `true` if `end.is_some()`.

---

#### `relation`

Classifies the Allen relation between `self` and `other`. Returns `None` if either interval is open-ended and the relation cannot be determined.

```rust
pub fn relation(&self, other: &Interval<T>) -> Option<AllenRelation>
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `other` | `&Interval<T>` | yes | -- | The interval to compare against. |

**Returns:** `Option<AllenRelation>` -- one of 13 relations, or `None` if undetermined.

---

#### `before`

Tests whether `self` ends strictly before or at the start of `other`.

```rust
pub fn before(&self, other: &Interval<T>) -> bool
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `other` | `&Interval<T>` | yes | -- | The interval to compare against. |

**Returns:** `bool` -- `true` if `self.end <= other.start`. Returns `false` if `self` is open-ended.

---

#### `meets`

Tests whether `self` ends exactly where `other` starts.

```rust
pub fn meets(&self, other: &Interval<T>) -> bool
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `other` | `&Interval<T>` | yes | -- | The interval to compare against. |

**Returns:** `bool` -- `true` if `self.end == other.start`. Returns `false` if `self` is open-ended.

---

#### `intersects`

Tests whether two intervals share any time.

```rust
pub fn intersects(&self, other: &Interval<T>) -> bool
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `other` | `&Interval<T>` | yes | -- | The interval to test against. |

**Returns:** `bool` -- `true` if the intervals overlap. Handles open-ended intervals on either side.

---

### Trait implementations

| Trait | Bounds | Notes |
|-------|--------|-------|
| `Debug` | `T: Debug` | Derived. |
| `Clone` | `T: Clone` | Derived. |
| `PartialEq` | `T: PartialEq` | Derived. |
| `Eq` | `T: Eq` | Derived. |
| `Hash` | `T: Hash` | Derived. |
| `Display` | `T: Display` | Formats as `[start, end)` or `[start, inf)`. |

---

## `AllenRelation`

Allen's 13 mutually exclusive temporal relations between two bounded intervals A and B. Try them interactively in the [Allen Interval Visualizer](../playground/allen-visualizer).

```rust reference file=tests/reference_interval.rs#allen_relation
```

### Variants

| Variant | Visual | Description |
|---------|--------|-------------|
| `Before` | `AAA....BBB` | A ends before B starts. A gap separates them. |
| `After` | `BBB....AAA` | A starts after B ends. Inverse of `Before`. |
| `Meets` | `AAABBB` | A ends exactly where B starts. No gap, no overlap. |
| `MetBy` | `BBBAAA` | A starts exactly where B ends. Inverse of `Meets`. |
| `Overlaps` | `AAA__` / `..BBB` | A starts before B, A ends during B. Partial overlap, A first. |
| `OverlappedBy` | `..AAA` / `BBB__` | B starts before A, B ends during A. Partial overlap, B first. |
| `Starts` | `AA...` / `BBBBB` | A and B start together, A ends before B. A is a prefix of B. |
| `StartedBy` | `AAAAA` / `BB...` | A and B start together, A ends after B. B is a prefix of A. |
| `During` | `.AAA.` / `BBBBB` | A is entirely within B. A starts after B, A ends before B. |
| `Contains` | `AAAAA` / `.BBB.` | B is entirely within A. Inverse of `During`. |
| `Finishes` | `...AA` / `BBBBB` | A and B end together, A starts after B. A is a suffix of B. |
| `FinishedBy` | `AAAAA` / `...BB` | A and B end together, A starts before B. B is a suffix of A. |
| `Equals` | `AAAAA` / `BBBBB` | A and B have identical start and end. |

### Methods

#### `inverse`

Returns the inverse relation (swaps A and B).

```rust
pub fn inverse(self) -> AllenRelation
```

**Returns:** `AllenRelation` -- the relation that holds when A and B are swapped.

| Input | Output |
|-------|--------|
| `Before` | `After` |
| `After` | `Before` |
| `Meets` | `MetBy` |
| `MetBy` | `Meets` |
| `Overlaps` | `OverlappedBy` |
| `OverlappedBy` | `Overlaps` |
| `Starts` | `StartedBy` |
| `StartedBy` | `Starts` |
| `During` | `Contains` |
| `Contains` | `During` |
| `Finishes` | `FinishedBy` |
| `FinishedBy` | `Finishes` |
| `Equals` | `Equals` |

---

#### `is_before_or_meets`

Tests whether the relation implies A ends before or exactly when B starts.

```rust
pub fn is_before_or_meets(self) -> bool
```

**Returns:** `bool` -- `true` for `Before` and `Meets`, `false` for all others.

---

### Trait implementations

| Trait | Notes |
|-------|-------|
| `Debug` | Derived. |
| `Clone` | Derived. |
| `Copy` | Derived. |
| `PartialEq` | Derived. |
| `Eq` | Derived. |
| `Hash` | Derived. |
