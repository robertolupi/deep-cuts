# CCREP: Cognitive Consensus and Result Enrichment Protocol (Google DeepThink Design)

## Conceptual Genesis and Core Architectural Paradigm
In contemporary software engineering environments driven by artificial intelligence, collaborative systems have transitioned from simple, sequential pipelines to highly complex, multi-agent networks. The conceptual foundation of the Cognitive Consensus and Result Enrichment Protocol (CCREP) is rooted in the practical evolution of local, offline-first applications, notably the "Deep Cuts" desktop audio analysis platform built with Rust, Tauri, and Svelte 5. 

Early coordination models within this environment relied on a manual, file-based interaction pattern termed "Two AIs, One Notepad". In that asynchronous setup, specialized language models—specifically Claude for implementation and Svelte structures, and Gemini for architectural research and long-document stress-testing—collaborated by appending contributions to shared text files stored within a local Git repository under `doc/collab/sessions/`. The human developer acted as a physical context relay and verification gateway, inspectably tracking execution logs via standard Git diffs.

While functional for small-scale development, this manual relay configuration suffers from latency bottlenecks, high token consumption, and an inability to scale to multi-threaded execution environments. CCREP addresses these limitations by automating this manual interaction pattern, replacing the human relay with an active, centralized Model Context Protocol (MCP) server that acts as a state coordinator, database, and test-execution orchestrator. The protocol establishes a qualitative consensus mechanism—a "Cognitive Paxos"—where agreement is achieved not merely on a mutual exclusion lock or database transaction index, but on the verified, progressive quality of the software artifacts under development.

CCREP orchestrates four distinct, specialized entities within its software execution context:
* **Claude (Implementation Specialist)**: Optimized for low-latency, syntactically precise code generation in Rust, Svelte 5 (utilizing its fine-grained reactivity model), and TypeScript.
* **Gemini (Architectural and Research Specialist)**: Tailored for processing extensive documentation, formulating mathematical proofs, modeling hyperparameter spaces, and detecting high-level systemic vulnerabilities.
* **Codex (Verification Engine)**: An automated execution harness responsible for building binaries, compiling dependencies, executing unit and integration test suites, performing static analysis, and returning precise performance metrics.
* **Human Supervisor (Strategic Director)**: Represented in the system state as Roberto, this role establishes the target thresholds for golden metrics, defines strategic engineering goals, and provides manual overrides when automated convergence loops fail.

The protocol is specifically designed to eliminate the "knowing-doing gap" in multi-agent systems. This gap is a common structural failure where intent-aligned ideas are generated during high-level agent reasoning but fail to materialize in the final realized code artifact. By combining automated verification with multi-agent peer review, CCREP guarantees that conceptual design choices are translated into verified code.

---

## The Protocol Loop: Formal State Machine and Execution Dynamics
The operational lifecycle of CCREP is governed by a state machine managed by the centralized MCP server. The server acts as an active blackboard, coordinating five primary states: Proposal, Evaluation, Critique, Iterative Enrichment, and Consensus Gate.

### Table 1: Cognitive Consensus State Transitions

| Source State | Target State | Triggering Condition | Primary Executing Agent | System Action & Side Effects |
|---|---|---|---|---|
| **Idle** | **Proposal** | Task assignment via backlog queue or manual strategic direction | Claude or Gemini | Spawn isolated Git branch; generate source code or parameter updates; call `submit_proposal()`. |
| **Proposal** | **Evaluation** | Receipt of `submit_proposal()` payload by the centralized MCP server | Codex | Checkout branch in microVM; execute compiler, lint suites, and calculate F1 performance metrics. |
| **Evaluation** | **Critique** | Generation of the structured `EvaluationReport` | Gemini or Claude | Fetch code diffs and evaluation metrics; identify edge cases; write and submit Critique payload. |
| **Critique** | **Enrichment** | Verdict is registered as `critique_with_amendment` | Claude or Gemini | Read peer critique; execute localized modifications to resolve identified vulnerabilities. |
| **Critique** | **Consensus Gate** | Verdict is registered as `approve` with active quorum clearance | MCP Orchestrator | Calculate weighted voting tallies, evaluate Kendall's concordance, and check for golden metric regression. |
| **Consensus Gate** | **Merged** | All automated, peer-review, and human verification gates are cleared | MCP Orchestrator | Execute fast-forward merge to `main`; delete feature branch; update agent performance metadata. |
| **Any State** | **Escalated** | Exceeded iteration budget $k_{\text{max}}$ or metric divergence detected | Human Supervisor | Halt automated loops; preserve workspace state; alert developer for manual review. |

