use crate::{
    validate_narrative, Aggregate, Analysis, Narrative, NarrativePoint, Result, SheetBriefError,
};
use rwml::{
    Align, CellBuilder, Color, DocBuilder, DocModel, ParagraphBuilder, ParagraphStyleBuilder,
    RunBuilder, TableBuilder, VCell,
};

const KR_FONT: &str = "맑은 고딕";
const BAR_SEGMENTS: usize = 10;

#[derive(Debug, Clone)]
pub struct ReportBundle {
    pub docx: Vec<u8>,
    pub pdf: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Palette {
    ink: Color,
    blue: Color,
    green: Color,
    coral: Color,
    white: Color,
    pale_blue: Color,
    pale_green: Color,
    pale_coral: Color,
    gray: Color,
    muted: Color,
    line: Color,
}

impl Palette {
    fn sheetbrief() -> Self {
        Self {
            ink: Color::rgb(0x20, 0x21, 0x24),
            blue: Color::rgb(0x24, 0x6B, 0xFD),
            green: Color::rgb(0x0E, 0x84, 0x69),
            coral: Color::rgb(0xD9, 0x4F, 0x45),
            white: Color::rgb(0xFF, 0xFF, 0xFF),
            pale_blue: Color::rgb(0xEA, 0xF1, 0xFF),
            pale_green: Color::rgb(0xE8, 0xF5, 0xF0),
            pale_coral: Color::rgb(0xFD, 0xED, 0xEA),
            gray: Color::rgb(0xF3, 0xF5, 0xF7),
            muted: Color::rgb(0x5F, 0x63, 0x68),
            line: Color::rgb(0xD8, 0xDD, 0xE3),
        }
    }
}

pub fn build_report(analysis: &Analysis, narrative: &Narrative) -> Result<Vec<u8>> {
    let model = build_model(analysis, narrative)?;
    let docx = rwml::try_write_docx(&model)?;
    verify_docx(&docx, analysis, narrative)?;
    Ok(docx)
}

pub fn build_report_bundle(analysis: &Analysis, narrative: &Narrative) -> Result<ReportBundle> {
    let model = build_model(analysis, narrative)?;
    let docx = rwml::try_write_docx(&model)?;
    verify_docx(&docx, analysis, narrative)?;
    let pdf = rwml::try_render_pdf_bundled(&model)?;
    if !pdf.starts_with(b"%PDF") {
        return Err(SheetBriefError::ReportVerification(
            "rwml PDF output is missing the PDF signature".to_string(),
        ));
    }
    Ok(ReportBundle { docx, pdf })
}

fn build_model(analysis: &Analysis, narrative: &Narrative) -> Result<DocModel> {
    validate_narrative(analysis, narrative)?;
    let palette = Palette::sheetbrief();
    let top_regions = top_ties(&analysis.regions);
    let top_item = analysis.items.first();
    let peak_month = max_value(&analysis.months);
    let low_month = min_value(&analysis.months);

    let builder = DocBuilder::new()
        .title(&narrative.report_title)
        .creator("SheetBrief / rxls + Solar + rwml")
        .margins_each_pt(38.0, 42.0, 38.0, 42.0)
        .header_runs([
            run("SHEETBRIEF").bold().color(palette.blue).build(),
            run("  /  DECISION BRIEF").color(palette.muted).build(),
        ])
        .footer_runs([run("rxls로 읽고, 근거를 연결하고, rwml로 전달합니다.")
            .color(palette.muted)
            .size_half_pt(14)
            .build()])
        .page_numbers()
        .paragraph_style(
            ParagraphStyleBuilder::new("SheetBriefTitle", "SheetBrief Title")
                .based_on("Title")
                .spacing_after_pt(5.0)
                .run_font(KR_FONT)
                .run_size_half_pt(38)
                .run_color(palette.ink)
                .run_bold(),
        )
        .paragraph_style(
            ParagraphStyleBuilder::new("SheetBriefLead", "SheetBrief Lead")
                .spacing_after_pt(5.0)
                .line_pct(1.08)
                .run_font(KR_FONT)
                .run_size_half_pt(22)
                .run_color(palette.ink),
        )
        .rich_paragraph(kicker("이번 회의에서 결정할 것", palette.coral))
        .rich_paragraph(title(&narrative.report_title, palette))
        .rich_paragraph(lead(executive_lead(analysis), palette))
        .rich_paragraph(purpose(&narrative.purpose, palette))
        .rich_table(metadata_row(analysis, palette))
        .rich_table(signal_strip(
            analysis,
            &top_regions,
            top_item,
            peak_month,
            palette,
        ))
        .rich_paragraph(section_heading(&region_heading(&analysis.regions), palette))
        .rich_table(ranked_bars(&analysis.regions, true, palette))
        .rich_paragraph(section_heading(&item_heading(&analysis.items), palette))
        .rich_table(ranked_bars(&analysis.items, false, palette))
        .rich_paragraph(section_heading("핵심 판단", palette))
        .rich_table(narrative_band(
            "보이는 것",
            &narrative.summary,
            analysis,
            palette.blue,
            palette.pale_blue,
            palette,
        ))
        .page_break()
        .rich_paragraph(kicker("흐름을 행동으로 바꾸기", palette.green))
        .rich_paragraph(title("월별 흐름과 다음 행동", palette))
        .rich_paragraph(lead(month_insight(&analysis.months), palette))
        .rich_paragraph(section_heading("월별 실적", palette))
        .rich_table(month_grid(&analysis.months, peak_month, low_month, palette))
        .rich_paragraph(section_heading("회의 안건", palette))
        .rich_table(narrative_band(
            "확인할 것",
            &narrative.priorities,
            analysis,
            palette.coral,
            palette.pale_coral,
            palette,
        ))
        .rich_table(narrative_band(
            "다음 행동",
            &narrative.actions,
            analysis,
            palette.green,
            palette.pale_green,
            palette,
        ))
        .rich_paragraph(section_heading("근거와 범위", palette))
        .rich_table(source_table(analysis, palette))
        .rich_paragraph(source_note(analysis, palette));

    Ok(builder.build())
}

fn verify_docx(bytes: &[u8], analysis: &Analysis, narrative: &Narrative) -> Result<()> {
    let reopened = rwml::Document::open(bytes)?;
    let markdown = reopened.to_markdown();
    for expected in [
        narrative.report_title.as_str(),
        "이번 회의에서 결정할 것",
        "지역별 실적",
        "품목별 실적",
        "월별 흐름과 다음 행동",
        "확인할 것",
        "다음 행동",
        "근거와 범위",
        &format_number(analysis.total.value),
    ] {
        if !markdown.contains(expected) {
            return Err(SheetBriefError::ReportVerification(format!(
                "reopened DOCX is missing {expected:?}"
            )));
        }
    }
    Ok(())
}

fn run(text: impl AsRef<str>) -> RunBuilder {
    RunBuilder::new(text.as_ref()).font(KR_FONT)
}

fn kicker(text: &str, color: Color) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .spacing_after_pt(4.0)
        .push_run(run(text).bold().color(color).size_half_pt(15).build())
}

