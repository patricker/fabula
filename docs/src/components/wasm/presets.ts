/**
 * Pre-built example patterns and graphs for the playground and step-through.
 * Organized by category for the dropdown menu.
 */

export interface Preset {
  label: string;
  description: string;
  pattern: string;
  graph: string;
}

export interface PresetGroup {
  label: string;
  presets: Preset[];
}

export const PRESET_GROUPS: PresetGroup[] = [
  {
    label: 'Narrative Arcs',
    presets: [
      {
        label: 'Violation of Hospitality',
        description: 'Expect 1 match: Alice enters, Bob is hospitable, then Bob harms Alice. The variables ?guest=alice and ?host=bob are bound across all three stages.',
        pattern: `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest
  }
}`,
        graph: `graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  @3 e3.eventType = "harm"
  @3 e3.actor -> bob
  @3 e3.target -> alice
  now = 10
}`,
      },
      {
        label: 'Hospitality — Guest Escapes',
        description: 'Expect 0 matches: Same as above, but Alice leaves at t=3 before the harm at t=4. The "unless between" negation fires and kills the match. Gap analysis shows why.',
        pattern: `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest
  }
}`,
        graph: `graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  @3 eMid.eventType = "leaveTown"
  @3 eMid.actor -> alice
  @4 e3.eventType = "harm"
  @4 e3.actor -> bob
  @4 e3.target -> alice
  now = 10
}`,
      },
      {
        label: 'Romantic Redemption Arc',
        description: 'Expect 1 match: Alice has two negative romantic events, then a positive one. All three stages bind ?char=alice. The pattern captures a redemption arc.',
        pattern: `pattern romantic_arc {
  stage e1 {
    e1.tag = "negative"
    e1.tag = "romantic"
    e1.actor -> ?char
  }
  stage e2 {
    e2.tag = "negative"
    e2.tag = "romantic"
    e2.actor -> ?char
  }
  stage e3 {
    e3.tag = "positive"
    e3.tag = "romantic"
    e3.actor -> ?char
  }
}`,
        graph: `graph {
  @1 e1.tag = "negative"
  @1 e1.tag = "romantic"
  @1 e1.actor -> alice
  @2 e2.tag = "negative"
  @2 e2.tag = "romantic"
  @2 e2.actor -> alice
  @3 e3.tag = "positive"
  @3 e3.tag = "romantic"
  @3 e3.actor -> alice
  now = 10
}`,
      },
      {
        label: 'Two Impulsive Betrayals',
        description: 'Expect 1 match: A character with trait "impulsive" betrays twice. The global "unless" negation checks for reconciliation between betrayals — none here, so it matches.',
        pattern: `pattern two_impulsive_betrayals {
  stage e1 {
    e1.eventType = "betray"
    e1.actor -> ?char
    char.trait = "impulsive"
  }
  stage e2 {
    e2.eventType = "betray"
    e2.actor -> ?char
  }
  unless {
    mid.eventType = "reconcile"
    mid.actor -> ?char
  }
}`,
        graph: `graph {
  @0 char.trait = "impulsive"
  @1 e1.eventType = "betray"
  @1 e1.actor -> char
  @3 e2.eventType = "betray"
  @3 e2.actor -> char
  now = 10
}`,
      },
      {
        label: 'Betrayals Blocked by Reconciliation',
        description: 'Expect 0 matches: Same pattern, but a reconciliation event at t=2 falls between the two betrayals. The global negation fires and blocks the match.',
        pattern: `pattern two_impulsive_betrayals {
  stage e1 {
    e1.eventType = "betray"
    e1.actor -> ?char
    char.trait = "impulsive"
  }
  stage e2 {
    e2.eventType = "betray"
    e2.actor -> ?char
  }
  unless {
    mid.eventType = "reconcile"
    mid.actor -> ?char
  }
}`,
        graph: `graph {
  @0 char.trait = "impulsive"
  @1 e1.eventType = "betray"
  @1 e1.actor -> char
  @2 mid.eventType = "reconcile"
  @2 mid.actor -> char
  @3 e2.eventType = "betray"
  @3 e2.actor -> char
  now = 10
}`,
      },
    ],
  },
  {
    label: 'Negation Windows',
    presets: [
      {
        label: 'Broken Promise (unless after)',
        description: 'Expect 1 match: Alice promises at t=1 and breaks it at t=3. No apology happens after the promise, so the "unless after" negation does not fire.',
        pattern: `pattern broken_promise {
  stage e1 {
    e1.eventType = "promise"
    e1.actor -> ?char
  }
  stage e2 {
    e2.eventType = "break_promise"
    e2.actor -> ?char
  }
  unless after e1 {
    mid.eventType = "apologize"
    mid.actor -> ?char
  }
}`,
        graph: `graph {
  @1 e1.eventType = "promise"
  @1 e1.actor -> alice
  @3 e2.eventType = "break_promise"
  @3 e2.actor -> alice
  now = 10
}`,
      },
      {
        label: 'Broken Promise — Apology Saves',
        description: 'Expect 0 matches: Same pattern, but Alice apologizes at t=2 (after the promise). The "unless after" negation fires and blocks the broken-promise detection.',
        pattern: `pattern broken_promise {
  stage e1 {
    e1.eventType = "promise"
    e1.actor -> ?char
  }
  stage e2 {
    e2.eventType = "break_promise"
    e2.actor -> ?char
  }
  unless after e1 {
    mid.eventType = "apologize"
    mid.actor -> ?char
  }
}`,
        graph: `graph {
  @1 e1.eventType = "promise"
  @1 e1.actor -> alice
  @2 mid.eventType = "apologize"
  @2 mid.actor -> alice
  @3 e2.eventType = "break_promise"
  @3 e2.actor -> alice
  now = 10
}`,
      },
      {
        label: 'Kept Promise',
        description: 'Expect 1 match: Alice promises at t=1 and fulfills at t=5. No revocation happens between them. Try adding a "revoke" event at t=3 to see the negation block it.',
        pattern: `pattern kept_promise {
  stage e1 {
    e1.eventType = "promise"
    e1.actor -> ?char
  }
  stage e2 {
    e2.eventType = "fulfill"
    e2.actor -> ?char
  }
  unless between e1 e2 {
    mid.eventType = "revoke"
    mid.actor -> ?char
  }
}`,
        graph: `graph {
  @1 e1.eventType = "promise"
  @1 e1.actor -> alice
  @5 e2.eventType = "fulfill"
  @5 e2.actor -> alice
  now = 10
}`,
      },
    ],
  },
  {
    label: 'Value Constraints',
    presets: [
      {
        label: 'Low Loyalty Check',
        description: 'Expect 1 match: Loyalty is 0.3, which is < 0.5. Try changing the loyalty value to 0.8 to see the match disappear and gap analysis explain why.',
        pattern: `pattern low_loyalty {
  stage e {
    e.eventType = "loyalty_check"
    e.loyalty < 0.5
  }
}`,
        graph: `graph {
  @1 e.eventType = "loyalty_check"
  @1 e.loyalty = 0.3
  now = 5
}`,
      },
      {
        label: 'Morale in Range',
        description: 'Expect 1 match: Morale is 0.5, within the [0.3, 0.7] range. Uses two constraints (>= and <=) on the same stage. Try 0.1 or 0.9 to break it.',
        pattern: `pattern uncertain_morale {
  stage e {
    e.eventType = "morale_check"
    e.morale >= 0.3
    e.morale <= 0.7
  }
}`,
        graph: `graph {
  @1 e.eventType = "morale_check"
  @1 e.morale = 0.5
  now = 5
}`,
      },
    ],
  },
  {
    label: 'Temporal',
    presets: [
      {
        label: 'Strict Ordering',
        description: 'Expect 1 match: Enter at t=1, leave at t=5 — correct temporal order. Try swapping the timestamps to see the match disappear (stages enforce strict ordering).',
        pattern: `pattern enter_then_leave {
  stage e1 {
    e1.eventType = "enter"
    e1.actor -> ?char
  }
  stage e2 {
    e2.eventType = "leave"
    e2.actor -> ?char
  }
}`,
        graph: `graph {
  @1 e1.eventType = "enter"
  @1 e1.actor -> alice
  @5 e2.eventType = "leave"
  @5 e2.actor -> alice
  now = 10
}`,
      },
      {
        label: 'Allen: During',
        description: 'Expect 1 match: Sortie [3,5) occurs entirely within siege [1,100). Uses bounded intervals and an explicit "temporal inner during outer" Allen constraint.',
        pattern: `pattern sortie_during_siege {
  stage outer {
    outer.eventType = "siege"
  }
  stage inner {
    inner.eventType = "sortie"
  }
  temporal inner during outer
}`,
        graph: `graph {
  @1..100 outer.eventType = "siege"
  @3..5 inner.eventType = "sortie"
  now = 50
}`,
      },
      {
        label: 'Winnow 7-Step Sequence',
        description: 'Expect 0 matches: Alice enters, Bob and Charlie both host her (creating forked partial matches), then Alice leaves — killing all her matches. Best viewed in the Step-Through debugger to watch forking and negation unfold.',
        pattern: `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
  unless between e1 e3 {
    eMid.eventType = "leaveTown"
    eMid.actor -> ?guest
  }
}`,
        graph: `graph {
  // Step 1: Alice enters town
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  // Step 2: Bob shows hospitality to Alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  // Step 3: Charlie also shows hospitality to Alice
  @3 e3.eventType = "showHospitality"
  @3 e3.actor -> charlie
  @3 e3.target -> alice
  // Step 4: Dave enters town (second guest)
  @4 e4.eventType = "enterTown"
  @4 e4.actor -> dave
  // Step 5: Alice leaves — kills her partial matches
  @5 e5.eventType = "leaveTown"
  @5 e5.actor -> alice
  // Step 6: Bob harms Alice (but she left — no match)
  @6 e6.eventType = "harm"
  @6 e6.actor -> bob
  @6 e6.target -> alice
  // Step 7: Charlie harms Alice (same — blocked)
  @7 e7.eventType = "harm"
  @7 e7.actor -> charlie
  @7 e7.target -> alice
  now = 10
}`,
      },
    ],
  },
  {
    label: 'Debugging',
    presets: [
      {
        label: 'Gap Analysis — Missing Stage',
        description: 'Expect 0 matches: Enter and hospitality are present, but no harm event exists. The gap analysis panel shows stage e3 as "unmatched" and tells you exactly which clause failed.',
        pattern: `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
  stage e3 {
    e3.eventType = "harm"
    e3.actor -> ?host
    e3.target -> ?guest
  }
}`,
        graph: `graph {
  @1 e1.eventType = "enterTown"
  @1 e1.actor -> alice
  @2 e2.eventType = "showHospitality"
  @2 e2.actor -> bob
  @2 e2.target -> alice
  // No harm event — what does the engine say?
  now = 10
}`,
      },
      {
        label: 'Gap Analysis — Empty Graph',
        description: 'Expect 0 matches: Graph has no edges at all. Gap analysis shows every stage as "unmatched", starting from stage e1. Add edges to watch stages light up one by one.',
        pattern: `pattern violation_of_hospitality {
  stage e1 {
    e1.eventType = "enterTown"
    e1.actor -> ?guest
  }
  stage e2 {
    e2.eventType = "showHospitality"
    e2.actor -> ?host
    e2.target -> ?guest
  }
}`,
        graph: `graph {
  now = 10
}`,
      },
    ],
  },
];

/** Flat list of all presets for easy lookup. */
export const ALL_PRESETS: Preset[] = PRESET_GROUPS.flatMap(g => g.presets);
