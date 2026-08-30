#![forbid(unsafe_code)]

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{sse::Event, sse::KeepAlive, sse::Sse, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::Rng;
use reqwest::{redirect::Policy, Url};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    Json as McpJson, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sheetbrief::{build_agent_context, Narrative, MAX_WORKBOOK_BYTES};
use std::{
    collections::HashMap,
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

const MAX_BASE64_BYTES: usize = MAX_WORKBOOK_BYTES.div_ceil(3) * 4 + 4;
const DOCX_MEDIA_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const DOWNLOAD_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_DOWNLOADS: usize = 64;
const LEGACY_SESSION_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_LEGACY_SESSIONS: usize = 64;
const LEGACY_EVENT_BUFFER: usize = 32;
const MAX_MCP_BODY_BYTES: usize = MAX_BASE64_BYTES + 64 * 1024;
const DEFAULT_ALLOWED_FILE_HOSTS: &[&str] = &["storage.azure.com", ".blob.core.windows.net"];

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub public_base_url: Option<String>,
    pub allowed_file_hosts: Vec<String>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("SHEETBRIEF_PUBLIC_BASE_URL")
            .or_else(|_| std::env::var("RENDER_EXTERNAL_URL"))
        {
            config.public_base_url = Some(normalize_public_base_url(&value)?);
        }
        if let Ok(value) = std::env::var("SHEETBRIEF_ALLOWED_FILE_HOSTS") {
            config.allowed_file_hosts.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(normalize_host_pattern),
            );
            config.allowed_file_hosts.sort();
            config.allowed_file_hosts.dedup();
        }
        Ok(config)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            public_base_url: None,
            allowed_file_hosts: DEFAULT_ALLOWED_FILE_HOSTS
                .iter()
                .map(|host| (*host).to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct StoredFile {
    file_name: String,
    media_type: String,
    bytes: Arc<Vec<u8>>,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct LegacySession {
    sender: mpsc::Sender<Result<Event, Infallible>>,
    expires_at: Instant,
}

#[derive(Debug)]
struct AppState {
    config: ServerConfig,
    downloads: Mutex<HashMap<String, StoredFile>>,
    legacy_sessions: Mutex<HashMap<String, LegacySession>>,
}

impl AppState {
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            downloads: Mutex::new(HashMap::new()),
            legacy_sessions: Mutex::new(HashMap::new()),
        }
    }

    fn report_payload(&self, file_name: &str, media_type: &str, bytes: Vec<u8>) -> ReportPayload {
        let byte_length = bytes.len();
        let sha256 = sha256_hex(&bytes);
        let Some(base_url) = self.config.public_base_url.as_deref() else {
            return ReportPayload {
                file_name: file_name.to_string(),
                media_type: media_type.to_string(),
                byte_length,
                sha256,
                download_url: None,
                base64: Some(STANDARD.encode(bytes)),
            };
        };

        let token = random_token();
        let download_url = format!("{base_url}/downloads/{token}/{file_name}");
        if let Ok(mut downloads) = self.downloads.lock() {
            let now = Instant::now();
            downloads.retain(|_, file| file.expires_at > now);
            if downloads.len() >= MAX_DOWNLOADS {
                if let Some(oldest) = downloads
                    .iter()
                    .min_by_key(|(_, file)| file.expires_at)
                    .map(|(token, _)| token.clone())
                {
                    downloads.remove(&oldest);
                }
            }
            downloads.insert(
                token,
                StoredFile {
                    file_name: file_name.to_string(),
                    media_type: media_type.to_string(),
                    bytes: Arc::new(bytes),
                    expires_at: now + DOWNLOAD_TTL,
                },
            );
            ReportPayload {
                file_name: file_name.to_string(),
                media_type: media_type.to_string(),
                byte_length,
                sha256,
                download_url: Some(download_url),
                base64: None,
            }
        } else {
            ReportPayload {
                file_name: file_name.to_string(),
                media_type: media_type.to_string(),
                byte_length,
                sha256,
                download_url: None,
                base64: Some(STANDARD.encode(bytes)),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SheetBriefMcp {
    tool_router: ToolRouter<Self>,
    state: Arc<AppState>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeWorkbookRequest {
    #[serde(alias = "fileName")]
    #[schemars(description = "Original workbook file name, without a directory path")]
    pub file_name: Option<String>,
    #[schemars(
        description = "Standard base64 encoded workbook bytes; use exactly one input source"
    )]
    pub workbook_base64: Option<String>,
    #[serde(alias = "fileUrl")]
    #[schemars(
        description = "HTTPS URL returned by a Timely Upload node; use exactly one input source"
    )]
    pub file_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildReportRequest {
    #[serde(alias = "fileName")]
    #[schemars(description = "Original workbook file name, without a directory path")]
    pub file_name: Option<String>,
    #[schemars(
        description = "Standard base64 encoded workbook bytes; use exactly one input source"
    )]
    pub workbook_base64: Option<String>,
    #[serde(alias = "fileUrl")]
    #[schemars(
        description = "HTTPS URL returned by a Timely Upload node; use exactly one input source"
    )]
    pub file_url: Option<String>,
    #[schemars(
        description = "Optional SheetBrief Narrative object authored from returned fact IDs"
    )]
    pub narrative: Option<Value>,
    #[schemars(description = "Also return an rwml-rendered PDF preview")]
    pub include_pdf: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AnalyzeWorkbookResponse {
    pub context: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BuildReportResponse {
    pub context: Value,
    pub report: ReportPayload,
    pub preview_pdf: Option<ReportPayload>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReportPayload {
    pub file_name: String,
    pub media_type: String,
    pub byte_length: usize,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
}

impl SheetBriefMcp {
    fn with_state(state: Arc<AppState>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            state,
        }
    }

    pub fn new() -> Self {
        Self::with_state(Arc::new(AppState::new(ServerConfig::default())))
    }
}

impl Default for SheetBriefMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl SheetBriefMcp {
    #[tool(
        name = "analyze_workbook",
        description = "Parse one spreadsheet from base64 or a Timely Upload fileUrl with rxls and return compact deterministic facts, evidence IDs, source ranges, and decision signals."
    )]
    pub async fn analyze_workbook(
        &self,
        Parameters(request): Parameters<AnalyzeWorkbookRequest>,
    ) -> Result<McpJson<AnalyzeWorkbookResponse>, String> {
        let (bytes, file_name) = load_workbook(
            &self.state.config,
            request.file_name,
            request.workbook_base64,
            request.file_url,
        )
        .await?;
        let analysis = tokio::task::spawn_blocking(move || {
            sheetbrief::analyze_workbook(&bytes, file_name).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("analysis worker failed: {error}"))??;
        let context = serde_json::to_value(build_agent_context(&analysis))
            .map_err(|error| error.to_string())?;
        Ok(McpJson(AnalyzeWorkbookResponse { context }))
    }

    #[tool(
        name = "build_report",
        description = "Generate an evidence-linked editable DOCX and optional PDF from base64 or a Timely Upload fileUrl. Narrative points must cite returned evidence IDs and contain no raw numeric literals."
    )]
    pub async fn build_report(
        &self,
        Parameters(request): Parameters<BuildReportRequest>,
    ) -> Result<McpJson<BuildReportResponse>, String> {
        let (bytes, file_name) = load_workbook(
            &self.state.config,
            request.file_name,
            request.workbook_base64,
            request.file_url,
        )
        .await?;
        let narrative = request
            .narrative
            .map(serde_json::from_value::<Narrative>)
            .transpose()
            .map_err(|error| format!("invalid narrative object: {error}"))?;
        let include_pdf = request.include_pdf.unwrap_or(false);
        let (analysis, report_bytes, pdf_bytes) = tokio::task::spawn_blocking(move || {
            let analysis = sheetbrief::analyze_workbook(&bytes, file_name)
                .map_err(|error| error.to_string())?;
            let narrative = match narrative {
                Some(narrative) => narrative,
                None => {
                    sheetbrief::default_narrative(&analysis).map_err(|error| error.to_string())?
                }
            };
            if include_pdf {
                let bundle = sheetbrief::build_report_bundle(&analysis, &narrative)
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((analysis, bundle.docx, Some(bundle.pdf)))
            } else {
                let report = sheetbrief::build_report(&analysis, &narrative)
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((analysis, report, None))
            }
        })
        .await
        .map_err(|error| format!("report worker failed: {error}"))??;

        let context = serde_json::to_value(build_agent_context(&analysis))
            .map_err(|error| error.to_string())?;
        Ok(McpJson(BuildReportResponse {
            context,
            report: self.state.report_payload(
                "sheetbrief-report.docx",
                DOCX_MEDIA_TYPE,
                report_bytes,
            ),
            preview_pdf: pdf_bytes.map(|bytes| {
                self.state
                    .report_payload("sheetbrief-report.pdf", "application/pdf", bytes)
            }),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SheetBriefMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Pass the Timely Upload node's fileUrl and fileName to analyze_workbook. Use only returned facts, then call build_report with fact-linked Korean narrative and include_pdf=true. Return download_url values when present.",
        )
    }
}

pub fn build_router(cancellation_token: CancellationToken, token: Option<String>) -> Router {
    build_router_with_config(cancellation_token, token, ServerConfig::default())
}

pub fn build_router_with_config(
    cancellation_token: CancellationToken,
    token: Option<String>,
    config: ServerConfig,
) -> Router {
    let allowed_mcp_hosts = streamable_http_allowed_hosts(&config);
    let state = Arc::new(AppState::new(config));
    let service_state = state.clone();
    let service: StreamableHttpService<SheetBriefMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(SheetBriefMcp::with_state(service_state.clone())),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_allowed_hosts(allowed_mcp_hosts)
                .with_sse_keep_alive(None)
                .with_cancellation_token(cancellation_token),
        );
    let transports = Router::new()
        .nest_service("/mcp", service)
        .route("/sse", get(legacy_sse))
        .route("/message", post(legacy_message))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_MCP_BODY_BYTES));
    let transports = match token {
        Some(token) => transports.layer(middleware::from_fn_with_state(
            Arc::new(token),
            require_bearer,
        )),
        None => transports,
    };
    let downloads = Router::new()
        .route("/downloads/{token}/{file_name}", get(download_report))
        .with_state(state);
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({ "status": "ok" })) }),
        )
        .merge(downloads)
        .merge(transports)
}

