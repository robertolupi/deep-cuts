import re
from pathlib import Path
from typing import Dict, Any, List

def parse_comments_for_tags(content: str) -> Dict[str, List[str]]:
    """Find inline tags @concept, @skill, and @documents in comments."""
    tags = {"concept": [], "skill": [], "documents": []}
    
    concept_pat = re.compile(r'@concept\s+(\S+)')
    skill_pat = re.compile(r'@skill\s+(\S+)')
    doc_pat = re.compile(r'@documents\s+(\S+)')
    
    for line in content.splitlines():
        # Match if the line contains a tag
        if "@" in line:
            m_c = concept_pat.search(line)
            if m_c:
                tags["concept"].append(m_c.group(1))
            m_s = skill_pat.search(line)
            if m_s:
                tags["skill"].append(m_s.group(1))
            m_d = doc_pat.search(line)
            if m_d:
                tags["documents"].append(m_d.group(1))
    return tags

def parse_rust_file(content: str, rel_path: str) -> Dict[str, Any]:
    facts = {
        "defines": [],
        "implements": [],
        "calls": [],
        "uses_command_map": False
    }
    
    # Extract tags
    tags = parse_comments_for_tags(content)
    for c in tags["concept"]:
        facts["defines"].append(c)
    for s in tags["skill"]:
        facts["defines"].append(f"skill:{s}")
    for d in tags["documents"]:
        facts["defines"].append(f"doc_link:{d}")
        
    # Match #[tauri::command] followed by fn definition
    cmd_pattern = re.compile(r'#\[tauri::command\]\s*(?:\/\/[^\n]*\n\s*)*?(?:#\[[^\]]+\]\s*)*?(?:pub\s+)?(?:async\s+)?fn\s+(\w+)')
    for match in cmd_pattern.finditer(content):
        cmd_name = match.group(1)
        facts["defines"].append(f"cmd:{cmd_name}")
        
    # Match implements of AnalysisPass or BatchAnalysisPass
    impl_pattern = re.compile(r'impl\s+(AnalysisPass|BatchAnalysisPass)\s*(?:<[^>]+>)?\s*for\s+(\w+)')
    for match in impl_pattern.finditer(content):
        trait_name = match.group(1)
        struct_name = match.group(2)
        facts["implements"].append(trait_name)
        facts["defines"].append(struct_name)
        
    return facts

def parse_ts_svelte_file(content: str, rel_path: str) -> Dict[str, Any]:
    facts = {
        "defines": [],
        "calls": [],
        "uses_command_map": False
    }
    
    # Extract tags
    tags = parse_comments_for_tags(content)
    for c in tags["concept"]:
        facts["defines"].append(c)
        
    # Check if this file imports or uses the CommandMap / ipc wrapper
    if "$lib/ipc" in content or "src/lib/ipc" in content:
        facts["uses_command_map"] = True
        
    # Find calls via invoke("cmd_name")
    invoke_pattern = re.compile(r'invoke\(\s*["\']([^"\']+)["\']')
    for match in invoke_pattern.finditer(content):
        cmd_name = match.group(1)
        facts["calls"].append(f"cmd:{cmd_name}")
        
    # Check for direct imports of @tauri-apps/api
    if "from '@tauri-apps/api" in content or 'from "@tauri-apps/api' in content:
        facts["calls"].append("cmd:direct_api")
        
    return facts

def parse_markdown_file(content: str, rel_path: str) -> Dict[str, Any]:
    facts = {
        "frontmatter": {},
        "documents": [],
        "concept_covers": []
    }
    
    # Parse frontmatter yaml
    if content.startswith("---"):
        end = content.find("\n---", 3)
        if end != -1:
            fm_text = content[3:end]
            for line in fm_text.splitlines():
                if ":" in line:
                    key, val = line.split(":", 1)
                    key = key.strip()
                    val = val.strip()
                    if val.startswith(('"', "'")) and val.endswith(('"', "'")):
                        val = val[1:-1]
                    facts["frontmatter"][key] = val
                    
    # Process standard links/relationships from frontmatter
    related_code = facts["frontmatter"].get("related_code", "")
    if related_code:
        for entity in re.split(r'[,\s]+', related_code):
            entity = entity.strip()
            if entity:
                facts["documents"].append(entity)
                
    related_skills = facts["frontmatter"].get("related_skills", "")
    if related_skills:
        for skill in re.split(r'[,\s]+', related_skills):
            skill = skill.strip()
            if skill:
                facts["documents"].append(f"skill:{skill}")
                
    # Search for inline tags
    tags = parse_comments_for_tags(content)
    for d in tags["documents"]:
        facts["documents"].append(d)
    for c in tags["concept"]:
        facts["documents"].append(c)
        facts["concept_covers"].append(c)
        
    # Infer skill mappings from directory structure
    path_parts = Path(rel_path).parts
    if "skills" in path_parts:
        idx = path_parts.index("skills")
        if idx + 1 < len(path_parts):
            skill_name = path_parts[idx + 1]
            facts["frontmatter"]["skill_name"] = skill_name
            if skill_name == "add-analysis-pass":
                facts["documents"].append("AnalysisPass")
                facts["documents"].append("BatchAnalysisPass")
            elif skill_name == "add-ipc-command":
                facts["documents"].append("CommandMap")
                
    return facts
