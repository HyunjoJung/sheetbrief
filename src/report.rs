use crate::{
    validate_narrative, Aggregate, Analysis, Narrative, NarrativePoint, Result, SheetBriefError,
};
use rwml::{
    Align, Block, CellBuilder, ChartBuilder, Color, DocBuilder, ParagraphBuilder,
    ParagraphStyleBuilder, RunBuilder, TableBuilder, VCell,
};

const KR_FONT: &str = "맑은 고딕";

#[derive(Clone, Copy)]
struct Palette {
    ink: Color,
    blue: Color,
    coral: Color,
    white: Color,
    ice: Color,
    mint: Color,
    amber: Color,
    gray: Color,
    muted: Color,
    line: Color,
}

impl Palette {
    fn sheetbrief() -> Self {
        Self {
            ink: Color::rgb(0x1B, 0x1F, 0x23),
            blue: Color::rgb(0x25, 0x57, 0xD6),
            coral: Color::rgb(0xE9, 0x5D, 0x46),
            white: Color::rgb(0xFF, 0xFF, 0xFF),
            ice: Color::rgb(0xEE, 0xF3, 0xFF),
            mint: Color::rgb(0xEA, 0xF7, 0xF0),
            amber: Color::rgb(0xFF, 0xF2, 0xD8),
            gray: Color::rgb(0xF3, 0xF4, 0xF6),
            muted: Color::rgb(0x62, 0x6A, 0x73),
            line: Color::rgb(0xD9, 0xDE, 0xE5),
        }
    }
}

pub fn build_report(analysis: &Analysis, narrative: &Narrative) -> Result<Vec<u8>> {
    validate_narrative(analysis, narrative)?;

    let palette = Palette::sheetbrief();
    let top_regions = top_ties(&analysis.regions);
    let top_item = analysis.items.first();
    let top_month = analysis
        .months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value));

    let mut builder = DocBuilder::new()
        .title(&narrative.report_title)
        .creator("SheetBrief / rxls + rwml")
        .margins_each_pt(44.0, 46.0, 43.0, 46.0)
        .header_runs([
            run("SHEETBRIEF").bold().color(palette.blue).build(),
            run("  /  판매 실적 브리프").color(palette.muted).build(),
        ])
        .footer_runs([run("출처 추적 가능한 편집형 DOCX")
            .color(palette.muted)
            .size_half_pt(16)
            .build()])
        .page_numbers()
        .paragraph_style(
            ParagraphStyleBuilder::new("SheetBriefTitle", "SheetBrief Title")
                .based_on("Title")
                .spacing_after_pt(7.0)
                .run_font(KR_FONT)
                .run_size_half_pt(42)
                .run_color(palette.ink)
                .run_bold(),
        )
        .paragraph_style(
            ParagraphStyleBuilder::new("SheetBriefLead", "SheetBrief Lead")
                .spacing_after_pt(7.0)
                .line_pct(1.12)
                .run_font(KR_FONT)
                .run_size_half_pt(23)
                .run_color(palette.ink),
        )
        .rich_paragraph(kicker("회의 전에 보는 한 장 요약", palette.coral))
        .rich_paragraph(
            ParagraphBuilder::new().style("SheetBriefTitle").push_run(
                run(&narrative.report_title)
                    .bold()
                    .color(palette.ink)
                    .build(),
            ),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .style("SheetBriefLead")
                .push_run(run(executive_lead(analysis)).color(palette.ink).build()),
        )
        .rich_paragraph(
            ParagraphBuilder::new().spacing_after_pt(12.0).push_run(
                run(&narrative.purpose)
                    .color(palette.muted)
                    .size_half_pt(19)
                    .build(),
            ),
        )
        .rich_paragraph(section_label("핵심 지표", palette.blue))
        .rich_table(kpi_table(
            analysis,
            &top_regions,
            top_item,
            top_month,
            palette,
        ))
        .rich_paragraph(section_heading("지역과 품목, 어디가 큰가", palette.ink))
        .rich_table(comparison_charts(analysis, palette))
        .rich_paragraph(section_heading("검증 상태", palette.ink))
        .rich_table(source_status_table(analysis, palette))
        .rich_paragraph(
            ParagraphBuilder::new()
                .spacing_before_pt(5.0)
                .spacing_after_pt(0.0)
                .push_run(
                    run(format!("SHA-256  {}", analysis.input.sha256))
                        .color(palette.muted)
                        .size_half_pt(15)
                        .build(),
                ),
        );

    builder = builder
        .page_break()
        .rich_paragraph(kicker("흐름에서 회의 안건까지", palette.coral))
        .rich_paragraph(
            ParagraphBuilder::new().style("SheetBriefTitle").push_run(
                run("월별 흐름과 다음 판단")
                    .bold()
                    .color(palette.ink)
                    .build(),
            ),
        )
        .rich_paragraph(
            ParagraphBuilder::new().style("SheetBriefLead").push_run(
                run(month_insight(&analysis.months))
                    .color(palette.ink)
                    .build(),
            ),
        )
        .chart(month_chart(&analysis.months))
        .rich_paragraph(section_heading("오늘 회의에서 정리할 것", palette.ink))
        .rich_table(decision_cards(analysis, narrative, palette));

    let model = builder.build();
    let bytes = rwml::try_write_docx(&model)?;
    let reopened = rwml::Document::open(&bytes)?;
    let markdown = reopened.to_markdown();
    for expected in [
        narrative.report_title.as_str(),
        "핵심 지표",
        "지역별 비교",
        "품목별 비교",
        "월별 흐름과 다음 판단",
        "검증 상태",
        &format_number(analysis.total.value),
    ] {
        if !markdown.contains(expected) {
            return Err(SheetBriefError::ReportVerification(format!(
                "reopened DOCX is missing {expected:?}"
            )));
        }
    }
    Ok(bytes)
}

