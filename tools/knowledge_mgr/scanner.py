import os
import re
import subprocess
from pathlib import Path
from typing import List, Dict, Any, Optional
from .database import KnowledgeDB
from .embeddings import EmbeddingGenerator
from . import parser

def get_git_worktrees(root_path: Path) -> List[Path]:
    """Get list of active git worktree paths."""
    try:
        res = subprocess.run(
            ["git", "worktree", "list"],
            cwd=str(root_path),
            capture_output=True,
            text=True,
            check=True
        )
        worktrees = []
        for line in res.stdout.splitlines():
            if line.strip():
                parts = line.split()
                if parts:
                    worktrees.append(Path(parts[0]).resolve())
        return worktrees
    except Exception:
        return [root_path.resolve()]

def should_index_file(path: Path) -> bool:
    """Filter out test files, node_modules, target, etc."""
    parts = path.parts
    if "node_modules" in parts or "target" in parts or ".git" in parts or ".venv" in parts:
        return False
    if path.name.endswith((".test.ts", ".test.js", ".spec.ts", ".spec.js", ".d.ts")):
        return False
    if path.name == "INDEX.md":  # Skip skill index to prevent self-reference loops
        return False
    return path.suffix in (".rs", ".ts", ".js", ".svelte", ".md")

def chunk_markdown(content: str) -> List[Dict[str, str]]:
    """Chunk Markdown into sections based on H2 (##) or H1 (#) headers."""
    chunks = []
    lines = content.splitlines()
    
    current_header = "_intro"
    current_lines = []
    
    for line in lines:
        if line.startswith("## "):
            # Save previous chunk
            if current_lines:
                chunks.append({
                    "header": current_header,
                    "content": "\n".join(current_lines).strip()
                })
            current_header = line[3:].strip()
            current_lines = []
        elif line.startswith("# ") and current_header == "_intro":
            # Set title/intro header
            current_header = line[2:].strip()
        else:
            current_lines.append(line)
            
    if current_lines:
        chunks.append({
            "header": current_header,
            "content": "\n".join(current_lines).strip()
        })
        
    return chunks

def scan_file(db: KnowledgeDB, embedder: Optional[EmbeddingGenerator], root_path: Path, file_path: Path, force: bool = False):
    """Scan a single file, extract facts/embeddings and update DB."""
    abs_path = file_path.resolve()
    rel_path = str(abs_path)  # Store absolute path for cross-worktree uniqueness and clicking
    
    try:
        mtime = os.path.getmtime(abs_path)
    except OSError:
        return

    # Check database to see if already scanned and unchanged
    if not force:
        cursor = db.conn.cursor()
        cursor.execute("SELECT mtime FROM files WHERE path = ?", (rel_path,))
        row = cursor.fetchone()
        if row and row["mtime"] == mtime:
            return  # Up to date

    # Clear prior facts for this file
    db.clear_file_facts(rel_path)

    try:
        content = abs_path.read_text(encoding="utf-8")
    except Exception:
        return  # Skip unreadable/binary files

    suffix = abs_path.suffix
    
    if suffix == ".rs":
        facts = parser.parse_rust_file(content, rel_path)
        for d in facts["defines"]:
            db.insert_define(rel_path, d)
        for i in facts["implements"]:
            db.insert_implement(rel_path, i)
        for c in facts["calls"]:
            db.insert_call(rel_path, c)
            
    elif suffix in (".ts", ".js", ".svelte"):
        facts = parser.parse_ts_svelte_file(content, rel_path)
        for d in facts["defines"]:
            db.insert_define(rel_path, d)
        for c in facts["calls"]:
            db.insert_call(rel_path, c)
        if facts["uses_command_map"]:
            db.insert_uses_command_map(rel_path)
            
    elif suffix == ".md":
        facts = parser.parse_markdown_file(content, rel_path)
        fm = facts["frontmatter"]
        
        # Save frontmatter
        status = fm.get("status", "proposed")
        owner = fm.get("owner", "unknown")
        db.insert_doc_frontmatter(rel_path, status, owner)
        
        # Save documents and concept_covers
        for d in facts["documents"]:
            db.insert_document(rel_path, d)
        for cc in facts["concept_covers"]:
            db.insert_concept_cover(cc, rel_path)
            
        # Semantic embedding chunking
        if embedder:
            sections = chunk_markdown(content)
            for sec in sections:
                header = sec["header"]
                sec_text = sec["content"]
                if not sec_text.strip():
                    continue
                node_id = f"{rel_path}:{header}"
                node_content = f"File: {abs_path.name}\nSection: {header}\n\n{sec_text}"
                try:
                    emb = embedder.generate_embedding(node_content)
                    db.insert_node_embedding(node_id, node_content, emb)
                except Exception as e:
                    # Silent skip or minimal print if embedding fails (e.g. tokenizer/onnx mismatch)
                    pass

    db.insert_file(rel_path, mtime)

def scan_all(root_path: Path, db: KnowledgeDB, embedder: Optional[EmbeddingGenerator], force: bool = False, parallel: bool = False):
    """Scan the entire codebase or active worktrees."""
    roots_to_scan = [root_path]
    if parallel:
        roots_to_scan = get_git_worktrees(root_path)
        
    for scan_root in roots_to_scan:
        for dirpath, _, filenames in os.walk(str(scan_root)):
            for fname in filenames:
                fpath = Path(dirpath) / fname
                if should_index_file(fpath):
                    scan_file(db, embedder, scan_root, fpath, force=force)
                    
    # Prune files that no longer exist in the active paths
    cursor = db.conn.cursor()
    cursor.execute("SELECT path FROM files")
    all_paths = [row["path"] for row in cursor.fetchall()]
    for p in all_paths:
        path_obj = Path(p)
        # Check if it still belongs to any scanned root and exists
        exists_in_roots = False
        for scan_root in roots_to_scan:
            try:
                # Is p inside scan_root?
                Path(p).relative_to(scan_root)
                if path_obj.exists():
                    exists_in_roots = True
                    break
            except ValueError:
                pass
        if not exists_in_roots:
            db.clear_file_facts(p)
