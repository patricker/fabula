---
sidebar_position: 6
title: Scoring and Surprise
---

# Scoring and Surprise

You run a sifting pattern against a simulation and get 47 matches. Most are mundane. "Betrayal" fired between the rebels and the crown -- again. It fired between two minor factions nobody cares about. It fired nine times in the last 20 ticks because the simulation is in a war phase and betrayals are cheap.

But buried in those 47 matches is something worth finding. A loyal character -- someone whose trait profile makes betrayal rare -- betrayed the king. That match is the one a player would remember, the one a narrative manager should surface, the one that would make a good story beat. The other 46 are noise.

Pattern matching found all 47. It did its job. Now you need a second pass: which of these matches actually matter?

## The ranking problem

Sifting patterns find structure. A pattern like "betrayal followed by exile with no reconciliation between" defines a shape, and the engine returns every instance of that shape in the data. But "every instance" is too many. The engine does not know which instances are interesting -- it matches syntax, not significance.

This is a general problem in story sifting. A rich simulation produces a combinatorial explosion of pattern matches. Most of them are structurally valid but narratively boring. The engine's job is completeness: find every match. The scorer's job is discrimination: rank them.

Scoring finds *interesting* structure. It takes the raw match set and ranks it by how unexpected each match is, using the same information-theoretic framework that underlies compression, signal processing, and entropy measurement. The core idea: surprise is inversely related to probability. Things you could have predicted carry little information. Things you could not have predicted carry a lot.

Fabula provides three independent scoring signals. They answer different questions and can be combined in any ranking function.

## Shannon surprise

The foundation of all three scorers is a single formula: `-log2(p)`, where `p` is the probability of the event.

If something happens 50% of the time, it carries 1 bit of surprise -- you need one yes/no question to predict it. If it happens 1% of the time, it carries about 6.6 bits. If it happens every time, it carries 0 bits -- you already knew it would happen, so learning that it happened tells you nothing.

This is Shannon's self-information. The logarithm gives two useful properties. First, surprise scales sublinearly: an event 10x rarer than another is only ~3.3 bits more surprising, not 10x. This prevents rare events from dominating rankings through sheer improbability. Second, surprises of independent events add: if A carries 3 bits and B carries 2 bits, seeing both carries 5 bits. This makes the metric composable.

The "bits" unit is concrete -- it is the number of binary questions you would need to ask to predict the outcome. A fair coin flip: 1 bit. A roll of a 6-sided die: ~2.6 bits. A specific character betraying a specific king in a world with 200 characters and 5 kings: potentially many bits, depending on the base rates.

Common events carry few bits. Rare events carry many. This is the entire intuition. Everything that follows applies this formula at different granularities.

## Pattern-level scoring

The first question: is this *pattern* firing more or less than expected?

`SurpriseScorer` compares observed match rates to baselines you provide. You tell it what you expect, and it tells you what deviates from that expectation.

The scorer tracks how often each pattern fires across observation rounds (one round per `evaluate()` call in batch mode, or per manual `tick()` in incremental mode). It then compares the observed frequency to a user-provided baseline probability. If you told the scorer "expect betrayal to match in 10% of rounds" and it is matching in 90% of rounds, that is *negative* surprise -- the pattern is over-represented. It fires so often that each individual match is uninteresting.

If you told it "expect reconciliation to match in 50% of rounds" and it has matched in 1% of rounds, that is high surprise. Something is preventing reconciliations. Every reconciliation that does happen is noteworthy.

The score is `-log2(observed / baseline)`. Positive means rarer than expected. Negative means more common. Near zero means the pattern is behaving as predicted. Laplace smoothing (`p = (count + 1) / (rounds + 1)`) handles the zero-observation case -- a pattern that has never fired gets a finite surprise score, not infinity.

This is coarse-grained. It treats all matches of a pattern identically. A betrayal between rebels and crown gets the same score as a betrayal between a loyal character and the king. For finer discrimination, you need property-level scoring.

## Property-level scoring

The second question: among matches of the same pattern, which ones involve unusual entities?

Pattern-level scoring says "betrayal is rare right now." But if you have 12 betrayal matches and need to pick one to surface, it cannot help -- they all share the same pattern score. You need to look inside the match.

This is the StU insight from Kreminski et al. (ICIDS 2022): "Select the Unexpected." Two matches of "betrayal" are not equally interesting. One involves faction=rebels and target_role=merchant -- common properties that appear in most matches. Another involves trait=loyal and target_role=king -- rare properties that almost never appear in betrayal matches. The second match is more surprising, not because the pattern is unusual, but because the *participants* are.

