---
sidebar_position: 5
title: Cybersecurity
description: Detect multi-stage attack patterns with temporal graph sifting
---

# Cybersecurity

import PatternPlayground from '@site/src/components/wasm/PatternPlayground';

Cyber attacks unfold as sequences of events across hosts and accounts. Sifting patterns detect multi-stage attack chains by joining events through shared entities -- credentials, hosts, accounts -- with temporal ordering and exception clauses. The same mechanism that finds broken promises in narrative also finds lateral movement in a network.

| | |
|---|---|
| **Time** | ~15 minutes |
| **Prerequisites** | [What is Sifting?](/docs/learn/what-is-sifting) |

---

## 1. Lateral Movement

An attacker authenticates across multiple hosts using the same stolen credential. Three logins, three hosts, one credential -- that is the signal.

<PatternPlayground
  defaultPattern={`pattern lateral_movement {
  stage e1 {
    e1.type = "login"
    e1.credential -> ?credential
    e1.host -> ?host_a
  }
  stage e2 {
    e2.type = "login"
    e2.credential -> ?credential
    e2.host -> ?host_b
  }
  stage e3 {
    e3.type = "login"
    e3.credential -> ?credential
    e3.host -> ?host_c
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "login"
  @1 e1.credential -> stolen_cred
  @1 e1.host -> server1

  @3 e2.type = "login"
  @3 e2.credential -> stolen_cred
  @3 e2.host -> server2

  @5 e3.type = "login"
  @5 e3.credential -> stolen_cred
  @5 e3.host -> server3

  @2 e4.type = "login"
  @2 e4.credential -> janes_cred
  @2 e4.host -> workstation1

  @4 e5.type = "login"
  @4 e5.credential -> janes_personal
  @4 e5.host -> workstation2

  now = 10
}`}
  compact
/>

**Result:** 1 match -- `stolen_cred` on `server1`, `server2`, `server3`. Jane's logins use different credentials on each host, so the `?credential` join never binds across all three stages.

:::tip What to notice
The `?credential` variable appears in all three stages. This is the join condition: only login sequences that reuse the same credential across distinct hosts will match. Legitimate users rotating credentials naturally evade this pattern.
:::

---

## 2. Impossible Travel

A user authenticates from two distant locations with no VPN session between. The VPN event would explain the geographic jump -- its absence is the signal.

<PatternPlayground
  defaultPattern={`pattern impossible_travel {
  stage e1 {
    e1.type = "login"
    e1.user -> ?user
    e1.location -> ?loc_a
  }
  stage e2 {
    e2.type = "login"
    e2.user -> ?user
    e2.location -> ?loc_b
  }
  unless between e1 e2 {
    mid.type = "vpn_connect"
    mid.user -> ?user
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "login"
  @1 e1.user -> alice
  @1 e1.location -> new_york

  @3 e2.type = "login"
  @3 e2.user -> alice
  @3 e2.location -> tokyo

  @2 e3.type = "login"
  @2 e3.user -> bob
  @2 e3.location -> london

  @5 e4.type = "login"
  @5 e4.user -> bob
  @5 e4.location -> paris

  @4 mid.type = "vpn_connect"
  @4 mid.user -> bob

  now = 10
}`}
  compact
/>

**Result:** 1 match -- Alice logged in from `new_york` then `tokyo` with no VPN between. Bob also jumped cities (`london` to `paris`), but his VPN connection at t=4 falls between his two logins, so `unless between` kills that match.

:::tip What to notice
The negation clause binds `?user`, so only a VPN event for the *same user* counts as an exception. A VPN connection by a different user does not suppress the alert. This is the precision that variable-scoped negation provides.
:::

---

## 3. Credential Stuffing

Multiple failed logins to the same account from different sources, with no successful login between. The successful login resets the count -- only uninterrupted failure sequences match.