### Phase 1: Proposal
An active agent (e.g., Claude) claims a development task from the backlog, initializes a feature branch (e.g., `feature/optimize-boundary-detection`), and performs the required modifications, such as writing a reactive Svelte 5 UI component or implementing a high-performance Rust audio processing module. Upon completion, the agent calls the MCP server's `submit_proposal()` endpoint, transmitting the branch metadata, commit SHA, and a detailed summary of changes.

### Phase 2: Automated Evaluation
The MCP server intercepts the proposal, locks the branch state, and provisions an isolated workspace. The Codex verification engine is triggered to execute compiler runs, standard linters, and performance tests. This step extracts quantitative metrics, including test coverage ratios, compilation times, memory consumption profiles, and statistical values (such as boundary detection F1 metrics and p-values). These metrics are compiled into a structured, read-only `EvaluationReport` stored directly on the blackboard.

### Phase 3: Peer Review & Critique
The MCP server selects a peer review agent (e.g., Gemini) with relevant expertise to evaluate the proposal. This agent reads the modified source files alongside the generated `EvaluationReport`. It analyzes the codebase for subtle vulnerabilities, such as memory leaks in unsafe Rust blocks, incorrect reactivity patterns in Svelte 5 components, or deviations from architectural specifications. The peer agent then publishes a structured Critique payload containing a formal verdict (`approve`, `critique_with_amendment`, or `reject`), a confidence score, and specific code changes.

### Phase 4: Iterative Enrichment
If the critique includes amendments, the proposal enters the iterative enrichment phase. The original proposer agent (or a third agent assigned by the orchestrator) pulls the critique details, applies the requested improvements to the branch, and re-submits the code. This loop repeats, with the qualitative value of the codebase rising across iterations, guided by continuous automated testing.

### Phase 5: Consensus Gate
When a critique yields an approval and the automated tests pass with zero regressions, the proposal reaches the Consensus Gate. The MCP server evaluates whether the branch satisfies the multi-agent quorum requirements. Upon clearing the gate, the server executes a fast-forward merge of the branch into the production branch (`main`), completing the lifecycle.

---

## Preventing Cognitive Divergence and Loop Termination
A primary risk in peer-to-peer large language model loops is cognitive divergence. Lacking a centralized control mechanism, agents can fall into recursive edit-war loops, endlessly swapping naming conventions, modifying formatting, or oscillating hyperparameter values back and forth. CCREP prevents this behavior by implementing Textual Simulated Annealing (TSA) and Textual Learning Rate Decay (TLRD).

TSA maps the physical thermodynamic cooling process to agent reasoning. Let $T_k$ represent the system temperature at iteration $k$, defined by a geometric cooling schedule:
$$T_k = T_0 \cdot \gamma^k$$
where $T_0$ is the initial starting temperature (typically configured to $1.0$) and $\gamma \in [0.90, 0.995]$ is the decay constant. The temperature $T_k$ is passed directly into the agent prompt templates, dynamically controlling their semantic dispersion and creative license.

### Table 2: Temperature-Dependent Prompting and Allowed Edit Operations