`StuScorer` tracks the empirical frequency of each property across all observed matches of a pattern. "actor_trait=ambitious" appeared in 60% of betrayal matches. "actor_trait=loyal" appeared in 3%. When scoring a new match, the scorer looks up the frequency of each property in that match and aggregates them into a single score. A match full of common properties scores high (unsurprising). A match with rare properties scores low (surprising).

The caller extracts properties. The scorer only does frequency math. This keeps it domain-agnostic -- you decide what counts as a "property" (traits, factions, relationship types, location categories), and the scorer tells you how rare the combination is. The separation is deliberate: property extraction is domain logic that varies per application, while frequency aggregation is pure statistics.

Properties should be categorical attributes, not entity IDs. "actor_faction=rebels" is a good property. "actor=char_147" is not -- entity IDs have near-uniform frequency in rich simulations, making every match score identically. This is the "everything is rare" failure mode: when every property is unique, rarity carries no signal.

Good properties: character traits, faction membership, relationship types, emotional states, location categories. These have uneven distributions -- some values are common, others rare -- and that variance is exactly what the scorer needs to discriminate.

## Four aggregation modes

Different applications have different theories of what makes a match surprising. The aggregation mode controls how per-property frequencies combine into a single score.

**ArithmeticMean** (default). Average frequency across all properties. A match with one rare property (freq 0.05) and one common property (freq 0.80) scores 0.425. Balanced and stable -- no single property dominates. This is the original StU heuristic from the Kreminski et al. paper. Lower score means more surprising.

**TfIdf**. Total information content: `sum(-log2(freq))` across all properties. This is the only mode where higher means more surprising -- polarity is reversed. A property with frequency 0.05 contributes ~4.3 bits; one with frequency 0.80 contributes ~0.3 bits. A match with many moderately-rare properties can outscore a match with one very-rare property, because TfIdf accumulates rather than averages. Choose this when the *total amount* of unusual information matters more than the *average* rarity.

**GeometricMean**. The nth root of the product of frequencies. A single rare property pulls the entire score down multiplicatively. If three properties have frequencies 0.8, 0.7, 0.02, the geometric mean is about 0.1 -- dominated by the outlier. Compare to the arithmetic mean of 0.5, which hides the rare property. Use GeometricMean when outlier rarity matters but you do not want to ignore the common properties entirely.

**Min**. Only the rarest property matters. The score equals the frequency of the single most surprising property. Everything else is ignored. A "bottleneck" theory of surprise: the match is exactly as surprising as its most unusual component. Use this for strict filtering -- "show me matches where at least one property is genuinely rare."

All modes except TfIdf follow the convention that lower score = more surprising. TfIdf reverses this because it measures total information content, which increases with rarity.

Which mode to choose? Start with ArithmeticMean. It is the most studied (it is the original StU heuristic) and produces stable rankings. Switch to Min if you find that matches with one extraordinary property are being diluted by several common ones. Switch to GeometricMean if Min feels too aggressive and you want a middle ground. Use TfIdf when you want to reward matches that carry the most total information -- useful when properties are numerous and you want rare ones to accumulate rather than average out.

## Cold-start problem

With one observation, every property is "rare." Laplace smoothing prevents zero frequencies, but it does not prevent a scorer with sparse data from confidently declaring things surprising when it has barely seen any evidence.

The scorer applies confidence weighting: `confidence = 1 - 1/(total_matches + 1)`. This factor lerps the final score toward "unsurprising" until enough data accumulates.

The ramp-up is fast but cautious:

| Matches observed | Confidence | Effect |
|---|---|---|
| 1 | 0.50 | Score is 50% attenuated toward unsurprising |
| 5 | 0.83 | Mostly trusting the data |
| 10 | 0.91 | Attenuation is minor |
| 100 | 0.99 | Effectively no attenuation |

For modes where lower = more surprising (ArithmeticMean, GeometricMean, Min), the attenuation lerps the score toward 1.0 (unsurprising). For TfIdf (where higher = more surprising), it lerps toward 0.0. Same idea, reversed polarity.

The practical effect: in the first few ticks of a simulation, the scorer withholds judgment. A property that appeared in 1 out of 1 matches looks common, but the scorer knows it has no basis for confidence. By tick 10, frequencies are stabilizing, and the scorer lets the data speak.

## Correlated properties

The aggregation modes assume properties are independent. In practice, they often are not.

