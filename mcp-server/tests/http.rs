use base64::{engine::general_purpose::STANDARD, Engine as _};
use rmcp::{
    model::{CallToolRequestParams, ClientInfo},
    transport::StreamableHttpClientTransport,
    ServiceExt,
};
use serde_json::{json, Map, Value};
use sheetbrief_mcp::{build_router, build_router_with_config, ServerConfig};
use tokio_util::sync::CancellationToken;

const BASELINE: &[u8] = include_bytes!("../../data/regional-sales-baseline.xlsx");

#[tokio::test]
async fn streamable_http_lists_tools_and_builds_a_reopenable_docx() {
    let cancellation_token = CancellationToken::new();
    let app = build_router(cancellation_token.child_token(), None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    cancellation_token.cancelled_owned().await;
                })
                .await
                .unwrap();
        }
    });

    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = ClientInfo::default().serve(transport).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let mut names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["analyze_workbook", "build_report"]);

    let input = json!({
        "file_name": "regional-sales-baseline.xlsx",
        "workbook_base64": STANDARD.encode(BASELINE),
    });
    let result = client
        .call_tool(
            CallToolRequestParams::new("analyze_workbook")
                .with_arguments(as_arguments(input.clone())),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let analysis = result.structured_content.unwrap();
    assert_eq!(analysis["context"]["signals"]["total"], "total.volume");
    assert_eq!(analysis["context"]["dataset"]["data_rows"], 50);

    let mut report_input = input;
    report_input["include_pdf"] = Value::Bool(true);
    let result = client
        .call_tool(
            CallToolRequestParams::new("build_report").with_arguments(as_arguments(report_input)),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let response = result.structured_content.unwrap();
    let report = STANDARD
        .decode(response["report"]["base64"].as_str().unwrap())
        .unwrap();
    assert_eq!(response["report"]["byte_length"], report.len());
    assert!(report.starts_with(b"PK"));
    let document = rwml::Document::open(&report).unwrap();
    assert!(document.to_markdown().contains("292,000"));
    let pdf = STANDARD
        .decode(response["preview_pdf"]["base64"].as_str().unwrap())
        .unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    let _ = client.cancel().await;
    cancellation_token.cancel();
    server.await.unwrap();
}

#[tokio::test]
async fn legacy_sse_transport_enforces_auth_and_calls_tools_for_timely() {
    let cancellation_token = CancellationToken::new();
    let token = "timely-test-token-0123456789abcdef";
    let app = build_router(cancellation_token.child_token(), Some(token.to_string()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    cancellation_token.cancelled_owned().await;
                })
                .await
                .unwrap();
        }
    });

    let sse_url = format!("http://{address}/sse");
    let unauthorized = reqwest::get(&sse_url).await.unwrap();
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    let http_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let response = http_client.get(&sse_url).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let mut events = LegacySseReader::new(response);
    let endpoint_event = events.next_event().await;
    assert!(endpoint_event.contains("event: endpoint"));
    let endpoint = event_data(&endpoint_event);
    assert!(endpoint.starts_with("/message?sessionId="));

    post_legacy(
        &http_client,
        address,
        &endpoint,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "timely-test", "version": "1.0" }
            }
        }),
    )
    .await;
    let initialized = events.next_json().await;
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], "2024-11-05");

    post_legacy(
        &http_client,
        address,
        &endpoint,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    post_legacy(
        &http_client,
        address,
        &endpoint,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    )
    .await;
    let listed = events.next_json().await;
    let mut names = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["analyze_workbook", "build_report"]);

    post_legacy(
        &http_client,
        address,
        &endpoint,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "analyze_workbook",
                "arguments": {
                    "file_name": "regional-sales-baseline.xlsx",
                    "workbook_base64": STANDARD.encode(BASELINE)
                }
            }
        }),
    )
    .await;
    let analyzed = events.next_json().await;
    assert_eq!(
        analyzed["result"]["structuredContent"]["context"]["dataset"]["data_rows"],
        50
    );
    assert_eq!(analyzed["result"]["isError"], false);

    post_legacy(
        &http_client,
        address,
        &endpoint,
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "build_report",
                "arguments": {
                    "file_name": "regional-sales-baseline.xlsx",
                    "workbook_base64": STANDARD.encode(BASELINE),
                    "include_pdf": true
                }
            }
        }),
    )
    .await;
    let built = events.next_json().await;
    assert_eq!(built["result"]["isError"], false);
    let structured = &built["result"]["structuredContent"];
    let docx = STANDARD
        .decode(structured["report"]["base64"].as_str().unwrap())
        .unwrap();
    let pdf = STANDARD
        .decode(structured["preview_pdf"]["base64"].as_str().unwrap())
        .unwrap();
    assert!(docx.starts_with(b"PK"));
    assert!(rwml::Document::open(&docx).is_ok());
    assert!(pdf.starts_with(b"%PDF"));

    drop(events);
    cancellation_token.cancel();
    server.await.unwrap();
}