<PatternPlayground
  defaultPattern={`pattern credential_stuffing {
  stage e1 {
    e1.type = "failed_login"
    e1.account -> ?account
    e1.source -> ?src_a
  }
  stage e2 {
    e2.type = "failed_login"
    e2.account -> ?account
    e2.source -> ?src_b
  }
  unless between e1 e2 {
    mid.type = "successful_login"
    mid.account -> ?account
  }
}`}
  defaultGraph={`graph {
  @1 e1.type = "failed_login"
  @1 e1.account -> admin_account
  @1 e1.source -> ip_10_0_0_1

  @2 e2.type = "failed_login"
  @2 e2.account -> admin_account
  @2 e2.source -> ip_10_0_0_2

  @3 e3.type = "failed_login"
  @3 e3.account -> admin_account
  @3 e3.source -> ip_10_0_0_3

  @4 e4.type = "successful_login"
  @4 e4.account -> admin_account

  @6 e5.type = "failed_login"
  @6 e5.account -> service_acct
  @6 e5.source -> ip_192_168_1_1

  @8 e6.type = "successful_login"
  @8 e6.account -> service_acct

  @9 e7.type = "failed_login"
  @9 e7.account -> service_acct
  @9 e7.source -> ip_192_168_1_2

  now = 10
}`}
  compact
/>

**Result:** Matches on `admin_account` -- failed logins from `ip_10_0_0_1` and `ip_10_0_0_2` (and from `ip_10_0_0_1` and `ip_10_0_0_3`, etc.) all occur before the successful login at t=4 with no success between consecutive failures. The `service_acct` pair (t=6, t=9) is split by a successful login at t=8, so `unless between` blocks that match.

:::tip What to notice
The `?account` join ensures only failures targeting the same account are correlated. The `unless between` resets detection when a legitimate login succeeds. Different `?src_a` and `?src_b` bindings capture the distributed nature of the attack -- multiple source IPs hitting one target.
:::

---

## Pattern Comparison

| Pattern | Stages | Join variable | Negation | Detection signal |
|---------|--------|---------------|----------|-----------------|
| Lateral movement | 3 | `?credential` | none | Same credential, multiple hosts |
| Impossible travel | 2 | `?user` | `unless between` (VPN) | Location jump without VPN |
| Credential stuffing | 2 | `?account` | `unless between` (success) | Consecutive failures, no intervening success |

Three mechanisms do all the work: **stages** define the temporal sequence, **variable joins** correlate events across stages, and **negation windows** suppress matches when a legitimate explanation exists.

---

## Mapping your data

Windows Event Log and syslog entries map to fabula edges as follows:

| Real-world field | Fabula edge |
|---|---|
| EventRecordID | source node |
| EventID or message type | label |
| Computer, TargetUserName, SourceAddress | target values/nodes |
| TimeCreated | interval start (point event: `[t, t+1)`) |

Most security events are instantaneous, so they become point intervals. The Computer and TargetUserName fields become target nodes, enabling variable joins that correlate events across hosts and accounts.

---

## Limitations and false positives

These patterns are starting points, not production-ready rules. Each has known blind spots:

- **Lateral movement:** Credential rotation evades the join on `?credential`. The pattern only catches reuse of the *same* credential -- an attacker who steals a new credential per host is invisible.
- **Impossible travel:** VPN and proxy use create false positives. A user routing through a VPN exit node in another country looks identical to impossible travel. You need additional context (e.g., known VPN IP ranges) to filter these.
- **Credential stuffing:** Distributed attacks from many source IPs may not trigger the pattern if each IP attempts only once. The pattern requires two failures from *different* sources hitting the same account -- a single-attempt-per-IP botnet spreads below this threshold.
- **Mitigation:** Combine sifting patterns with statistical baselines. Surprise scoring ranks matches by anomaly, so a lateral movement match involving a service account that *always* authenticates across hosts scores low. Tighten metric gap constraints (`gap ..300` for "within 5 minutes") to reduce the window. Add negation for known-safe patterns (`unless between` for scheduled credential rotation events).

---

## How fabula compares

- **vs Splunk correlation searches:** Threshold-based ("5 failed logins in 10 minutes"). No graph structure, no variable joins across events, no negation windows. Fabula patterns express multi-stage attack chains with entity correlation.
- **vs Elastic EQL:** Supports `sequence by [field] with maxspan` -- the closest analogue to fabula's staged joins with gap constraints. However, EQL has no interval algebra, limited negation (no `unless_after`, no `unless_global`), and no composition operators for reusable pattern fragments.
- **vs Sigma rules:** Single-event signatures. Sigma describes what one log line looks like, not temporal sequences across multiple events. Fabula operates at the sequence level with cross-event joins.

---

## Where to go next

- [Getting Started](/docs/getting-started) -- Build these patterns in Rust, from `cargo new` to working alerts.
- [Pattern Cookbook](/docs/guides/pattern-cookbook) -- More pattern recipes including repeat/range for threshold detection.
- [Scoring Reference](/docs/reference/scoring) -- Rank matched alerts by surprise to prioritize triage.