fn run(text: impl AsRef<str>) -> RunBuilder {
    RunBuilder::new(text.as_ref()).font(KR_FONT)
}

fn kicker(text: &str, color: Color) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .spacing_after_pt(5.0)
        .push_run(run(text).bold().color(color).size_half_pt(16).build())
}

fn section_label(text: &str, color: Color) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .spacing_before_pt(2.0)
        .spacing_after_pt(5.0)
        .push_run(run(text).bold().color(color).size_half_pt(17).build())
}

fn section_heading(text: &str, color: Color) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .heading_level(2)
        .spacing_before_pt(13.0)
        .spacing_after_pt(7.0)
        .push_run(run(text).bold().color(color).size_half_pt(25).build())
}

fn executive_lead(analysis: &Analysis) -> String {
    let regions = top_ties(&analysis.regions);
    let region_names = join_labels(&regions);
    let item = analysis
        .items
        .first()
        .map(|value| value.label.as_str())
        .unwrap_or("상위 품목");
    let month = analysis
        .months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.label.as_str())
        .unwrap_or("최고 월");
    let region_phrase = if regions.len() > 1 {
        format!("{region_names}가 지역 선두를 나눠 가졌고")
    } else {
        format!("{region_names}가 지역 선두이고")
    };
    format!("{region_phrase}, {item}가 품목 중 가장 큽니다. 월별 최고치는 {month}에 나타났습니다.")
}

fn top_ties(values: &[Aggregate]) -> Vec<&Aggregate> {
    let Some(top_value) = values.first().map(|value| value.value) else {
        return Vec::new();
    };
    values
        .iter()
        .take_while(|value| value.value == top_value)
        .collect()
}

fn join_labels(values: &[&Aggregate]) -> String {
    match values {
        [] => "상위 지역".to_string(),
        [only] => only.label.clone(),
        [left, right] => format!("{}·{}", left.label, right.label),
        many => many
            .iter()
            .map(|value| value.label.as_str())
            .collect::<Vec<_>>()
            .join("·"),
    }
}