#[tokio::test]
async fn public_server_returns_a_capability_download_url_instead_of_base64() {
    let cancellation_token = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_router_with_config(
        cancellation_token.child_token(),
        None,
        ServerConfig {
            public_base_url: Some(format!("http://{address}")),
            ..ServerConfig::default()
        },
    );
    let server = tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    cancellation_token.cancelled_owned().await;
                })
                .await
                .unwrap();
        }
    });

    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = ClientInfo::default().serve(transport).await.unwrap();
    let input = json!({
        "file_name": "regional-sales-baseline.xlsx",
        "workbook_base64": STANDARD.encode(BASELINE),
        "include_pdf": false,
    });
    let result = client
        .call_tool(CallToolRequestParams::new("build_report").with_arguments(as_arguments(input)))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let response = result.structured_content.unwrap();
    assert!(response["report"].get("base64").is_none());
    let download_url = response["report"]["download_url"].as_str().unwrap();
    let download = reqwest::get(download_url).await.unwrap();
    assert_eq!(download.status(), reqwest::StatusCode::OK);
    assert_eq!(
        download.headers()[reqwest::header::CONTENT_TYPE],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    let bytes = download.bytes().await.unwrap();
    assert!(bytes.starts_with(b"PK"));
    assert!(rwml::Document::open(&bytes).is_ok());

    let _ = client.cancel().await;
    cancellation_token.cancel();
    server.await.unwrap();
}

fn as_arguments(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

struct LegacySseReader {
    response: reqwest::Response,
    buffer: String,
}

impl LegacySseReader {
    fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            buffer: String::new(),
        }
    }

    async fn next_event(&mut self) -> String {
        loop {
            self.buffer = self.buffer.replace("\r\n", "\n");
            if let Some(end) = self.buffer.find("\n\n") {
                let event = self.buffer[..end].to_string();
                self.buffer.drain(..end + 2);
                if !event.starts_with(':') {
                    return event;
                }
            }
            let chunk = self
                .response
                .chunk()
                .await
                .unwrap()
                .expect("SSE stream ended before the next event");
            self.buffer.push_str(&String::from_utf8_lossy(&chunk));
        }
    }

    async fn next_json(&mut self) -> Value {
        let event = self.next_event().await;
        assert!(
            event.contains("event: message"),
            "unexpected SSE event: {event}"
        );
        serde_json::from_str(&event_data(&event)).unwrap()
    }
}

fn event_data(event: &str) -> String {
    event
        .lines()
        .filter_map(|line| {
            line.strip_prefix("data: ")
                .or_else(|| line.strip_prefix("data:"))
        })
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n")
}

async fn post_legacy(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    endpoint: &str,
    message: &Value,
) {
    let response = client
        .post(format!("http://{address}{endpoint}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(message.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
}