async fn require_bearer(
    State(expected): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Response {
    let actual = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let authorized = actual.is_some_and(|actual| {
        actual.len() == expected.len() && bool::from(actual.as_bytes().ct_eq(expected.as_bytes()))
    });
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized" })),
        )
            .into_response();
    }
    next.run(request).await
}

#[derive(Debug, Deserialize)]
struct LegacySessionQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn legacy_sse(State(state): State<Arc<AppState>>) -> Response {
    let session_id = random_token();
    let (sender, receiver) = mpsc::channel(LEGACY_EVENT_BUFFER);
    let endpoint = Event::default()
        .event("endpoint")
        .data(format!("/message?sessionId={session_id}"));
    if sender.try_send(Ok(endpoint)).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let now = Instant::now();
    let Ok(mut sessions) = state.legacy_sessions.lock() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    sessions.retain(|_, session| session.expires_at > now && !session.sender.is_closed());
    if sessions.len() >= MAX_LEGACY_SESSIONS {
        if let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.expires_at)
            .map(|(session_id, _)| session_id.clone())
        {
            sessions.remove(&oldest);
        }
    }
    sessions.insert(
        session_id,
        LegacySession {
            sender,
            expires_at: now + LEGACY_SESSION_TTL,
        },
    );
    drop(sessions);

    Sse::new(ReceiverStream::new(receiver))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

