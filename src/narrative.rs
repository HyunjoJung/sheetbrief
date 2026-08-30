use crate::{Analysis, Result, SheetBriefError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_POINTS_PER_SECTION: usize = 4;
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
    let top_regions = top_ties(&analysis.regions).collect::<Vec<_>>();
    let top_item = analysis.items.first().map(|value| value.fact_id.clone());
    let top_month = analysis
        .months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.fact_id.clone());
    let low_month = analysis
        .months
        .iter()
        .min_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.fact_id.clone());
    let bottom_region = analysis.regions.last().map(|value| value.fact_id.clone());

    let mut summary_evidence = vec![analysis.total.fact_id.clone()];
    summary_evidence.extend(top_regions.iter().cloned());
    if let Some(bottom_region) = bottom_region.as_ref() {
        summary_evidence.push(bottom_region.clone());
    }
    let mut summary = vec![NarrativePoint {
        text: "선두 지역과 최저 지역의 격차가 이번 회의의 첫 번째 의사결정 포인트입니다."
            .to_string(),
        evidence_ids: summary_evidence,
    }];
    if let Some(top_item) = top_item {
        summary.push(NarrativePoint {
            text: "상위 품목에 실적이 집중돼 있어 반복 가능한 판매 요인을 확인할 가치가 있습니다."
                .to_string(),
            evidence_ids: vec![top_item],
        });
    }
    if let (Some(top_month), Some(low_month)) = (top_month.as_ref(), low_month.as_ref()) {
        summary.push(NarrativePoint {
            text: "월별 최고점과 최저점의 차이가 커서 변동 원인을 분리해 볼 필요가 있습니다."
                .to_string(),
            evidence_ids: vec![top_month.clone(), low_month.clone()],
        });
    }

    let mut priorities = Vec::new();
    if let Some(bottom_region) = bottom_region.as_ref() {
        priorities.push(NarrativePoint {
            text: "최저 지역의 채널 구성과 담당 계정 변화를 먼저 확인합니다.".to_string(),
            evidence_ids: vec![bottom_region.clone()],
        });
    }
    if let Some(top_month) = top_month.as_ref() {
        priorities.push(NarrativePoint {
            text: "최고 월의 성과가 일회성인지 다음 기간에도 재현 가능한지 확인합니다.".to_string(),
            evidence_ids: vec![top_month.clone()],
        });
    }
    if let Some(low_month) = low_month.as_ref() {
        priorities.push(NarrativePoint {
            text: "최저 월의 하락이 실제 수요 변화인지 집계 범위의 영향인지 분리합니다."
                .to_string(),
            evidence_ids: vec![low_month.clone()],
        });
    }

    let narrative = Narrative {
        report_title: "판매 실적 의사결정 브리프".to_string(),
        purpose:
            "지역·품목·월별 변화를 한 번에 비교해, 회의에서 결정할 우선순위와 담당 행동을 정리했습니다."
                .to_string(),
        summary,
        priorities,
        actions: vec![
            NarrativePoint {
                text: "최저 지역의 회복 과제에 담당자와 완료 기한을 지정합니다.".to_string(),
                evidence_ids: bottom_region
                    .clone()
                    .into_iter()
                    .chain(top_regions.iter().cloned())
                    .collect(),
            },
            NarrativePoint {
                text: "최고 월을 만든 지역과 품목의 실행 방식을 다음 계획에 반영합니다."
                    .to_string(),
                evidence_ids: top_month
                    .clone()
                    .into_iter()
                    .chain(top_regions.iter().cloned())
                    .collect(),
            },
            NarrativePoint {
                text: "다음 회의부터 목표와 비교 기간이 함께 들어오도록 입력 양식을 표준화합니다."
                    .to_string(),
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