fn kpi_table(
    analysis: &Analysis,
    top_regions: &[&Aggregate],
    top_item: Option<&Aggregate>,
    top_month: Option<&Aggregate>,
    palette: Palette,
) -> TableBuilder {
    let region_names = join_labels(top_regions);
    let region_detail = top_regions
        .first()
        .map(|value| format!("각 {}", format_number(value.value)))
        .unwrap_or_else(|| "-".to_string());
    let (item_name, item_value) = split_aggregate(top_item);
    let (month_name, month_value) = split_aggregate(top_month);

    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.white)
        .border_size_eighths(8)
        .col_widths_pct([0.31, 0.23, 0.23, 0.23])
        .row([
            kpi_cell(
                "전체 합계",
                &format_number(analysis.total.value),
                &format!("{} 행 합계", analysis.total.row_count),
                palette.blue,
                palette.white,
                true,
            ),
            kpi_cell(
                "선두 지역",
                &region_names,
                &region_detail,
                palette.ice,
                palette.ink,
                false,
            ),
            kpi_cell(
                "선두 품목",
                &item_name,
                &item_value,
                palette.mint,
                palette.ink,
                false,
            ),
            kpi_cell(
                "최고 월",
                &month_name,
                &month_value,
                palette.amber,
                palette.ink,
                false,
            ),
        ])
}

fn split_aggregate(aggregate: Option<&Aggregate>) -> (String, String) {
    aggregate.map_or_else(
        || ("-".to_string(), "-".to_string()),
        |value| (value.label.clone(), format_number(value.value)),
    )
}

fn kpi_cell(
    label: &str,
    value: &str,
    detail: &str,
    fill: Color,
    text_color: Color,
    primary: bool,
) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .spacing_after_pt(3.0)
                .push_run(run(label).bold().color(text_color).size_half_pt(15).build()),
        )
        .rich_paragraph(
            ParagraphBuilder::new().spacing_after_pt(2.0).push_run(
                run(value)
                    .bold()
                    .color(text_color)
                    .size_half_pt(if primary { 35 } else { 25 })
                    .build(),
            ),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .push_run(run(detail).color(text_color).size_half_pt(15).build()),
        )
        .shading(fill)
        .valign(VCell::Center)
        .margins_twips(130, 130, 130, 130)
}

fn comparison_charts(analysis: &Analysis, palette: Palette) -> TableBuilder {
    let region_chart =
        aggregate_bar_chart(&analysis.regions, 308, 212, "지역별 Volume 비교 막대 차트");
    let item_chart = aggregate_bar_chart(&analysis.items, 308, 212, "품목별 Volume 비교 막대 차트");

    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.line)
        .border_size_eighths(2)
        .col_widths_pct([0.5, 0.5])
        .row([
            chart_cell(
                "지역별 비교",
                &region_insight(&analysis.regions),
                region_chart,
                &exact_values(&analysis.regions),
                palette,
            ),
            chart_cell(
                "품목별 비교",
                &item_insight(&analysis.items),
                item_chart,
                &exact_values(&analysis.items),
                palette,
            ),
        ])
}

fn aggregate_bar_chart(
    values: &[Aggregate],
    width_px: u32,
    height_px: u32,
    alt: &str,
) -> ChartBuilder {
    ChartBuilder::bar()
        .categories(values.iter().rev().map(|value| value.label.clone()))
        .series("Volume", values.iter().rev().map(|value| value.value))
        .size_px(width_px, height_px)
        .alt(alt)
}

fn chart_cell(
    label: &str,
    insight: &str,
    chart: ChartBuilder,
    exact: &str,
    palette: Palette,
) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(section_label(label, palette.blue))
        .rich_paragraph(
            ParagraphBuilder::new()
                .spacing_after_pt(2.0)
                .line_pct(1.05)
                .push_run(
                    run(insight)
                        .bold()
                        .color(palette.ink)
                        .size_half_pt(19)
                        .build(),
                ),
        )
        .push_block(Block::Chart(chart.build()))
        .rich_paragraph(
            ParagraphBuilder::new()
                .spacing_before_pt(2.0)
                .push_run(run(exact).color(palette.muted).size_half_pt(14).build()),
        )
        .valign(VCell::Top)
        .margins_twips(115, 130, 105, 130)
}

