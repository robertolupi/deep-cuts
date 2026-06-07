# CCREP: Cognitive Consensus and Result Enrichment Protocol

## 0. Key Framing: Do Not Literally Implement Paxos

Use Paxos/Raft as metaphors for roles and invariants, not as the core algorithm.

Paxos chooses a value once a majority of acceptors accept it; the learner then discovers the chosen value (see [Paxos Made Simple](https://lamport.azurewebsites.net/pubs/paxos-simple.pdf)). Raft makes consensus easier to reason about by decomposing it into leader election, log replication, and safety (see [Raft Consensus Algorithm](https://raft.github.io/)). But we are not solving unreliable distributed replication; we have a central coordinator (the MCP server). 

The useful abstraction is:
> **A value is not a branch. A value is an immutable tuple:**
>
> `(task_id, commit_sha, evaluation_report, critique_set, vote_set, gate_policy_version)`

The MCP server is the leader/orchestrator/blackboard. The agents are proposers, reviewers, verifiers, and amendors. The “chosen value” is the commit that satisfies the merge gate.

---

# 1. State Machine

Use two nested state machines: one for the **Task**, one for each **ProposalVersion**.

## Task Lifecycle

```text
OPEN
  -> CLAIMED
  -> HAS_PROPOSALS
  -> CANDIDATE_SELECTION        # only if multiple competing proposals exist
  -> CONSENSUS_READY
  -> HUMAN_REVIEW_REQUIRED      # optional
  -> MERGED
  -> CLOSED

Terminal alternates:
  ABANDONED
  PARKED
  REJECTED
```

## Proposal Lifecycle

```text
DRAFTING
  -> SUBMITTED
  -> EVALUATING
  -> EVALUATION_FAILED
  -> REVIEWING
  -> REVISION_REQUESTED
  -> SUPERSEDED_BY_REVISION
  -> APPROVED
  -> CONSENSUS_CANDIDATE
  -> MERGED

Terminal alternates:
  REJECTED
  ABANDONED
  EXPIRED
```

## Important Transition Rules

| Transition                        | Trigger                             | Rule                                                                              |
| --------------------------------- | ----------------------------------- | --------------------------------------------------------------------------------- |
| `SUBMITTED -> EVALUATING`         | `submit_proposal()`                 | Branch is resolved to immutable `commit_sha`.                                     |
| `EVALUATING -> EVALUATION_FAILED` | Tests/lints/golden metrics fail     | No peer vote can override hard automated failure, except explicit human override. |
| `EVALUATING -> REVIEWING`         | Automated report passes minimum bar | Coordinator requests reviews from weighted domains.                               |
| `REVIEWING -> REVISION_REQUESTED` | Blocking critique exists            | Review must include evidence and suggested resolution.                            |
| `REVISION_REQUESTED -> SUBMITTED` | Agent submits new commit            | Old approvals expire; old critiques remain as carry-over claims.                  |
| `REVIEWING -> APPROVED`           | Quorum satisfied                    | No blocking critiques, hard checks pass, weighted approval met.                   |
| `APPROVED -> MERGED`              | Merge gate passes                   | Coordinator merges exact commit SHA, not mutable branch head.                     |

## Core Invariants

1. **Immutable proposal version**: a proposal version points to a fixed commit SHA. If the branch moves, that creates a new revision.
2. **Evaluations are content-addressed**: cache by `(commit_sha, eval_suite_hash, dataset_hash, env_hash)`.
3. **Votes expire on code change**: any new commit invalidates previous approvals, though prior critiques can remain open/closed.
4. **No self-approval**: the author can explain or amend, but cannot satisfy peer quorum.
5. **The consensus state is derived**: agents do not write `ConsensusState` directly. The server computes it from append-only events.

---

# 2. Loop Termination and Anti-Divergence

The main danger is not deadlock; it is **taste churn**. Agents can keep “improving” naming, style, prompt phrasing, thresholds, or architecture without measurable gain.

So each task needs a **revision budget**, a **quality convergence rule**, and a **critique admissibility rule**.

## Termination Conditions

Use all of these together:

```text
Stop iterating when any of these is true:

1. Hard gate passes + weighted peer quorum passes.
2. Revision budget exceeded.
3. No material quality improvement for K consecutive revisions.
4. Remaining critiques are non-blocking and below severity threshold.
5. Same unresolved disagreement repeats N times.
6. Human override closes the loop.
```

Suggested defaults:

```yaml
termination_policy:
  max_revisions: 5
  max_review_rounds: 3
  patience_rounds_without_improvement: 2
  min_metric_delta:
    boundary_f1_at_3s: 0.002
    test_coverage: 0.005
    latency_ms_relative: 0.01
  max_open_blocking_critiques: 0
  max_open_major_critiques: 1
  allow_merge_with_minor_critiques: true
```

For Deep Cuts, this matters especially for ML-ish changes like boundary thresholds. A tiny F1 fluctuation should not restart the loop unless it clears a predefined minimum effect size.

## Critique Admissibility Rule

A critique should be accepted into the blocking set only if it is:
```text
specific + actionable + evidence-linked + severity-classified
```

* **Bad critique**: “This feels too complex.”
* **Good critique**: “The new duration prior assumes median section length ≥ 12s, but the SALAMI subset has many shorter transitions. This may suppress valid boundaries. Evidence: eval report `salami_phase0`, tracks 14, 27, 41. Suggested amendment: cap prior penalty below 8s.”

The MCP server should classify critiques:

```yaml
critique_classes:
  blocking:
    - correctness_regression
    - data_loss
    - security_or_sandbox_escape
    - test_failure
    - golden_metric_regression
  major:
    - likely_bug
    - serious_architecture_debt
    - unhandled_edge_case
  minor:
    - naming
    - style
    - local_readability
  advisory:
    - future_refactor
    - alternative_design
```

Only `blocking` and some `major` critiques can prevent merge.

## Simulated Annealing for Agent Creativity

Simulated annealing accepts worse moves early with a probability that falls as “temperature” decreases; the classic formulation uses temperature to reduce the chance of accepting uphill/worse moves over time (see [Optimization by Simulated Annealing](https://www.science.org/doi/10.1126/science.220.4598.671)).

For CCREP, use “temperature” to control both:
1. **Prompt freedom**
2. **Acceptance of exploratory revisions**

Example schedule:
$$T_r = \max(T_{\text{min}}, T_0 \times \gamma^r)$$
where $T_0 = 1.0$, $\gamma = 0.55$, $T_{\text{min}} = 0.05$, and $r$ is the revision number.

| Revision | Temperature | Agent Behavior |
|---|---|---|
| `r = 0` | High | Explore alternatives, challenge assumptions, propose larger changes. |
| `r = 1–2` | Medium | Address top critiques, compare tradeoffs, avoid unrelated edits. |
| `r >= 3` | Low | Minimal diffs only; fix blockers; no naming/style churn. |

Acceptance rule for exploratory revisions:
$$\Delta Q = Q_{\text{new}} - Q_{\text{best}}$$
$$\text{If } \Delta Q \ge 0 \text{ accept; else accept with probability } e^{\Delta Q / T_r}$$
*Note: Never merge a worse candidate just because annealing accepted it for exploration. Annealing governs the search process, not the final gate.*

Prompt template knobs should be generated from temperature:

```yaml
annealing_prompt_controls:
  high_temperature:
    allow_new_architecture: true
    allow_new_files: true
    allow_parameter_search: broad
    require_minimal_diff: false
  medium_temperature:
    allow_new_architecture: false
    allow_new_files: limited
    allow_parameter_search: targeted
    require_minimal_diff: mostly
  low_temperature:
    allow_new_architecture: false
    allow_new_files: false
    allow_parameter_search: only_if_metric_regression
    require_minimal_diff: true
```

---

# 3. Weighted Specialized Consensus

Not all approvals are equal. A Svelte lifecycle bug, Rust migration, and mathematical prior should not be voted on by the same quorum.

## Agent Profile

Each agent gets a capability vector:

```json
{
  "agent_id": "claude",
  "capabilities": {
    "rust": 0.85,
    "typescript": 0.80,
    "svelte": 0.80,
    "architecture": 0.55,
    "ml_research": 0.35,
    "verification": 0.45,
    "refactoring": 0.75
  },
  "base_reliability": 0.75
}
```

Example task domain vector:

```json
{
  "task_id": "boundary-viterbi-duration-prior",
  "domains": {
    "ml_research": 0.45,
    "rust": 0.20,
    "verification": 0.25,
    "architecture": 0.10
  }
}
```

Weight formula:
$$\text{weight}(\text{agent}, \text{task}) = \min\left(\text{max\_agent\_weight}, \text{base\_reliability} \times \text{domain\_match} \times \text{calibration} \times \text{independence} \times \text{evidence}\right)$$

Where:
* $\text{domain\_match} = \text{dot}(\text{agent\_capability}, \text{task\_domain})$
* $\text{independence} = 0.0$ if $\text{author} == \text{reviewer}$; $0.5$ if same agent family; $1.0$ otherwise.
* $\text{evidence} = 1.0$ if review cites evaluations/diffs/tests; $0.6$ if plausible but not linked; $0.2$ if generic.

Cap any one agent at `0.45` of total peer weight so a single highly weighted agent cannot rubber-stamp the merge.

## Merge Quorum Policy

```yaml
consensus_gate:
  automated:
    require_all_hard_checks_pass: true
    forbid_golden_metric_regression: true
  peer:
    total_weighted_approval_threshold: 0.70
    required_domain_thresholds:
      implementation: 0.55
      verification: 0.50
      architecture_or_research: 0.45
    require_independent_reviewer: true
    forbid_open_blocking_critiques: true
  human:
    required_for:
      - public_api_change
      - destructive_migration
      - license_change
      - model_or_dataset_change
      - large_architecture_change
```

## Vote Types
* `APPROVE`
* `REQUEST_CHANGES`
* `ABSTAIN`
* `VETO` (must include evidence and fall under valid categories: data loss, security escape, reproducible regression, license violation, or migration risk).

---

# 4. Condorcet for Competing Branches

Condorcet voting (see [Voting Methods - Stanford Encyclopedia of Philosophy](https://plato.stanford.edu/entries/voting-methods/)) is useful when you have multiple viable branches and no obvious scalar objective. In Condorcet-style methods, voters rank candidates, and candidates are compared pairwise; a Condorcet winner is one that beats every other candidate head-to-head.

Use it for cases like:
* Branch A: Nelder-Mead threshold optimization
* Branch B: CMA-ES optimization
* Branch C: Random search + hand-tuned priors

If you have a clear metric like F1, latency, or memory, use that as an objective gate first. 

Recommended flow:
1. Run automated evaluation on all candidate branches.
2. Remove any branch that fails hard gates.
3. Keep Pareto-nondominated branches.
4. Ask agents to rank remaining branches with evidence.
5. Build weighted pairwise matrix:
   $$\text{M}[A, B] = \sum \text{weight}(\text{voter}) \text{ for voters preferring A over B}$$
   A beats B if $\text{M}[A, B] > \text{M}[B, A] + \text{margin}$.
6. If a Condorcet winner exists, choose it.
7. If there is a cycle, use maximin / minimax regret, request one more evaluation, or yield to a human decision brief.

---

# 5. Formal Representation of Qualitative State

Define consensus state as a reducer over an append-only event log:
$$\text{ConsensusState}(\text{task\_id}) = \text{reduce}(\text{TaskCreated}, \text{ProposalSubmitted}, \text{EvaluationCompleted}, \text{CritiqueSubmitted}, \text{VoteSubmitted}, \dots)$$

The qualitative state is not a scalar. It is a vector plus constraints:
```text
Q(proposal) = {
  hard_checks,
  golden_metrics,
  performance_metrics,
  coverage_metrics,
  static_analysis,
  risk_findings,
  open_critiques,
  approvals,
  confidence
}
```

A proposal is mergeable iff:
1. all hard checks pass;
2. no golden metric regresses beyond tolerance;
3. all blocking critiques are closed or human-overridden;
4. weighted quorum passes;
5. proposal commit still descends from accepted base or has been rebased and re-evaluated.

---

# 6. JSON Schema Specification

Defined using JSON Schema Draft 2020-12 (see [JSON Schema Draft 2020-12 Specification](https://json-schema.org/draft/2020-12)):

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://deep-cuts.local/schemas/ccrep.schema.json",
  "title": "CCREP Core Objects",
  "oneOf": [
    { "$ref": "#/$defs/Proposal" },
    { "$ref": "#/$defs/EvaluationReport" },
    { "$ref": "#/$defs/Critique" },
    { "$ref": "#/$defs/ConsensusState" }
  ],
  "$defs": {
    "AgentId": {
      "type": "string",
      "pattern": "^[A-Za-z0-9_.-]+$"
    },
    "IsoTime": {
      "type": "string",
      "format": "date-time"
    },
    "GitRef": {
      "type": "object",
      "required": ["repo", "commit_sha"],
      "properties": {
        "repo": { "type": "string" },
        "branch": { "type": "string" },
        "commit_sha": {
          "type": "string",
          "pattern": "^[a-f0-9]{40,64}$"
        },
        "base_commit_sha": {
          "type": "string",
          "pattern": "^[a-f0-9]{40,64}$"
        }
      },
      "additionalProperties": false
    },
    "Proposal": {
      "type": "object",
      "required": [
        "proposal_id",
        "task_id",
        "revision",
        "author",
        "git",
        "created_at",
        "description",
        "change_summary",
        "status"
      ],
      "properties": {
        "proposal_id": { "type": "string" },
        "task_id": { "type": "string" },
        "revision": { "type": "integer", "minimum": 0 },
        "supersedes": { "type": ["string", "null"] },
        "author": { "$ref": "#/$defs/AgentId" },
        "git": { "$ref": "#/$defs/GitRef" },
        "created_at": { "$ref": "#/$defs/IsoTime" },
        "description": { "type": "string" },
        "change_summary": {
          "type": "array",
          "items": { "type": "string" }
        },
        "claimed_domains": {
          "type": "object",
          "additionalProperties": {
            "type": "number",
            "minimum": 0,
            "maximum": 1
          }
        },
        "expected_eval_suites": {
          "type": "array",
          "items": { "type": "string" }
        },
        "status": {
          "enum": [
            "submitted",
            "evaluating",
            "evaluation_failed",
            "reviewing",
            "revision_requested",
            "approved",
            "consensus_candidate",
            "merged",
            "rejected",
            "abandoned",
            "superseded"
          ]
        }
      },
      "additionalProperties": false
    },
    "EvaluationReport": {
      "type": "object",
      "required": [
        "report_id",
        "proposal_id",
        "commit_sha",
        "suite_id",
        "suite_hash",
        "environment_hash",
        "dataset_hash",
        "started_at",
        "completed_at",
        "status",
        "hard_checks",
        "metrics"
      ],
      "properties": {
        "report_id": { "type": "string" },
        "proposal_id": { "type": "string" },
        "commit_sha": {
          "type": "string",
          "pattern": "^[a-f0-9]{40,64}$"
        },
        "suite_id": { "type": "string" },
        "suite_hash": { "type": "string" },
        "environment_hash": { "type": "string" },
        "dataset_hash": { "type": "string" },
        "started_at": { "$ref": "#/$defs/IsoTime" },
        "completed_at": { "$ref": "#/$defs/IsoTime" },
        "status": {
          "enum": ["passed", "failed", "error", "timeout", "cancelled"]
        },
        "hard_checks": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["name", "passed"],
            "properties": {
              "name": { "type": "string" },
              "passed": { "type": "boolean" },
              "details": { "type": "string" },
              "log_uri": { "type": "string" }
            },
            "additionalProperties": false
          }
        },
        "metrics": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "required": ["value", "direction"],
            "properties": {
              "value": { "type": "number" },
              "baseline_value": { "type": ["number", "null"] },
              "delta": { "type": ["number", "null"] },
              "threshold": { "type": ["number", "null"] },
              "direction": { "enum": ["higher_is_better", "lower_is_better", "target"] },
              "passed": { "type": ["boolean", "null"] },
              "p_value": { "type": ["number", "null"], "minimum": 0, "maximum": 1 }
            },
            "additionalProperties": false
          }
        },
        "artifacts": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["kind", "uri"],
            "properties": {
              "kind": { "type": "string" },
              "uri": { "type": "string" },
              "mime_type": { "type": "string" }
            },
            "additionalProperties": false
          }
        }
      },
      "additionalProperties": false
    },
    "Critique": {
      "type": "object",
      "required": [
        "critique_id",
        "proposal_id",
        "reviewer",
        "created_at",
        "stance",
        "summary",
        "findings"
      ],
      "properties": {
        "critique_id": { "type": "string" },
        "proposal_id": { "type": "string" },
        "reviewer": { "$ref": "#/$defs/AgentId" },
        "created_at": { "$ref": "#/$defs/IsoTime" },
        "stance": {
          "enum": ["approve", "request_changes", "abstain", "veto"]
        },
        "summary": { "type": "string" },
        "findings": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["finding_id", "severity", "category", "claim"],
            "properties": {
              "finding_id": { "type": "string" },
              "severity": {
                "enum": ["advisory", "minor", "major", "blocking", "critical"]
              },
              "category": {
                "enum": [
                  "correctness",
                  "performance",
                  "architecture",
                  "security",
                  "testing",
                  "maintainability",
                  "style",
                  "research_assumption",
                  "metric_regression"
                ]
              },
              "claim": { "type": "string" },
              "evidence": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "kind": { "enum": ["file_line", "eval_metric", "test_log", "benchmark", "reasoning"] },
                    "uri": { "type": "string" },
                    "details": { "type": "string" }
                  },
                  "additionalProperties": false
                }
              },
              "suggested_patch": {
                "type": ["object", "null"],
                "properties": {
                  "format": { "enum": ["unified_diff", "parameter_change", "natural_language"] },
                  "content": { "type": "string" }
                },
                "additionalProperties": false
              },
              "blocks_merge": { "type": "boolean" }
            },
            "additionalProperties": false
          }
        }
      },
      "additionalProperties": false
    },
    "ConsensusState": {
      "type": "object",
      "required": [
        "task_id",
        "state",
        "computed_at",
        "gate_policy_version",
        "candidate_proposals",
        "votes",
        "weighted_tallies",
        "open_blocking_findings",
        "decision"
      ],
      "properties": {
        "task_id": { "type": "string" },
        "state": {
          "enum": [
            "collecting_proposals",
            "evaluating",
            "reviewing",
            "revision_required",
            "candidate_selection",
            "consensus_ready",
            "human_review_required",
            "merged",
            "parked",
            "rejected"
          ]
        },
        "computed_at": { "$ref": "#/$defs/IsoTime" },
        "gate_policy_version": { "type": "string" },
        "candidate_proposals": {
          "type": "array",
          "items": { "type": "string" }
        },
        "votes": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["agent_id", "proposal_id", "vote", "weight", "domains"],
            "properties": {
              "agent_id": { "$ref": "#/$defs/AgentId" },
              "proposal_id": { "type": "string" },
              "vote": { "enum": ["approve", "request_changes", "abstain", "veto"] },
              "weight": { "type": "number", "minimum": 0, "maximum": 1 },
              "domains": {
                "type": "object",
                "additionalProperties": {
                  "type": "number",
                  "minimum": 0,
                  "maximum": 1
                }
              },
              "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
            },
            "additionalProperties": false
          }
        },
        "weighted_tallies": {
          "type": "object",
          "properties": {
            "approve": { "type": "number" },
            "request_changes": { "type": "number" },
            "abstain": { "type": "number" },
            "veto": { "type": "number" }
          },
          "additionalProperties": false
        },
        "domain_quorum_status": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "properties": {
              "required": { "type": "number" },
              "current": { "type": "number" },
              "satisfied": { "type": "boolean" }
            },
            "additionalProperties": false
          }
        },
        "open_blocking_findings": {
          "type": "array",
          "items": { "type": "string" }
        },
        "decision": {
          "type": "object",
          "required": ["mergeable", "reason"],
          "properties": {
            "mergeable": { "type": "boolean" },
            "selected_proposal_id": { "type": ["string", "null"] },
            "reason": { "type": "string" },
            "next_actions": {
              "type": "array",
              "items": { "type": "string" }
            }
          },
          "additionalProperties": false
        }
      },
      "additionalProperties": false
    }
  }
}
```

---

# 7. MCP Server Integration Blueprint

The MCP specification defines three primitives: **tools** for executable actions, **resources** for structured contextual data, and **prompts** for reusable model instructions.

## Tools
* `register_agent(profile)`
* `claim_task(task_id, agent_id)`
* `submit_proposal(task_id, branch, commit_sha, description, expected_eval_suites)`
* `run_evaluation(proposal_id, suite_id)`
* `submit_critique(proposal_id, critique)`
* `submit_vote(proposal_id, vote, confidence, rationale)`
* `submit_candidate_ranking(task_id, ranking, rationale)`
* `request_amendment(proposal_id, critique_ids)`
* `submit_revision(previous_proposal_id, branch, commit_sha, description)`
* `compute_consensus(task_id)`
* `merge_proposal(proposal_id, target_branch)` (requires human confirmation for API, license, model/database, or architectural changes).
* `human_override(task_id, action, rationale)`
* `abandon_proposal(proposal_id, rationale)`

## Resources

Use stable URIs:
* `ccrep://tasks/{task_id}`
* `ccrep://tasks/{task_id}/consensus`
* `ccrep://tasks/{task_id}/events`
* `ccrep://proposals/{proposal_id}`
* `ccrep://proposals/{proposal_id}/diff`
* `ccrep://proposals/{proposal_id}/evaluation/latest`
* `ccrep://proposals/{proposal_id}/critiques`
* `ccrep://proposals/{proposal_id}/votes`
* `ccrep://queues/{agent_id}`
* `ccrep://reports/{evaluation_report_id}`
* `ccrep://artifacts/{artifact_id}`

