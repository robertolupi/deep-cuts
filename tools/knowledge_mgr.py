#!/usr/bin/env python3
import sys
import argparse
import subprocess
from pathlib import Path
from typing import Dict, List, Set, Tuple, Any

from knowledge_mgr.database import KnowledgeDB, resolve_db_path
from knowledge_mgr.embeddings import EmbeddingGenerator
from knowledge_mgr.scanner import scan_all, should_index_file
from knowledge_mgr.parser import parse_rust_file, parse_ts_svelte_file, parse_markdown_file
from knowledge_mgr.server import main as run_server

def get_violations_from_facts(facts: dict) -> List[Dict[str, Any]]:
    """Helper to evaluate linter rules in-memory on a dictionary of extracted facts."""
    with KnowledgeDB(Path(":memory:")) as db:
        # Insert files
        all_files = set()
        for table in ["defines", "implements", "calls", "uses_command_map", "documents", "concept_covers"]:
            for row in facts.get(table, []):
                if table in ("defines", "implements", "calls", "documents"):
                    all_files.add(row[0])
                elif table == "uses_command_map":
                    all_files.add(row[0] if isinstance(row, str) else row[0])
                elif table == "concept_covers":
                    all_files.add(row[1])
        for f in all_files:
            db.insert_file(f, 0.0)
            
        # Populate facts
        for f, entity in facts.get("defines", []):
            db.insert_define(f, entity)
        for f, trait in facts.get("implements", []):
            db.insert_implement(f, trait)
        for f, target in facts.get("calls", []):
            db.insert_call(f, target)
        for f in facts.get("uses_command_map", []):
            db.insert_uses_command_map(f)
        for f, target in facts.get("documents", []):
            db.insert_document(f, target)
        for concept, f in facts.get("concept_covers", []):
            db.insert_concept_cover(concept, f)
        for f, (status, owner) in facts.get("doc_frontmatter", {}).items():
            db.insert_doc_frontmatter(f, status, owner)
            
        return db.get_rule_violations()

def get_facts_from_git_target(root_path: Path, target: str) -> dict:
    """Extract structural facts from a target Git commit or branch."""
    facts = {
        "defines": set(),
        "implements": set(),
        "calls": set(),
        "uses_command_map": set(),
        "documents": set(),
        "concept_covers": set(),
        "doc_frontmatter": {}
    }
    
    try:
        res = subprocess.run(
            ["git", "ls-tree", "-r", "--name-only", target],
            cwd=str(root_path),
            capture_output=True,
            text=True,
            check=True
        )
    except Exception as e:
        print(f"Error listing files in target '{target}': {e}")
        return facts
        
    for line in res.stdout.splitlines():
        rel_path = line.strip()
        if not rel_path:
            continue
        fpath = Path(rel_path)
        if not should_index_file(fpath):
            continue
            
        try:
            show_res = subprocess.run(
                ["git", "show", f"{target}:{rel_path}"],
                cwd=str(root_path),
                capture_output=True,
                text=True,
                check=True
            )
            content = show_res.stdout
        except Exception:
            continue
            
        abs_path_str = str((root_path / rel_path).resolve())
        suffix = fpath.suffix
        
        if suffix == ".rs":
            f = parse_rust_file(content, abs_path_str)
            for d in f["defines"]: facts["defines"].add((abs_path_str, d))
            for i in f["implements"]: facts["implements"].add((abs_path_str, i))
            for c in f["calls"]: facts["calls"].add((abs_path_str, c))
        elif suffix in (".ts", ".js", ".svelte"):
            f = parse_ts_svelte_file(content, abs_path_str)
            for d in f["defines"]: facts["defines"].add((abs_path_str, d))
            for c in f["calls"]: facts["calls"].add((abs_path_str, c))
            if f["uses_command_map"]: facts["uses_command_map"].add(abs_path_str)
        elif suffix == ".md":
            f = parse_markdown_file(content, abs_path_str)
            status = f["frontmatter"].get("status", "proposed")
            owner = f["frontmatter"].get("owner", "unknown")
            facts["doc_frontmatter"][abs_path_str] = (status, owner)
            for d in f["documents"]: facts["documents"].add((abs_path_str, d))
            for cc in f["concept_covers"]: facts["concept_covers"].add((cc, abs_path_str))
            
    return facts

