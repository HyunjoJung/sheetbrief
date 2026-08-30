use sheetbrief::{
    analyze_workbook, build_agent_context, build_report, build_report_bundle, default_narrative,
    validate_narrative, Narrative, NarrativePoint, SheetBriefError,
};

const BASELINE: &[u8] = include_bytes!("../data/regional-sales-baseline.xlsx");
const DEMO: &[u8] = include_bytes!("../data/meeting-sales-demo.xlsx");
const DEMO_XLS: &[u8] = include_bytes!("../data/meeting-sales-demo.xls");
const DEMO_XLSB: &[u8] = include_bytes!("../data/meeting-sales-demo.xlsb");
const DEMO_ODS: &[u8] = include_bytes!("../data/meeting-sales-demo.ods");

#[test]
fn baseline_matches_the_dataset_oracle() {
    let analysis = analyze_workbook(BASELINE, "regional-sales-baseline.xlsx").unwrap();

    assert_eq!(analysis.input.format, "xlsx");
    assert_eq!(
        analysis.input.sha256,
        "4cb7740635dc265bb84aa0b0ae16ee522b6f82241c40993aec369f912a18af34"
    );
    assert_eq!(analysis.parser.sheets, 1);
    assert_eq!(analysis.parser.cells, 204);
    assert_eq!(analysis.parser.formulas, 0);
    assert!(analysis.parser.warnings.is_empty());
    assert!(!analysis.parser.partial);
    assert_eq!(analysis.dataset.sheet, "Sheet1");
    assert_eq!(analysis.dataset.header_row, 1);
    assert_eq!(analysis.dataset.data_rows, 50);
    assert_eq!(analysis.dataset.source_range, "A2:D51");
    assert_eq!(analysis.total.value, 292_000.0);
    assert_eq!(analysis.total.row_count, 50);

    assert_group(
        &analysis.regions,
        &[
            ("East", 90_000.0, 14),
            ("South", 90_000.0, 13),
            ("North", 75_000.0, 13),
            ("West", 37_000.0, 10),
        ],
    );
    assert_group(
        &analysis.items,
        &[
            ("Grape", 99_000.0, 16),
            ("Apple", 87_000.0, 16),
            ("Orange", 61_000.0, 11),
            ("Pear", 45_000.0, 7),
        ],
    );
    assert_group(
        &analysis.months,
        &[
            ("January", 6_000.0, 1),
            ("February", 29_000.0, 4),
            ("March", 18_000.0, 2),
            ("April", 21_000.0, 3),
            ("May", 3_000.0, 1),
            ("June", 30_000.0, 4),
            ("July", 44_000.0, 7),
            ("August", 29_000.0, 5),
            ("September", 18_000.0, 3),
            ("October", 41_000.0, 7),
            ("November", 32_000.0, 6),
            ("December", 21_000.0, 7),
        ],
    );
}

#[test]
fn generated_docx_reopens_and_contains_authoritative_values() {
    let analysis = analyze_workbook(BASELINE, "regional-sales-baseline.xlsx").unwrap();
    let narrative = default_narrative(&analysis).unwrap();
    let bytes = build_report(&analysis, &narrative).unwrap();

    assert!(bytes.starts_with(b"PK"));
    let document = rwml::Document::open(&bytes).unwrap();
    let markdown = document.to_markdown();
    for expected in [
        "판매 실적 의사결정 브리프",
        "이번 회의에서 결정할 것",
        "지역별 실적",
        "품목별 실적",
        "월별 흐름과 다음 행동",
        "292,000",
        "East",
        "90,000",
        "Grape",
        "99,000",
        "7월",
        "44,000",
        "목표치 미제공",
        "A2:D51",
    ] {
        assert!(markdown.contains(expected), "missing {expected:?}");
    }
    assert!(!markdown.contains("region.east.volume"));
    assert!(!markdown.contains("context.target"));

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(archive.by_name("word/document.xml").is_ok());
    assert!(archive.by_name("word/charts/chart1.xml").is_err());
}

#[test]
fn korean_demo_preserves_the_public_baseline_and_builds_agent_context() {
    let analysis = analyze_workbook(DEMO, "meeting-sales-demo.xlsx").unwrap();
    assert_eq!(analysis.input.format, "xlsx");
    assert_eq!(analysis.parser.sheets, 2);
    assert_eq!(analysis.dataset.sheet, "판매실적");
    assert_eq!(analysis.dataset.header_row, 2);
    assert_eq!(analysis.dataset.source_range, "A3:D52");
    assert_eq!(analysis.dataset.data_rows, 50);
    assert_eq!(analysis.total.value, 292_000.0);
    let top_regions = analysis.regions[..2]
        .iter()
        .map(|region| region.label.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        top_regions,
        std::collections::BTreeSet::from(["남부", "동부"])
    );
    assert_eq!(analysis.items[0].label, "포도");
    assert_eq!(analysis.months[6].label, "7월");

    let context = build_agent_context(&analysis);
    assert_eq!(context.dataset.sheet, "판매실적");
    assert_eq!(context.signals.total, "total.volume");
    assert_eq!(context.signals.top_regions.len(), 2);
    assert_eq!(
        context.signals.top_item.as_deref(),
        Some(analysis.items[0].fact_id.as_str())
    );
    assert_eq!(
        context.signals.peak_month.as_deref(),
        analysis
            .months
            .iter()
            .find(|month| month.label == "7월")
            .map(|month| month.fact_id.as_str())
    );
    assert_eq!(
        context.signals.low_month.as_deref(),
        analysis
            .months
            .iter()
            .find(|month| month.label == "5월")
            .map(|month| month.fact_id.as_str())
    );
}

