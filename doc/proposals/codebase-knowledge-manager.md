---
status: proposed
owner: Roberto
last_verified: 2026-06-08
implemented_by:
superseded_by:
related_code:
related_skills: write-docs, dev-guidelines
---

# Codebase Knowledge Manager using Structural Rules & Semantic Embeddings

## 1. Recommendation and Goal

We propose building a **hybrid Codebase Knowledge Manager** that combines **deterministic structural rules** over extracted code facts with **semantic vector embeddings** (using a local `all-MiniLM-L6-v2` ONNX model). Phase 1 should implement the deterministic checks as SQLite queries over fact tables; Google Mangle remains an optional follow-up if the rule set grows into recursive graph traversal or clearer Datalog-style reasoning.

The goal is to eliminate **agent knowledge drift**—a failure state where an AI agent working in an isolated git worktree implements redundant code, violates architectural boundaries, or fails to update documentation because it cannot load the full repository into its context window.

---

## 2. User & Agent-Visible Behavior

### A. The Agent Startup Check (Querying the Index)
When an agent starts a task, it is directed by `AGENTS.md` to run the query wrapper tool to resolve the code and documentation context:

```bash
python3 tools/query_knowledge.py "segmenting audio tracks"
```

**System Response:**
```json
{
  "semantic_matches": [
    {
      "node": "concept:SAX",
      "similarity": 0.87,
      "summary": "Symbolic Aggregate Approximation for structural audio analysis"
    }
  ],
  "structural_context": [
    {
      "relation": "implements(File, 'BatchAnalysisPass')",
      "file": "src-tauri/src/scanner/sax.rs",
      "status": "active"
    },
    {
      "relation": "documents(Doc, 'BatchAnalysisPass')",
      "file": "skills/add-analysis-pass/SKILL.md",
      "note": "Required reading for adding batch/structural pipeline steps"
    }
  ]
}
```

### B. The Pre-Commit Linter Gate
When a developer or agent attempts to commit changes, the optional pre-commit hook runs the verification engine. If a documentation sync error is derived, the commit is aborted:

```text
[KNOWLEDGE LINT FAILURE]
File: src-tauri/src/analysis/my_new_pass.rs
Issue: Struct "MyNewPass" implements rust_trait("AnalysisPass"), but no semantic
       document is registered linking this pass to a skill.
Fix: Update skills/add-analysis-pass/SKILL.md to include the new pass, or add a
     docstring to "MyNewPass" containing a "@concept" tag.
```

---

## 3. Data Model & Architecture

The system uses a split-plane SQLite layout: extracted structural facts and semantic node embeddings live in `scratch/codebase_index.db`. If a later phase adopts Mangle, those same facts can also be exported to `scratch/facts.mang`.

```
                              [ Source Files ]
                                 /        \
                   (AST Parsing)/          \(Semantic Extraction)
                               /            \
                              v              v
               +--------------+              +---------------------+
               | Deterministic|              |   LLM Agent / ONNX  |
               |  AST Scripts |              |  Embeddings Parser  |
               +--------------+              +---------------------+
                      |                                 |
                      | (Structural Facts)              | (Semantic Node Vectors)
                      v                                 v
             [ extracted fact tables ]        [ scratch/codebase_index.db ]
                      |                                 |
                      |                                 | (Similarity Joins)
                      v                                 v
             +-----------------------------------------------------+
             |              Structural Rule Evaluator               |
             |       Evaluates rules to detect drift / verify      |
             +-----------------------------------------------------+
```

### A. Extracted Fact Schema
We define the structural and semantic entities as SQLite fact tables in Phase 1. If Mangle is later adopted, the same facts can be exported to `tools/schema.mang`:

```prolog
// Entities
declare file(name: string).
declare rust_trait(name: string).
declare tauri_command(name: string).
declare concept(name: string).

// Structural Relations (extracted via AST parsing)
declare defines(file: string, entity: string).
declare implements(file: string, trait: string).
declare calls(file: string, target: string).
declare uses_command_map(file: string).

// Semantic Relations (extracted via LLM/Prose parsing)
declare documents(doc_file: string, target_entity: string).
declare concept_covers(concept_name: string, file: string).
declare similar(query: string, concept: string, score: float).
```