fn region_insight(regions: &[Aggregate]) -> String {
    let leaders = top_ties(regions);
    let Some(top) = leaders.first() else {
        return "지역별 값을 확인할 수 없습니다.".to_string();
    };
    let leader_names = join_labels(&leaders);
    let leader_phrase = if leaders.len() > 1 {
        "공동 선두"
    } else {
        "선두"
    };
    let tail = regions.last();
    match tail {
        Some(tail) if tail.label != top.label => format!(
            "{leader_names}가 {}(각 {})이고, {}는 {}입니다.",
            leader_phrase,
            format_number(top.value),
            tail.label,
            format_number(tail.value)
        ),
        _ => format!(
            "{leader_names}가 {}({})입니다.",
            leader_phrase,
            format_number(top.value)
        ),
    }
}

fn item_insight(items: &[Aggregate]) -> String {
    let Some(top) = items.first() else {
        return "품목별 값을 확인할 수 없습니다.".to_string();
    };
    format!(
        "{}가 {}으로 가장 큰 품목입니다.",
        top.label,
        format_number(top.value)
    )
}

fn exact_values(values: &[Aggregate]) -> String {
    values
        .iter()
        .map(|value| format!("{} {}", value.label, format_number(value.value)))
        .collect::<Vec<_>>()
        .join("  ·  ")
}

fn month_chart(months: &[Aggregate]) -> ChartBuilder {
    ChartBuilder::line()
        .categories(months.iter().map(|value| value.label.clone()))
        .series("Volume", months.iter().map(|value| value.value))
        .size_px(650, 245)
        .alt("월별 Volume 흐름 선 차트")
}

fn month_insight(months: &[Aggregate]) -> String {
    let top = months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value));
    let bottom = months
        .iter()
        .min_by(|left, right| left.value.total_cmp(&right.value));
    match (top, bottom) {
        (Some(top), Some(bottom)) => format!(
            "{}가 {}으로 가장 높고, {}가 {}으로 가장 낮습니다. 피크와 저점의 차이는 {}입니다.",
            top.label,
            format_number(top.value),
            bottom.label,
            format_number(bottom.value),
            format_number(top.value - bottom.value)
        ),
        _ => "월별 흐름을 확인할 수 없습니다.".to_string(),
    }
}

fn decision_cards(analysis: &Analysis, narrative: &Narrative, palette: Palette) -> TableBuilder {
    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.white)
        .border_size_eighths(8)
        .col_widths_pct([0.34, 0.33, 0.33])
        .row([
            decision_card(
                "보이는 것",
                &narrative.summary,
                analysis,
                palette.ice,
                palette.blue,
                palette,
            ),
            decision_card(
                "확인할 것",
                &narrative.priorities,
                analysis,
                palette.amber,
                palette.coral,
                palette,
            ),
            decision_card(
                "다음 행동",
                &narrative.actions,
                analysis,
                palette.mint,
                palette.blue,
                palette,
            ),
        ])
}

fn decision_card(
    title: &str,
    points: &[NarrativePoint],
    analysis: &Analysis,
    fill: Color,
    accent: Color,
    palette: Palette,
) -> CellBuilder {
    let mut cell = CellBuilder::new().rich_paragraph(
        ParagraphBuilder::new()
            .spacing_after_pt(7.0)
            .push_run(run(title).bold().color(accent).size_half_pt(21).build()),
    );
    for (index, point) in points.iter().enumerate() {
        cell = cell.rich_paragraph(
            ParagraphBuilder::new()
                .spacing_after_pt(6.0)
                .line_pct(1.08)
                .push_run(
                    run(format!("{:02}  ", index + 1))
                        .bold()
                        .color(accent)
                        .size_half_pt(15)
                        .build(),
                )
                .push_run(run(&point.text).color(palette.ink).size_half_pt(17).build()),
        );
    }
    let evidence = evidence_text(analysis, points);
    if !evidence.is_empty() {
        cell = cell.rich_paragraph(
            ParagraphBuilder::new().spacing_before_pt(3.0).push_run(
                run(format!("근거  {evidence}"))
                    .color(palette.muted)
                    .size_half_pt(13)
                    .build(),
            ),
        );
    }
    cell.shading(fill)
        .valign(VCell::Top)
        .margins_twips(140, 145, 135, 145)
}

