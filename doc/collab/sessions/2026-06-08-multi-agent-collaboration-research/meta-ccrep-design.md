# CCREP: Cognitive Consensus and Result Enrichment Protocol (Meta Design)

## Conceptual Genesis and Core Architectural Paradigm
This design is a response to the problem multi-agent labs face: transitioning from a simple file guard/lock model to a *quality* lock model. It treats the Model Context Protocol (MCP) server as a cognitive blackboard rather than a basic file coordinator. This design builds upon the Courier Protocol, the maildir spool, and baton contention concepts, incorporating ideas from recent work on voting versus consensus in LLM debates.

---

## The Protocol Loop: Formal State Machine and Execution Dynamics

The operational lifecycle of CCREP is governed by a state machine managed by the centralized MCP server. The server acts as a blackboard coordinating the following states:

```
IDLE ──> PROPOSED ──> EVALUATING ──> REVIEWING ──> AMENDING ──> VOTING ──> MERGE_READY ──> MERGED
                             ▲__________________│ (revision loop)
```

### Protocol States
* **IDLE**: The default inactive state.
* **PROPOSED**: An agent calls the `submit_proposal()` endpoint, providing details of the branch and their intent.
* **EVALUATING**: The MCP server checks out the code in an isolated worktree, runs the evaluation harness, and generates an `EvaluationReport`.
* **REVIEWING**: Peer agents read the `EvaluationReport` and the code diffs, then submit a structured `Critique`.
* **AMENDING**: The proposer agent applies amendments to address feedback, incrementing the revision counter.
* **VOTING**: Weighted votes are cast on the proposal. If more than one competing branch exists for the task, a Condorcet ranking is performed.
* **MERGE_READY**: The consensus gate is cleared when quorum and criteria are met, allowing the MCP server to merge the branch.
* **MERGED**: The proposal is fast-forward merged into the production branch, and workspace metadata is updated.

This is a Paxos-like mechanism, but the value being agreed upon is not an arbitrary log entry; it is the **evaluation vector** representing the quality of the software artifacts.

---

## Preventing Cognitive Divergence and Loop Termination

LLM teams can oscillate when left unchecked (e.g., endlessly changing formatting or style choices). Recent research indicates that voting boosts performance on reasoning tasks by 13.2%, whereas consensus boosts performance on knowledge tasks by 2.8%—but both degrade if iterations run unchecked. CCREP manages this with three complementary termination levers:

### 1. Revision Budget and Plateau Detection
* **Hard Cap**: A limit of `max_revisions = 5`.
* **Soft Stop**: If F1 score difference is $\Delta\text{F1@3s} < 0.001$ and lint error difference is $\Delta\text{lint\_errors} = 0$ for two consecutive revisions, the loop stops.
* *Rationale*: Quality gains come from verifier-guided refinement rather than random sampling diversity. When the verifier metrics plateau, execution halts.

### 2. Simulated Annealing for Prompts
Prompt templates dynamically cool the semantic dispersion based on iteration count:
* **Initial Temperature**: $T_0 = 1.0$ (encouraging broad/creative ideas like "explore 3 radically different thresholds").
* **Geometric Decay**: $T_n = T_0 \cdot 0.7^n$ for revision $n$.
* **Prompt Directives by Temperature**:
  * $T_n > 0.7$: "Propose bold architectural alternatives."
  * $0.3 \le T_n \le 0.7$: "Make targeted surgical edits."
  * $T_n < 0.3$: "Only fix failing tests, no style changes allowed."

### 3. Edit-War Detection
* **Metric**: Compute the normalized Levenshtein distance between revision $n$ and $n-2$.
* **Trigger**: If the distance is $< 0.05$ and the authors alternate, the system flags an active "oscillation".
* **Mediation**: The MCP server halts the loop, forces a third agent (e.g., Codex) to propose a compromise diff, and resets the temperature to $0.5$.

---

## Weighted Consensus, Domain Expertise, and Pairwise Resolvers

Votes are weighted dynamically depending on the domain of the changed files (e.g., if `.rs` files are modified, implementation weights dominate). 

### Domain Quorum Weight Matrix

| Agent | Implementation | Architecture | Verification | Human |
|---|---|---|---|---|
| **Claude** | 0.6 | 0.2 | 0.2 | — |
| **Gemini** | 0.2 | 0.6 | 0.2 | — |
| **Codex** | 0.2 | 0.2 | 0.6 | — |
| **Roberto** | 1.0 (Veto) | 1.0 | 1.0 | — |

