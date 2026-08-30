# SheetBrief MCP contract

## Transport

- Current Timely Agent endpoint: `/mcp` with MCP Streamable HTTP
- Legacy endpoint: `/sse` with SSE message posting at `/message`
- Implementation: official Rust SDK `rmcp 3.1.4` for Streamable HTTP and a
  bounded compatibility adapter for Timely's legacy SSE handshake
- Health endpoint: `GET /healthz`
- Default bind: `127.0.0.1:8787`

The service refuses a non-loopback bind unless `SHEETBRIEF_API_TOKEN` is set.
The token must contain at least 32 characters and MCP requests require
`Authorization: Bearer <token>`. TLS belongs at the deployment edge.

## Tools

### `analyze_workbook`

Input:

```json
{
  "file_name": "sales.xlsx",
  "file_url": "https://storage.example/signed/sales.xlsx"
}
```

Returns a compact `AgentContext`, not the full parser dump. It contains the
input identity, selected sheet and range, parser state, signal fact IDs, the
complete bounded fact catalog, missing-context IDs, and the narrative contract
Solar must follow.

### `build_report`

Input:

```json
{
  "file_name": "sales.xlsx",
  "file_url": "https://storage.example/signed/sales.xlsx",
  "narrative": {
    "report_title": "판매 실적 의사결정 브리프",
    "purpose": "회의에서 우선순위와 다음 행동을 정합니다.",
    "summary": [
      {"text": "선두 지역과 최저 지역의 격차를 먼저 봅니다.", "evidence_ids": ["..."]}
    ],
    "priorities": [
      {"text": "최저 지역의 채널 구성을 확인합니다.", "evidence_ids": ["..."]}
    ],
    "actions": [
      {"text": "회복 과제의 담당자와 기한을 지정합니다.", "evidence_ids": ["..."]}
    ]
  },
  "include_pdf": true
}
```

Every narrative point must cite known `evidence_ids` and cannot contain raw
ASCII digits. The renderer inserts authoritative values from the cited facts.
When `narrative` is `null`, SheetBrief uses its deterministic fallback.

The response contains:

- `context`: the same compact `AgentContext`
- `report`: DOCX payload metadata and a capability download URL
- `preview_pdf`: the equivalent PDF payload when `include_pdf` is true

The DOCX is reopened with `rwml` before return. PDF rendering uses the same
`rwml` document model and bundled fonts. When `SHEETBRIEF_PUBLIC_BASE_URL` is
set, generated files are held in memory for 15 minutes and returned through
unguessable `/downloads/...` URLs. Local mode without a public base URL falls
back to base64 for test and CLI clients.

## Bounds

- Supported containers: `.xls`, `.xlsx`, `.xlsm`, `.xlsb`, `.ods`
- Decoded workbook maximum: 10 MiB
- Data row maximum: 100,000
- Input transport: exactly one of `file_url` or standard `workbook_base64`
- `file_url`: HTTPS port 443, no credentials, no redirects, streamed size limit
- Default remote hosts: `storage.azure.com`, `*.blob.core.windows.net`, and
  `raw.githubusercontent.com` for the pinned public demo
- Additional exact hosts or suffixes: `SHEETBRIEF_ALLOWED_FILE_HOSTS`
- DNS results are checked for public addresses and pinned into the HTTP client

## Timely gate

Timely's current Agent UI accepts a Claude-standard `mcpServers` JSON object.
Its HTTP template contains a server URL and request headers, so SheetBrief uses
the deployed `/mcp` endpoint with an `Authorization: Bearer ...` header. The
published legacy SDK returns Upload-node objects with `fileUrl`, `fileName`,
and `fileType`; its
[workflow executor](https://github.com/timely-hub/timely-gpt-sdk/blob/e0fb8e394986438a330e6536d25526b490a20793/src/workflow/workflow-executor.ts#L294-L320)
invokes remote MCP nodes when their transport is `sse`. SheetBrief retains that
compatibility endpoint and maps the first two Upload fields directly to both
tools.

The hosted connector integration ran successfully on 2026-08-30 against
`https://sheetbrief-mcp.onrender.com/mcp`. Solar Pro4 invoked both tools with the
pinned public workbook, received the complete compact context, and returned
working DOCX and PDF capability URLs. Independent public smoke tests cover
base64 and URL input, both transports, bearer authentication, host filtering,
size limits, and download signatures.

The Timely direct Agent attachment surface currently exposes a workspace-local
`file://` path. SheetBrief rejects it because a remote server cannot read
Timely's workspace. General private-file runs therefore use the builder's
Upload-node `fileUrl` mapping; the direct Agent contest demo uses the pinned
public HTTPS workbook.
