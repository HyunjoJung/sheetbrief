# Verification

Last verified: 2026-08-30

## Deterministic baseline

The pinned workbook produces all expected values:

- XLSX, one sheet, 204 cells, 50 data rows, no parser warning, no partial parse
- Total Volume: 292,000
- Top regions: East and South, 90,000 each
- Top item: Grape, 99,000
- Top month: July, 44,000
- Source range: `Sheet1!A2:D51`

The generated `analysis.json` is byte-deterministic for repeated runs.

## Automated gates

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace tests cover aggregate oracles, unknown evidence rejection, raw
numeric narrative rejection, DOCX reopening, MCP initialization, tool listing,
workbook analysis, report delivery, and reopening the report returned through
MCP.

## Visual gate

The baseline DOCX was opened and exported by Microsoft Word for Microsoft 365,
then all three A4 pages were rasterized at 144 DPI and inspected. The current
layout has no clipped text, overlapping elements, missing Korean glyphs, split
tables, or unintended blank pages.

Generated files under `artifacts/` are local QA outputs and are intentionally
ignored by Git.

## External gate

Timely attachment handoff and downloadable DOCX delivery are not yet verified.
Passing the local official-MCP-client test does not close that product gate.