### B. SQLite Vector Schema (`scratch/codebase_index.db`)
To support natural language queries, we store the vector embeddings of codebase concepts and documentation blocks in SQLite using `sqlite-vec`. This should follow the repo's existing `vec0` virtual-table pattern from `src-tauri/migrations/05_audio_embeddings.sql` and `src-tauri/migrations/11_description_embeddings.sql`:

```sql
CREATE TABLE node_embedding_metadata (
    node_id TEXT PRIMARY KEY,
    content_text TEXT NOT NULL
);

CREATE VIRTUAL TABLE node_embeddings USING vec0(
    node_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);
```

The embedding backend is the local `all-MiniLM-L6-v2` ONNX path already used by the repo (`src-tauri/src/embeddings.rs`, plus the export tooling under `tools/`). This avoids adding a separate embedding daemon or model runtime.

---

## 4. Structural Rules for Architectural Verification

We define the first rules as SQL checks over extracted facts. The Datalog forms below document the intended logic and can become `tools/rules.mang` if Mangle is adopted later.

### Rule 1: Detect Direct IPC Calls (Bypassing typed CommandMap)
Any frontend file calling a Tauri command directly instead of using `$lib/ipc.ts` is flagged:

```prolog
direct_ipc_violation(File, Command) :-
    calls(File, Command),
    defines("src-tauri/src/lib.rs", Command),
    not uses_command_map(File).
```

### Rule 2: Detect Undocumented Analysis Passes
Any Rust module implementing the `AnalysisPass` or `BatchAnalysisPass` traits must have a corresponding document link:

```prolog
undocumented_analysis_pass(File, Struct) :-
    defines(File, Struct),
    implements(File, "AnalysisPass"),
    not has_documentation(Struct).

undocumented_analysis_pass(File, Struct) :-
    defines(File, Struct),
    implements(File, "BatchAnalysisPass"),
    not has_documentation(Struct).

has_documentation(Struct) :-
    documents(Doc, Struct).
```

### Rule 3: Propagating Semantic Concept Drift
If code references a concept that has been marked as `superseded` or `rejected` in the documentation frontmatter, flag it:

```prolog
uses_stale_concept(File, Concept, Status) :-
    concept_covers(Concept, File),
    documents(Doc, Concept),
    frontmatter_status(Doc, Status),
    stale_status(Status).

stale_status("superseded").
stale_status("rejected").
```

---
 
## 5. Operational Modes: Solo vs. Parallel
 
To remain lean, the codebase knowledge manager runs independently of other coordination tools by default, only activating multi-agent integration when parallel work is active.
 
### A. Solo Mode (Default)
By default, the system operates in **Solo Mode**. It has zero dependencies on `bot-collab` or `ccrep`.
*   **Scanner**: Scans *only* the local branch/directory of the current checkout.
*   **Database**: Writes to the local `scratch/codebase_index.db` file in the current directory.
*   **Linter**: The optional repository hook runs `tools/knowledge_mgr.py lint` locally, validating that the author's local commits do not violate style constraints (like direct IPC imports) or drift from local documentation files.
*   **Query**: Developers and individual bots use `tools/knowledge_mgr.py query` locally to explore the codebase.
 
### B. Parallel Mode (Conditional Multi-Worktree Integration)
Parallel mode is activated only when the Python CLI is run with the `--parallel` flag, or automatically by the CCREP evaluator when a proposal is being reviewed across active worktrees.
*   **Shared Plane**: It shifts the database to the canonical root (`$(git-common)/../scratch/codebase_index.db`) and scans *all* active branches listed in `git worktree list`.
*   **CCREP Integration**: When a proposal is submitted to CCREP, CCREP's evaluation runner calls the Python linter in parallel mode. If the structural rules derive a knowledge drift or a cross-worktree duplicate symbol conflict, the CCREP evaluation fails (exit status 1), setting the proposal to `revision_required` in the CCREP ledger.
*   **bot-collab Integration**: During active sessions, the Python indexer can run as an MCP server (`tools/knowledge_mgr.py serve`) in `.mcp.json`, following the same pattern as `tools/collab_mcp/server.py` and `tools/ccrep/server.py`. Active agents call `knowledge_mgr/query` for real-time context and use the existing `collab_mcp` mailboxes to resolve cross-worktree warnings flagged by the linter.