## Prompts

* `ccrep.review_proposal`
* `ccrep.amend_from_critiques`
* `ccrep.rank_candidates`
* `ccrep.explain_vote`
* `ccrep.human_merge_brief`
* `ccrep.postmortem_after_merge`

Prompt argument example:
```json
{
  "task_id": "boundary-viterbi-duration-prior",
  "proposal_id": "prop_123",
  "role": "research_reviewer",
  "temperature_phase": "medium",
  "allowed_critique_classes": ["correctness", "metric_regression", "research_assumption"],
  "forbidden_churn": ["formatting", "renaming_without_bug"]
}
```

---

# 8. Branch Checkout and Evaluation Execution

The MCP coordinator owns a local worktree pool:
```text
/var/deep-cuts-ccrep/
  repos/deep-cuts.git
  worktrees/
    prop_123_a13f.../
    prop_124_b91c.../
  eval-cache/
  artifacts/
  logs/
```

On `submit_proposal()`:
1. Verify branch exists.
2. Resolve branch to commit SHA.
3. Create immutable proposal ref: `refs/ccrep/proposals/{proposal_id} -> commit_sha`.
4. Create disposable git worktree.
5. Run selected evaluation suites.
6. Store `EvaluationReport`.
7. Compute next required reviewers.

Evaluation cache key:
$$\text{cache\_key} = \text{sha256}(\text{commit\_sha} + \text{eval\_suite\_hash} + \text{dataset\_hash} + \text{environment\_hash} + \text{harness\_version})$$

