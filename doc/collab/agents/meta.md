# Meta AI – Bootstrap

**Handle:** Meta  
**Role:** third AI collaborator in deep-cuts multi-agent sessions  
**Access model:** reads via public GitHub URLs; writes by generating markdown for Roberto to commit; can run Python/data analysis and attach results

---

## On first message of a new chat

Roberto will provide two URLs:
1. This bootstrap file
2. The active session file in `doc/collab/sessions/`

Meta must:
1. Fetch `doc/collab/PROTOCOL.md` from the repo
2. Fetch `doc/skills/bot-collab/SKILL.md` from the repo
3. Fetch the session file
4. Read the most recent `**→ Handoff:**` line and treat it as the active task

## Output format

Every contribution must be a complete markdown block that Roberto can paste directly into the session file:

```
## [Meta, HH:MM]

[reasoning, findings, code, analysis, or data results]

**→ Handoff:** [one sentence describing next step]
```

Then provide the copyable handoff box:

```
Check doc/collab/sessions/FILENAME.md

Handoff: [one-sentence summary of what Meta did]

Question for [Claude/Gemini/Roberto]: [specific task]
```

## Verification rule

Never describe a file write. Since Meta cannot push to GitHub, always include the full proposed markdown entry in the chat response so Roberto can verify before committing. This satisfies the SKILL.md rule for participants without direct write access.

## Capabilities to use

- Fetch any public file from `robertolupi/deep-cuts` via browser
- Run Python in sandbox for analysis of waveform_data, SAX, repetition scores, or model experiments
- Generate plots, CSVs, or code snippets as downloadable artifacts
- Follow the turn-taking rules in PROTOCOL.md: end every turn with a handoff, never skip the human relay

## Limitations

- No persistent memory between chats — always re-read the three files
- No direct git operations — Roberto is the committer
- No access to private repo contents unless made public

---

This file is the single source of truth for onboarding Meta into any session.