| Temp. Regime ($T_k$) | Primary Prompts Directives | Permitted Code Modifications | Maximum Token Budget Factor |
|---|---|---|---|
| **High** ($0.70 < T_k \leq 1.0$) | "Propose broad architectural modifications, explore alternative library options, and redesign system interfaces." | Core system refactoring, dependency swaps, API modifications. | $1.5 \times$ baseline allocation |
| **Medium** ($0.30 < T_k \leq 0.70$) | "Preserve the established architecture. Focus on local function optimization and correcting logic errors." | Internal function changes, hyperparameter tuning, test coverage expansion. | $1.0 \times$ baseline allocation |
| **Low** ($0.05 < T_k \leq 0.30$) | "Do not introduce new structural patterns. Execute minimal, target-driven edits to fix specific test failures." | Variable renaming, static type alignments, localized bug fixing. | $0.5 \times$ baseline allocation |
| **Terminal** ($0.00 < T_k \leq 0.05$) | "Apply exact syntax corrections recommended by the linter. No stylistic alterations are permitted." | Direct linter-guided edits, greedy down-hill configuration changes. | $0.2 \times$ baseline allocation |

At high temperatures, agents are encouraged to explore a wider solution space. As $T_k$ decays, the prompt templates restrict the allowed edit operations, shifting from global architectural alterations to minor bug fixes. This is supported by TLRD, which limits the volume of characters changed per commit as iterations progress.

To guarantee loop termination under all conditions, the protocol enforces three complementary stopping criteria:
1. **Revision Budget Limit ($k_{\text{max}}$)**: If the consensus status remains unresolved after a configured number of iterations (typically $k_{\text{max}} = 8$), the MCP server locks the feature branch and escalates the state to the human supervisor for manual intervention.
2. **Metric Convergence Threshold ($\epsilon$)**: Let $M_k$ represent the multi-dimensional vector of evaluation metrics at iteration $k$, containing the boundary F1 score, compilation duration, and unit test coverage ratio. If the norm of the difference between consecutive runs falls below a configured threshold, the system halts iteration:
   $$\|M_k - M_{k-1}\| < \epsilon$$
3. **Energy-State Divergence Detection**: The system monitors the "energy" of the codebase, defined as the failure rate of compile runs or test assertions. If the code energy fails to improve by a target margin $\Delta E_{\text{min}}$ over three successive iterations, the loop terminates early to prevent token waste.

---

## Weighted Consensus, Domain Expertise, and Pairwise Resolvers
In heterogeneous agent teams, treating all votes equally introduces systemic vulnerabilities. For example, an implementation-focused agent should carry more weight when validating frontend Svelte 5 state reactivity, whereas an architectural agent should hold sway over mathematical optimization choices. CCREP addresses this by employing a domain-conditional, weighted consensus voting framework.

### Domain-Conditional Quorum Weights
The voting weight $w_{i,D}$ for agent $a_i$ in task domain $D$ is computed dynamically by the consensus engine using four performance factors:
$$w_{i,D} = \text{ComputeWeight}(\alpha_i, \beta_{i,D}, \rho_i, \kappa_i)$$
where $\alpha_i$ represents global historical verdict accuracy, $\beta_{i,D}$ is the domain-specific verdict accuracy tracked via an Exponentially Weighted Moving Average (EWMA), $\rho_i$ is a recency factor, and $\kappa_i$ represents historical peer corroboration. The consensus engine applies a Bayes-optimal log-odds mapping to translate these metrics into quorum weights:
$$w_{i,D}^* = \ln\left( \frac{\beta_{i,D}}{1 - \beta_{i,D}} \right)$$
This formula ensures that a proven expert in Svelte UI components holds a higher voting weight than a generalist model within that domain, preventing less competent models from overriding specialized decisions.