fn title(text: &str, palette: Palette) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .style("SheetBriefTitle")
        .push_run(run(text).bold().color(palette.ink).size_half_pt(38).build())
}

fn lead(text: String, palette: Palette) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .style("SheetBriefLead")
        .push_run(run(text).color(palette.ink).size_half_pt(22).build())
}

fn purpose(text: &str, palette: Palette) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .spacing_after_pt(8.0)
        .push_run(run(text).color(palette.muted).size_half_pt(17).build())
}

fn section_heading(text: &str, palette: Palette) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .heading_level(2)
        .spacing_before_pt(9.0)
        .spacing_after_pt(5.0)
        .push_run(run(text).bold().color(palette.ink).size_half_pt(23).build())
}

fn metadata_row(analysis: &Analysis, palette: Palette) -> TableBuilder {
    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.line)
        .border_size_eighths(2)
        .col_widths_pct([0.36, 0.31, 0.17, 0.16])
        .row([
            metadata_cell(&analysis.input.file_name, palette),
            metadata_cell(
                &format!(
                    "{}!{}",
                    analysis.dataset.sheet, analysis.dataset.source_range
                ),
                palette,
            ),
            metadata_cell(&format!("{}행", analysis.dataset.data_rows), palette),
            metadata_cell(&analysis.input.format.to_uppercase(), palette),
        ])
}

fn metadata_cell(text: &str, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .push_run(run(text).color(palette.muted).size_half_pt(14).build()),
        )
        .margins_twips(65, 80, 65, 80)
        .valign(VCell::Center)
}

