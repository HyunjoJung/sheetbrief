# Verification

Last verified: 2026-08-30

## Deterministic data oracle

The pinned libxlsxwriter workbook produces the following authoritative values:

- 50 data rows and 204 cells
- total volume 292,000
- East and South tied at 90,000
- Grape at 99,000
- July at 44,000
- source range `Sheet1!A2:D51`

`meeting-sales-demo.xlsx` keeps the same 50 numeric rows and translates the
headers and dimensions into Korean. Its derived `.xls`, `.xlsb`, and `.ods`
variants are checked against the same aggregate oracle. Repeated analysis of
the pinned source produces byte-identical pretty-printed JSON.

## Automated gates

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace tests cover:

- aggregate and source-range oracles
- Korean schema aliases and four workbook containers
- compact Solar context construction
- unknown or duplicated evidence rejection
- raw numeric narrative rejection
- DOCX generation and reopening
- native PDF signature and non-trivial payload
- official MCP client initialization, tool listing, analysis, and report delivery
- Timely-compatible legacy SSE authentication and tool invocation
- current MCP Streamable HTTP initialization and tool invocation
- public-mode capability URL generation and DOCX download

The current workspace run contains 13 passing tests: seven data/report tests,
three MCP unit tests, and three MCP transport integration tests.

`cargo audit` reports zero known vulnerabilities. It reports two informational
maintenance warnings (`rustybuzz 0.20.1` and `ttf-parser 0.25.1`) through
`rwml -> krilla 0.8.2`; neither advisory reports a vulnerability. The repository
runs a scheduled RustSec workflow and tracks upstream replacements through
Dependabot.

The Timely skill packager sorts entries and fixes ZIP timestamps. Two
consecutive builds were byte-identical at SHA-256
`5c631a66c9c490a59e7eda383aeabc29117264aefe1320d7f273b5167386e4e9`.

## Linux container gate

The multi-stage Docker image was rebuilt from the lockfile with Rust 1.92 and
run as UID/GID 65532 on Debian Bookworm. The final smoke run verified:

- `/healthz` returned `{"status":"ok"}`;
- unauthenticated `/sse` returned HTTP 401;
- authenticated `/sse` emitted the legacy MCP `endpoint` event;
- authenticated `/mcp` initialized with protocol version `2025-06-18`;
- the Korean demo produced a 6,901-byte DOCX and 32,387-byte PDF;
- both capability URLs downloaded successfully and had valid ZIP/PDF signatures;
- the analyzed dataset contained 50 rows at `A3:D52`.

## Visual gate

The Korean demo report was rendered directly with `rwml`, inspected as PDF,
and rasterized at 130 DPI. It is exactly two A4 pages. Both pages were checked
for clipped text, overlaps, missing Korean glyphs, split tables, malformed bars,
and unintended blank pages; none were observed. The first page contains the
decision and comparisons, and the second contains the monthly flow and actions.

Generated files under `artifacts/` are local QA outputs and are intentionally
ignored by Git. The visually approved copies under `docs/assets/` and
`docs/samples/` are tracked as the public output preview.

## External Timely gate

Local verification does not prove Timely interoperability. Do not mark this gate
complete until a real Timely agent can:

1. initialize the deployed `/sse` endpoint and list both tools;
2. map Upload `fileUrl` and `fileName` without exposing base64 or the signed URL;
3. receive the compact analysis context without truncation;
4. return downloadable DOCX and PDF files; and
5. enforce the bearer token at the deployment edge.