Suites example:
```yaml
eval_suites:
  rust_core:
    commands:
      - cargo test
      - cargo clippy -- -D warnings
      - cargo fmt --check

  tauri_svelte:
    commands:
      - pnpm test
      - pnpm lint
      - pnpm check

  salami_boundary_phase0:
    commands:
      - uv run python scripts/evaluate_salami_phase0.py
    metrics:
      - boundary_f1_at_3s
      - precision_at_3s
      - recall_at_3s
      - false_positive_rate
      - p_value_vs_baseline
```

*Note: Run untrusted branches in a sandbox (disabled network, CPU/memory limits, read-only).*

---

# 9. Agent Scheduling Algorithm

The coordinator selects the next action based on consensus state:

```python
def select_next_action(task):
    if task.has_submitted_proposal_without_eval():
        return Assignment(role="orchestrator", agent="codex", action="run_evaluation")

    if task.latest_eval_failed():
        return Assignment(role="amender", agent=task.proposer, action="fix_eval_failures")

    if task.needs_research_review():
        return Assignment(role="reviewer", agent="gemini", action="critique_research_assumptions")

    if task.needs_implementation_review():
        return Assignment(role="reviewer", agent="claude", action="critique_code_quality")

    if task.has_blocking_critiques():
        return Assignment(role="amender", agent=best_amender(task), action="apply_amendments")

    if task.has_multiple_candidates():
        return Assignment(role="ranker", agent=eligible_agents(), action="rank_candidates")

    if task.gate_satisfied() and task.requires_human():
        return Assignment(role="human", agent="roberto", action="review_merge_brief")

    if task.gate_satisfied():
        return Assignment(role="orchestrator", agent="codex", action="merge")

    return Assignment(role="idle")
```

---

# 10. Storage Model

Use an append-only event log plus materialized views in SQLite.

Materialized tables:
* `agents`, `tasks`, `task_domain_vectors`, `proposals`
* `evaluation_reports`, `critiques`, `critique_findings`
* `votes`, `candidate_rankings`, `assignments`
* `gate_policies`, `event_log`, `merge_records`

---

# 11. Minimal Viable Implementation Path

1. **Phase 1 (Evidence Ledger)**: Basic proposal submission, test runs, and critique logging. No competing branches.
2. **Phase 2 (Anti-divergence)**: Revision budgets, critique severity levels, and invalidation of votes on code changes.
3. **Phase 3 (Weighted Quorums)**: Task domain vectors and capability-based dynamic voting weights.
4. **Phase 4 (Competing Branches)**: Condorcet ranking algorithms for Pareto-optimal branches.

---

# 12. Core Principle

The protocol should not ask:
> “Do agents agree?”

It should ask:
> “Has this exact commit accumulated enough reproducible evidence, domain-specialist approval, and unresolved-risk closure to become the next accepted state?”

That is the difference between a chat loop and a quality ratchet.
