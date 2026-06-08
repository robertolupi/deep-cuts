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
We modify the repository's bootstrap files ([AGENTS.md](file:///Users/rlupi/src/deep-cuts-agy/AGENTS.md) and [GEMINI.md](file:///Users/rlupi/src/deep-cuts-agy/GEMINI.md)) to mandate a knowledge check:

> **IMPORTANT**: Before editing any code or documentation, run the Go query tool or use the native MCP server:
> `tools/dc-knowledge-mgr query "<brief summary of your current task>"`
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
Instead of maintaining a complex, standalone graph visualization, we combine the **Wiki** (our existing flat Markdown documentation under `doc/` and `skills/`) with the **Go/Ollama indexer**:

1. **The Wiki is the Source of Truth**: Humans and bots write clear, simple Markdown documents detailing concepts, skills, and decisions.
2. **The Indexer is the Search Engine**: The Go/Ollama tool indexes the Wiki and codebase, building the semantic connections.
3. **The Bot is the Reader**: The bot queries the index, receives a curated subset of 2–3 Wiki pages and 2–3 code files, and reads *only* those files, keeping token usage minimal.

---

## 4. Design: The Go + Ollama + SQLite-Vec Stack (Go-Native MCP)

We design the tool `tools/dc-knowledge-mgr` as a single Go binary compiled locally. It serves two modes: a standalone **CLI** for developers/hooks, and a **native MCP Server** (JSON-RPC 2.0 over stdin/stdout) for AI agents.

```
                  +--------------------------+
                  |  tools/dc-knowledge-mgr  |
                  +--------------------------+
                               |
            +------------------+------------------+
            | (CLI Mode)                          | (serve Mode)
            v                                     v
   - `dc-knowledge-mgr lint`             - Speaks Model Context Protocol (JSON-RPC)
   - Runs on git pre-commit              - Registered in `.mcp.json`
   - Scans AST & frontmatter             - Exposes native MCP Tools:
   - Validates Datalog rules                * `knowledge_mgr/query(text)`
                                            * `knowledge_mgr/check_rules()`
```

### A. Go MCP Server Libraries
We implement the server directly in Go using a standard library-friendly wrapper like `github.com/mark3labs/mcp-go`. This allows us to start the server via:
```bash
tools/dc-knowledge-mgr serve
```

### B. Verification in Pre-Commit
The Go tool's CLI mode is registered in the repository's pre-commit hook:
1. It scans the modified files.
2. It runs Mangle queries to assert that all modified code concepts are documented.
3. If an agent changes code annotated with `@concept SAX` but the linter detects that `doc/research/sax_structure.md` is marked as `superseded`, it prompts the agent to update the code concept tag.

---

## 5. Proposed Next Steps

1. **Phase 1: Bootstrap the Go CLI & MCP Server**:
   * Create `tools/knowledge_mgr/` in Go.
   * Pull `github.com/mark3labs/mcp-go` and set up the `serve` JSON-RPC transport over stdin/stdout.
   * Implement basic Rust/TypeScript comment parsers for `@concept` and `@skill` tags.
2. **Phase 2: Integrate Ollama**:
   * Implement the HTTP client in Go to call Ollama on `localhost:11434/api/embeddings`.
   * Initialize a local SQLite db `scratch/codebase_index.db` to cache embeddings.
3. **Phase 3: Hook into Bot Entry Points**:
   * Register the Go server command `tools/dc-knowledge-mgr serve` in `.mcp.json`.
   * Update `AGENTS.md` and `GEMINI.md` to document the index checking workflow.
