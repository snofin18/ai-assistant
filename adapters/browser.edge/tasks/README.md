# Browser Edge benchmark tasks

This directory owns declarative Edge benchmark task packages.

## T5.1

`t5.1.books-list-to-csv.json` declares the approved T5.1 extraction contract:

1. open the three deterministic loopback pages;
2. read each bounded product-list DOM fragment through
   `edge.cdp.dom_read`;
3. leave CSV normalization and serialization to the zero-dependency eval
   harness under `eval/tasks/edge/t5.1/`.

The external smoke target is `https://books.toscrape.com/`, but it is not used
for the 10-run acceptance baseline.

## Runtime integration

TASK-107 added deterministic task-package assertions for
`edge.cdp.navigate`, `edge.cdp.dom_read`, and
`edge.cdp.wait_network_idle`. The T5.1 package now renders six
executable steps without `MissingAssertion`, and its rendered postconditions are
accepted by `assistant_verify`.

This resolves the runtime-contract portion of `DRIFT-056-2`. It does not claim a
real browser run: TASK-056 remains Blocked until Edge uses the dedicated profile,
the local fixture passes 10/10, and the approved external smoke passes 3/3.

`RuntimeCdpProvider` now uses the real loopback CDP transport, and TASK-112
provides production handlers for `navigate`, `dom.read`, and
`wait_network_idle` plus an Edge/Chrome `ToolRegistry` builder. The builder is
verified through the real in-process `ToolBus` with a scripted CDP runtime.

This is still not real-browser acceptance: form fill, download, screenshot,
click, browser auto-launch wiring inside the CLI production root, and TASK-056
10/10 external smoke remain pending.