### C. Hook Installation and Worktree Isolation
The pre-commit gate is an opt-in repo hook, not assumed infrastructure. The implementation should provide a checked-in installer such as `tools/install_knowledge_hook.py` that writes a small `.git/hooks/pre-commit` wrapper in the Git common directory.

Because Git hooks are shared across worktrees, the hook must pass the current working directory explicitly to `tools/knowledge_mgr.py lint --root "$PWD"`. Solo Mode must ignore sibling worktrees unless `--parallel` is set by the caller or by CCREP. This preserves the default "local checkout only" behavior even when the repository has multiple worktrees.
 
---

## 6. Testing and Verification Plan

### A. Fact Extractor Tests
We will write unit tests in `tools/tests/test_fact_extractor.py` that run against a mock codebase fixture:
*   Verify that adding a mock Tauri handler correctly generates a `defines(file, tauri_command)` fact.
*   Verify that Svelte store imports are correctly resolved.

### B. Structural Rule Validation
We will verify rule checks using test fact assertions:
*   Feed a mock `direct_ipc_violation` fact database and assert that the rule evaluator flags the violation.
*   Feed a synchronized codebase layout and assert that `undocumented_analysis_pass` returns empty.

---

## 7. Rejected Alternatives

1. **VMware Differential Datalog (DDlog)**:
   * **Reason for Rejection**: Highly complex Haskell compilation toolchain. Since the project is archived, it introduces long-term maintenance liabilities. For the initial rule set, a non-incremental interpreter is expected to be sufficient; any performance claim must be verified before it becomes an implementation requirement.
2. **Neo4j / External Graph Databases**:
   * **Reason for Rejection**: Requires running a JVM-based graph database daemon locally. This violates the rule to keep dependencies lean and the main app lightweight.
3. **Pure Vector Search (No Structural Rules)**:
   * **Reason for Rejection**: Similarity search alone cannot enforce logical invariants (e.g., "if X calls Y, Z must exist"). We need deterministic rule checks, implemented first as SQL over extracted facts, to build strict validation gates.
4. **Plain SQLite Anti-Joins Only**:
   * **Reason for Rejection**: The first rules are expressible as `NOT EXISTS` and `LEFT JOIN ... IS NULL` queries over extracted facts, so the Phase 1 implementation should prototype those SQL checks first. Mangle remains a candidate only if the rule set grows into recursive graph traversal or if Datalog materially improves rule readability. This keeps the first implementation aligned with the repo's existing SQLite-heavy tooling.

---

## 8. Proposed Next Steps

```
+-----------------------------------------------------------------------------------+
|  PHASE 1: AST Scanner & Structural Rule Setup                                     |
|  - Write tools/extract_structural_facts.py using regex/AST.                       |
|  - Write SQL checks for basic constraints (IPC & Passes).                         |
|  - Add tools/rules.mang only if recursive Datalog rules become necessary.         |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  PHASE 2: Local Embedding Generator                                               |
|  - Set up tools/generate_node_embeddings.py using all-MiniLM-L6-v2 ONNX.           |
|  - Store embeddings in scratch/codebase_index.db using SQLite-Vec.                |
|  - Generate similarity facts for structural rule checks and query results.        |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  PHASE 3: Semantic Docs Agent                                                     |
|  - Set up tools/extract_semantic_facts.py.                                        |
|  - Run a lightweight LLM task to extract concept links from changed markdown.     |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  PHASE 4: Pre-Commit Linter Integration                                           |
|  - Add tools/install_knowledge_hook.py for opt-in pre-commit setup.               |
|  - Register query_codebase_index tool in .mcp.json for agent checkout tasks.      |
+-----------------------------------------------------------------------------------+
```
