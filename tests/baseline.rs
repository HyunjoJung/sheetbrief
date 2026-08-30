use sheetbrief::{
    analyze_workbook, build_report, default_narrative, validate_narrative, Narrative,
    NarrativePoint, SheetBriefError,
};

const BASELINE: &[u8] = include_bytes!("../data/regional-sales-baseline.xlsx");

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
        "판매 실적 회의 브리프",
        "핵심 지표",
        "지역별 비교",
        "품목별 비교",
        "월별 흐름과 다음 판단",
        "292,000",
        "East",
        "90,000",
        "Grape",
        "99,000",
        "July",
        "44,000",
        "목표치 미제공",
        "A2:D51",
    ] {
        assert!(markdown.contains(expected), "missing {expected:?}");
    }
    assert!(!markdown.contains("region.east.volume"));
    assert!(!markdown.contains("context.target"));

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(archive.by_name("word/charts/chart1.xml").is_ok());
    assert!(archive.by_name("word/charts/chart2.xml").is_ok());
    assert!(archive.by_name("word/charts/chart3.xml").is_ok());
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