async fn legacy_message(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LegacySessionQuery>,
    Json(message): Json<Value>,
) -> Response {
    let sender = {
        let now = Instant::now();
        let Ok(mut sessions) = state.legacy_sessions.lock() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        sessions.retain(|_, session| session.expires_at > now && !session.sender.is_closed());
        let Some(session) = sessions.get_mut(&query.session_id) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        session.expires_at = now + LEGACY_SESSION_TTL;
        session.sender.clone()
    };

    if let Some(response) = legacy_rpc_response(state.clone(), message).await {
        let event = Event::default().event("message").data(response.to_string());
        if sender.send(Ok(event)).await.is_err() {
            if let Ok(mut sessions) = state.legacy_sessions.lock() {
                sessions.remove(&query.session_id);
            }
            return StatusCode::GONE.into_response();
        }
    }
    StatusCode::ACCEPTED.into_response()
}

async fn legacy_rpc_response(state: Arc<AppState>, message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str);
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
        return id.map(|id| legacy_rpc_error(id, -32600, "invalid JSON-RPC request"));
    }
    let method = method.unwrap_or_default();
    let id = id?;

    let result = match method {
        "initialize" => {
            let requested = message
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05");
            let protocol_version = match requested {
                "2024-11-05" | "2025-03-26" | "2025-06-18" => requested,
                _ => "2024-11-05",
            };
            json!({
                "protocolVersion": protocol_version,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "sheetbrief-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "Pass Timely fileUrl and fileName to analyze_workbook, author fact-linked Korean narrative, then call build_report with include_pdf=true."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": legacy_tools() }),
        "tools/call" => legacy_call_tool(state, message.get("params")).await,
        _ => return Some(legacy_rpc_error(id, -32601, "method not found")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn legacy_tools() -> Vec<Value> {
    let analyze_schema = serde_json::to_value(schemars::schema_for!(AnalyzeWorkbookRequest))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let report_schema = serde_json::to_value(schemars::schema_for!(BuildReportRequest))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    vec![
        json!({
            "name": "analyze_workbook",
            "description": "Parse one spreadsheet from base64 or a Timely Upload fileUrl with rxls and return compact deterministic facts, evidence IDs, source ranges, and decision signals.",
            "inputSchema": analyze_schema
        }),
        json!({
            "name": "build_report",
            "description": "Generate an evidence-linked editable DOCX and optional PDF from base64 or a Timely Upload fileUrl. Narrative points must cite returned evidence IDs and contain no raw numeric literals.",
            "inputSchema": report_schema
        }),
    ]
}

async fn legacy_call_tool(state: Arc<AppState>, params: Option<&Value>) -> Value {
    let name = params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    let arguments = params
        .and_then(|params| params.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let service = SheetBriefMcp::with_state(state);
    let output = match name {
        Some("analyze_workbook") => match serde_json::from_value(arguments) {
            Ok(request) => service
                .analyze_workbook(Parameters(request))
                .await
                .map(|McpJson(response)| response),
            Err(error) => Err(format!("invalid analyze_workbook input: {error}")),
        }
        .and_then(|response| serde_json::to_value(response).map_err(|error| error.to_string())),
        Some("build_report") => match serde_json::from_value(arguments) {
            Ok(request) => service
                .build_report(Parameters(request))
                .await
                .map(|McpJson(response)| response),
            Err(error) => Err(format!("invalid build_report input: {error}")),
        }
        .and_then(|response| serde_json::to_value(response).map_err(|error| error.to_string())),
        Some(other) => Err(format!("unknown tool {other:?}")),
        None => Err("tools/call params must contain a tool name".to_string()),
    };

    match output {
        Ok(structured) => {
            let text = serde_json::to_string(&structured)
                .unwrap_or_else(|_| "SheetBrief tool completed".to_string());
            json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": structured,
                "isError": false
            })
        }
        Err(error) => json!({
            "content": [{ "type": "text", "text": error }],
            "isError": true
        }),
    }
}

fn legacy_rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

async fn download_report(
    State(state): State<Arc<AppState>>,
    Path((token, file_name)): Path<(String, String)>,
) -> Response {
    let file = state.downloads.lock().ok().and_then(|mut downloads| {
        let now = Instant::now();
        downloads.retain(|_, file| file.expires_at > now);
        downloads
            .get(&token)
            .filter(|file| file.file_name == file_name)
            .cloned()
    });
    let Some(file) = file else {
        return StatusCode::NOT_FOUND.into_response();
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, file.media_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.file_name),
        )
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from((*file.bytes).clone()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn load_workbook(
    config: &ServerConfig,
    file_name: Option<String>,
    workbook_base64: Option<String>,
    file_url: Option<String>,
) -> Result<(Vec<u8>, String), String> {
    match (workbook_base64, file_url) {
        (Some(encoded), None) => decode_workbook(file_name, &encoded),
        (None, Some(url)) => download_workbook(config, file_name, &url).await,
        (Some(_), Some(_)) => Err("provide exactly one of workbook_base64 or file_url".to_string()),
        (None, None) => Err("provide exactly one of workbook_base64 or file_url".to_string()),
    }
}

async fn download_workbook(
    config: &ServerConfig,
    file_name: Option<String>,
    raw_url: &str,
) -> Result<(Vec<u8>, String), String> {
    let url = Url::parse(raw_url).map_err(|error| format!("file_url is invalid: {error}"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("file_url must be an HTTPS URL without embedded credentials".to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "file_url must contain a host".to_string())?
        .to_ascii_lowercase();
    if !host_allowed(&host, &config.allowed_file_hosts) {
        return Err(format!(
            "file_url host {host:?} is not allowed; configure SHEETBRIEF_ALLOWED_FILE_HOSTS"
        ));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    if port != 443 {
        return Err("file_url must use the standard HTTPS port".to_string());
    }

    let address = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|error| format!("file_url host resolution failed: {error}"))?
        .find(|address| is_public_ip(address.ip()))
        .ok_or_else(|| "file_url host did not resolve to a public address".to_string())?;
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .resolve(&host, address)
        .build()
        .map_err(|error| format!("file download client failed: {error}"))?;
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| format!("file_url download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "file_url returned HTTP {}; redirects are not followed",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WORKBOOK_BYTES as u64)
    {
        return Err(format!(
            "remote workbook exceeds the {} byte input limit",
            MAX_WORKBOOK_BYTES
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("file_url body failed: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_WORKBOOK_BYTES {
            return Err(format!(
                "remote workbook exceeds the {} byte input limit",
                MAX_WORKBOOK_BYTES
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("file_url returned an empty file".to_string());
    }

    let file_name = file_name
        .or_else(|| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "workbook.xlsx".to_string());
    validate_file_name(&file_name)?;
    Ok((bytes, file_name))
}

fn decode_workbook(file_name: Option<String>, encoded: &str) -> Result<(Vec<u8>, String), String> {
    if encoded.len() > MAX_BASE64_BYTES {
        return Err(format!(
            "encoded workbook exceeds the {} byte input limit",
            MAX_WORKBOOK_BYTES
        ));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| format!("workbook_base64 is invalid: {error}"))?;
    if bytes.len() > MAX_WORKBOOK_BYTES {
        return Err(format!(
            "decoded workbook is {} bytes; limit is {} bytes",
            bytes.len(),
            MAX_WORKBOOK_BYTES
        ));
    }
    let file_name = file_name.unwrap_or_else(|| "workbook.xlsx".to_string());
    validate_file_name(&file_name)?;
    Ok((bytes, file_name))
}

fn validate_file_name(file_name: &str) -> Result<(), String> {
    if file_name.is_empty()
        || file_name.len() > 255
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.chars().any(char::is_control)
    {
        return Err(
            "file_name must be a non-empty base name of at most 255 bytes without control characters"
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_public_base_url(value: &str) -> Result<String, String> {
    let url = Url::parse(value)
        .map_err(|error| format!("SHEETBRIEF_PUBLIC_BASE_URL is invalid: {error}"))?;
    let loopback_http = url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        });
    if url.scheme() != "https" && !loopback_http {
        return Err("SHEETBRIEF_PUBLIC_BASE_URL must use HTTPS (or loopback HTTP)".to_string());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "SHEETBRIEF_PUBLIC_BASE_URL must be an origin without credentials, path, query, or fragment"
                .to_string(),
        );
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn streamable_http_allowed_hosts(config: &ServerConfig) -> Vec<String> {
    let mut hosts = StreamableHttpServerConfig::default().allowed_hosts;
    if let Some(public_base_url) = config.public_base_url.as_deref() {
        if let Ok(url) = Url::parse(public_base_url) {
            if let Some(host) = url.host_str() {
                hosts.push(host.to_ascii_lowercase());
            }
        }
    }
    hosts.sort();
    hosts.dedup();
    hosts
}

fn normalize_host_pattern(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('*')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn host_allowed(host: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|pattern| {
        host.eq_ignore_ascii_case(pattern)
            || (pattern.starts_with('.') && host.len() > pattern.len() && host.ends_with(pattern))
    })
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || octets[0] >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(ipv4);
    }
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex_lower(&bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_and_oversized_base64_before_decoding() {
        assert!(decode_workbook(Some("../sales.xlsx".to_string()), "AA==").is_err());
        assert!(decode_workbook(None, &"A".repeat(MAX_BASE64_BYTES + 1)).is_err());
    }

    #[test]
    fn file_host_allowlist_and_ip_filter_reject_ssrf_targets() {
        let config = ServerConfig::default();
        assert!(host_allowed(
            "account.blob.core.windows.net",
            &config.allowed_file_hosts
        ));
        assert!(host_allowed(
            "storage.azure.com",
            &config.allowed_file_hosts
        ));
        assert!(!host_allowed(
            "blob.core.windows.net.evil.test",
            &config.allowed_file_hosts
        ));
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_public_ip("10.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn public_base_url_requires_a_clean_https_origin() {
        assert_eq!(
            normalize_public_base_url("https://sheetbrief.example/").unwrap(),
            "https://sheetbrief.example"
        );
        assert!(normalize_public_base_url("http://sheetbrief.example").is_err());
        assert!(normalize_public_base_url("https://sheetbrief.example/path").is_err());
    }

    #[test]
    fn public_base_url_extends_streamable_http_host_allowlist() {
        let config = ServerConfig {
            public_base_url: Some("https://SheetBrief.Example".to_string()),
            ..ServerConfig::default()
        };
        let hosts = streamable_http_allowed_hosts(&config);
        assert!(hosts.iter().any(|host| host == "sheetbrief.example"));
        assert!(hosts.iter().any(|host| host == "127.0.0.1"));
        assert!(!hosts.iter().any(|host| host == "evil.example"));
    }
}
