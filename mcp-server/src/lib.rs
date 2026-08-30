#![forbid(unsafe_code)]

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
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
use sheetbrief::{Narrative, MAX_WORKBOOK_BYTES};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;

const MAX_BASE64_BYTES: usize = MAX_WORKBOOK_BYTES.div_ceil(3) * 4 + 4;
const DOCX_MEDIA_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

#[derive(Debug, Clone)]
pub struct SheetBriefMcp {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyzeWorkbookRequest {
    #[schemars(description = "Original workbook file name, without a directory path")]
    pub file_name: Option<String>,
    #[schemars(description = "Standard base64 encoded XLS, XLSX, XLSB, or ODS bytes")]
    pub workbook_base64: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildReportRequest {
    #[schemars(description = "Original workbook file name, without a directory path")]
    pub file_name: Option<String>,
    #[schemars(description = "Standard base64 encoded XLS, XLSX, XLSB, or ODS bytes")]
    pub workbook_base64: String,
    #[schemars(
        description = "Optional SheetBrief Narrative object authored from returned fact IDs"
    )]
    pub narrative: Option<Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BuildReportResponse {
    pub analysis: Value,
    pub report: ReportPayload,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReportPayload {
    pub file_name: String,
    pub media_type: String,
    pub byte_length: usize,
    pub sha256: String,
    pub base64: String,
}

impl SheetBriefMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
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
        description = "Parse one spreadsheet with rxls and return deterministic aggregates, fact IDs, row counts, source ranges, and parser diagnostics."
    )]
    pub async fn analyze_workbook(
        &self,
        Parameters(request): Parameters<AnalyzeWorkbookRequest>,
    ) -> Result<McpJson<Value>, String> {
        let (bytes, file_name) = decode_workbook(request.file_name, &request.workbook_base64)?;
        let analysis = tokio::task::spawn_blocking(move || {
            sheetbrief::analyze_workbook(&bytes, file_name).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("analysis worker failed: {error}"))??;
        let value = serde_json::to_value(analysis).map_err(|error| error.to_string())?;
        Ok(McpJson(value))
    }

    #[tool(
        name = "build_report",
        description = "Generate an evidence-linked editable DOCX from one spreadsheet. Optional narrative points must cite fact IDs and contain no raw numeric literals."
    )]
    pub async fn build_report(
        &self,
        Parameters(request): Parameters<BuildReportRequest>,
    ) -> Result<McpJson<BuildReportResponse>, String> {
        let (bytes, file_name) = decode_workbook(request.file_name, &request.workbook_base64)?;
        let narrative = request
            .narrative
            .map(serde_json::from_value::<Narrative>)
            .transpose()
            .map_err(|error| format!("invalid narrative object: {error}"))?;
        let (analysis, report_bytes) = tokio::task::spawn_blocking(move || {
            let analysis = sheetbrief::analyze_workbook(&bytes, file_name)
                .map_err(|error| error.to_string())?;
            let narrative = match narrative {
                Some(narrative) => narrative,
                None => {
                    sheetbrief::default_narrative(&analysis).map_err(|error| error.to_string())?
                }
            };
            let report = sheetbrief::build_report(&analysis, &narrative)
                .map_err(|error| error.to_string())?;
            Ok::<_, String>((analysis, report))
        })
        .await
        .map_err(|error| format!("report worker failed: {error}"))??;

        let analysis = serde_json::to_value(analysis).map_err(|error| error.to_string())?;
        Ok(McpJson(BuildReportResponse {
            analysis,
            report: ReportPayload {
                file_name: "sheetbrief-report.docx".to_string(),
                media_type: DOCX_MEDIA_TYPE.to_string(),
                byte_length: report_bytes.len(),
                sha256: sha256_hex(&report_bytes),
                base64: STANDARD.encode(report_bytes),
            },
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SheetBriefMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Analyze a workbook first. Give Solar only the returned bounded facts, then call build_report with fact-linked narrative or the deterministic fallback.",
        )
    }
}

pub fn build_router(cancellation_token: CancellationToken, token: Option<String>) -> Router {
    let service: StreamableHttpService<SheetBriefMcp, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(SheetBriefMcp::new()),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_sse_keep_alive(None)
                .with_cancellation_token(cancellation_token),
        );
    let mcp = Router::new().nest_service("/mcp", service);
    let mcp = match token {
        Some(token) => mcp.layer(middleware::from_fn_with_state(
            Arc::new(token),
            require_bearer,
        )),
        None => mcp,
    };
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({ "status": "ok" })) }),
        )
        .merge(mcp)
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
    if file_name.is_empty()
        || file_name.len() > 255
        || file_name.contains('/')
        || file_name.contains('\\')
    {
        return Err("file_name must be a non-empty base name of at most 255 bytes".to_string());
    }
    Ok((bytes, file_name))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
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
}
