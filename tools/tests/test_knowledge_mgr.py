import pytest
import struct
from pathlib import Path
from knowledge_mgr.parser import parse_rust_file, parse_ts_svelte_file, parse_markdown_file
from knowledge_mgr.database import KnowledgeDB
from knowledge_mgr.embeddings import EmbeddingGenerator
from knowledge_mgr.scanner import chunk_markdown

def test_rust_parser():
    content = """
    // @concept AudioSAX
    // @skill add-analysis-pass
    /// Some description here
    #[tauri::command]
    pub async fn sax_align(track_id: i64) -> Result<(), String> {
        Ok(())
    }
    
    impl AnalysisPass for SaxPass {
        fn run(&self) {}
    }
    """
    facts = parse_rust_file(content, "src-tauri/src/scanner/sax.rs")
    assert "AudioSAX" in facts["defines"]
    assert "skill:add-analysis-pass" in facts["defines"]
    assert "cmd:sax_align" in facts["defines"]
    assert "SaxPass" in facts["defines"]
    assert "AnalysisPass" in facts["implements"]

def test_ts_svelte_parser():
    content = """
    // @concept PlaybackView
    import { invoke } from "@tauri-apps/api/core";
    
    function play() {
        invoke("play_track", { id: 123 });
    }
    """
    facts = parse_ts_svelte_file(content, "src/lib/Player.svelte")
    assert "PlaybackView" in facts["defines"]
    assert "cmd:play_track" in facts["calls"]
    assert "cmd:direct_api" in facts["calls"]  # imports from @tauri-apps/api directly
    assert not facts["uses_command_map"]
    
    # Test file that uses the command map
    content_ok = """
    import { safeInvoke } from "$lib/ipc";
    """
    facts_ok = parse_ts_svelte_file(content_ok, "src/lib/Player2.svelte")
    assert facts_ok["uses_command_map"]

def test_markdown_parser():
    content = """---
status: superseded
owner: Roberto
related_code: SaxPass, SaxAlign
related_skills: add-analysis-pass
---
# SAX Structure

@concept AudioSAX
"""
    facts = parse_markdown_file(content, "doc/research/sax.md")
    assert facts["frontmatter"]["status"] == "superseded"
    assert facts["frontmatter"]["owner"] == "Roberto"
    assert "SaxPass" in facts["documents"]
    assert "SaxAlign" in facts["documents"]
    assert "skill:add-analysis-pass" in facts["documents"]
    assert "AudioSAX" in facts["documents"]
    assert "AudioSAX" in facts["concept_covers"]

def test_markdown_chunking():
    content = """# Main Title
Intro text here.

## Section 1
Content of section 1.

## Section 2
Content of section 2.
"""
    chunks = chunk_markdown(content)
    assert len(chunks) == 3
    assert chunks[0]["header"] == "Main Title"
    assert chunks[0]["content"] == "Intro text here."
    assert chunks[1]["header"] == "Section 1"
    assert chunks[1]["content"] == "Content of section 1."
    assert chunks[2]["header"] == "Section 2"
    assert chunks[2]["content"] == "Content of section 2."

def test_rules_engine():
    # Setup in-memory db
    with KnowledgeDB(Path(":memory:")) as db:
        # 1. Test Direct IPC Violation
        # File 1 calls play_track directly and doesn't use command map.
        db.insert_file("src/lib/Player.svelte", 1.0)
        db.insert_call("src/lib/Player.svelte", "cmd:play_track")
        # Define play_track in a Rust file
        db.insert_file("src-tauri/src/lib.rs", 1.0)
        db.insert_define("src-tauri/src/lib.rs", "cmd:play_track")
        
        violations = db.get_rule_violations()
        assert len(violations) == 1
        assert violations[0]["rule_id"] == "direct_ipc_violation"
        assert "play_track" in violations[0]["message"]
        
        # Resolve it by addinguses_command_map
        db.insert_uses_command_map("src/lib/Player.svelte")
        violations = db.get_rule_violations()
        assert len(violations) == 0
        
        # 2. Test Undocumented Analysis Pass
        # SaxPass implements AnalysisPass, but no documents link it.
        db.insert_file("src-tauri/src/scanner/sax.rs", 1.0)
        db.insert_define("src-tauri/src/scanner/sax.rs", "SaxPass")
        db.insert_implement("src-tauri/src/scanner/sax.rs", "AnalysisPass")
        
        violations = db.get_rule_violations()
        assert len(violations) == 1
        assert violations[0]["rule_id"] == "undocumented_analysis_pass"
        assert "SaxPass" in violations[0]["message"]
        
        # Resolve by documenting it
        db.insert_file("skills/add-analysis-pass/SKILL.md", 1.0)
        db.insert_document("skills/add-analysis-pass/SKILL.md", "SaxPass")
        violations = db.get_rule_violations()
        assert len(violations) == 0
        
        # 3. Test Stale Concept Reference
        db.insert_file("src/lib/Player.svelte", 1.0)
        db.insert_concept_cover("AudioSAX", "src/lib/Player.svelte")
        db.insert_file("doc/research/sax.md", 1.0)
        db.insert_document("doc/research/sax.md", "AudioSAX")
        db.insert_doc_frontmatter("doc/research/sax.md", "superseded", "Roberto")
        
        violations = db.get_rule_violations()
        assert len(violations) == 1
        assert violations[0]["rule_id"] == "uses_stale_concept"
        assert "AudioSAX" in violations[0]["message"]
        
        # Change status to proposed (active) to resolve
        db.insert_doc_frontmatter("doc/research/sax.md", "proposed", "Roberto")
        violations = db.get_rule_violations()
        assert len(violations) == 0

def test_embeddings_generator():
    root = Path(__file__).parent.parent.parent
    try:
        generator = EmbeddingGenerator(root)
        vec = generator.generate_embedding("Codebase Knowledge Manager test query")
        assert len(vec) == 384
        # Norm should be 1.0 (L2 normalised)
        norm = sum(x*x for x in vec)**0.5
        assert abs(norm - 1.0) < 1e-4
    except FileNotFoundError:
        pytest.skip("Models not found/downloaded in test path, skipping ONNX check")