#[test]
fn all_advertised_containers_preserve_the_demo_oracle() {
    for (bytes, file_name, format) in [
        (DEMO_XLS, "meeting-sales-demo.xls", "xls"),
        (DEMO, "meeting-sales-demo.xlsx", "xlsx"),
        (DEMO_XLSB, "meeting-sales-demo.xlsb", "xlsb"),
        (DEMO_ODS, "meeting-sales-demo.ods", "ods"),
    ] {
        let analysis = analyze_workbook(bytes, file_name).unwrap();
        assert_eq!(analysis.input.format, format, "{file_name}");
        assert_eq!(analysis.dataset.data_rows, 50, "{file_name}");
        assert_eq!(analysis.total.value, 292_000.0, "{file_name}");

        let top_regions = analysis.regions[..2]
            .iter()
            .map(|region| region.label.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            top_regions,
            std::collections::BTreeSet::from(["남부", "동부"]),
            "{file_name}"
        );
        assert_eq!(analysis.items[0].label, "포도", "{file_name}");
        assert_eq!(analysis.items[0].value, 99_000.0, "{file_name}");
        assert_eq!(analysis.months[6].label, "7월", "{file_name}");
        assert_eq!(analysis.months[6].value, 44_000.0, "{file_name}");
    }
}

#[test]
fn report_bundle_contains_reopenable_docx_and_native_pdf() {
    let analysis = analyze_workbook(DEMO, "meeting-sales-demo.xlsx").unwrap();
    let narrative = default_narrative(&analysis).unwrap();
    let bundle = build_report_bundle(&analysis, &narrative).unwrap();
    assert!(bundle.docx.starts_with(b"PK"));
    assert!(bundle.pdf.starts_with(b"%PDF"));
    assert!(bundle.pdf.len() > 10_000);
    assert!(rwml::Document::open(&bundle.docx)
        .unwrap()
        .to_markdown()
        .contains("남부·동부"));
}

#[test]
fn narrative_rejects_raw_numbers_and_unknown_evidence() {
    let analysis = analyze_workbook(BASELINE, "regional-sales-baseline.xlsx").unwrap();
    let mut narrative = default_narrative(&analysis).unwrap();
    narrative.summary[0].text = "전체 Volume은 292000입니다.".to_string();
    assert!(matches!(
        validate_narrative(&analysis, &narrative),
        Err(SheetBriefError::InvalidNarrative(_))
    ));

    let invalid = Narrative {
        report_title: "보고서".to_string(),
        purpose: "회의 준비".to_string(),
        summary: vec![NarrativePoint {
            text: "확인했습니다.".to_string(),
            evidence_ids: vec!["fact.does.not.exist".to_string()],
        }],
        priorities: vec![NarrativePoint {
            text: "확인이 필요합니다.".to_string(),
            evidence_ids: vec!["context.target".to_string()],
        }],
        actions: vec![NarrativePoint {
            text: "자료를 보완합니다.".to_string(),
            evidence_ids: vec!["context.target".to_string()],
        }],
    };
    assert!(matches!(
        validate_narrative(&analysis, &invalid),
        Err(SheetBriefError::InvalidNarrative(_))
    ));

    let mut oversized = default_narrative(&analysis).unwrap();
    oversized.summary = vec![oversized.summary[0].clone(); 5];
    assert!(matches!(
        validate_narrative(&analysis, &oversized),
        Err(SheetBriefError::InvalidNarrative(message))
            if message == "summary contains more than 4 points"
    ));
}

#[test]
fn analysis_json_is_deterministic() {
    let first = analyze_workbook(BASELINE, "regional-sales-baseline.xlsx").unwrap();
    let second = analyze_workbook(BASELINE, "regional-sales-baseline.xlsx").unwrap();
    assert_eq!(
        serde_json::to_vec_pretty(&first).unwrap(),
        serde_json::to_vec_pretty(&second).unwrap()
    );
}

fn assert_group(actual: &[sheetbrief::Aggregate], expected: &[(&str, f64, usize)]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.label, expected.0);
        assert_eq!(actual.value, expected.1);
        assert_eq!(actual.row_count, expected.2);
    }
}