### Quorum Rules
* The sum of the weights of all approving agents must be $\ge 1.5$.
* Quorum must include at least one implementation vote and at least one architecture vote for code changes.
* Human approval (Roberto) can override any state, but is optional under standard automated convergence.

### Pairwise Preference Resolution via Schulze Method
When choosing between multiple competing implementation branches (e.g., Branch A vs Branch B vs Branch C):
* Agents submit a complete ranking of preferences (e.g., Gemini ranks $B > A > C$).
* The MCP server calculates the strongest path $p[X, Y]$ using the Schulze Method (Floyd-Warshall variant) to identify the Condorcet winner and resolve circular majorities.

---

## Formal Representation of Qualitative State Schemas

### Proposal Schema
```json
{
  "id": "prop-20260608-001",
  "author": "claude",
  "task_id": "boundary-threshold-tune",
  "git": {
    "base": "main",
    "branch": "feat/boundary-f1",
    "commit": "a3f9c1d"
  },
  "description": "Optimize duration prior for F1@3s",
  "domains": ["architecture", "implementation"],
  "created_at": "2026-06-08T07:15:00Z",
  "revision": 2,
  "temperature": 0.49
}
```

### EvaluationReport Schema
```json
{
  "proposal_id": "prop-20260608-001",
  "revision": 2,
  "metrics": {
    "F1_at_3s": 0.872,
    "delta_F1": 0.004,
    "p_value": 0.018,
    "lint_errors": 0,
    "test_pass_rate": 0.997
  },
  "artifacts": {
    "test_log": "evals/001-r2.log",
    "coverage_html": "evals/001-r2-cov/"
  },
  "verdict": "pass"
}
```

### Critique Schema
```json
{
  "proposal_id": "prop-20260608-001",
  "reviewer": "gemini",
  "type": "amendment",
  "comments": "Memory leak risk in boundary buffer; suggest Arc<Mutex> instead of Rc",
  "suggested_diff": "@@ -42 +42 @@...",
  "severity": "major",
  "weight_applied": 0.6
}
```

### ConsensusState Schema
```json
{
  "proposal_id": "prop-20260608-001",
  "votes": [
    {"voter": "claude", "weight": 0.6, "decision": "approve"},
    {"voter": "gemini", "weight": 0.6, "decision": "approve_with_amendment"},
    {"voter": "codex", "weight": 0.6, "decision": "approve"}
  ],
  "tally": {"approve_weight": 1.8, "reject_weight": 0},
  "quorum_met": true,
  "domains_covered": ["implementation", "architecture", "verification"],
  "status": "MERGE_READY",
  "termination_reason": "metric_plateau"
}
```

---

## Model Context Protocol (MCP) Blackboard Integration Blueprint

The MCP server coordinates worktree environments, task execution, and agent activation:

### Tools (Agent-Callable)
* `submit_proposal(branch, description, domains)`: Registers a new proposal; returns a `proposal_id`.
* `run_evaluation(proposal_id)`: Checks out the branch using `git worktree add`, runs the compilation/testing suites, and generates the `EvaluationReport`.
* `submit_critique(proposal_id, markdown, diff)`: Records a critique and notifies the proposer via their doorbell FIFO.
* `cast_vote(proposal_id, ranking[])`: Registers voter rankings and updates `ConsensusState`.
* `request_amendment(proposal_id)`: Locks the branch for modification and increments the temperature.
* `merge_proposal(proposal_id)`: Merges the branch to main if `quorum_met` is true.

### Resources (Read-Only)
* `mcp://proposals/{id}`: Proposal details and change history.
* `mcp://evals/{id}/r{n}`: Static or live-generated `EvaluationReport` for revision $n$.
* `mcp://consensus/{id}`: Live consensus tallies.

### Prompts (Injected by MCP)
* `reviewer_prompt`: Custom template including latest metrics, temperature constraints, and the reviewer's specialized domain weights.
* `proposer_prompt`: Contextual template including peer critiques and target cooling instructions.

### Workspace Isolation & Event Loops
* **Worktrees**: MCP manages isolated test execution workspaces under `.mcp/worktrees/` to prevent checkout state pollution.
* **Locks**: Relies on lexicographical lock ordering (`session.md` < `proposal.json` < `eval.db`) to avoid deadlocks.
* **Maildir Spool**: Agents write turns to `spool/<agent>/new/`. The MCP server coordinates routing by delivering message envelopes (e.g. `REVIEW_REQUEST`) and triggers agent execution via doorbell FIFOs.
* **Log Scribe**: To avoid write collisions, agents never write to `session.md` directly. A dedicated MCP Scribe process appends a linearized summary to the main log file once a proposal transitions to `MERGED`.