def get_facts_from_dir(dir_path: Path) -> dict:
    """Extract structural facts from a local directory path."""
    import os
    facts = {
        "defines": set(),
        "implements": set(),
        "calls": set(),
        "uses_command_map": set(),
        "documents": set(),
        "concept_covers": set(),
        "doc_frontmatter": {}
    }
    
    for dirpath, _, filenames in os.walk(str(dir_path)):
        for fname in filenames:
            fpath = Path(dirpath) / fname
            if should_index_file(fpath):
                try:
                    content = fpath.read_text(encoding="utf-8")
                except Exception:
                    continue
                abs_path_str = str(fpath.resolve())
                suffix = fpath.suffix
                if suffix == ".rs":
                    f = parse_rust_file(content, abs_path_str)
                    for d in f["defines"]: facts["defines"].add((abs_path_str, d))
                    for i in f["implements"]: facts["implements"].add((abs_path_str, i))
                    for c in f["calls"]: facts["calls"].add((abs_path_str, c))
                elif suffix in (".ts", ".js", ".svelte"):
                    f = parse_ts_svelte_file(content, abs_path_str)
                    for d in f["defines"]: facts["defines"].add((abs_path_str, d))
                    for c in f["calls"]: facts["calls"].add((abs_path_str, c))
                    if f["uses_command_map"]: facts["uses_command_map"].add(abs_path_str)
                elif suffix == ".md":
                    f = parse_markdown_file(content, abs_path_str)
                    status = f["frontmatter"].get("status", "proposed")
                    owner = f["frontmatter"].get("owner", "unknown")
                    facts["doc_frontmatter"][abs_path_str] = (status, owner)
                    for d in f["documents"]: facts["documents"].add((abs_path_str, d))
                    for cc in f["concept_covers"]: facts["concept_covers"].add((cc, abs_path_str))
    return facts

def cmd_lint(args) -> int:
    root = Path(args.root).resolve()
    db_path = resolve_db_path(str(root), parallel=args.parallel)
    
    embedder = None
    if not args.no_embed:
        try:
            embedder = EmbeddingGenerator(root)
        except Exception as e:
            print(f"Warning: Embedding generator offline: {e}")
            
    with KnowledgeDB(db_path) as db:
        print(f"Scanning files in {root} (Parallel Mode = {args.parallel}) …")
        scan_all(root, db, embedder, force=args.force, parallel=args.parallel)
        
        violations = db.get_rule_violations()
        
    if violations:
        print("\n✗ KNOWLEDGE LINT FAILURE:")
        for v in violations:
            print(f"  [{v['rule_id']}] File: {v['file']}")
            print(f"    Issue: {v['message']}")
        return 1
        
    print("\n✓ No knowledge manager violations detected.")
    return 0

def cmd_query(args) -> int:
    root = Path(args.root).resolve()
    db_path = resolve_db_path(str(root), parallel=args.parallel)
    
    try:
        embedder = EmbeddingGenerator(root)
        query_vec = embedder.generate_embedding(args.text)
    except Exception as e:
        print(f"Error loading models/generating embedding: {e}")
        return 1
        
    with KnowledgeDB(db_path) as db:
        matches = db.semantic_query(query_vec, limit=args.limit)
        
    if not matches:
        print("No semantic matches found.")
        return 0
        
    print(f"\nSemantic matches for: '{args.text}'")
    for i, m in enumerate(matches, start=1):
        print(f"\n{i}. Node: {m['node_id']} (Similarity: {m['similarity']:.4f})")
        print("─" * 40)
        print(m["content_text"])
    return 0

