[CmdletBinding()]
param(
    [string]$BaseUrl = "http://127.0.0.1:8787",
    [Parameter(Mandatory = $true)]
    [string]$Token,
    [string]$Workbook = (Join-Path $PSScriptRoot "..\data\meeting-sales-demo.xlsx"),
    [string]$FileUrl,
    [string]$FileName
)

$ErrorActionPreference = "Stop"
$headers = @{
    Authorization = "Bearer $Token"
    Accept = "application/json, text/event-stream"
}

function Invoke-McpRequest {
    param(
        [hashtable]$Payload,
        [string]$SessionId
    )

    $requestHeaders = $headers.Clone()
    if ($SessionId) {
        $requestHeaders["mcp-session-id"] = $SessionId
    }
    $response = Invoke-WebRequest `
        -Uri "$BaseUrl/mcp" `
        -Method Post `
        -Headers $requestHeaders `
        -ContentType "application/json" `
        -Body ($Payload | ConvertTo-Json -Depth 20 -Compress)
    $dataLine = $response.Content -split "`n" |
        Where-Object { $_ -match '^data: \{' } |
        Select-Object -Last 1
    [pscustomobject]@{
        Response = $response
        Json = if ($dataLine) {
            ($dataLine.Substring(6).Trim() | ConvertFrom-Json -Depth 30)
        }
        else {
            $null
        }
    }
}

$initialize = Invoke-McpRequest -Payload @{
    jsonrpc = "2.0"
    id = 1
    method = "initialize"
    params = @{
        protocolVersion = "2025-06-18"
        capabilities = @{}
        clientInfo = @{ name = "sheetbrief-smoke"; version = "1.0" }
    }
}
$sessionId = $initialize.Response.Headers["mcp-session-id"] | Select-Object -First 1
if (-not $sessionId) {
    throw "MCP initialization did not return a session ID"
}

[void](Invoke-McpRequest -SessionId $sessionId -Payload @{
        jsonrpc = "2.0"
        method = "notifications/initialized"
    })

$inputArguments = @{}
if ($FileUrl) {
    if (-not $FileName) {
        $FileName = [IO.Path]::GetFileName(([Uri]$FileUrl).AbsolutePath)
    }
    if (-not $FileName) {
        throw "FileName is required when it cannot be derived from FileUrl"
    }
    $inputArguments.file_name = $FileName
    $inputArguments.file_url = $FileUrl
    $sourceMode = "file_url"
}
else {
    $workbookPath = (Resolve-Path -LiteralPath $Workbook).Path
    $inputArguments.file_name = [IO.Path]::GetFileName($workbookPath)
    $inputArguments.workbook_base64 = [Convert]::ToBase64String(
        [IO.File]::ReadAllBytes($workbookPath)
    )
    $sourceMode = "workbook_base64"
}

$analysis = Invoke-McpRequest -SessionId $sessionId -Payload @{
    jsonrpc = "2.0"
    id = 2
    method = "tools/call"
    params = @{
        name = "analyze_workbook"
        arguments = $inputArguments
    }
}
if ($analysis.Json.error) {
    throw ($analysis.Json.error | ConvertTo-Json -Compress)
}
if ($analysis.Json.result.isError) {
    throw ($analysis.Json.result | ConvertTo-Json -Depth 20 -Compress)
}

$reportArguments = $inputArguments.Clone()
$reportArguments.include_pdf = $true
$result = Invoke-McpRequest -SessionId $sessionId -Payload @{
    jsonrpc = "2.0"
    id = 3
    method = "tools/call"
    params = @{
        name = "build_report"
        arguments = $reportArguments
    }
}

if ($result.Json.error) {
    throw ($result.Json.error | ConvertTo-Json -Compress)
}
if ($result.Json.result.isError) {
    throw ($result.Json.result | ConvertTo-Json -Depth 20 -Compress)
}
$structured = $result.Json.result.structuredContent
$docxUrl = $structured.report.download_url
$pdfUrl = $structured.preview_pdf.download_url
if (-not $docxUrl -or -not $pdfUrl) {
    throw "MCP response did not contain both download URLs"
}

$docxPath = Join-Path ([IO.Path]::GetTempPath()) "sheetbrief-smoke.docx"
$pdfPath = Join-Path ([IO.Path]::GetTempPath()) "sheetbrief-smoke.pdf"
try {
    Invoke-WebRequest -Uri $docxUrl -OutFile $docxPath
    Invoke-WebRequest -Uri $pdfUrl -OutFile $pdfPath
    $docx = [IO.File]::ReadAllBytes($docxPath)
    $pdf = [IO.File]::ReadAllBytes($pdfPath)
    if ($docx.Length -lt 2 -or $docx[0] -ne 0x50 -or $docx[1] -ne 0x4b) {
        throw "Downloaded DOCX does not have a ZIP signature"
    }
    if ($pdf.Length -lt 4 -or [Text.Encoding]::ASCII.GetString($pdf, 0, 4) -ne "%PDF") {
        throw "Downloaded PDF does not have a PDF signature"
    }

    [pscustomobject]@{
        status = "ok"
        session_id = $sessionId
        source_mode = $sourceMode
        docx_bytes = $docx.Length
        pdf_bytes = $pdf.Length
        source_rows = $structured.context.dataset.data_rows
        source_range = $structured.context.dataset.source_range
    } | ConvertTo-Json -Compress
}
finally {
    [IO.File]::Delete($docxPath)
    [IO.File]::Delete($pdfPath)
}
