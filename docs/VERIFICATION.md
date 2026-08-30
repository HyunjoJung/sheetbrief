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
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
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
- configured public-host acceptance with unrelated-host rejection

The current workspace run contains 15 passing tests: seven data/report tests,
four MCP unit tests, and four MCP transport integration tests.

## Dependency migration gate

The initial Dependabot queue was migrated and closed on 2026-08-30:

- `actions/checkout` 6 to 7 ([PR #3](https://github.com/HyunjoJung/sheetbrief/pull/3));
- `rand` 0.9.5 to 0.10.2, including the `RngCore` to `Rng` API migration
  ([PR #1](https://github.com/HyunjoJung/sheetbrief/pull/1));
- `sha2` 0.10.9 to 0.11.0 ([PR #2](https://github.com/HyunjoJung/sheetbrief/pull/2));
- `base64` 0.22.1 to 0.23.1 ([PR #4](https://github.com/HyunjoJung/sheetbrief/pull/4)); and
- `zip` 2.4.2 to 8.6.0 ([PR #5](https://github.com/HyunjoJung/sheetbrief/pull/5)).

Each migration passed the Rust 1.92 CI and RustSec gates before merge. The
combined `main` result then passed the same gates again, including all four
workbook-container regressions and both MCP transports.

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
- the Korean demo produced a 7,195-byte DOCX and 32,480-byte PDF;
- both capability URLs downloaded successfully and had valid ZIP/PDF signatures;
- the analyzed dataset contained 50 rows at `A3:D52`.

The runtime uses a dedicated writable XDG Fontconfig cache for UID/GID 65532;
the final smoke log contained no Fontconfig cache warnings.

## Public HTTPS transport gate

The release container was exposed through a temporary TLS tunnel and tested
from its public origin. `scripts/smoke-mcp.ps1` and `scripts/smoke-sse.ps1`
both used the public GitHub demo workbook through `file_url`, not local bytes.
The runs verified:

- Streamable HTTP protocol `2025-06-18` initialization;
- legacy SSE protocol `2024-11-05` initialization and both tool definitions;
- `analyze_workbook` followed by `build_report` over both transports;
- HTTPS workbook retrieval through the explicit host allowlist;
- public capability downloads with valid DOCX ZIP and PDF signatures; and
- identical 50-row `A3:D52`, 7,195-byte DOCX, and 32,480-byte PDF results.

The Streamable HTTP server now derives its DNS-rebinding host allowlist from the
validated public base URL while retaining rmcp's loopback defaults. A regression
test proves the configured public host is accepted and an unrelated host still
receives HTTP 403.

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

Public HTTPS verification proves both transports and the Timely-shaped URL
input, but it does not prove Timely's hosted connector mapping. Do not mark this
gate complete until a real Timely agent can:

1. initialize the deployed `/sse` endpoint and list both tools;
2. map Upload `fileUrl` and `fileName` without exposing base64 or the signed URL;
3. receive the compact analysis context without truncation;
4. return downloadable DOCX and PDF files; and
5. enforce the bearer token at the deployment edge.
