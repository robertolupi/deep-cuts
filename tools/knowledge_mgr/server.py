import os
from pathlib import Path
from mcp.server.fastmcp import FastMCP
from .database import KnowledgeDB, resolve_db_path
from .embeddings import EmbeddingGenerator

mcp = FastMCP("knowledge_mgr")

def _get_root() -> Path:
    return Path(os.environ.get("KMGR_REPO_ROOT", ".")).resolve()

def _get_db_path() -> Path:
    root = _get_root()
    env_db = os.environ.get("KMGR_DB")
    if env_db:
        return Path(env_db).resolve()
    parallel = os.environ.get("KMGR_PARALLEL", "0") == "1"
    return resolve_db_path(str(root), parallel=parallel)

@mcp.tool()
def query(text: str, limit: int = 5) -> dict:
    """Run a semantic similarity query against the codebase index.
    
    Returns matching document sections and concepts.
    """
    root = _get_root()
    db_path = _get_db_path()
    embedder = EmbeddingGenerator(root)
    query_vec = embedder.generate_embedding(text)
    
    with KnowledgeDB(db_path) as db:
        matches = db.semantic_query(query_vec, limit=limit)
        
    return {"matches": matches}

@mcp.tool()
def check_rules() -> dict:
    """Run structural linter rules to identify knowledge drift or style violations."""
    db_path = _get_db_path()
    with KnowledgeDB(db_path) as db:
        violations = db.get_rule_violations()
    return {"violations": violations}

def main():
    mcp.run()

if __name__ == "__main__":
    main()
