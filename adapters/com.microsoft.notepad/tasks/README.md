# Notepad benchmark tasks

This directory contains declarative benchmark tasks for the Notepad adapter.
TASK-036 owns T1.1, TASK-037 owns T1.2, and TASK-038 owns T1.3.

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

## T1.3

`t1.3.new-tab-write-save-as.json` defines a persistent-write task:

1. validate an absolute target path that must not already exist;
2. create one new tab and verify that it starts empty;
3. write the requested Unicode text through the Host editor-value action,
   then read it back and compare canonical LF text;
4. show a high-risk diff and request `once` approval;
5. call the registered `notepad.file.save_as` tool, resolve the cross-process
   `#32770` Shell dialog, enter the target path, and click Save;
6. read the target back from disk and verify the title is clean.

The current adapter has no registered `notepad.file.write_text` tool. T1.3
therefore declares a Host `set_editor_value` precondition, requires an
immediate readback, and does not invent a model-visible tool.

`notepad.file.save_as` has `overwrite_existing=false` as a contract constant.
If the target exists at preflight time or appears in the race window before
Save As, the task returns `PolicyDenied`, must not call the tool, and must
leave the target byte-identical. A future automatic overwrite path requires a
separate backup, explicit confirmation, and a new tool contract.

The Save As dialog is a cross-process `#32770` child process. Its identity must
be checked against the Notepad process. Filename entry requires explicit
foreground acquisition and a Unicode keyboard path; missing capability is a
fail-closed `CapabilityMissing`, not a silent fallback.

## Open-file boundary

The current adapter tool set has no `notepad.file.open` tool. T1.1 therefore
declares `open_mode=platform_open_file` as a task precondition and records the
gap as DRIFT-036-1. This directory does not invent an unregistered tool.

## Evaluation fixtures

Each task directory has its own zero-dependency fixture generator. Generated
fixtures, including the T1.1 1 MB file and T1.3 protected targets, are created
on demand and are not committed.
