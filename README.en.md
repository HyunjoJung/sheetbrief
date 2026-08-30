# SheetBrief

**SheetBrief turns spreadsheet data into a decision-ready business brief.**

SheetBrief reads `.xls`, `.xlsx`, `.xlsb`, and `.ods` workbooks through `rxls`,
builds a deterministic fact contract with source ranges, lets Solar write only
against those evidence IDs, and produces an editable two-page DOCX plus PDF
through `rwml`.

```text
workbook bytes -> rxls -> evidence-linked facts -> validated narrative
               -> rwml DOCX/PDF
```

[한국어](README.md) | [MABC package](submission/MABC-2026.md) |
[MCP contract](docs/MCP-CONTRACT.md) | [Deployment](docs/DEPLOYMENT.md) |
[Verification](docs/VERIFICATION.md)

## Output preview

[Open the two-page PDF](docs/samples/sheetbrief-demo.pdf) |
[Download the editable DOCX](docs/samples/sheetbrief-demo.docx)

| Page 1: decision and comparisons | Page 2: flow and actions |
| --- | --- |
| ![Decision brief page one](docs/assets/report-page-1.png) | ![Decision brief page two](docs/assets/report-page-2.png) |

## Preliminary skill scope

The current contest slice accepts one sales table with region, item, volume,
and month columns (English or Korean aliases). Code computes every numeric
fact. The model supplies concise Korean observations and actions, and every
point must cite returned `fact_id` or `context_id` values. Missing targets,
comparison periods, and prices are represented explicitly instead of inferred.

## Run the public demo

```powershell
cargo run -- run .\data\meeting-sales-demo.xlsx .\artifacts\mabc-demo
```

This writes deterministic `analysis.json`, an editable `report.docx`, and a
native `report.pdf` preview. See [data/README.md](data/README.md) for provenance.

The server exposes legacy SSE at `/sse` for Timely and modern Streamable HTTP
at `/mcp` for current MCP clients. Both transports expose the same two tools.

## Verify

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT. The bundled public workbook fixture retains its BSD-2-Clause notice.