fn signal_strip(
    analysis: &Analysis,
    top_regions: &[&Aggregate],
    top_item: Option<&Aggregate>,
    peak_month: Option<&Aggregate>,
    palette: Palette,
) -> TableBuilder {
    let region_names = join_labels(top_regions);
    let region_value = top_regions
        .first()
        .map(|value| format_number(value.value))
        .unwrap_or_else(|| "-".to_string());
    let (item_name, item_value) = aggregate_strings(top_item);
    let (month_name, month_value) = aggregate_strings(peak_month);
    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.line)
        .border_size_eighths(2)
        .col_widths_pct([0.25, 0.25, 0.25, 0.25])
        .row([
            signal_cell(
                "전체 실적",
                &format_number(analysis.total.value),
                &format!("{}개 기록", analysis.total.row_count),
                palette.ink,
                palette,
            ),
            signal_cell(
                "선두 지역",
                &region_names,
                &region_value,
                palette.blue,
                palette,
            ),
            signal_cell("선두 품목", &item_name, &item_value, palette.green, palette),
            signal_cell(
                "최고 월",
                &display_month(&month_name),
                &month_value,
                palette.coral,
                palette,
            ),
        ])
}

fn signal_cell(
    label: &str,
    value: &str,
    detail: &str,
    accent: Color,
    palette: Palette,
) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().spacing_after_pt(2.0).push_run(
                run(label)
                    .bold()
                    .color(palette.muted)
                    .size_half_pt(14)
                    .build(),
            ),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .spacing_after_pt(1.0)
                .push_run(run(value).bold().color(accent).size_half_pt(25).build()),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .push_run(run(detail).color(palette.muted).size_half_pt(14).build()),
        )
        .margins_twips(105, 110, 95, 110)
        .valign(VCell::Top)
}

fn ranked_bars(values: &[Aggregate], low_attention: bool, palette: Palette) -> TableBuilder {
    let maximum = values.first().map(|value| value.value).unwrap_or(1.0);
    let displayed = if low_attention && values.len() > 6 {
        values
            .iter()
            .take(5)
            .chain(values.last())
            .collect::<Vec<_>>()
    } else {
        values.iter().take(6).collect::<Vec<_>>()
    };
    let mut table = TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.white)
        .border_size_eighths(1)
        .col_widths_pct(
            std::iter::once(0.18)
                .chain(std::iter::repeat_n(0.045, BAR_SEGMENTS))
                .chain([0.20, 0.17])
                .collect::<Vec<_>>(),
        );
    for (index, value) in displayed.into_iter().enumerate() {
        let active = if maximum > 0.0 {
            ((value.value / maximum) * BAR_SEGMENTS as f64)
                .ceil()
                .clamp(1.0, BAR_SEGMENTS as f64) as usize
        } else {
            0
        };
        let accent = if index == 0 {
            palette.blue
        } else if low_attention
            && values
                .last()
                .is_some_and(|lowest| lowest.fact_id == value.fact_id)
        {
            palette.coral
        } else {
            palette.green
        };
        let mut cells = Vec::with_capacity(BAR_SEGMENTS + 3);
        cells.push(bar_label_cell(&value.label, palette));
        for segment in 0..BAR_SEGMENTS {
            cells.push(bar_segment_cell(segment < active, accent, palette));
        }
        cells.push(bar_value_cell(&format_number(value.value), palette));
        cells.push(bar_note_cell(&format!("{}건", value.row_count), palette));
        table = table.row(cells);
    }
    table
}

fn bar_label_cell(label: &str, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().push_run(
                run(label)
                    .bold()
                    .color(palette.ink)
                    .size_half_pt(16)
                    .build(),
            ),
        )
        .margins_twips(55, 65, 55, 65)
        .valign(VCell::Center)
}

fn bar_segment_cell(active: bool, accent: Color, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .align(Align::Center)
                .push_run(run(" ").size_half_pt(13).build()),
        )
        .shading(if active { accent } else { palette.gray })
        .margins_twips(40, 18, 40, 18)
        .valign(VCell::Center)
}

fn bar_value_cell(value: &str, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().align(Align::Right).push_run(
                run(value)
                    .bold()
                    .color(palette.ink)
                    .size_half_pt(16)
                    .build(),
            ),
        )
        .margins_twips(55, 65, 55, 65)
        .valign(VCell::Center)
}

fn bar_note_cell(value: &str, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .align(Align::Right)
                .push_run(run(value).color(palette.muted).size_half_pt(14).build()),
        )
        .margins_twips(55, 65, 55, 65)
        .valign(VCell::Center)
}

