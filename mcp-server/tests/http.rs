use base64::{engine::general_purpose::STANDARD, Engine as _};
use rmcp::{
    model::{CallToolRequestParams, ClientInfo},
    transport::StreamableHttpClientTransport,
    ServiceExt,
};
use serde_json::{json, Map, Value};
use sheetbrief_mcp::build_router;
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
    let names = tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>();
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
    assert_eq!(analysis["total"]["value"], 292_000.0);
    assert_eq!(analysis["dataset"]["data_rows"], 50);

    let result = client
        .call_tool(CallToolRequestParams::new("build_report").with_arguments(as_arguments(input)))
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

    let _ = client.cancel().await;
    cancellation_token.cancel();
    server.await.unwrap();
}

fn as_arguments(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}