### The Consensus Matrix and Concordance Tracking
During a review cycle, $N$ agents rate $K$ competing proposal branches. These inputs populate a shared Consensus Matrix $M \in \mathbb{R}^{N \times K}$. The system measures the degree of agreement across the agent team by calculating Kendall's Coefficient of Concordance $W$. The coefficient $W$ is defined as:
$$W = \frac{12 S}{N^2(K^3 - K) - N \sum_{j=1}^{N} T_j}$$
where $S$ is the sum of squared deviations of rank sums from their mean:
$$S = \sum_{i=1}^{K} (R_i - \bar{R})^2$$
The mean rank sum is defined as:
$$\bar{R} = \frac{N(K + 1)}{2}$$
and $T_j$ is the correction factor for tied ranks assigned by agent $j$:
$$T_j = \sum_{g=1}^{G_j} (t_g^3 - t_g)$$
where $t_g$ is the number of tied ranks in the $g$-th tie group.

The concordance $W$ is bounded between $0$ (no agreement) and $1$ (perfect agreement). To evaluate the statistical significance of $W$, the consensus engine maps it to Friedman's chi-square distribution:
$$\chi^2 = N(K - 1)W$$
with $K - 1$ degrees of freedom. If $W$ falls below a defined threshold $W_{\text{thresh}} = 0.65$ or fails the significance test at $\alpha = 0.05$, the coordinator calculates individual deviation scores to identify the most discordant agent. The coordinator then routes targeted feedback to that agent in the next iteration, rather than re-running the entire team.

### Resolving Cyclical Preferences with the Schulze Method
When multiple competing implementation branches exist (e.g., Branch A, B, and C), standard voting rules are vulnerable to Anscombe's or Ostrogorski's paradoxes, where pairwise majorities conflict and form cyclical loops. To resolve these cycles, CCREP employs the Schulze Method, a Condorcet-consistent voting system.

Let $d[X, Y]$ represent the number of agents who prefer branch $X$ to branch $Y$. The consensus engine constructs a directed graph where nodes are candidate branches and edge weights represent pairwise defeat strengths. The engine then computes the strongest path $p[X, Y]$ from branch $X$ to branch $Y$ using a modified Floyd-Warshall path-finding algorithm:

```python
# Initialize path strengths
for X in candidates:
    for Y in candidates:
        if X != Y:
            if d[X][Y] > d[Y][X]:
                p[X][Y] = d[X][Y]
            else:
                p[X][Y] = 0

# Compute strongest paths
for Z in candidates:
    for X in candidates:
        if X != Z:
            for Y in candidates:
                if Y != X and Y != Z:
                    p[X][Y] = max(p[X][Y], min(p[X][Z], p[Z][Y]))
```

Branch $A$ defeats branch $B$ if $p[A, B] > p[B, A]$. The Schulze Method is mathematically superior to Tideman's Ranked Pairs for this protocol because its path calculation is highly parallelizable, aligning well with high-concurrency multi-agent architectures.

---

## Formal Representation of Qualitative State Schemas
To ensure structured, type-safe communication across the blackboard, CCREP enforces strict JSON schemas for the four primary protocol components.