def cmd_diff(args) -> int:
    root = Path(args.root).resolve()
    target = args.target
    
    # Resolve target facts
    target_path = Path(target)
    if target_path.exists() and target_path.is_dir():
        print(f"Comparing current local files against directory: {target} …")
        target_facts = get_facts_from_dir(target_path)
    else:
        # Verify git ref
        try:
            subprocess.run(
                ["git", "rev-parse", "--verify", target],
                cwd=str(root),
                capture_output=True,
                check=True
            )
        except Exception:
            print(f"Error: Target '{target}' is not a valid directory or Git reference.")
            return 1
            
        print(f"Comparing current local files against Git target '{target}' …")
        target_facts = get_facts_from_git_target(root, target)
        
    # Get current facts from a fast local scan
    print("Parsing current worktree files …")
    current_facts = get_facts_from_dir(root)
    
    # Diff defines
    added_defines = current_facts["defines"] - target_facts["defines"]
    removed_defines = target_facts["defines"] - current_facts["defines"]
    
    # Diff violations
    print("Evaluating linter rules …")
    target_violations = get_violations_from_facts(target_facts)
    current_violations = get_violations_from_facts(current_facts)
    
    # Helper to format violations for comparison
    def get_violation_key(v):
        return (v["rule_id"], v["file"], v["entity"])
        
    target_violation_map = {get_violation_key(v): v for v in target_violations}
    current_violation_map = {get_violation_key(v): v for v in current_violations}
    
    added_violations = [v for k, v in current_violation_map.items() if k not in target_violation_map]
    resolved_violations = [v for k, v in target_violation_map.items() if k not in current_violation_map]
    
    # Print results
    print(f"\n=== Knowledge Manager Diff ===")
    
    if added_defines:
        print("\n+ Added Symbols / Concepts:")
        for path, entity in sorted(added_defines):
            fname = Path(path).name
            print(f"  + {entity} in {fname}")
            
    if removed_defines:
        print("\n- Removed Symbols / Concepts:")
        for path, entity in sorted(removed_defines):
            fname = Path(path).name
            print(f"  - {entity} in {fname}")
            
    if added_violations:
        print("\n✗ Added Rule Violations:")
        for v in added_violations:
            print(f"  + [{v['rule_id']}] File: {Path(v['file']).name} -> {v['message']}")
            
    if resolved_violations:
        print("\n✓ Resolved Rule Violations:")
        for v in resolved_violations:
            print(f"  - [{v['rule_id']}] File: {Path(v['file']).name} -> {v['message']}")
            
    if not (added_defines or removed_defines or added_violations or resolved_violations):
        print("\nNo differences in symbols, concepts, or violations detected.")
        
    return 0

def main():
    parser = argparse.ArgumentParser(description="Codebase Knowledge Manager CLI")
    subparsers = parser.add_subparsers(dest="cmd", required=True)
    
    # Lint command
    p_lint = subparsers.add_parser("lint", help="Verify codebase style rules and documentation links")
    p_lint.add_argument("--root", default=".", help="Root path of the checkout")
    p_lint.add_argument("--parallel", action="store_true", help="Sync/check all active git worktrees")
    p_lint.add_argument("--force", action="store_true", help="Force re-scanning of unchanged files")
    p_lint.add_argument("--no-embed", action="store_true", help="Disable generating semantic embeddings during scan")
    
    # Query command
    p_query = subparsers.add_parser("query", help="Query the codebase index semantically")
    p_query.add_argument("text", help="Natural language query string")
    p_query.add_argument("--root", default=".", help="Root path of the checkout")
    p_query.add_argument("--parallel", action="store_true", help="Search the parallel shared database")
    p_query.add_argument("--limit", type=int, default=5, help="Number of matching results to return")
    
    # Diff command
    p_diff = subparsers.add_parser("diff", help="Diff symbols and violations between checkout and target")
    p_diff.add_argument("target", nargs="?", default="main", help="Target git commit, branch, or directory path (default: main)")
    p_diff.add_argument("--root", default=".", help="Root path of the checkout")
    
    # Serve command
    p_serve = subparsers.add_parser("serve", help="Start the stdio Model Context Protocol (MCP) server")
    
    args = parser.parse_args()
    
    if args.cmd == "lint":
        sys.exit(cmd_lint(args))
    elif args.cmd == "query":
        sys.exit(cmd_query(args))
    elif args.cmd == "diff":
        sys.exit(cmd_diff(args))
    elif args.cmd == "serve":
        run_server()

if __name__ == "__main__":
    main()
