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

## Runtime integration

TASK-107 added deterministic task-package assertions for
`browser.edge.cdp.navigate`, `browser.edge.cdp.dom.read`, and
`browser.edge.cdp.wait_network_idle`. The T5.1 package now renders six
executable steps without `MissingAssertion`, and its rendered postconditions are
accepted by `assistant_verify`.

This resolves the runtime-contract portion of `DRIFT-056-2`. It does not claim a
real browser run: TASK-056 remains Blocked until Edge uses the dedicated profile,
the local fixture passes 10/10, and the approved external smoke passes 3/3.

Package rendering is still not a production execution path. The current
`RuntimeCdpProvider` returns synthetic `about:blank` / empty DOM results, and no
production browser-tool handlers are wired into the agent assembly. This is
recorded as `DRIFT-056-3`; real Edge cannot be claimed until those runtime
pieces are implemented under a separate card.
