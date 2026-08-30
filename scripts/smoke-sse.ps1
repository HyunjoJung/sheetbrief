[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BaseUrl,
    [Parameter(Mandatory = $true)]
    [string]$Token,
    [Parameter(Mandatory = $true)]
    [string]$FileUrl,
    [string]$FileName
)

$ErrorActionPreference = "Stop"
$BaseUrl = $BaseUrl.TrimEnd('/')
if (-not $FileName) {
    $FileName = [IO.Path]::GetFileName(([Uri]$FileUrl).AbsolutePath)
}
if (-not $FileName) {
    throw "FileName is required when it cannot be derived from FileUrl"
}

function Read-SseEvent {
    param([IO.StreamReader]$Reader)

    $eventName = $null
    $dataLines = [Collections.Generic.List[string]]::new()
    while ($true) {
        $line = $Reader.ReadLineAsync().GetAwaiter().GetResult()
        if ($null -eq $line) {
            throw "SSE stream ended before the next event"
        }
        if ($line.Length -eq 0) {
            if ($eventName -or $dataLines.Count -gt 0) {
                return [pscustomobject]@{
                    Event = $eventName
                    Data = $dataLines -join "`n"
                }
            }
            continue
        }
        if ($line.StartsWith(':')) {
            continue
        }
        if ($line.StartsWith('event:')) {
            $eventName = $line.Substring(6).TrimStart()
        }
        elseif ($line.StartsWith('data:')) {
            $dataLines.Add($line.Substring(5).TrimStart())
        }
    }
}

function Send-McpMessage {
    param(
        [Net.Http.HttpClient]$Client,
        [Uri]$Endpoint,
        [hashtable]$Payload
    )

    $json = $Payload | ConvertTo-Json -Depth 30 -Compress
    $content = [Net.Http.StringContent]::new(
        $json,
        [Text.Encoding]::UTF8,
        "application/json"
    )
    try {
        $response = $Client.PostAsync($Endpoint, $content).GetAwaiter().GetResult()
        try {
            if ($response.StatusCode -ne [Net.HttpStatusCode]::Accepted) {
                $body = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                throw "MCP message POST failed: $($response.StatusCode) $body"
            }
        }
        finally {
            $response.Dispose()
        }
    }
    finally {
        $content.Dispose()
    }
}

function Read-McpJson {
    param([IO.StreamReader]$Reader)

    $event = Read-SseEvent -Reader $Reader
    if ($event.Event -ne "message") {
        throw "Expected an MCP message event, got '$($event.Event)'"
    }
    $event.Data | ConvertFrom-Json -Depth 40
}

$handler = [Net.Http.HttpClientHandler]::new()
$client = [Net.Http.HttpClient]::new($handler)
$client.Timeout = [TimeSpan]::FromMinutes(2)
$client.DefaultRequestHeaders.Authorization = [Net.Http.Headers.AuthenticationHeaderValue]::new(
    "Bearer",
    $Token
)
$client.DefaultRequestHeaders.Accept.ParseAdd("text/event-stream")

$sseResponse = $null
$reader = $null
try {
    $sseResponse = $client.GetAsync(
        "$BaseUrl/sse",
        [Net.Http.HttpCompletionOption]::ResponseHeadersRead
    ).GetAwaiter().GetResult()
    if (-not $sseResponse.IsSuccessStatusCode) {
        throw "SSE connection failed: $($sseResponse.StatusCode)"
    }
    $stream = $sseResponse.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
    $reader = [IO.StreamReader]::new($stream)

    $endpointEvent = Read-SseEvent -Reader $reader
    if ($endpointEvent.Event -ne "endpoint") {
        throw "Expected the endpoint event, got '$($endpointEvent.Event)'"
    }
    $endpoint = [Uri]::new([Uri]$BaseUrl, $endpointEvent.Data)

    Send-McpMessage -Client $client -Endpoint $endpoint -Payload @{
        jsonrpc = "2.0"
        id = 1
        method = "initialize"
        params = @{
            protocolVersion = "2024-11-05"
            capabilities = @{}
            clientInfo = @{ name = "sheetbrief-sse-smoke"; version = "1.0" }
        }
    }
    $initialized = Read-McpJson -Reader $reader
    if ($initialized.id -ne 1) {
        throw "Unexpected initialize response"
    }

    Send-McpMessage -Client $client -Endpoint $endpoint -Payload @{
        jsonrpc = "2.0"
        method = "notifications/initialized"
    }
    Send-McpMessage -Client $client -Endpoint $endpoint -Payload @{
        jsonrpc = "2.0"
        id = 2
        method = "tools/list"
        params = @{}
    }
    $listed = Read-McpJson -Reader $reader
    $toolNames = @($listed.result.tools | ForEach-Object name | Sort-Object)
    if (($toolNames -join ',') -ne "analyze_workbook,build_report") {
        throw "Unexpected MCP tools: $($toolNames -join ', ')"
    }

    $inputArguments = @{ file_name = $FileName; file_url = $FileUrl }
    Send-McpMessage -Client $client -Endpoint $endpoint -Payload @{
        jsonrpc = "2.0"
        id = 3
        method = "tools/call"
        params = @{ name = "analyze_workbook"; arguments = $inputArguments }
    }
    $analyzed = Read-McpJson -Reader $reader
    if ($analyzed.result.isError) {
        throw ($analyzed.result | ConvertTo-Json -Depth 30 -Compress)
    }

    $reportArguments = $inputArguments.Clone()
    $reportArguments.include_pdf = $true
    Send-McpMessage -Client $client -Endpoint $endpoint -Payload @{
        jsonrpc = "2.0"
        id = 4
        method = "tools/call"
        params = @{ name = "build_report"; arguments = $reportArguments }
    }
    $built = Read-McpJson -Reader $reader
    if ($built.result.isError) {
        throw ($built.result | ConvertTo-Json -Depth 30 -Compress)
    }

    $structured = $built.result.structuredContent
    $docx = $client.GetByteArrayAsync([Uri]$structured.report.download_url).GetAwaiter().GetResult()
    $pdf = $client.GetByteArrayAsync([Uri]$structured.preview_pdf.download_url).GetAwaiter().GetResult()
    if ($docx.Length -lt 2 -or $docx[0] -ne 0x50 -or $docx[1] -ne 0x4b) {
        throw "Downloaded DOCX does not have a ZIP signature"
    }
    if ($pdf.Length -lt 4 -or [Text.Encoding]::ASCII.GetString($pdf, 0, 4) -ne "%PDF") {
        throw "Downloaded PDF does not have a PDF signature"
    }

    [pscustomobject]@{
        status = "ok"
        transport = "sse"
        protocol_version = $initialized.result.protocolVersion
        tools = $toolNames
        source_rows = $structured.context.dataset.data_rows
        source_range = $structured.context.dataset.source_range
        docx_bytes = $docx.Length
        pdf_bytes = $pdf.Length
    } | ConvertTo-Json -Compress
}
finally {
    if ($reader) {
        $reader.Dispose()
    }
    if ($sseResponse) {
        $sseResponse.Dispose()
    }
    $client.Dispose()
    $handler.Dispose()
}
