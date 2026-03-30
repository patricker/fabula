# Fabula Golden Test Specification

Exhaustive behavioral coverage derived from Felt (ICIDS 2019), Winnow (AIIDE 2021),
their reference implementations, and the Winnow paper's figures.

**Coverage status key:**
- COVERED = test exists in `integration.rs`, `edge_cases.rs`, or `fabula-test-suite/`
- NEW = test must be written

---

## Category 1: Compiler / Pattern Construction

These tests verify that patterns are correctly built — correct stage count, clause
count, variable extraction, negation window bounds, etc. Winnow's `tests.js` compiles
4 patterns and renders their DataScript query output. Fabula uses a builder API instead
of a text DSL, but the structural equivalences must be verified.

### 1.1 violationOfHospitality pattern structure (NEW)

**Source:** `winnow-ref/tests.js` lines 2-19, `winnow-ref/compiler.js`

**Winnow DSL:**
```clj
(pattern violationOfHospitality
  (event ?e1 where eventType: enterTown, actor: ?guest)
  (event ?e2 where eventType: showHospitality, actor: ?host, target: ?guest,
    ?host.value: communalism)
  (event ?e3 where tag: harm, actor: ?host, target: ?guest)
  (unless-event ?eMid between ?e1 ?e3 where eventType: leaveTown, actor: ?guest))
```

**Test: `pattern_voh_has_correct_structure`**
- Build pattern via `PatternBuilder`
- Assert: 3 stages
- Assert: stage 0 anchor = "e1", stage 1 anchor = "e2", stage 2 anchor = "e3"
- Assert: stage 0 has 2 clauses (eventType + actor bind)
- Assert: stage 1 has 3 clauses (eventType + actor bind + target bind)
  - NOTE: Winnow also has `?host.value: communalism` (dotted lvar access).
    Fabula would express this as an additional clause. See test 1.5.
- Assert: stage 2 has 3 clauses (tag + actor bind + target bind)
- Assert: 1 negation, with `between_start = "e1"`, `between_end = "e3"`
- Assert: negation has 2 clauses (eventType + actor bind)

### 1.2 twoImpulsiveBetrayals pattern structure (NEW)

**Source:** `winnow-ref/tests.js` lines 22-32

**Winnow DSL:**
```clj
(pattern twoImpulsiveBetrayals
  (event ?e1 where eventType: betray, actor: ?char, ?char.trait: impulsive)
  (event ?e2 where eventType: betray, actor: ?char)
  (unless-event where actor: ?char))
```

**Test: `pattern_two_impulsive_betrayals_structure`**
- Build pattern via `PatternBuilder`
- Assert: 2 stages
- Assert: stage 0 has 3 clauses (eventType, actor bind, and trait check on char)
- Assert: stage 1 has 2 clauses (eventType, actor bind)
- Assert: 1 negation, with DEFAULT bounds (first=e1, last=e2 — Winnow's default
  `between` when none is specified)
- Assert: negation has 1 clause (actor bind to existing char)

**Key detail:** Winnow defaults `unless-event` bounds to `between ?firstEvent ?lastEvent`
when no explicit `between` is given. In fabula this maps to `unless_global`.

### 1.3 romanticFailureThenSuccess pattern structure (NEW)

**Source:** `winnow-ref/tests.js` lines 34-45

**Winnow DSL:**
```clj
(pattern romanticFailureThenSuccess
  (event ?e1 where tag: negative, tag: romantic, (not tag: major), actor: ?char)
  (event ?e2 where tag: negative, tag: romantic, actor: ?char)
  (event ?e3 where tag: positive, tag: romantic, actor: ?char))
```

**Test: `pattern_romantic_failure_structure`**
- Build pattern via `PatternBuilder`
- Assert: 3 stages, no negation blocks
- Assert: stage 0 has 4 clauses: tag=negative, tag=romantic, NOT tag=major, actor bind
- Assert: stage 1 has 3 clauses: tag=negative, tag=romantic, actor bind
- Assert: stage 2 has 3 clauses: tag=positive, tag=romantic, actor bind

**Key detail:** `(not tag: major)` is **inline negation** — the event must NOT have
this tag. This is different from `unless-event` (temporal negation window). Fabula
expresses this as a negated clause within the stage.

### 1.4 criticismOfHypocrisy pattern structure (NEW)

**Source:** `winnow-ref/tests.js` lines 47-58

**Winnow DSL:**
```clj
(pattern criticismOfHypocrisy
  (event ?e1 where actor: ?hypocrite, (eventHarmsHeldValue ?e1 ?hypocrite))
  (event ?e2 where eventType: criticize, actor: ?critic, target: ?hypocrite,
    (opposedValues ?hypocrite ?critic)))
```

**Test: `pattern_criticism_of_hypocrisy_structure`**
- Build pattern via `PatternBuilder`
- Assert: 2 stages, no negation blocks
- Assert: stage 0 has at least 1 clause (actor bind) + rule-equivalent clauses
- Assert: stage 1 has at least 3 clauses (eventType, actor bind, target bind) + rule-equivalent

**Key detail:** `(eventHarmsHeldValue ?e1 ?hypocrite)` and `(opposedValues ?hypocrite ?critic)`
are Datalog rules in Winnow. Fabula has no rule system; these would be expressed as
multi-clause property lookups or external callbacks. This test documents the gap.

### 1.5 Dotted lvar access compilation (NEW)

**Source:** `winnow-ref/compiler.js` lines 165-212 (`compileAttrValuePair`),
`winnow-ref/tests.js` line 10 (`?host.value: communalism`)

**Test: `pattern_dotted_lvar_translates_to_property_clause`**
- In Winnow: `?host.value: communalism` compiles to `[?host "value" "communalism"]`
- In fabula: this must be expressed as an additional clause in the stage that
  follows the `host` variable from the bound entity
- Build a pattern with a "property check" clause on a bound variable
- Assert the clause exists and references the correct source node and label

### 1.6 Inline negation compilation (NEW)

**Source:** `winnow-ref/compiler.js` lines 87-103, `winnow-ref/tests.js` line 39

