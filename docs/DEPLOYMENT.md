# Deployment

SheetBrief MCP is stateless except for generated download files, which are held
in memory for 15 minutes. No database or persistent disk is required.

## Render free deployment

The repository includes `render.yaml` and a multi-stage `Dockerfile`.

1. Push the repository to a public Git host.
2. In Render, create a Blueprint from the repository.
3. Keep the generated `SHEETBRIEF_API_TOKEN` private and copy it into the Timely
   MCP connector's bearer-token setting.
4. Verify `https://<service>.onrender.com/healthz` returns `{"status":"ok"}`.
5. Register `https://<service>.onrender.com/mcp` in Timely Agent's HTTP MCP
   connector.

The same deployment also exposes legacy SSE at `/sse`. Use `/mcp` in the
current Timely Agent `mcpServers` JSON connector; `/sse` remains available for
older workflow clients that only implement the legacy handshake.

Render injects `RENDER_EXTERNAL_URL`; SheetBrief uses it automatically for the
capability download links. A free Render service sleeps after 15 minutes without
traffic and can take about a minute to wake. Call `/healthz` before a judged demo
and wait for a 200 response.

The default URL allowlist accepts Timely-style Azure storage URLs at
`storage.azure.com` and `*.blob.core.windows.net`. If the live Upload node
returns another host, append that exact host with
`SHEETBRIEF_ALLOWED_FILE_HOSTS`; do not use a catch-all wildcard.

## Local container verification

```powershell
docker build -t sheetbrief-mcp .
docker run --rm -p 8787:10000 `
  -e SHEETBRIEF_API_TOKEN=replace-with-at-least-32-random-characters `
  -e SHEETBRIEF_PUBLIC_BASE_URL=http://localhost:8787 `
  sheetbrief-mcp
```

Then check `http://localhost:8787/healthz`. The production public base URL must
use HTTPS; loopback HTTP is accepted only for local testing.

Run the full MCP and download smoke test with:

```powershell
.\scripts\smoke-mcp.ps1 `
  -BaseUrl http://127.0.0.1:8787 `
  -Token replace-with-at-least-32-random-characters
```

For a deployed service, exercise Timely's URL-shaped input over both transports:

```powershell
.\scripts\smoke-mcp.ps1 `
  -BaseUrl https://<service>.onrender.com `
  -Token $env:SHEETBRIEF_API_TOKEN `
  -FileUrl https://<allowed-host>/sales.xlsx `
  -FileName sales.xlsx

.\scripts\smoke-sse.ps1 `
  -BaseUrl https://<service>.onrender.com `
  -Token $env:SHEETBRIEF_API_TOKEN `
  -FileUrl https://<allowed-host>/sales.xlsx `
  -FileName sales.xlsx
```

Both scripts initialize the transport, call `analyze_workbook`, call
`build_report`, download both capability URLs, and verify DOCX/PDF signatures.

## Timely wiring

```text
Start -> File Upload -> Tool Node: analyze_workbook
      -> Agent: Solar Pro4 -> Tool Node: build_report -> End
```

Map Upload `fileUrl` to `file_url` and `fileName` to `file_name` in both tool
nodes. Map both result `download_url` values into the End response. Never put
the signed upload URL, API token, or base64 payload in the visible conversation.

In the current Timely UI, open `에이전트` and use the right panel:

1. `스킬 + -> .skill/.zip 업로드` for `artifacts/sheetbrief-timely.zip`.
2. `커넥터 + -> JSON 등록 -> http` for the deployed service:

```json
{
  "mcpServers": {
    "sheetbrief": {
      "description": "Evidence-linked spreadsheet decision briefs",
      "url": "https://<service>.onrender.com/mcp",
      "headers": {
        "Authorization": "Bearer <SHEETBRIEF_API_TOKEN>"
      }
    }
  }
}
```

Create the contest workflow from
`Labs -> 에이전트 빌더 -> 에이전트 만들기`, add the nodes shown above, and
select `Solar Pro4` in the Agent node. The corresponding controls are described
in Timely's official
[agent guide](https://timely-hub.github.io/timely-manual/user/getting-started/agents/),
[builder guide](https://timely-hub.github.io/timely-manual/user/reference/ai-agents/),
and [model guide](https://timely-hub.github.io/timely-manual/ai-models/).
