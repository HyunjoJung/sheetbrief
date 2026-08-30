use crate::{Analysis, Result, SheetBriefError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_POINTS_PER_SECTION: usize = 8;
const MAX_POINT_CHARS: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Narrative {
    pub report_title: String,
    pub purpose: String,
    pub summary: Vec<NarrativePoint>,
    pub priorities: Vec<NarrativePoint>,
    pub actions: Vec<NarrativePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NarrativePoint {
    pub text: String,
    pub evidence_ids: Vec<String>,
}

pub fn default_narrative(analysis: &Analysis) -> Result<Narrative> {
    let top_regions = top_ties(&analysis.regions);
    let top_item = analysis.items.first().map(|value| value.fact_id.clone());
    let top_month = analysis
        .months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.fact_id.clone());
    let bottom_region = analysis.regions.last().map(|value| value.fact_id.clone());
    let sparse_months = analysis
        .months
        .iter()
        .filter(|value| value.row_count == 1)
        .map(|value| value.fact_id.clone())
        .collect::<Vec<_>>();

    let mut summary = vec![NarrativePoint {
        text: "지역별 실적은 상위권과 하위권의 차이가 분명합니다.".to_string(),
        evidence_ids: std::iter::once(analysis.total.fact_id.clone())
            .chain(top_regions)
            .collect(),
    }];
    if let Some(top_item) = top_item {
        summary.push(NarrativePoint {
            text: "품목별로는 가장 큰 비중을 차지하는 항목이 뚜렷합니다.".to_string(),
            evidence_ids: vec![top_item],
        });
    }
    if let Some(top_month) = top_month {
        summary.push(NarrativePoint {
            text: "월별 흐름은 일정하지 않아 피크가 발생한 구간의 배경을 확인할 필요가 있습니다."
                .to_string(),
            evidence_ids: vec![top_month],
        });
    }

    let mut priorities = Vec::new();
    if let Some(bottom_region) = bottom_region.as_ref() {
        priorities.push(NarrativePoint {
            text: "낮은 지역 수치가 실제 성과인지, 채널이나 입력 범위 누락인지 먼저 확인합니다."
                .to_string(),
            evidence_ids: vec![bottom_region.clone()],
        });
    }
    if !sparse_months.is_empty() {
        priorities.push(NarrativePoint {
            text: "관측 기록이 적은 달은 실적보다 수집 범위의 영향을 받았을 수 있습니다."
                .to_string(),
            evidence_ids: sparse_months.clone(),
        });
    }
    priorities.push(NarrativePoint {
        text: "목표치와 비교 기간이 없어 달성 여부는 아직 판단하지 않습니다.".to_string(),
        evidence_ids: vec!["context.target".to_string()],
    });

    let narrative = Narrative {
        report_title: "판매 실적 회의 브리프".to_string(),
        purpose:
            "지역·품목·월별 흐름을 비교해, 오늘 회의에서 확인할 질문과 다음 행동을 정리했습니다."
                .to_string(),
        summary,
        priorities,
        actions: vec![
            NarrativePoint {
                text: "지역별 원본 담당자와 채널 범위를 대조합니다.".to_string(),
                evidence_ids: bottom_region.into_iter().collect(),
            },
            NarrativePoint {
                text: "관측 기록이 적은 달의 집계 완료 여부를 확인합니다.".to_string(),
                evidence_ids: sparse_months,
            },
            NarrativePoint {
                text: "목표치와 전기·전년 비교를 붙인 뒤 성과 판단을 확정합니다.".to_string(),
                evidence_ids: vec![
                    "context.target".to_string(),
                    "context.comparison_period".to_string(),
                ],
            },
        ],
    };
    validate_narrative(analysis, &narrative)?;
    Ok(narrative)
}

pub fn validate_narrative(analysis: &Analysis, narrative: &Narrative) -> Result<()> {
    validate_plain_text("report_title", &narrative.report_title, false)?;
    validate_plain_text("purpose", &narrative.purpose, false)?;

    let evidence_catalog = analysis
        .facts
        .iter()
        .map(|fact| fact.fact_id.as_str())
        .chain(
            analysis
                .contexts
                .iter()
                .map(|context| context.context_id.as_str()),
        )
        .collect::<BTreeSet<_>>();

    for (section_name, points) in [
        ("summary", &narrative.summary),
        ("priorities", &narrative.priorities),
        ("actions", &narrative.actions),
    ] {
        if points.is_empty() {
            return Err(SheetBriefError::InvalidNarrative(format!(
                "{section_name} must not be empty"
            )));
        }
        if points.len() > MAX_POINTS_PER_SECTION {
            return Err(SheetBriefError::InvalidNarrative(format!(
                "{section_name} contains more than {MAX_POINTS_PER_SECTION} points"
            )));
        }
        for point in points {
            validate_plain_text(section_name, &point.text, true)?;
            if point.evidence_ids.is_empty() {
                return Err(SheetBriefError::InvalidNarrative(format!(
                    "{section_name} point has no evidence IDs"
                )));
            }
            let mut unique = BTreeSet::new();
            for evidence_id in &point.evidence_ids {
                if !evidence_catalog.contains(evidence_id.as_str()) {
                    return Err(SheetBriefError::InvalidNarrative(format!(
                        "unknown evidence ID {evidence_id:?}"
                    )));
                }
                if !unique.insert(evidence_id) {
                    return Err(SheetBriefError::InvalidNarrative(format!(
                        "duplicate evidence ID {evidence_id:?}"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_plain_text(field: &str, text: &str, reject_digits: bool) -> Result<()> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(SheetBriefError::InvalidNarrative(format!(
            "{field} must not be empty"
        )));
    }
    if trimmed.chars().count() > MAX_POINT_CHARS {
        return Err(SheetBriefError::InvalidNarrative(format!(
            "{field} exceeds {MAX_POINT_CHARS} characters"
        )));
    }
    if reject_digits && trimmed.chars().any(|character| character.is_ascii_digit()) {
        return Err(SheetBriefError::InvalidNarrative(format!(
            "{field} contains a raw number; numeric evidence must be rendered from fact IDs"
        )));
    }
    Ok(())
}

fn top_ties(values: &[crate::Aggregate]) -> impl Iterator<Item = String> + '_ {
    let top_value = values.first().map(|value| value.value);
    values
        .iter()
        .take_while(move |value| Some(value.value) == top_value)
        .map(|value| value.fact_id.clone())
}
