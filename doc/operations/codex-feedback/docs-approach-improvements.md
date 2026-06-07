# Docs and Proposal Improvements

Date: 2026-06-06

## 1. Keep docs synchronized with implementation

Recent history shows docs and implementation moving fast around SAX/structure analysis. Add a lightweight "doc sync" checklist for feature commits:

- If a migration changed, update related design docs.
- If an analysis pass changed, update `skills/add-analysis-pass`.
- If an IPC command changed, update typed frontend command docs/mocks.
- If a proposal was implemented differently than planned, add an "Implemented outcome" note.
- If a feature was removed, mark the proposal or old approach as superseded.

