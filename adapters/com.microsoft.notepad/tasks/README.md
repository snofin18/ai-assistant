# Notepad benchmark tasks

This directory contains declarative benchmark tasks for the Notepad adapter.
TASK-036 owns T1.1 only. T1.2 and T1.3 are separate cards.

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

## Open-file boundary

The current adapter tool set has no `notepad.file.open` tool. T1.1 therefore
declares `open_mode=platform_open_file` as a task precondition and records the
gap as DRIFT-036-1. This directory does not invent an unregistered tool.

## Evaluation fixtures

`eval/tasks/notepad/t1.1/generate-fixtures.py` generates deterministic local
fixtures. The 1 MB fixture is generated on demand and is not committed.