**Test: `pattern_inline_not_creates_negated_clause`**
- In Winnow: `(not tag: major)` within an event clause compiles to `(not [?e1 "tag" "major"])`
- Build a fabula stage with a negated clause
- Assert the clause is marked as negated
- Assert it does NOT create a separate negation block (it's stage-local)

---

## Category 2: Batch Evaluation — Violation of Hospitality

The canonical 3-stage pattern from both the Felt README and all Winnow documentation.

### 2.1 Three stages match correctly (COVERED)

**Source:** `winnow-ref/tests.js` lines 106-181, Felt README, Winnow README

**Existing tests:**
- `integration.rs::batch_hospitality_matches`
- `fabula-test-suite/hospitality.rs::batch_hospitality_matches`

**Graph:** enterTown(alice, t=1) → showHospitality(bob→alice, t=2) → harm(bob→alice, t=3)
**Expected:** 1 match, guest=alice, host=bob

### 2.2 Guest leaving negates the match (COVERED)

**Source:** `winnow-ref/tests.js` sifting test "violationOfHospitality_2x" line 285

**Existing tests:**
- `integration.rs::batch_hospitality_negated_when_guest_leaves`
- `fabula-test-suite/hospitality.rs::batch_hospitality_negated_when_guest_leaves`

**Graph:** enter(alice, t=1) → showHospitality(bob→alice, t=2) → leaveTown(alice, t=2.5) → harm(bob→alice, t=3)
**Expected:** 0 matches (negation fires)

### 2.3 Unrelated character leaving does NOT negate (COVERED)

**Source:** Winnow semantics (bound variable consistency in negation)

**Existing tests:**
- `integration.rs::batch_hospitality_unrelated_leave_doesnt_negate`
- `fabula-test-suite/hospitality.rs::batch_hospitality_unrelated_leave`

**Graph:** enter(alice) → showHospitality(bob→alice) → leaveTown(charlie) → harm(bob→alice)
**Expected:** 1 match (charlie leaving doesn't kill alice's pattern)

### 2.4 Multiple guests create multiple matches (NEW)

**Source:** Implied by Winnow's variable binding semantics

**Test: `batch_hospitality_two_guests`**
**Graph:**
- enterTown(alice, t=1), enterTown(bob, t=2)
- showHospitality(eve→alice, t=3), showHospitality(eve→bob, t=4)
- harm(eve→alice, t=5), harm(eve→bob, t=6)
**Expected:** 2 matches — one for alice, one for bob, both with host=eve

### 2.5 Missing intermediate stage means no match (NEW)

**Source:** Implied by 3-stage sequential requirement

**Test: `batch_hospitality_missing_middle_stage`**
**Graph:**
- enterTown(alice, t=1), harm(bob→alice, t=3) — NO showHospitality
**Expected:** 0 matches

### 2.6 Hospitality with dotted lvar property check (NEW)

**Source:** `winnow-ref/tests.js` line 10 (`?host.value: communalism`)

**Test: `batch_hospitality_host_value_check`**
**Pattern:** Same as standard but stage 2 includes a clause checking that the host
entity has a `value` property equal to `communalism`.
**Graph (match):**
- enterTown(alice, t=1)
- bob has value=communalism
- showHospitality(bob→alice, t=2)
- harm(bob→alice, t=3)
**Expected:** 1 match

**Graph (no match):**
- Same but bob has value=individualism
**Expected:** 0 matches

---

## Category 3: Batch Evaluation — Two Impulsive Betrayals

### 3.1 Same impulsive character betrays twice with no actions between (NEW)

**Source:** `winnow-ref/tests.js` lines 22-32, Felt paper (ICIDS 2019)

**Test: `batch_two_impulsive_betrayals_match`**
**Pattern:**
```
stage e1: eventType=betray, actor=?char, ?char.trait=impulsive
stage e2: eventType=betray, actor=?char
unless_global: actor=?char (any event by the character)
```
**Graph:**
- alice has trait=impulsive
- betray(alice, t=1)
- betray(alice, t=2)
**Expected:** 1 match, char=alice

### 3.2 Intervening action by the same character kills the match (NEW)

**Source:** `winnow-ref/tests.js` line 32 (`(unless-event where actor: ?char)`)

**Test: `batch_two_impulsive_betrayals_intervening_action_blocks`**
**Graph:**
- alice has trait=impulsive
- betray(alice, t=1)
- getCoffee(alice, t=2) — intervening action
- betray(alice, t=3)
**Expected:** 0 matches (the global negation kills any pattern where the character acts between betrayals)

### 3.3 Intervening action by DIFFERENT character does NOT block (NEW)

**Source:** Winnow negation variable binding semantics

**Test: `batch_two_impulsive_betrayals_different_actor_doesnt_block`**
**Graph:**
- alice has trait=impulsive
- betray(alice, t=1)
- getCoffee(bob, t=2) — bob acts, not alice
- betray(alice, t=3)
**Expected:** 1 match (bob's action doesn't bind to ?char=alice)

### 3.4 Non-impulsive character does not match (NEW)

**Source:** `winnow-ref/tests.js` line 28 (`?char.trait: impulsive`)

**Test: `batch_two_betrayals_non_impulsive_no_match`**
**Graph:**
- alice has trait=cautious (NOT impulsive)
- betray(alice, t=1), betray(alice, t=2)
**Expected:** 0 matches (trait check fails)

---

## Category 4: Batch Evaluation — Romantic Failure Then Success

### 4.1 Standard romantic arc matches (COVERED)

**Source:** `winnow-ref/tests.js` lines 34-45

**Existing tests:**
- `integration.rs::batch_romantic_arc`
- `fabula-test-suite/romantic_arc.rs::batch_romantic_arc_matches`

### 4.2 Different characters no match (COVERED)

**Existing tests:**
- `integration.rs::batch_romantic_arc_different_characters_no_match`
- `fabula-test-suite/romantic_arc.rs::batch_romantic_arc_different_characters`

### 4.3 Inline negation: first negative event must NOT be major (NEW)

**Source:** `winnow-ref/tests.js` line 39 (`(not tag: major)`)

**Test: `batch_romantic_arc_first_negative_not_major`**
**Pattern:** Stage 1 has inline negation `NOT tag=major` (the original Winnow pattern).
**Graph (match):**
- flirtWith_rejected(mira, t=1) — tags: romantic, negative, awkward (no major)
- askOut_rejected(mira, t=2) — tags: romantic, negative, awkward, major
- flirtWith_accepted(mira, t=3) — tags: romantic, positive
**Expected:** 1 match (first event is negative+romantic WITHOUT major, second can be major)

**Graph (no match):**
- breakUp(mira, t=1) — tags: romantic, negative, major (HAS major)
- askOut_rejected(mira, t=2)
- flirtWith_accepted(mira, t=3)
**Expected:** 0 matches (first event has tag=major, inline negation blocks it)

### 4.4 Multiple romantic events produce combinatorial matches (NEW)

**Source:** Winnow binding semantics (all valid bindings returned)

**Test: `batch_romantic_arc_combinatorial`**
**Graph:**
- negative+romantic(mira, t=1)
- negative+romantic(mira, t=2)
- negative+romantic(mira, t=3)
- positive+romantic(mira, t=4)
**Expected:** 3 matches
- (e1=t1, e2=t2, e3=t4)
- (e1=t1, e2=t3, e3=t4)
- (e1=t2, e2=t3, e3=t4)

---

## Category 5: Batch Evaluation — Criticism of Hypocrisy

### 5.1 Hypocrite acts against own values then gets criticized (NEW)

**Source:** `winnow-ref/tests.js` lines 47-58

**Test: `batch_criticism_of_hypocrisy`**
**Pattern:**
```
stage e1: actor=?hypocrite, some evidence that event harms their held value
stage e2: eventType=criticize, actor=?critic, target=?hypocrite,
          ?critic and ?hypocrite have opposed values
```
**Graph:**
- alice has value=honesty
- bob has value=deception (opposed to honesty)
- alice performs event that harms honesty (e.g., lies, t=1)
- bob criticizes alice (t=2)
**Expected:** 1 match, hypocrite=alice, critic=bob

**Note:** In Winnow, `eventHarmsHeldValue` and `opposedValues` are Datalog rules.
In fabula, these must be expressed as concrete property clauses or multi-hop
traversals. The test documents the behavioral requirement; the pattern
construction may differ from Winnow's syntax.

---

## Category 6: Incremental Matching — Core Algorithm

Derived from `winnow-ref/runner.js` (`tryAdvance`) and the paper's figure
(`winnow-ref/figure.html`).

### 6.1 Three-stage incremental completion (COVERED)

**Source:** `winnow-ref/runner.js`, paper Figure 1

**Existing test:** `integration.rs::incremental_hospitality_three_stages`

### 6.2 Irrelevant event leaves pool unchanged (COVERED)

**Source:** `winnow-ref/figure.html` event 2 (irrelevantEvent by Mia)

**Existing test:** `integration.rs::irrelevant_edges_produce_no_events`

**Scenario:** After e1 matches, push an irrelevant event. Pool size unchanged, no
advance/complete/die events emitted. Paper figure row 2:
"An irrelevant event occurs. The pool of partial matches is unchanged."

### 6.3 Fork on second host — multiple partial matches (NEW)

**Source:** `winnow-ref/figure.html` event 5 (Jake shows hospitality to Yann)

**Test: `incremental_second_host_forks_partial_match`**
**Sequence:**
1. enterTown(yann, t=1) → 1 new partial match (guest=yann)
2. showHospitality(eve→yann, t=3) → fork: 1 new PM (host=eve), original survives
3. showHospitality(jake→yann, t=5) → fork: 1 new PM (host=jake), original PM survives
**Expected after step 3:**
- Original PM (e1 only) still active
- PM with host=eve still active
- PM with host=jake still active
- Total active PMs for this pattern: 3

### 6.4 Completed match extracted from pool (COVERED partially)

**Source:** `winnow-ref/figure.html` event 4 (Eve pickpockets Yann → complete)
`winnow-ref/tests.js` line 197 (filter out complete/die matches)

**Existing test:** `integration.rs::drain_completed_removes_matches`

**Additional test needed: `incremental_completed_match_has_correct_bindings`** (NEW)
**Sequence:** Full hospitality sequence.
**Expected:** Completed match has guest=yann, host=eve, e1/e2/e3 all bound.

### 6.5 Guest leaves → all related partial matches die (NEW)

**Source:** `winnow-ref/figure.html` event 6 (Yann leaves town)

**Test: `incremental_guest_leaves_kills_all_related_pms`**
**Sequence:**
1. enterTown(yann, t=1)
2. showHospitality(eve→yann, t=3) → now 2 PMs with guest=yann
3. showHospitality(jake→yann, t=5) → now 3 PMs with guest=yann
4. leaveTown(yann, t=6) → ALL three PMs should die
**Expected:** 0 active PMs remaining. 3 Negated events emitted.

**Key detail from figure:** "Yann leaves town. We mark all remaining partial matches
involving Yann as dead and remove them from the pool."

### 6.6 Harm after death has no effect (NEW)

**Source:** `winnow-ref/figure.html` event 7 (Jake harms Yann but no valid PMs left)

**Test: `incremental_harm_after_all_pms_dead_no_effect`**
**Sequence:** Same as 6.5, then add harm(jake→yann, t=7).
**Expected:** No events emitted. No completed matches. Pool remains at the
one empty prototype PM.

Paper figure: "Jake harms Yann—but there's no valid partial matches left for
this event to attach to, so nothing happens."

### 6.7 Partial match survival — original PM always returned (COVERED partially)

**Source:** `winnow-ref/runner.js` line 78: `return [partialMatch].concat(newPartialMatches)`

**Test: `incremental_original_pm_survives_after_advance`** (NEW)
**Sequence:** enterTown(alice, t=1) → showHospitality(bob→alice, t=2)
**Expected:**
- After step 1: PM with guest=alice (stage 1 bound), PLUS original empty PM
- After step 2: PM with guest=alice+host=bob (stages 1+2 bound),
  PLUS PM with only guest=alice (stages 1 bound, waiting for different host),
  PLUS original empty PM

This tests the critical Winnow behavior where "the same clause might match a
different future event with different bindings."

### 6.8 Dead and complete PMs don't advance further (NEW)

**Source:** `winnow-ref/runner.js` lines 31-32:
`if (partialMatch.lastStep === "die") return [partialMatch];`
`if (partialMatch.lastStep === "complete") return [partialMatch];`

**Test: `incremental_dead_and_complete_pms_inert`**
- Create a completed PM and a dead PM
- Push new events
- Assert neither PM changes state or spawns children

---

## Category 7: Incremental Matching — Negation

### 7.1 Negation kills partial match (COVERED)

**Existing test:** `integration.rs::incremental_hospitality_negation_kills`

### 7.2 Unrelated character's event doesn't kill (COVERED)

**Existing test:** `integration.rs::incremental_hospitality_unrelated_leave_doesnt_kill`

### 7.3 Negation checked before advance (NEW)

**Source:** `winnow-ref/runner.js` lines 40-54 (negation check comes before advance check)

**Test: `incremental_negation_checked_before_advance`**
**Pattern:** enter → show_hospitality → harm, unless leaveTown between e1 and e3
**Sequence:**
1. enterTown(alice, t=1) — PM advances to stage 1
2. showHospitality(bob→alice, t=2) — PM advances to stage 2
3. Push an event that is BOTH a leaveTown(alice) AND could theoretically match
   stage 3 (some contrived dual-matching event).
**Expected:** PM is killed by negation, NOT completed. Negation check has priority.

**Why this matters:** `tryAdvance` checks negation constraints BEFORE trying to
advance to the next stage. An event that matches both a negation constraint and
the next stage must kill, not complete.

### 7.4 Negation applicability — only when window is open (NEW)

**Source:** `winnow-ref/runner.js` lines 8-12 (`applicableConstraint`)

**Test: `incremental_negation_only_applicable_when_window_open`**
**Pattern:** e1 → e2, unless_between(e1, e2) where actor=?char and eventType=cancel
**Sequence:**
1. Push leaveTown(alice, t=0) BEFORE any PM exists — should not affect anything
2. enterTown(alice, t=1) — PM initiates, negation window now open (start bound, end not)
3. cancel(alice, t=2) — should kill PM because window is open
**Expected:** PM killed at step 3, not affected at step 1.

The runner checks:
- `hasBinding(pm, constraint.betweenStart)` → true (e1 is bound)
- `hasBinding(pm, constraint.betweenEnd)` → false (e2 not yet bound)
- Therefore constraint is applicable

### 7.5 Negation NOT applicable once window closes (NEW)

**Source:** `winnow-ref/runner.js` line 10: `if (hasBinding(pm, constraint.betweenEnd)) return false;`

**Test: `incremental_negation_not_applicable_after_window_closes`**
**Pattern:** e1 → e2 → e3, unless_between(e1, e2) where eventType=cancel
**Sequence:**
1. event matching e1 (t=1)
2. event matching e2 (t=2) — window for "between e1 and e2" is now CLOSED
3. cancel event (t=3) — should NOT kill, because the window is closed
4. event matching e3 (t=4)
**Expected:** PM completes at step 4. The cancel at step 3 is outside the window.

### 7.6 Selective negation kills only matching-variable PMs (COVERED)

**Existing test:** `edge_cases.rs::incremental_negation_kills_only_matching_variable_bindings`

### 7.7 Death details include constraint and event info (COVERED)

**Existing test:** `integration.rs::negation_event_includes_details`

---

## Category 8: Incremental Matching — Winnow `tests.js` Sifting Test

The file `winnow-ref/tests.js` defines a specific sifting test scenario
(`violationOfHospitality_2x`) with 7 events. This must be ported exactly.

### 8.1 Full 7-event sequence (NEW)

**Source:** `winnow-ref/tests.js` lines 278-289

**Test: `incremental_winnow_hospitality_2x_full_sequence`**
**Pattern:** violationOfHospitality (compiled against all 4 test patterns)
**Events in order:**
```
1. {eventType: "enterTown", actor: 1}
2. {eventType: "irrelevantEvent", actor: 3}
3. {eventType: "showHospitality", actor: 2, target: 1}
4. {eventType: "irrelevantEvent", actor: 1}
5. {eventType: "stealFrom", tags: ["harm"], actor: 2, target: 1}
6. {eventType: "leaveTown", actor: 1}
7. {eventType: "attack", tags: ["harm"], actor: 2, target: 1}
```

**Expected behavior at each step:**
1. PM for violationOfHospitality advances (e1 bound: guest=1)
2. No change to VoH PMs (irrelevant)
3. PM advances (e2 bound: host=2)
4. No change (irrelevant — actor is the guest, but eventType doesn't match any stage)
5. PM completes (e3 bound via tag=harm, actor=2, target=1)
   Also: the original PM with only e1 bound forks a new advance at e2 wait
6. PM(s) still waiting for e3 are killed by negation (leaveTown by guest=1)
   The completed match from step 5 is NOT affected (it's already complete)
7. No valid PMs left that could match this harm event (all killed in step 6)

**Key insight:** Event 5 produces a complete match AND leaves surviving PMs
(the original PM that matched e1 still waits for other hosts). Event 6 kills
those survivors. Event 7 has no effect.

### 8.2 `testGetAllMatches` function equivalent (NEW)

**Source:** `winnow-ref/tests.js` lines 206-218

**Test: `batch_winnow_test_get_all_matches`**
**Setup:** Create 5 characters (Mira, Emin, Sarah, Vincent, Zach), all with value=communalism.
**Events:**
```
{eventType: "enterTown", actor: 1}
{eventType: "showHospitality", actor: 2, target: 1}
{eventType: "stealFrom", tags: ["harm"], actor: 2, target: 1}
{eventType: "attack", tags: ["harm"], actor: 2, target: 1}
```
**Run against all 4 compiled patterns.**
**Expected:** At least 1 complete match for violationOfHospitality. Verify the
bindings: guest = character 1, host = character 2.

---

## Category 9: Temporal Ordering

### 9.1 Batch rejects wrong temporal order (COVERED)

**Existing test:** `integration.rs::batch_rejects_wrong_temporal_order`

### 9.2 Incremental rejects temporally inverted match (COVERED)

**Existing test:** `edge_cases.rs::incremental_temporal_ordering_enforced`

### 9.3 Same-timestamp events cannot sequence (COVERED)

**Existing test:** `edge_cases.rs::events_at_identical_timestamps_cannot_sequence`

### 9.4 Winnow EID-based ordering → fabula interval.start ordering (NEW)

**Source:** `winnow-ref/compiler.js` line 391: `[(< ${eventClauses.map(ec => ec.eventLvar).join(" ")})]`

**Test: `temporal_strict_less_than_between_stages`**
**Pattern:** 3 stages (e1, e2, e3)
**Graph:** Events at t=1, t=2, t=3 (correct order)
**Expected:** Match succeeds.
**Graph:** Events at t=1, t=1, t=3 (e1 and e2 same time)
**Expected:** Match fails (strict less-than, not less-than-or-equal).

Winnow emits `[(< ?e1 ?e2 ?e3)]` which is strict less-than on entity IDs.
Fabula should enforce strict less-than on interval start times.

### 9.5 Long temporal gaps still match (NEW)

**Source:** Winnow semantics — only ordering matters, not distance

**Test: `temporal_large_gap_still_matches`**
**Pattern:** e1 then e2
**Graph:** e1 at t=1, e2 at t=1000000
**Expected:** 1 match (no maximum gap constraint)

---

## Category 10: Negation Edge Cases

### 10.1 Empty negation body doesn't block (COVERED)

**Existing test:** `edge_cases.rs::empty_negation_no_clauses`

### 10.2 Negation before window start doesn't block (COVERED)

**Existing test:** `edge_cases.rs::negation_before_window_start_does_not_block`

### 10.3 Negation at exact window boundary (COVERED)

**Existing test:** `edge_cases.rs::negation_at_exact_window_boundary`

### 10.4 unless_global on single stage (COVERED)

**Existing test:** `edge_cases.rs::unless_global_single_stage_works`

### 10.5 unless_after blocks match (COVERED)

**Existing test:** `integration.rs::batch_unless_after_blocks_match`

### 10.6 unless_global blocks match (COVERED)

**Existing test:** `integration.rs::batch_unless_global`

### 10.7 Double negation — two unless-event blocks (NEW)

**Source:** Winnow supports multiple `unless-event` clauses per pattern

**Test: `batch_double_negation`**
**Pattern:**
```
stage e1: eventType=promise, actor=?char
stage e2: eventType=fulfill, actor=?char
unless_between(e1, e2): eventType=break_promise, actor=?char
unless_between(e1, e2): eventType=forget, actor=?char
```
**Graph (match):** promise(alice, t=1), fulfill(alice, t=3)
**Expected:** 1 match

**Graph (blocked by first negation):** promise(alice, t=1), break_promise(alice, t=2), fulfill(alice, t=3)
**Expected:** 0 matches

**Graph (blocked by second negation):** promise(alice, t=1), forget(alice, t=2), fulfill(alice, t=3)
**Expected:** 0 matches

### 10.8 Negation with multi-clause body (NEW)

**Source:** `winnow-ref/compiler.js` — unless-event body supports multiple attr/val pairs

**Test: `batch_negation_multi_clause_body`**
**Pattern:**
```
stage e1: eventType=enter, actor=?guest
stage e2: eventType=harm, actor=?host, target=?guest
unless_between(e1, e2): eventType=leave, actor=?guest, destination=?town
```
(The negation has TWO clauses: eventType AND destination)
**Graph (match):** enter(alice) → leave(alice, destination=otherTown) → harm(bob→alice)
Wait — this should depend on whether destination matters. The key point:
**A negation event must match ALL clauses in the negation body.**
If the leave event matches eventType=leave and actor=?guest but does NOT match
the destination clause, the negation should NOT fire.

### 10.9 Negation after pattern completion (NEW)

**Source:** Winnow semantics — completed matches are final

**Test: `incremental_negation_after_completion_no_effect`**
**Pattern:** e1 → e2, unless_between(e1, e2) where eventType=cancel
**Sequence:**
1. Event matches e1 (t=1)
2. Event matches e2 (t=2) → PM completes
3. cancel event (t=3)
**Expected:** Completed match survives. Negation events that arrive after completion
do not retroactively invalidate completed matches.

---

## Category 11: Value Constraints

### 11.1 ValueConstraint::Lt matches (COVERED)

**Existing test:** `integration.rs::batch_value_constraint_lt`

### 11.2 ValueConstraint::Lt no match (COVERED)

**Existing test:** `integration.rs::batch_value_constraint_lt_no_match`

### 11.3 ValueConstraint::Gt (NEW)

**Test: `batch_value_constraint_gt`**
**Graph:** loyalty=0.8
**Pattern:** constraint Gt(0.5) on loyalty
**Expected:** 1 match

### 11.4 ValueConstraint::Between (NEW)

**Test: `batch_value_constraint_between`**
**Graph:** loyalty=0.5
**Pattern:** constraint Between(0.3, 0.7) on loyalty
**Expected:** 1 match

**Graph:** loyalty=0.1
**Expected:** 0 matches

### 11.5 ValueConstraint::Eq (NEW)

**Test: `batch_value_constraint_eq`**
**Graph:** status="active"
**Pattern:** constraint Eq("active")
**Expected:** 1 match

### 11.6 Cross-variant constraint (COVERED)

**Existing test:** `edge_cases.rs::cross_variant_constraint_between`

### 11.7 NaN comparisons (COVERED)

**Existing test:** `edge_cases.rs::memvalue_nan_comparisons`

### 11.8 Reversed Between bounds (COVERED)

**Existing test:** `edge_cases.rs::between_reversed_bounds_never_matches`

### 11.9 Equal Between bounds (COVERED)

**Existing test:** `edge_cases.rs::between_equal_bounds_matches_only_exact`

---

## Category 12: Gap Analysis (whyNot)

### 12.1 Empty graph shows first stage unmatched (COVERED)

**Existing test:** `integration.rs::why_not_empty_graph`

### 12.2 Nonexistent pattern returns None (COVERED)

**Existing test:** `edge_cases.rs::why_not_nonexistent_pattern`

### 12.3 Matched pattern shows all matched (COVERED)

**Existing test:** `edge_cases.rs::why_not_matched_pattern_shows_all_matched`

### 12.4 Stops at first unmatched stage (COVERED)

**Existing test:** `edge_cases.rs::why_not_stops_at_first_unmatched_stage`

### 12.5 Partially matched stage shows which clauses fail (NEW)

**Source:** Felt's `whyNot` function tests clauses individually

**Test: `gap_analysis_partially_matched_stage`**
**Pattern:** stage with 3 clauses: eventType=harm, actor=?attacker, tag=combat
**Graph:** ev1 has eventType=harm, actor=bob, but NOT tag=combat
**Expected:** `why_not` reports stage 0 as PartiallyMatched:
- clause 0 (eventType): matched
- clause 1 (actor): matched
- clause 2 (tag): unmatched

### 12.6 Gap analysis with negation (NEW)

**Source:** Felt paper: "whyNot debugging function testing pattern clauses individually"

**Test: `gap_analysis_blocked_by_negation`**
**Pattern:** e1 → e2, unless_between(e1, e2): eventType=cancel
**Graph:** All positive stages match, BUT a cancel event exists in the window.
**Expected:** `why_not` should indicate that all stages matched but negation blocked.

### 12.7 Multi-stage gap analysis propagates bindings (NEW)

**Source:** Felt's clause-by-clause testing

**Test: `gap_analysis_second_stage_fails_due_to_binding`**
**Pattern:** e1: eventType=enter, actor=?person. e2: eventType=leave, actor=?person.
**Graph:** enter(alice, t=1), leave(bob, t=2) — different actors.
**Expected:** Stage 0 matches (person=alice). Stage 1 unmatched because no leave event
has actor=alice.

---

## Category 13: Multi-Pattern

### 13.1 Multiple patterns fire independently (COVERED)

**Existing test:** `integration.rs::multiple_patterns_fire_independently`

### 13.2 All 4 Winnow patterns registered simultaneously (NEW)

**Source:** `winnow-ref/tests.js` lines 60-65, 68-69 (all 4 patterns compiled together)

**Test: `multi_pattern_all_four_winnow_patterns`**
**Setup:** Register violationOfHospitality, twoImpulsiveBetrayals,
romanticFailureThenSuccess, criticismOfHypocrisy.
**Graph:** Events that match violationOfHospitality but NOT the others.
**Expected:** Exactly 1 match total, for violationOfHospitality.
Other patterns have 0 complete matches but may have partial matches.

### 13.3 Cross-pattern interaction: events advance different patterns (NEW)

**Source:** Winnow benchmarks (multiple patterns in pool simultaneously)

**Test: `multi_pattern_shared_events_advance_different_patterns`**
**Setup:** Pattern A = "enter then harm" (2 stages). Pattern B = "enter then help" (2 stages).
**Graph:** enter(alice, t=1), help(alice, t=2), harm(alice, t=3)
**Expected:**
- Pattern B completes (enter → help)
- Pattern A completes (enter → harm)
- Both advance independently from the same enterTown event

---

## Category 14: Edge Cases — Empty Inputs

### 14.1 Empty pattern no stages (COVERED)

**Existing test:** `edge_cases.rs::empty_pattern_no_stages`

### 14.2 Empty stage no clauses (COVERED)

**Existing test:** `edge_cases.rs::empty_stage_no_clauses`

### 14.3 Empty stage incremental never advances (COVERED)

**Existing test:** `edge_cases.rs::empty_stage_incremental_never_advances`

### 14.4 Empty graph with registered patterns (COVERED)

**Existing test:** `edge_cases.rs::empty_graph_with_registered_patterns`

---

## Category 15: Edge Cases — Duplicates and Self-Reference

### 15.1 Duplicate edges produce duplicate matches (COVERED)

**Existing test:** `edge_cases.rs::duplicate_edges_produce_duplicate_matches`

### 15.2 Same event cannot satisfy two stages (COVERED)

**Existing test:** `edge_cases.rs::same_event_cannot_satisfy_two_stages`

### 15.3 Variable consistency across stages (COVERED)

**Existing test:** `edge_cases.rs::variable_consistency_across_stages`

### 15.4 Self-referential edge (COVERED)

**Existing test:** `edge_cases.rs::self_referential_edge`

### 15.5 Source equals target variable (COVERED)

**Existing test:** `edge_cases.rs::source_equals_target_variable_self_loop_only`

---

## Category 16: Edge Cases — Scale

### 16.1 Ten-stage pattern (COVERED)

**Existing test:** `edge_cases.rs::ten_stage_pattern`

### 16.2 Partial matches accumulate unboundedly (COVERED)

**Existing test:** `edge_cases.rs::partial_matches_accumulate_unboundedly`

### 16.3 Large graph batch evaluation (COVERED)

**Existing test:** `edge_cases.rs::large_graph_batch_evaluation`

### 16.4 Winnow benchmark: pool sizes 10-1000 (NEW)

**Source:** `winnow-ref/benchmark.html` lines 150-209

**Test: `benchmark_incremental_pool_scaling`**
- Register violationOfHospitality pattern
- Create N initial empty partial matches (N = 10, 50, 100)
- Push 100 random events from the allEventSpecs list
- Assert: no panics, pool size stays bounded, completes in reasonable time
- This is a performance/stress test, not a correctness test

---

## Category 17: Edge Cases — Batch vs Incremental Consistency

### 17.1 Out-of-order insertion: incremental misses, batch finds (COVERED)

**Existing test:** `edge_cases.rs::out_of_order_insertion_incremental_misses_match`

### 17.2 In-order: batch and incremental agree (COVERED)

**Existing test:** `edge_cases.rs::batch_and_incremental_agree_on_simple_case`

### 17.3 Negation consistency: batch and incremental agree (NEW)

**Source:** Winnow's dual execution model (batch query + incremental runner)

**Test: `batch_incremental_negation_consistency`**
**Pattern:** violationOfHospitality
**Graph built incrementally in chronological order:**
1. enterTown(alice, t=1)
2. showHospitality(bob→alice, t=2)
3. leaveTown(alice, t=3) — should kill
4. harm(bob→alice, t=4)
**Expected:** Both batch and incremental produce 0 matches.

**Same graph without leaveTown:**
**Expected:** Both batch and incremental produce 1 match.

### 17.4 Multiple matches consistency (NEW)

**Source:** Winnow binding semantics

**Test: `batch_incremental_multi_match_consistency`**
**Pattern:** 2-stage: eventType=enter → eventType=leave, same actor
**Graph:**
- enter(alice, t=1), enter(bob, t=2), leave(alice, t=3), leave(bob, t=4)
**Expected:** Both batch and incremental produce 2 matches (alice's enter→leave, bob's enter→leave).

---

## Category 18: drain_completed

### 18.1 drain on empty engine (COVERED)

**Existing test:** `edge_cases.rs::drain_completed_on_empty_engine`

### 18.2 drain removes completed, preserves active (COVERED)

**Existing tests:**
- `integration.rs::drain_completed_removes_matches`
- `edge_cases.rs::drain_completed_preserves_active_matches`

### 18.3 drain is idempotent (NEW)

**Test: `drain_completed_idempotent`**
- Complete a match
- First drain: returns 1 match
- Second drain: returns 0 matches
- Assert engine state is consistent

### 18.4 drain during active matching (NEW)

**Test: `drain_completed_interleaved_with_events`**
- Push events to complete match A
- Drain → get match A
- Push events to complete match B
- Drain → get match B (not A again)

---

## Category 19: Single-Stage Patterns

### 19.1 Single-stage completes immediately (COVERED)

**Existing test:** `integration.rs::incremental_single_stage_completes_immediately`

### 19.2 Single-stage with unless_after batch (COVERED)

**Existing test:** `edge_cases.rs::single_stage_with_unless_after_batch`

### 19.3 Single-stage with unless_after incremental consistency (COVERED)

**Existing test:** `edge_cases.rs::single_stage_with_unless_after_incremental_consistency`

---

## Category 20: Interval Algebra (Fabula Extension)

These are fabula-specific extensions beyond Felt/Winnow.

### 20.1 Zero-length interval (COVERED)

**Existing test:** `edge_cases.rs::interval_zero_length`

### 20.2 Open-ended intervals (COVERED)

**Existing test:** `edge_cases.rs::interval_open_ended_relation_always_none`

### 20.3 Intersects edge cases (COVERED)

**Existing test:** `edge_cases.rs::interval_intersects_edge_cases`

### 20.4 Allen Before relation for temporal constraint (NEW)

**Test: `allen_before_constraint_matches`**
**Pattern:** e1 Before e2 (explicit temporal constraint)
**Graph:** e1=[1,3), e2=[5,7) → Before holds
**Expected:** Match

**Graph:** e1=[1,5), e2=[3,7) → Overlaps, not Before
**Expected:** No match

### 20.5 Allen During relation (NEW)

**Test: `allen_during_constraint`**
**Pattern:** e1 During e2 (the assassination happened during the feast)
**Graph:** feast=[1,10), assassination=[3,5)
**Expected:** Match

### 20.6 Allen Overlaps relation (NEW)

**Test: `allen_overlaps_constraint`**
**Pattern:** e1 Overlaps e2
**Graph:** siege=[1,7), harvest=[5,10)
**Expected:** Match

### 20.7 Open-ended interval fallback to start comparison (COVERED)

**Existing test:** `edge_cases.rs::open_ended_interval_fails_non_before_temporal_constraints`

---

## Category 21: Felt Architectural Patterns (from Case Studies)

These are not direct test cases from code, but behavioral patterns implied by
the Felt paper's three case studies. They document what the sifting engine must
support for real-world use.

### 21.1 Starfreighter: Parametrized storylets as sifting patterns (NEW — design test)

**Source:** Felt paper section "Case Study: Starfreighter"

**Test: `design_storylet_preconditions_as_patterns`**
- A "storylet" is a narrative unit with preconditions
- The preconditions ARE sifting patterns (same query language)
- Verify: a pattern can serve as both "what happened?" (sifting) and
  "what can happen next?" (precondition check)
- Build a pattern that checks for a relationship state (e.g., trust > 0.5)
- Evaluate it as a sifting query → get matches
- Same pattern used as action precondition → same matches

### 21.2 CMCK: Reflection actions producing intent tokens (NEW — design test)

**Source:** Felt paper section "Case Study: CMCK"

**Test: `design_reflection_produces_intent_token`**
- Felt distinguishes "reflection actions" (sift → produce intent) from
  "external actions" (consume intent → change world)
- This is NOT in fabula's scope (no action system), but the sifting pattern
  for the reflection step IS in scope
- Verify: a pattern can detect "character X experienced event Y" and bind
  variables that would feed into an intent token

### 21.3 Diarytown: Cardinality-many tags (NEW)

**Source:** Felt paper: Diarytown uses DataScript with cardinality-many attributes

**Test: `batch_cardinality_many_tags`**
- An event entity has multiple tags (friendly, romantic, major)
- A pattern matching `tag=romantic` should find this event
- A pattern matching `tag=romantic AND tag=major` should find this event
- A pattern matching `tag=romantic AND tag=combat` should NOT find this event

This tests the MemGraph's ability to handle multi-valued edges (same source + label,
different values), which maps to DataScript's `:db.cardinality/many`.

---

## Category 22: Winnow Parser (if DSL is implemented)

These tests verify the text DSL parser, adapted from `winnow-ref/parser.js`.
Currently marked as "planned" in fabula's DESIGN.md.

### 22.1 Tokenizer: whitespace, commas, colons as delimiters (NEW)

**Source:** `winnow-ref/parser.js` lines 1-5

**Test: `parser_whitespace_comma_colon_as_delimiters`**
- Input: `eventType: enterTown, actor: ?guest`
- Expected tokens: `eventType`, `enterTown`, `actor`, `?guest`
- Commas and colons are whitespace in Winnow

### 22.2 Tokenizer: semicolon line comments (NEW)

**Source:** `winnow-ref/parser.js` lines 14-18

**Test: `parser_semicolon_comments`**
- Input containing `;; this is a comment`
- Expected: comment is stripped, surrounding tokens preserved

### 22.3 Tokenizer: string literals (NEW)

**Source:** `winnow-ref/parser.js` lines 33-53

**Test: `parser_string_literals`**
- Input: `"quoted string"`
- Expected: token with type=string, text="quoted string"

### 22.4 Parser: nested parentheses (NEW)

**Source:** `winnow-ref/parser.js` lines 68-88

**Test: `parser_nested_parens`**
- Input: `(pattern name (event ?e where (not tag major)))`
- Expected: correctly nested AST

### 22.5 Full round-trip: Winnow DSL → compiled pattern → batch evaluation (NEW)

**Source:** `winnow-ref/tests.js` — the full pipeline

**Test: `dsl_round_trip_violation_of_hospitality`**
- Parse the violationOfHospitality DSL text
- Compile to a fabula Pattern
- Evaluate against the standard hospitality graph
- Assert 1 match with correct bindings

---

## Summary Statistics

| Category | Total Tests | COVERED | NEW |
|----------|------------|---------|-----|
| 1. Pattern Construction | 6 | 0 | 6 |
| 2. Batch: Hospitality | 6 | 3 | 3 |
| 3. Batch: Two Betrayals | 4 | 0 | 4 |
| 4. Batch: Romantic Arc | 4 | 2 | 2 |
| 5. Batch: Hypocrisy | 1 | 0 | 1 |
| 6. Incremental Core | 8 | 2 | 6 |
| 7. Incremental Negation | 7 | 4 | 3 |
| 8. Winnow tests.js Replay | 2 | 0 | 2 |
| 9. Temporal Ordering | 5 | 3 | 2 |
| 10. Negation Edge Cases | 9 | 6 | 3 |
| 11. Value Constraints | 9 | 6 | 3 |
| 12. Gap Analysis | 7 | 4 | 3 |
| 13. Multi-Pattern | 3 | 1 | 2 |
| 14. Empty Inputs | 4 | 4 | 0 |
| 15. Duplicates/Self-Ref | 5 | 5 | 0 |
| 16. Scale | 4 | 3 | 1 |
| 17. Batch/Incr Consistency | 4 | 2 | 2 |
| 18. drain_completed | 4 | 2 | 2 |
| 19. Single-Stage | 3 | 3 | 0 |
| 20. Allen Intervals | 7 | 4 | 3 |
| 21. Felt Arch Patterns | 3 | 0 | 3 |
| 22. DSL Parser | 5 | 0 | 5 |
| **TOTAL** | **113** | **54** | **59** |

---

## Appendix A: Winnow Event Spec Catalog

All event types from `winnow-ref/tests.js` lines 106-138, for use in test generation:

| eventType | tags |
|-----------|------|
| getCoffeeWith | friendly |
| physicallyAttack | unfriendly, harms, major |
| disparagePublicly | unfriendly, harms |
| sendPostcard | friendly |
| insult | unfriendly |
| insultDismissively | unfriendly, highStatus |
| rejectSuperiority | unfriendly, lowStatus |
| flirtWith_accepted | romantic, positive |
| flirtWith_rejected | romantic, negative, awkward |
| askOut_accepted | romantic, positive, major |
| askOut_rejected | romantic, negative, awkward, major |
| propose_accepted | romantic, positive, major |
| propose_rejected | romantic, negative, awkward, major |
| breakUp | romantic, negative, major |
| buyLunchFor | friendly |
| inviteIntoGroup | highStatus, friendly, helps |
| shunFromGroup | highStatus, unfriendly, harms |
| apologizeTo | friendly |
| begForFavor | lowStatus |
| extortFavor | highStatus |
| callInFavor | highStatus |
| callInExtortionateFavor | highStatus, harms |
| askForHelp | lowStatus, friendly |
| deferToExpertise | career, lowStatus |
| deliberatelySabotage | career, unfriendly, harms, major |
| collab:phoneItIn | career, harms |
| collab:goAboveAndBeyond | career, helps |

## Appendix B: Winnow DataScript Schema

From `winnow-ref/tests.js` lines 93-104:

```javascript
{
  curse:  {":db/cardinality": ":db.cardinality/many"},
  value:  {":db/cardinality": ":db.cardinality/many"},
  actor:  {":db/valueType": ":db.type/ref"},
  cause:  {":db/valueType": ":db.type/ref"},
  source: {":db/valueType": ":db.type/ref"},
  target: {":db/valueType": ":db.type/ref"},
  projectContributor: {":db/valueType": ":db.type/ref", ":db/cardinality": ":db.cardinality/many"},
  tag:    {":db/cardinality": ":db.cardinality/many"},
}
```

Key implications for fabula:
- `actor`, `target`, `cause`, `source` are reference types → `add_ref` in MemGraph
- `tag`, `value`, `curse` are cardinality-many → same source+label can have multiple values
- `projectContributor` is both ref and cardinality-many

## Appendix C: Figure Walkthrough (Paper Figure)

The `winnow-ref/figure.html` walks through 7 events against the breakHospitality pattern.
This is the canonical incremental matching example from the Winnow AIIDE 2021 paper.

| Step | Event | Pool Change | Explanation |
|------|-------|------------|-------------|
| 0 | [initial] | 1 empty PM | One empty PM per registered pattern |
| 1 | enterTown(Yann) | +1 PM (accept) | Fork: empty PM stays, new PM with e1=1,guest=Yann |
| 2 | irrelevantEvent(Mia) | no change | Irrelevant event, pool unchanged |
| 3 | showHospitality(Eve→Yann) | +1 PM (accept) | Fork from PM_1: new PM with e2=3,host=Eve |
| 4 | pickpocket(Eve→Yann, harm) | +1 PM (complete) | Fork from PM_13: complete with e3=4 |
| 5 | showHospitality(Jake→Yann) | +1 PM (accept) | Fork from PM_1: new PM with e2=5,host=Jake |
| 6 | leaveTown(Yann) | 3 PMs die | All PMs with guest=Yann killed by negation |
| 7 | harm(Jake→Yann) | no change | No valid PMs left to advance |