### Proposal Schema
The `Proposal` payload records the git branch context, author details, and target development domain.

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CCREP_Proposal",
  "type": "object",
  "properties": {
    "proposal_id": { "type": "string", "format": "uuid" },
    "branch_name": { "type": "string" },
    "commit_sha": { "type": "string", "pattern": "^[0-9a-f]{40}$" },
    "author": { "type": "string", "enum": ["claude", "gemini", "human"] },
    "timestamp": { "type": "string", "format": "date-time" },
    "task_domain": { "type": "string", "enum": ["rust_core", "svelte_ui", "tauri_integration", "math_optimization"] },
    "description": { "type": "string" },
    "changes_summary": {
      "type": "array",
      "items": { "type": "string" }
    }
  },
  "required": ["proposal_id", "branch_name", "commit_sha", "author", "timestamp", "task_domain", "description"]
}
```

### EvaluationReport Schema
The `EvaluationReport` compiles compiler states, lint diagnostics, and performance metrics generated by the test runner.

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CCREP_EvaluationReport",
  "type": "object",
  "properties": {
    "evaluation_id": { "type": "string", "format": "uuid" },
    "proposal_id": { "type": "string", "format": "uuid" },
    "timestamp": { "type": "string", "format": "date-time" },
    "build_success": { "type": "boolean" },
    "lint_failures": { "type": "integer", "minimum": 0 },
    "test_suite": {
      "type": "object",
      "properties": {
        "total_tests": { "type": "integer" },
        "passed": { "type": "integer" },
        "failed": { "type": "integer" },
        "failed_details": {
          "type": "array",
          "items": { "type": "string" }
        }
      },
      "required": ["total_tests", "passed", "failed"]
    },
    "metrics": {
      "type": "object",
      "properties": {
        "f1_score_at_3s": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
        "p_value": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
        "test_coverage_ratio": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
        "memory_leak_bytes": { "type": "integer" }
      },
      "required": ["f1_score_at_3s", "p_value", "test_coverage_ratio"]
    }
  },
  "required": ["evaluation_id", "proposal_id", "timestamp", "build_success", "lint_failures", "test_suite", "metrics"]
}
```

### Critique Schema
The `Critique` payload documents peer review decisions, confidence scores, targeted concerns, and proposed code changes.

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CCREP_Critique",
  "type": "object",
  "properties": {
    "critique_id": { "type": "string", "format": "uuid" },
    "proposal_id": { "type": "string", "format": "uuid" },
    "author": { "type": "string", "enum": ["claude", "gemini", "codex"] },
    "timestamp": { "type": "string", "format": "date-time" },
    "verdict": { "type": "string", "enum": ["approve", "critique_with_amendment", "reject"] },
    "confidence_score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "comments": { "type": "string" },
    "concerns": {
      "type": "array",
      "items": { "type": "string" }
    },
    "suggested_diffs": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "file_path": { "type": "string" },
          "original_hunk": { "type": "string" },
          "suggested_hunk": { "type": "string" }
        },
        "required": ["file_path", "original_hunk", "suggested_hunk"]
      }
    }
  },
  "required": ["critique_id", "proposal_id", "author", "timestamp", "verdict", "confidence_score", "comments"]
}
```

### ConsensusState Schema
The `ConsensusState` tracks active quorums, temperature decay progress, calculated concordance metrics, and the status of the merge process.

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CCREP_ConsensusState",
  "type": "object",
  "properties": {
    "session_id": { "type": "string", "format": "uuid" },
    "proposal_id": { "type": "string", "format": "uuid" },
    "current_iteration": { "type": "integer" },
    "current_temperature": { "type": "number" },
    "kendalls_w": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "votes": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "agent_id": { "type": "string" },
          "domain_weight": { "type": "number" },
          "vote_choice": { "type": "string", "enum": ["approve", "amend", "reject"] },
          "timestamp": { "type": "string", "format": "date-time" }
        },
        "required": ["agent_id", "domain_weight", "vote_choice"]
      }
    },
    "consensus_status": { "type": "string", "enum": ["pending_review", "iterating", "approved_for_merge", "manually_escalated"] },
    "final_decision_log": { "type": "string" }
  },
  "required": ["session_id", "proposal_id", "current_iteration", "current_temperature", "votes", "consensus_status"]
}
```

---

## Model Context Protocol (MCP) Blackboard Integration Blueprint
The central coordinator in CCREP is implemented as an expanded Model Context Protocol (MCP) server, acting as a stateful, persistent blackboard. The server handles JSON-RPC 2.0 messages, managing local processes via standard input/output (stdio) transport and remote processes using Server-Sent Events (SSE) for real-time streaming.

### Workspace Isolation and Environment Sandboxing
To isolate compilation side effects and prevent state pollution across parallel agent threads, the coordinator schedules all compilation, linting, and testing tasks within isolated microVM containers.

