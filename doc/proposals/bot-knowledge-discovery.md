---
status: proposed
owner: Roberto
last_verified: 2026-06-08
implemented_by:
superseded_by:
related_code:
related_skills: write-docs, dev-guidelines
---

# Bot Knowledge Discovery & Codebase Wiki Integration

## 1. Goal
To design a system that makes codebase concepts **discoverable for AI bots**, resolving the context-window bottleneck. We outline the semantic questions this system must answer, how bots discover these concepts, and how we merge a structured codebase indexer with a lightweight, human-readable developer Wiki.

---

## 2. Brainstorming: Questions the System Must Answer

When a bot is instantiated, it lacks the history of decisions. To perform safely, it must be able to resolve four classes of questions:

```
+-----------------------------------------------------------------------------------+
| 1. CONCEPT GROUNDING                                                              |
| "What is SAX and where is it implemented?"                                        |
| -> Resolves semantic term to code file (sax.rs) and design doc (sax_structure.md)  |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 2. ARCHITECTURAL DEPENDENCIES                                                     |
| "If I modify database table X, what Tauri commands & Svelte stores are affected?" |
| -> Traverses the relation graph to show the data flow pipeline.                  |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 3. COMPLIANCE & BEST PRACTICES                                                     |
| "I want to trigger an IPC call, what are the typing rules?"                       |
| -> Resolves task to skill instructions (add-ipc-command/SKILL.md) and CommandMap. |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
| 4. DECISION REASONING & PROVENANCE                                                |
| "Why did we drop the waveform_fingerprint table?"                                 |
| -> Searches historical session logs and git commit logs for migration 31.         |
+-----------------------------------------------------------------------------------+
```

---

## 3. How to Make Concepts Discoverable for Bots

Bots are reactive—they follow instructions in their system prompts, entry points (`AGENTS.md`), and active skills. We make concepts discoverable using a three-tiered approach:

### A. The Entry-Point Hook
We modify the repository's bootstrap files (`AGENTS.md` and `GEMINI.md`) to mandate a knowledge check:

> **IMPORTANT**: Before editing any code or documentation, run the local knowledge query tool or use the native MCP server:
> `tools/knowledge_mgr.py query "<brief summary of your current task>"`
> Or call the MCP tool: `knowledge_mgr/query(text="<brief summary>")`.
> Load the returned files, database schemas, and skills into your context before proposing changes.

This forces the bot to query the indexer as its very first action.

### B. Inline Concept Grounding (JSDoc/Rustdoc Tags)
Developers and bots tag their code inline with custom annotations. The AST parser extracts these and registers them as relations:

**In Rust (`src-tauri/src/scanner/sax.rs`):**
```rust
/// @concept SAX
/// @skill add-analysis-pass
/// Implements symbolic aggregate approximation for track segmentation.
pub struct SaxPass;
```

**In Svelte (`src/lib/stores/player.svelte.ts`):**
```typescript
/**
 * @concept AudioPlayback
 * @uses CommandMap:play_track
 * Coordinates Wavesurfer.js audio states with Tauri.
 */
class PlayerStore { ... }
```

### C. The Hybrid "Semantic Index + Wiki" Approach
Instead of maintaining a complex, standalone graph visualization, we combine the **Wiki** (our existing flat Markdown documentation under `doc/` and `skills/`) with a local Python indexer:

1. **The Wiki is the Source of Truth**: Humans and bots write clear, simple Markdown documents detailing concepts, skills, and decisions.
2. **The Indexer is the Search Engine**: The Python tool indexes the Wiki and codebase, building the semantic connections.
3. **The Bot is the Reader**: The bot queries the index, receives a curated subset of 2–3 Wiki pages and 2–3 code files, and reads *only* those files, keeping token usage minimal.

---

## 4. Design: Python + ONNX MiniLM + SQLite-Vec Stack

We design the tool as `tools/knowledge_mgr.py`, backed by a small package under `tools/knowledge_mgr/`. It serves two modes: a standalone **CLI** for developers/hooks, and an **MCP Server** (JSON-RPC 2.0 over stdin/stdout) for AI agents. This matches the existing Python MCP pattern used by `tools/collab_mcp/` and `tools/ccrep/`.

```
                  +--------------------------+
                  |  tools/knowledge_mgr.py  |
                  +--------------------------+
                               |
            +------------------+------------------+
            | (CLI Mode)                          | (serve Mode)
            v                                     v
   - `knowledge_mgr.py lint`             - Speaks Model Context Protocol (JSON-RPC)
   - Optional git pre-commit hook        - Registered in `.mcp.json`
   - Scans AST & frontmatter             - Exposes native MCP Tools:
   - Validates structural rules             * `knowledge_mgr/query(text)`
                                            * `knowledge_mgr/check_rules()`
```

### A. Python MCP Server Pattern
We implement the server using the same lightweight Python MCP wrapper pattern already present in `tools/collab_mcp/server.py` and `tools/ccrep/server.py`. This allows us to start the server via:
```bash
tools/knowledge_mgr.py serve
```

### B. Embedding Backend
Semantic embeddings use the repo's existing local `all-MiniLM-L6-v2` ONNX path rather than Ollama. The implementation should reuse the model/export conventions already present around `src-tauri/src/embeddings.rs` and `tools/export_sentence_onnx.py`, storing vectors in `scratch/codebase_index.db` with `sqlite-vec`.

### C. Verification in Pre-Commit
The Python tool's CLI mode can be installed into an optional repository pre-commit hook:
1. It scans the modified files.
2. It runs structural rule checks to assert that all modified code concepts are documented.
3. If an agent changes code annotated with `@concept SAX` but the linter detects that `doc/research/sax_structure.md` is marked as `superseded`, it prompts the agent to update the code concept tag.

Because Git hooks are shared across worktrees, the hook must pass the current checkout path to `tools/knowledge_mgr.py lint --root "$PWD"` and must not scan sibling worktrees unless invoked with `--parallel`.

---

## 5. Proposed Next Steps

1. **Phase 1: Bootstrap the Python CLI & MCP Server**:
   * Create `tools/knowledge_mgr/` plus a `tools/knowledge_mgr.py` entry point.
   * Follow the `tools/collab_mcp/` and `tools/ccrep/` stdio MCP server pattern.
   * Implement basic Rust/TypeScript comment parsers for `@concept` and `@skill` tags.
2. **Phase 2: Integrate ONNX MiniLM Embeddings**:
   * Reuse the local `all-MiniLM-L6-v2` ONNX embedding path.
   * Initialize a local SQLite db `scratch/codebase_index.db` with `sqlite-vec` virtual tables to cache embeddings.
3. **Phase 3: Hook into Bot Entry Points**:
   * Register the Python server command `tools/knowledge_mgr.py serve` in `.mcp.json`.
   * Update `AGENTS.md` and `GEMINI.md` to document the index checking workflow.
