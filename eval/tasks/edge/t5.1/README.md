# T5.1 Edge fixture and CSV contract

This directory is the zero-dependency eval harness for TASK-056 T5.1.

## Scope

The harness defines the approved local fixture, CSV schema, negative cases, and
strict validator. It does not call the product runtime and does not claim that
Edge extraction is complete.

The production task package remains blocked until the browser tool assertion
mapping is added under a separately approved write scope.

## Generate the fixture

```powershell
python eval/tasks/edge/t5.1/generate_fixtures.py
```

Generated files stay under `fixtures/` and are not committed.

## Run the offline tests

```powershell
python -m unittest discover -s eval/tasks/edge/t5.1 -p "test_*.py"
```

The tests cover:

- three pages with 20 rows each;
- UTF-8, commas, quotes, and mixed-script titles;
- RFC 4180 CSV output with a UTF-8 no-BOM header;
- exact header, ordering, types, and loopback-origin checks;
- rejection of BOM, bare LF, missing fields, bad ratings, and cross-origin URLs.

## Serve the fixture

```powershell
python eval/tasks/edge/t5.1/serve_fixture.py
```

The server binds only to `127.0.0.1:8765`, disables directory listings, and
serves the generated pages with `Cache-Control: no-store`.

## Validate an extracted CSV

```powershell
python eval/tasks/edge/t5.1/validate_csv.py `
  --input "$env:TEMP\ai-assistant\t5.1\products.csv" `
  --expected eval/tasks/edge/t5.1/fixtures/expected.csv
```

Validation fails closed. A missing file, non-CRLF newline, empty field,
unexpected header, invalid rating, duplicate URL, or non-loopback URL produces a
non-zero exit code.

## External smoke

After the product task package becomes executable, the only allowed external
targets are pages 1 through 3 under `https://books.toscrape.com/`. This site is
static, intended for scraping practice, and requires no login. External smoke
does not replace the 10-run local baseline.

The live structure was checked on 2026-10-22: all three pages returned HTTP 200
with 20 `article.product_pod` rows each. The first row resolved to
`A Light in the Attic`, `£51.77`, `In stock`, rating class `Three`, and relative
URL `a-light-in-the-attic_1000/index.html`. The locked selector and normalization
contract is `external-smoke-contract.json`.
