# Notepad benchmark tasks

This directory contains declarative benchmark tasks for the Notepad adapter.
TASK-036 owns T1.1, TASK-037 owns T1.2, and T1.3 is a separate card.

## T1.1

`t1.1.open-read-full-text.json` defines a read-only task:

1. open a text file;
2. read its canonical text;
3. count logical lines;
4. return paragraphs containing requested literal keywords.

Normal files use the UIA text path (`notepad.file.read_text`). Files larger
than `max_text_bytes` use the L1 file channel and return `truncated=true`.
The task still reports `line_count_total` for the whole file, while keyword
analysis is explicitly scoped to the truncated prefix.

## T1.2

`t1.2.replace-save-approval-undo.json` defines a write task:

1. open a text file and resolve exactly one editor target;
2. capture canonical text and prepare L0 plus L1 rollback anchors;
3. compute the literal replacement, count, and approval diff;
4. request a `once` approval with `show_diff=true`;
5. replace all literal occurrences and request a second `once` approval to save;
6. verify replacement count, title state, and disk readback.

The replacement and save tools are the already-declared
`notepad.file.replace_text` and `notepad.file.save`. This task does not add or
pretend to register a new tool.

Rollback evidence is split by persistence boundary:

- before save, L0 `Ctrl+Z` must return the canonical text to the pre-replace
  anchor;
- after a successful save, L1 snapshot recovery must restore both the document
  text and disk content because `Ctrl+Z` only changes in-memory state;
- if L0 evidence is unavailable, L1 is the fallback and is verified explicitly.

Target resolution is always `error_if_ambiguous`. A replacement-count mismatch,
denied approval, or failed save postcondition fails closed before success is
reported.

## Open-file boundary

The current adapter tool set has no `notepad.file.open` tool. T1.1 therefore
declares `open_mode=platform_open_file` as a task precondition and records the
gap as DRIFT-036-1. This directory does not invent an unregistered tool.

## Evaluation fixtures

`eval/tasks/notepad/t1.1/generate-fixtures.py` generates deterministic local
fixtures. The 1 MB fixture is generated on demand and is not committed.