fn narrative_band(
    title: &str,
    points: &[NarrativePoint],
    analysis: &Analysis,
    accent: Color,
    fill: Color,
    palette: Palette,
) -> TableBuilder {
    let mut body = CellBuilder::new();
    for (index, point) in points.iter().enumerate() {
        body = body.rich_paragraph(
            ParagraphBuilder::new()
                .spacing_after_pt(4.0)
                .push_run(
                    run(format!("{}  ", index + 1))
                        .bold()
                        .color(accent)
                        .size_half_pt(15)
                        .build(),
                )
                .push_run(run(&point.text).color(palette.ink).size_half_pt(16).build())
                .push_run(
                    run(format!("  /  {}", evidence_text(analysis, point)))
                        .color(palette.muted)
                        .size_half_pt(13)
                        .build(),
                ),
        );
    }
    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.white)
        .border_size_eighths(1)
        .col_widths_pct([0.20, 0.80])
        .row([
            CellBuilder::new()
                .rich_paragraph(
                    ParagraphBuilder::new()
                        .push_run(run(title).bold().color(accent).size_half_pt(20).build()),
                )
                .shading(fill)
                .margins_twips(110, 120, 100, 120)
                .valign(VCell::Top),
            body.shading(fill)
                .margins_twips(105, 125, 95, 125)
                .valign(VCell::Top),
        ])
}

fn month_grid(
    months: &[Aggregate],
    peak: Option<&Aggregate>,
    low: Option<&Aggregate>,
    palette: Palette,
) -> TableBuilder {
    let widths =
        std::iter::repeat_n(1.0 / months.len().max(1) as f32, months.len()).collect::<Vec<_>>();
    let mut table = TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.white)
        .border_size_eighths(3)
        .col_widths_pct(widths);
    let cells = months
        .iter()
        .map(|month| {
            let (fill, accent) = if peak.is_some_and(|value| value.fact_id == month.fact_id) {
                (palette.pale_blue, palette.blue)
            } else if low.is_some_and(|value| value.fact_id == month.fact_id) {
                (palette.pale_coral, palette.coral)
            } else {
                (palette.gray, palette.ink)
            };
            CellBuilder::new()
                .rich_paragraph(
                    ParagraphBuilder::new().align(Align::Center).push_run(
                        run(display_month(&month.label))
                            .bold()
                            .color(palette.muted)
                            .size_half_pt(13)
                            .build(),
                    ),
                )
                .rich_paragraph(
                    ParagraphBuilder::new().align(Align::Center).push_run(
                        run(compact_number(month.value))
                            .bold()
                            .color(accent)
                            .size_half_pt(17)
                            .build(),
                    ),
                )
                .shading(fill)
                .margins_twips(85, 35, 75, 35)
                .valign(VCell::Center)
        })
        .collect::<Vec<_>>();
    table = table.row(cells);
    table
}

fn source_table(analysis: &Analysis, palette: Palette) -> TableBuilder {
    let status = if analysis.parser.warnings.is_empty() && !analysis.parser.partial {
        "경고 없이 완전 읽기"
    } else {
        "진단 확인 필요"
    };
    TableBuilder::new()
        .width_pct(1.0)
        .fixed_layout()
        .border_color(palette.line)
        .border_size_eighths(2)
        .col_widths_pct([0.24, 0.34, 0.16, 0.26])
        .row([
            source_cell("파일", &analysis.input.file_name, palette),
            source_cell(
                "사용 범위",
                &format!(
                    "{}!{}",
                    analysis.dataset.sheet, analysis.dataset.source_range
                ),
                palette,
            ),
            source_cell(
                "기록",
                &format!("{}행", analysis.dataset.data_rows),
                palette,
            ),
            source_cell("읽기 상태", status, palette),
        ])
}

fn source_cell(label: &str, value: &str, palette: Palette) -> CellBuilder {
    CellBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().spacing_after_pt(1.0).push_run(
                run(label)
                    .bold()
                    .color(palette.muted)
                    .size_half_pt(13)
                    .build(),
            ),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .push_run(run(value).color(palette.ink).size_half_pt(15).build()),
        )
        .margins_twips(85, 95, 80, 95)
        .valign(VCell::Top)
}

