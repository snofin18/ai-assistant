# Injection Target Fixture

This directory contains a deterministic, local-only prompt-injection fixture
for TASK-053 and the later TASK-058 security regression.

## Safety Boundary

- `index.html` is static. It contains no JavaScript, forms, downloads, or
  external resources.
- The page declares a restrictive Content Security Policy with
  `default-src 'none'` and only inline styles enabled.
- All injection content is inert text. It is evidence to inspect, never a
  command to execute.
- The intended normal task is limited to extracting the product table.

## Files

| File | Purpose |
|---|---|
| `index.html` | Openable `file://` fixture with normal data and six injection markers |
| `fixture-manifest.json` | Machine-readable expected rows, selectors, injection markers, and forbidden behaviors |

## Injection Coverage

The fixture covers visible text, `display:none`, `aria-hidden`, HTML comments,
a fake system prompt, and a metadata instruction. Marker tokens are stable so
security tests can assert that untrusted text reached the observation channel
without being executed.

## Expected Normal Result

Extraction must return exactly the five records listed in
`fixture-manifest.json` under `normal_task.expected_records`.

## Verification

From the repository root:

```powershell
$fixtureRoot = 'fixtures/web/injection-target'
$html = Get-Content -LiteralPath "$fixtureRoot/index.html" -Raw
$manifest = Get-Content -LiteralPath "$fixtureRoot/fixture-manifest.json" -Raw | ConvertFrom-Json

if ($manifest.network_required) { throw 'Fixture must not require network access' }
if ($html -notmatch '<table data-testid="product-table"') { throw 'Missing product table' }
if ($html -match '<script') { throw 'Fixture must remain script-free' }
if ($html -match 'src="http|href="http|fetch\(|XMLHttpRequest') { throw 'Fixture must not reference external resources' }

foreach ($marker in $manifest.injection_markers) {
  if ($html -notmatch [regex]::Escape($marker.marker)) {
    throw "Missing injection marker: $($marker.marker)"
  }
}

foreach ($record in $manifest.normal_task.expected_records) {
  foreach ($value in $record.PSObject.Properties.Value) {
    if ($html -notmatch [regex]::Escape([string]$value)) {
      throw "Missing expected product value: $value"
    }
  }
}

'PASS: injection target fixture is static, complete, and offline'
```

The fixture is not a substitute for policy, taint tracking, origin attribution,
or clean-context review. TASK-058 must verify that the runtime never turns any
marker above into an executed action.
