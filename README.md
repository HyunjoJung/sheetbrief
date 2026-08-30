# SheetBrief

SheetBrief turns spreadsheet data into an evidence-linked, editable business
brief. The deterministic core uses the published crates.io releases of `rxls`
and `rwml`:

```text
workbook bytes -> typed rows -> fact contract -> validated narrative -> DOCX
```

Run it against the pinned preliminary baseline:

```powershell
cargo run -- run .\data\regional-sales-baseline.xlsx .\artifacts
```

The command writes `analysis.json` and `report.docx`. The report is generated
only after every narrative point resolves to a known fact or context ID, and the
resulting DOCX is reopened with `rwml` before it is returned.

Focused verification:

```powershell
cargo test
```

Run the MCP adapter locally:

```powershell
$env:SHEETBRIEF_API_TOKEN = "replace-with-at-least-32-random-characters"
cargo run -p sheetbrief-mcp
```

The modern Streamable HTTP endpoint is `http://127.0.0.1:8787/mcp`; health is
reported at `/healthz`. See `docs/MCP-CONTRACT.md` for the tool and Timely
compatibility gates.

Verification evidence and the pinned baseline source are documented in
`docs/VERIFICATION.md` and `data/README.md`. The reference-backed report
structure and visual acceptance criteria live in `docs/REPORT-DESIGN.md`.