"Ambitious" and "king" might both be rare individually, but they always co-occur. If a character with the ambitious trait always targets the king, counting their rarities independently double-counts the same underlying signal. The match looks twice as surprising as it should be.

Pointwise Mutual Information (PMI) measures this. `PMI(a, b) = log2(P(a,b) / (P(a) * P(b)))`. If two properties co-occur exactly as often as chance predicts, PMI is 0. If they co-occur more than expected, PMI is positive. A PMI above 1 bit indicates meaningful correlation.

When PMI correction is enabled and a property pair exceeds the 1-bit threshold, the scorer replaces the less-rare member's frequency with its conditional frequency given the partner. Instead of treating "ambitious" as rare-in-general, it treats "ambitious given that king is also present" -- which is much higher if they always co-occur. This removes the redundant contribution without discarding the rarer property's signal.

Concretely: if "ambitious" appears in 20% of matches and "king" in 20%, but they co-occur in 18% (PMI >> 1 bit), the correction replaces "king"'s marginal frequency (0.20) with its conditional frequency given "ambitious" (0.18/0.20 = 0.90). The match still gets credit for "ambitious" being rare, but "king" no longer adds much because it was predictable given "ambitious."

The correction adds O(k^2) pair counting per observation, where k is the number of properties per match. For typical property counts of 2 to 8, this is negligible.

## Sequential surprise

The third question: is this *transition* between patterns unexpected?

Pattern-level surprise looks at each pattern in isolation. Property-level surprise looks at each match in isolation. Neither considers context: what happened *before* this match?

Pattern A completed, then pattern B completed. Is that sequence unusual? A betrayal followed by another betrayal might be boring if it happens constantly. A reconciliation after a betrayal might be surprising if it almost never occurs. The pattern and properties could be identical in both cases -- what makes the second one interesting is its predecessor.

`SequentialScorer` maintains a bigram model of pattern transitions. It records which pattern completed after which and builds conditional frequencies: `P(B | A)`. The surprise of a transition is `-log2(P(B | A))` -- standard Shannon surprise applied to the conditional probability. Higher means more surprising.

If after 100 observations, betrayal follows alliance 80 times, trade follows alliance 15 times, and reconciliation follows alliance 5 times, then: betrayal-after-alliance carries ~0.3 bits (expected), trade-after-alliance carries ~2.7 bits (unusual), and reconciliation-after-alliance carries ~4.3 bits (rare and noteworthy).

Laplace smoothing ensures that novel transitions (never observed before) get a high but finite surprise score rather than infinity. A pattern pair that has never been seen inherits a small pseudocount, keeping the math well-behaved.

Sequential surprise is independent of pattern-level and property-level surprise. A transition can be surprising even between two individually unsurprising patterns, and vice versa. A betrayal after a betrayal might be unremarkable by pattern frequency but highly surprising as a transition if the simulation usually de-escalates after the first one.

## Putting it together

The three signals answer orthogonal questions at different granularities.

**Pattern-level surprise** tells you which *patterns* are firing unusually. Use it to find simulation phases where expected dynamics are absent or unexpected dynamics dominate. Granularity: one score per pattern, shared by all matches of that pattern.

**Property-level surprise** tells you which *matches* within a pattern are unusual. Use it to rank a large set of matches and surface the ones with rare entity combinations. Granularity: one score per match, based on the specific entities and attributes involved.

**Sequential surprise** tells you which *transitions* between patterns are unexpected. Use it to find turning points -- moments where the simulation's trajectory shifts from a predictable sequence to something new. Granularity: one score per pair of consecutive pattern completions.

They are independent signals. A match can have low pattern-level surprise (betrayals are common right now), high property-level surprise (this particular betrayal involves a loyal character and a king), and moderate sequential surprise (a betrayal after an alliance is somewhat unusual). The three numbers capture different facets of why the match is interesting.

Combine them however your application demands -- weighted sum, product, max, or separate ranking passes. The scorers do not prescribe a combination strategy. A narrative manager might weight property surprise highest because it wants dramatic moments. A debugging tool might weight pattern surprise highest because it wants to find broken dynamics. A storytelling system might weight sequential surprise highest because it wants plot twists.

## Where to go next

- [Scoring Reference](../reference/scoring) -- full API for SurpriseScorer, StuScorer, and SequentialScorer
- [Narrative Quality](../reference/narratives) -- composite scoring for MCTS evaluation (thread tracking, tension arcs, pivot detection)
- [Design Decisions](./design-decisions) -- why scoring is implemented as post-processing, not engine modification
