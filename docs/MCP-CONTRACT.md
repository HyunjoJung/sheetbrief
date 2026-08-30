# SheetBrief MCP contract

## Transport

- Endpoint: `/mcp`
- Protocol: MCP Streamable HTTP using the official Rust SDK `rmcp 3.1.4`
- Responses may use Server-Sent Events as defined by Streamable HTTP.
- Health endpoint: `GET /healthz`
- Default bind: `127.0.0.1:8787`

The service refuses a non-loopback bind unless `SHEETBRIEF_API_TOKEN` is set.
When set, the token must contain at least 32 characters and MCP requests require
`Authorization: Bearer <token>`. TLS is the responsibility of the deployment
edge.

## Tools

### `analyze_workbook`

Input:

```json
{
  "file_name": "sales.xlsx",
  "workbook_base64": "..."
}
```

Returns the deterministic `Analysis` object: parser diagnostics, selected
schema, aggregates, fact IDs, source range, row counts, and missing business
context.

### `build_report`

Input:

```json
{
  "file_name": "sales.xlsx",
  "workbook_base64": "...",
  "narrative": null
}
```

`narrative` may contain Solar-authored `report_title`, `purpose`, `summary`,
`priorities`, and `actions`. Every point must cite known `evidence_ids` and may
not contain raw ASCII digits; the renderer inserts authoritative values from
the cited facts. When `narrative` is `null`, SheetBrief uses a deterministic
fallback narrative.

The result contains the same analysis plus a base64-encoded DOCX, media type,
file name, byte length, and SHA-256. The DOCX is reopened with `rwml` before the
tool returns it.

## Bounds

- Decoded workbook maximum: 10 MiB
- Data row maximum: 100,000
- Workbook input is accepted only as standard base64 in this preliminary slice.
- Remote URL fetching is intentionally absent until an allowlist and DNS/redirect
  rebinding policy are implemented.

## Timely gate

Timely publicly labels its remote MCP URL as an SSE endpoint, while the current
MCP specification and Rust SDK use Streamable HTTP. Do not mark Timely transport
complete until a real Timely agent has passed all of these checks:

1. Initialize and list both SheetBrief tools.
2. Transfer an attached workbook without copying base64 through the model text.
3. Receive `analysis` without truncation.
4. Make the generated DOCX downloadable in the Timely conversation.
5. Confirm authentication headers and observed request/response size limits.

If Timely requires the retired standalone HTTP+SSE transport rather than
Streamable HTTP, add a narrow compatibility adapter after observing the actual
handshake. Do not weaken the deterministic core or duplicate workbook logic.
