# Browser Edge benchmark tasks

This directory owns declarative Edge benchmark task packages.

## T5.1

`t5.1.books-list-to-csv.json` declares the approved T5.1 extraction contract:

1. open the three deterministic loopback pages;
2. read each bounded product-list DOM fragment through
   `browser.edge.cdp.dom.read`;
3. leave CSV normalization and serialization to the zero-dependency eval
   harness under `eval/tasks/edge/t5.1/`.

The external smoke target is `https://books.toscrape.com/`, but it is not used
for the 10-run acceptance baseline.

## Current execution blocker

The browser tools are declared, but the task-package assertion table in
`apps/agent-core/src/task_package_render.rs` does not yet map
`browser.edge.cdp.navigate` or `browser.edge.cdp.dom.read`. Loading this task
through `TaskPackageProvider` therefore fails closed with `MissingAssertion`.

Adding that mapping changes runtime behavior outside TASK-056's current write
scope. The package is committed as a reviewable WIP contract, not as a task
that can honestly be reported as executable. The blocker is recorded as
`DRIFT-056-2`.
