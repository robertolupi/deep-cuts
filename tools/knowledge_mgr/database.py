import sqlite3
import sqlite_vec
import subprocess
from pathlib import Path
from typing import List, Tuple, Dict, Any, Optional
import struct

def get_git_common_dir(root_path: str) -> Path:
    try:
        res = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            cwd=root_path,
            capture_output=True,
            text=True,
            check=True
        )
        git_common = Path(res.stdout.strip())
        if not git_common.is_absolute():
            git_common = (Path(root_path) / git_common).resolve()
        return git_common
    except Exception:
        return Path(root_path) / ".git"

def resolve_db_path(root_path: str, parallel: bool = False) -> Path:
    root = Path(root_path).resolve()
    if parallel:
        git_common = get_git_common_dir(str(root))
        path = git_common
        while path.name in (".git", "worktrees") or path.parent.name == "worktrees":
            path = path.parent
        shared_root = path
        shared_db = shared_root / "scratch" / "codebase_index.db"
        shared_db.parent.mkdir(parents=True, exist_ok=True)
        return shared_db
    else:
        local_db = root / "scratch" / "codebase_index.db"
        local_db.parent.mkdir(parents=True, exist_ok=True)
        return local_db

class KnowledgeDB:
    def __init__(self, db_path: Path):
        self.db_path = db_path
        self.conn = None

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def connect(self):
        self.conn = sqlite3.connect(str(self.db_path))
        self.conn.enable_load_extension(True)
        sqlite_vec.load(self.conn)
        self.conn.enable_load_extension(False)
        self.conn.row_factory = sqlite3.Row
        self._init_schema()

    def close(self):
        if self.conn:
            self.conn.close()
            self.conn = None

    def _init_schema(self):
        cursor = self.conn.cursor()
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                mtime REAL
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS defines (
                file TEXT,
                entity TEXT,
                PRIMARY KEY (file, entity)
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS implements (
                file TEXT,
                trait TEXT,
                PRIMARY KEY (file, trait)
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS calls (
                file TEXT,
                target TEXT,
                PRIMARY KEY (file, target)
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS uses_command_map (
                file TEXT PRIMARY KEY
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS documents (
                doc_file TEXT,
                target_entity TEXT,
                PRIMARY KEY (doc_file, target_entity)
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS concept_covers (
                concept_name TEXT,
                file TEXT,
                PRIMARY KEY (concept_name, file)
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS doc_frontmatter (
                doc_file TEXT PRIMARY KEY,
                status TEXT,
                owner TEXT
            )
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS node_embedding_metadata (
                node_id TEXT PRIMARY KEY,
                content_text TEXT NOT NULL
            )
        """)
        
        # Safe check for VIRTUAL TABLE creation
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='node_embeddings'")
        if not cursor.fetchone():
            cursor.execute("""
                CREATE VIRTUAL TABLE node_embeddings USING vec0(
                    node_id TEXT PRIMARY KEY,
                    embedding FLOAT[384]
                )
            """)
        self.conn.commit()

    def clear_file_facts(self, file_path: str):
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM defines WHERE file = ?", (file_path,))
        cursor.execute("DELETE FROM implements WHERE file = ?", (file_path,))
        cursor.execute("DELETE FROM calls WHERE file = ?", (file_path,))
        cursor.execute("DELETE FROM uses_command_map WHERE file = ?", (file_path,))
        cursor.execute("DELETE FROM documents WHERE doc_file = ?", (file_path,))
        cursor.execute("DELETE FROM concept_covers WHERE file = ? OR concept_name = ?", (file_path, file_path))
        cursor.execute("DELETE FROM doc_frontmatter WHERE doc_file = ?", (file_path,))
        cursor.execute("DELETE FROM node_embedding_metadata WHERE node_id = ? OR node_id LIKE ?", (file_path, f"{file_path}:%"))
        cursor.execute("DELETE FROM node_embeddings WHERE node_id = ? OR node_id LIKE ?", (file_path, f"{file_path}:%"))
        cursor.execute("DELETE FROM files WHERE path = ?", (file_path,))
        self.conn.commit()

    def insert_file(self, file_path: str, mtime: float):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR REPLACE INTO files (path, mtime) VALUES (?, ?)", (file_path, mtime))
        self.conn.commit()

    def insert_define(self, file_path: str, entity: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO defines (file, entity) VALUES (?, ?)", (file_path, entity))
        self.conn.commit()

    def insert_implement(self, file_path: str, trait: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO implements (file, trait) VALUES (?, ?)", (file_path, trait))
        self.conn.commit()

    def insert_call(self, file_path: str, target: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO calls (file, target) VALUES (?, ?)", (file_path, target))
        self.conn.commit()

    def insert_uses_command_map(self, file_path: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO uses_command_map (file) VALUES (?)", (file_path,))
        self.conn.commit()

    def insert_document(self, doc_file: str, target_entity: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO documents (doc_file, target_entity) VALUES (?, ?)", (doc_file, target_entity))
        self.conn.commit()

    def insert_concept_cover(self, concept_name: str, file_path: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR IGNORE INTO concept_covers (concept_name, file) VALUES (?, ?)", (concept_name, file_path))
        self.conn.commit()

    def insert_doc_frontmatter(self, doc_file: str, status: str, owner: str):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR REPLACE INTO doc_frontmatter (doc_file, status, owner) VALUES (?, ?, ?)", (doc_file, status, owner))
        self.conn.commit()

    def insert_node_embedding(self, node_id: str, content_text: str, embedding: List[float]):
        cursor = self.conn.cursor()
        cursor.execute("INSERT OR REPLACE INTO node_embedding_metadata (node_id, content_text) VALUES (?, ?)", (node_id, content_text))
        emb_bytes = struct.pack(f"{len(embedding)}f", *embedding)
        cursor.execute("INSERT OR REPLACE INTO node_embeddings (node_id, embedding) VALUES (?, ?)", (node_id, emb_bytes))
        self.conn.commit()

    def get_rule_violations(self) -> List[Dict[str, Any]]:
        cursor = self.conn.cursor()
        violations = []

        # Rule 1: Direct IPC violation
        cursor.execute("""
            SELECT c.file, c.target
            FROM calls c
            JOIN defines d ON c.target = d.entity
            WHERE d.entity LIKE 'cmd:%'
              AND c.file NOT IN (SELECT file FROM uses_command_map)
        """)
        for row in cursor.fetchall():
            command = row['target'][4:]  # strip 'cmd:'
            violations.append({
                "rule_id": "direct_ipc_violation",
                "file": row['file'],
                "entity": row['target'],
                "message": f"File {row['file']} calls Tauri command '{command}' directly instead of using $lib/ipc.ts"
            })

        # Rule 2: Undocumented analysis pass
        cursor.execute("""
            SELECT i.file, d.entity, i.trait
            FROM implements i
            JOIN defines d ON i.file = d.file
            LEFT JOIN documents doc ON d.entity = doc.target_entity
            WHERE i.trait IN ('AnalysisPass', 'BatchAnalysisPass')
              AND doc.target_entity IS NULL
        """)
        for row in cursor.fetchall():
            violations.append({
                "rule_id": "undocumented_analysis_pass",
                "file": row['file'],
                "entity": row['entity'],
                "message": f"Struct '{row['entity']}' implements Rust trait '{row['trait']}' in {row['file']}, but no documentation links this struct to a skill/doc."
            })

        # Rule 3: Stale concept reference
        cursor.execute("""
            SELECT cc.file, cc.concept_name, df.status, df.doc_file
            FROM concept_covers cc
            JOIN documents doc ON cc.concept_name = doc.target_entity
            JOIN doc_frontmatter df ON doc.doc_file = df.doc_file
            WHERE df.status IN ('superseded', 'rejected')
        """)
        for row in cursor.fetchall():
            violations.append({
                "rule_id": "uses_stale_concept",
                "file": row['file'],
                "entity": row['concept_name'],
                "message": f"File {row['file']} references concept '{row['concept_name']}' which is documented as '{row['status']}' in {row['doc_file']}."
            })

        return violations

    def semantic_query(self, query_vector: List[float], limit: int = 5) -> List[Dict[str, Any]]:
        cursor = self.conn.cursor()
        emb_bytes = struct.pack(f"{len(query_vector)}f", *query_vector)
        # Using sqlite-vec distance function for vec0 virtual tables.
        # We join on node_embedding_metadata to get text.
        # Under sqlite-vec, query matches are:
        cursor.execute("""
            SELECT 
                m.node_id, 
                m.content_text, 
                vec_distance_cosine(v.embedding, ?) AS distance
            FROM node_embeddings v
            JOIN node_embedding_metadata m ON v.node_id = m.node_id
            ORDER BY distance ASC
            LIMIT ?
        """, (emb_bytes, limit))
        
        results = []
        for row in cursor.fetchall():
            results.append({
                "node_id": row["node_id"],
                "content_text": row["content_text"],
                "similarity": 1.0 - row["distance"]  # Cosine similarity
            })
        return results

    def get_all_facts(self) -> Dict[str, List[Tuple[Any, ...]]]:
        cursor = self.conn.cursor()
        facts = {}
        for table in ["defines", "implements", "calls", "uses_command_map", "documents", "concept_covers", "doc_frontmatter"]:
            cursor.execute(f"SELECT * FROM {table}")
            facts[table] = [tuple(row) for row in cursor.fetchall()]
        return facts