Each running session is assigned a unique `Mcp-Session-Id` header, which maps to a dedicated, sandboxed workspace directory. The MCP server manages these environments using an automated checkout mechanism:
```text
[Submit Proposal] -> [Provision microVM] -> [Checkout Branch] -> [Execute Codex Verification] -> [Report Metric Artifacts]
```
This pipeline ensures that concurrent branch reviews do not conflict, and prevents untrusted code execution from affecting the parent blackboard state.

### Scheduling and Coordination via Causal Interaction Graphs
The MCP server schedules agent tasks using a Causal Interaction Graph (CIG). A CIG is formally defined as a directed acyclic graph:
$$G_{CIG} = (A, M, C)$$
where $A = \{a_1, a_2, \dots, a_n\}$ represents the set of specialized agents, $M = \{m_1, m_2, \dots, m_k\}$ is the set of message payloads exchanged on the blackboard, and $C \subseteq M \times M$ is the set of directed causal links indicating trigger-response relationships.

The graph maps agent inputs to outputs, allowing the MCP server to trace dependencies and order tasks. The server executes two main operations to coordinate workflows:
* $\text{get\_parents}(v)$: Returns all upstream nodes that contributed input to node $v$, enabling fast troubleshooting and fault localization. If a test fails, this function traces the issue back to the parent component change, preventing downstream "blame shifting".
* $\text{get\_descendants}(v)$: Identifies all downstream agents and artifacts affected if node $v$ is modified. This function allows the server to notify only affected peer agents for review, rather than running the entire team.

Using this topological structure, the MCP server automatically handles branch checkouts, coordinates the order of critique and amendment steps, and wakes up the next target agent.

### Table 3: Model Context Protocol Tool and Resource Definitions

| Tool/Resource Name | Execution Protocol Type | Input Parameters / Schema | Output Payload / Side Effects |
|---|---|---|---|
| `submit_proposal` | Tool Call | `{ branch_name: string, commit_sha: string, author: string, task_domain: string }` | Enters a new proposal into the database; provisions microVM; pulls target branch. |
| `submit_critique` | Tool Call | `{ proposal_id: uuid, author: string, verdict: string, confidence_score: number, comments: string }` | Appends a critique to the blackboard; updates the consensus matrix and calculates agreement. |
| `get_evaluation_report` | Resource Read | `uri: "ccrep://evaluations/{proposal_id}"` | Retrieves the static or live-generated compilation, lint, and F1 metrics for a proposal. |
| `get_consensus_state` | Resource Read | `uri: "ccrep://consensus/{proposal_id}"` | Exposes current voting tallies, domain weights, Kendall's $W$, and temperature progress. |
| `trigger_agent_wake` | Tool Call | `{ proposal_id: uuid, target_agent: string, temperature: number }` | Sends a Server-Sent Event (SSE) to wake and assign the next agent based on the CIG. |
| `sampling/createMessage` | Client Primitive | `{ messages: array, modelPreferences: object }` | Requests client-side LLM completions from within sandbox-constrained execution scripts. |

---

## Synthesis and Practical Implementation Outlook
The CCREP framework integrates automated software testing with multi-agent consensus, replacing ad-hoc code generation with a structured, verifiable engineering process. By formalizing agent coordination via the Model Context Protocol, the system establishes a reliable pipeline where code quality progressively rises across iterations.

```text
[Agent Proposal] -> [Codex Eval Run] -> [Peer Agent Review] -> [Decentralized Voting] -> [Consensus Gate Merge]
```

This model is particularly valuable for complex architectures—such as the "Deep Cuts" local audio analysis platform—where changes in Svelte 5 frontend reactivity must align perfectly with high-performance Rust core modules.

Moving forward, CCREP is well-positioned to support self-improving, resilient software engineering systems. By tracking agent performance metrics on a centralized blackboard, the system can dynamically optimize voting weights and task routing over time, enabling agent swarms to scale efficiently under real-world development conditions.
