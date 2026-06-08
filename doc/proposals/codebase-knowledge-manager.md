---
status: proposed
owner: Roberto
last_verified: 2026-06-08
implemented_by:
superseded_by:
related_code:
related_skills: write-docs, dev-guidelines
---

# Codebase Knowledge Manager using Google Mangle & Semantic Embeddings

## 1. Recommendation and Goal

We propose building a **hybrid Codebase Knowledge Manager** that combines **deterministic Datalog reasoning** (using Google Mangle) with **semantic vector embeddings** (using a local `all-MiniLM-L6-v2` ONNX model). 

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
When a developer or agent attempts to commit changes, the pre-commit hook runs the verification engine. If a documentation sync error is derived by Mangle, the commit is aborted:

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

The system utilizes a split-plane database schema located in `scratch/codebase_index.db` (SQLite) and evaluated via a Datalog facts ledger (`scratch/facts.mang`).

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
             [ scratch/facts.mang ]           [ scratch/codebase_index.db ]
                      |                                 |
                      |                                 | (Similarity Joins)
                      v                                 v
             +-----------------------------------------------------+
             |                 Mangle Datalog Engine               |
             |       Evaluates rules to detect drift / verify      |
             +-----------------------------------------------------+
```

### A. Datalog Predicates (`tools/schema.mang`)
We define the structural and semantic entities inside the Mangle schema:

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
To support natural language queries, we store the vector embeddings of codebase concepts and documentation blocks in an SQLite database using `sqlite-vec`:

```sql
CREATE TABLE node_embeddings (
    node_id TEXT PRIMARY KEY,        -- e.g., "concept:SAX" or "file:src/scanner/sax.rs"
    content_text TEXT NOT NULL,      -- The text used to generate the embedding
    embedding_vector F32_VEC(384)    -- 384-dimensional vector from all-MiniLM-L6-v2
);
```

---

## 4. Datalog Rules for Architectural Verification

We define the rules in `tools/rules.mang`. These rules are executed by Mangle during checks to verify correctness:

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
*   **Linter**: The pre-commit hook runs `dc-knowledge-mgr lint` locally, validating that the author's local commits do not violate style constraints (like direct IPC imports) or drift from local documentation files.
*   **Query**: Developers and individual bots use `dc-knowledge-mgr query` locally to explore the codebase.
 
### B. Parallel Mode (Conditional Multi-Worktree Integration)
Parallel mode is activated only when the Go CLI is run with the `--parallel` flag, or automatically when `git worktree list` detects multiple active directories. 
*   **Shared Plane**: It shifts the database to the canonical root (`$(git-common)/../scratch/codebase_index.db`) and scans *all* active branches listed in `git worktree list`.
*   **CCREP Integration**: When a proposal is submitted to CCREP, CCREP's evaluation runner (`tools/ccrep/evaluate.py`) calls the Go linter in parallel mode. If Mangle derives a knowledge drift or a cross-worktree duplicate symbol conflict, the CCREP evaluation fails (exit status 1), setting the proposal to `revision_required` in the CCREP ledger.
*   **bot-collab Integration**: During active sessions, the Go indexer runs as a native MCP server (`tools/dc-knowledge-mgr serve`) in `.mcp.json`. Active agents call `knowledge_mgr/query` for real-time context and use the existing `collab_mcp` mailboxes to resolve any cross-worktree warnings flagged by the linter.
 
---

## 6. Testing and Verification Plan

### A. Fact Extractor Tests
We will write unit tests in `tools/tests/test_fact_extractor.py` that run against a mock codebase fixture:
*   Verify that adding a mock Tauri handler correctly generates a `defines(file, tauri_command)` fact.
*   Verify that Svelte store imports are correctly resolved.

### B. Datalog Rule Validation
We will verify Mangle rules using test facts assertions:
*   Feed a mock `direct_ipc_violation` fact database and assert that Mangle flags the violation.
*   Feed a synchronized codebase layout and assert that `undocumented_analysis_pass` returns empty.

---

## 7. Rejected Alternatives

1. **VMware Differential Datalog (DDlog)**:
   * **Reason for Rejection**: Highly complex Haskell compilation toolchain. Since the project is archived, it introduces long-term maintenance liabilities. A simple interpreter run takes less than 15ms for a codebase of this size, making incremental differential compilation unnecessary.
2. **Neo4j / External Graph Databases**:
   * **Reason for Rejection**: Requires running a JVM-based graph database daemon locally. This violates the rule to keep dependencies lean and the main app lightweight.
3. **Pure Vector Search (No Datalog)**:
   * **Reason for Rejection**: Similarity search alone cannot enforce logical invariants (e.g., "if X calls Y, Z must exist"). We need Datalog's logical operators to build strict validation gates.

---

## 8. Proposed Next Steps

```
+-----------------------------------------------------------------------------------+
|  PHASE 1: AST Scanner & Mangle Setup                                              |
|  - Write tools/extract_structural_facts.py using regex/AST.                       |
|  - Write tools/rules.mang defining basic constraints (IPC & Passes).              |
|  - Run check via go run github.com/google/mangle/cmd/mangle.                       |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  PHASE 2: Local Embedding Generator                                               |
|  - Set up tools/generate_node_embeddings.py using all-MiniLM-L6-v2 ONNX.           |
|  - Store embeddings in scratch/codebase_index.db using SQLite-Vec.                |
|  - Generate similarity facts and feed them to Mangle.                             |
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
|  - Hook tools/check_knowledge.sh into the pre-commit config.                      |
|  - Register query_codebase_index tool in .mcp.json for agent checkout tasks.      |
+-----------------------------------------------------------------------------------+
```