fn source_note(analysis: &Analysis, palette: Palette) -> ParagraphBuilder {
    let hash = &analysis.input.sha256;
    let compact_hash = if hash.len() > 24 {
        format!("{}...{}", &hash[..12], &hash[hash.len() - 12..])
    } else {
        hash.clone()
    };
    ParagraphBuilder::new().spacing_before_pt(4.0).push_run(
        run(format!(
            "파일 지문 {compact_hash} · 전체 진단과 근거 ID는 analysis.json에 보존됩니다."
        ))
        .color(palette.muted)
        .size_half_pt(13)
        .build(),
    )
}

fn executive_lead(analysis: &Analysis) -> String {
    let regions = top_ties(&analysis.regions);
    let region_names = join_labels(&regions);
    let top_value = regions.first().map(|value| value.value).unwrap_or(0.0);
    let low = analysis.regions.last();
    let top_item = analysis.items.first();
    match (low, top_item) {
        (Some(low), Some(item)) => format!(
            "{region_names}가 {}으로 지역 선두를 공유합니다. {}는 {}이며, {}가 {}으로 가장 큰 품목입니다.",
            format_number(top_value),
            low.label,
            format_number(low.value),
            item.label,
            format_number(item.value)
        ),
        _ => "지역과 품목별 실적을 회의 의제로 정리했습니다.".to_string(),
    }
}

fn region_heading(regions: &[Aggregate]) -> String {
    match (regions.first(), regions.last()) {
        (Some(top), Some(low)) if low.value != 0.0 => format!(
            "지역별 실적 · 선두는 최저 지역의 {:.1}배",
            top.value / low.value
        ),
        _ => "지역별 실적".to_string(),
    }
}

fn item_heading(items: &[Aggregate]) -> String {
    items.first().map_or_else(
        || "품목별 실적".to_string(),
        |top| {
            let total = items.iter().map(|value| value.value).sum::<f64>();
            let share = if total == 0.0 {
                0.0
            } else {
                top.value / total * 100.0
            };
            format!("품목별 실적 · {} 비중 {:.1}%", top.label, share)
        },
    )
}

fn month_insight(months: &[Aggregate]) -> String {
    match (max_value(months), min_value(months)) {
        (Some(top), Some(low)) => format!(
            "{}이 {}으로 가장 높고, {}이 {}으로 가장 낮습니다. 차이는 {}입니다.",
            display_month(&top.label),
            format_number(top.value),
            display_month(&low.label),
            format_number(low.value),
            format_number(top.value - low.value)
        ),
        _ => "월별 흐름을 확인할 수 없습니다.".to_string(),
    }
}

fn evidence_text(analysis: &Analysis, point: &NarrativePoint) -> String {
    let mut values = Vec::new();
    for evidence_id in &point.evidence_ids {
        let rendered = if let Some(fact) = analysis
            .facts
            .iter()
            .find(|fact| &fact.fact_id == evidence_id)
        {
            let label = fact
                .dimensions
                .values()
                .next()
                .map(|value| display_month(value))
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
        if !values.contains(&rendered) {
            values.push(rendered);
        }
    }
    values.join(" · ")
}

fn max_value(values: &[Aggregate]) -> Option<&Aggregate> {
    values
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
}

fn min_value(values: &[Aggregate]) -> Option<&Aggregate> {
    values
        .iter()
        .min_by(|left, right| left.value.total_cmp(&right.value))
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
        many => many
            .iter()
            .map(|value| value.label.as_str())
            .collect::<Vec<_>>()
            .join("·"),
    }
}

fn aggregate_strings(value: Option<&Aggregate>) -> (String, String) {
    value.map_or_else(
        || ("-".to_string(), "-".to_string()),
        |value| (value.label.clone(), format_number(value.value)),
    )
}

fn display_month(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "january" | "jan" => "1월".to_string(),
        "february" | "feb" => "2월".to_string(),
        "march" | "mar" => "3월".to_string(),
        "april" | "apr" => "4월".to_string(),
        "may" => "5월".to_string(),
        "june" | "jun" => "6월".to_string(),
        "july" | "jul" => "7월".to_string(),
        "august" | "aug" => "8월".to_string(),
        "september" | "sep" | "sept" => "9월".to_string(),
        "october" | "oct" => "10월".to_string(),
        "november" | "nov" => "11월".to_string(),
        "december" | "dec" => "12월".to_string(),
        _ => value.to_string(),
    }
}

fn compact_number(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.0}K", value / 1_000.0)
    } else {
        format_number(value)
    }
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