fn evidence_text(analysis: &Analysis, points: &[NarrativePoint]) -> String {
    let mut evidence = Vec::new();
    for evidence_id in points.iter().flat_map(|point| point.evidence_ids.iter()) {
        let rendered = if let Some(fact) = analysis
            .facts
            .iter()
            .find(|fact| &fact.fact_id == evidence_id)
        {
            let label = fact
                .dimensions
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| "전체".to_string());
            format!("{} {}", label, format_number(fact.value))
        } else if let Some(context) = analysis
            .contexts
            .iter()
            .find(|context| &context.context_id == evidence_id)
        {
            context.description.clone()
        } else {
            continue;
        };
        if !evidence.contains(&rendered) {
            evidence.push(rendered);
        }
    }
    evidence.join(" · ")
}

fn source_status_table(analysis: &Analysis, palette: Palette) -> TableBuilder {
    let parser_status = if analysis.parser.warnings.is_empty() && !analysis.parser.partial {
        "경고 없음 · 완전 읽기".to_string()
    } else {
        format!(
            "경고 {} · {}",
            analysis.parser.warnings.len(),
            if analysis.parser.partial {
                "부분 복구"
            } else {
                "완전 읽기"
            }
        )
    };
    TableBuilder::new()
        .header_rows(1)
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.line)
        .border_size_eighths(2)
        .col_widths_pct([0.3, 0.26, 0.15, 0.29])
        .row([
            status_cell("입력 파일", palette.gray, palette.muted, true),
            status_cell("선택 범위", palette.gray, palette.muted, true),
            status_cell("데이터", palette.gray, palette.muted, true),
            status_cell("파서 상태", palette.gray, palette.muted, true),
        ])
        .row([
            status_cell(&analysis.input.file_name, palette.white, palette.ink, false),
            status_cell(
                &format!(
                    "{}!{}",
                    analysis.dataset.sheet, analysis.dataset.source_range
                ),
                palette.white,
                palette.ink,
                false,
            ),
            status_cell(
                &format!("{} 행", analysis.dataset.data_rows),
                palette.white,
                palette.ink,
                false,
            ),
            status_cell(&parser_status, palette.white, palette.ink, false),
        ])
}

fn status_cell(text: &str, fill: Color, text_color: Color, header: bool) -> CellBuilder {
    let mut value = run(text)
        .color(text_color)
        .size_half_pt(if header { 15 } else { 17 });
    if header {
        value = value.bold();
    }
    let mut cell = CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .align(Align::Left)
                .push_run(value.build()),
        )
        .shading(fill)
        .valign(VCell::Center)
        .margins_twips(90, 110, 90, 110);
    if header {
        cell = cell.header();
    }
    cell
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        let raw = (value as i64).to_string();
        let (sign, digits) = raw
            .strip_prefix('-')
            .map_or(("", raw.as_str()), |digits| ("-", digits));
        let mut grouped = String::with_capacity(raw.len() + raw.len() / 3);
        grouped.push_str(sign);
        for (index, character) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(character);
        }
        grouped
    } else {
        let mut rendered = format!("{value:.2}");
        while rendered.contains('.') && rendered.ends_with('0') {
            rendered.pop();
        }
        if rendered.ends_with('.') {
            rendered.pop();
        }
        rendered
    }
}
