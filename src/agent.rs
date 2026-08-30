use crate::{Analysis, ContextStatus, Fact};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentContext {
    pub schema_version: u32,
    pub task: String,
    pub input: AgentInput,
    pub dataset: AgentDataset,
    pub parser: AgentParser,
    pub signals: AgentSignals,
    pub facts: Vec<Fact>,
    pub contexts: Vec<ContextStatus>,
    pub narrative_contract: NarrativeContract,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentInput {
    pub file_name: String,
    pub format: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentDataset {
    pub sheet: String,
    pub source_range: String,
    pub data_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentParser {
    pub warning_count: usize,
    pub partial: bool,
    pub text_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSignals {
    pub total: String,
    pub top_regions: Vec<String>,
    pub top_item: Option<String>,
    pub peak_month: Option<String>,
    pub low_region: Option<String>,
    pub low_month: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrativeContract {
    pub sections: Vec<String>,
    pub point_text_policy: String,
    pub evidence_policy: String,
    pub max_points_per_section: usize,
}

pub fn build_agent_context(analysis: &Analysis) -> AgentContext {
    let top_regions = analysis
        .regions
        .first()
        .map(|top| {
            analysis
                .regions
                .iter()
                .take_while(|region| region.value == top.value)
                .map(|region| region.fact_id.clone())
                .collect()
        })
        .unwrap_or_default();
    let peak_month = analysis
        .months
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.fact_id.clone());
    let low_month = analysis
        .months
        .iter()
        .min_by(|left, right| left.value.total_cmp(&right.value))
        .map(|value| value.fact_id.clone());

    AgentContext {
        schema_version: 1,
        task: "Create a meeting-ready decision brief from bounded spreadsheet facts.".to_string(),
        input: AgentInput {
            file_name: analysis.input.file_name.clone(),
            format: analysis.input.format.clone(),
            sha256: analysis.input.sha256.clone(),
        },
        dataset: AgentDataset {
            sheet: analysis.dataset.sheet.clone(),
            source_range: analysis.dataset.source_range.clone(),
            data_rows: analysis.dataset.data_rows,
        },
        parser: AgentParser {
            warning_count: analysis.parser.warnings.len(),
            partial: analysis.parser.partial,
            text_truncated: analysis.parser.text_truncated,
        },
        signals: AgentSignals {
            total: analysis.total.fact_id.clone(),
            top_regions,
            top_item: analysis.items.first().map(|value| value.fact_id.clone()),
            peak_month,
            low_region: analysis.regions.last().map(|value| value.fact_id.clone()),
            low_month,
        },
        facts: analysis.facts.clone(),
        contexts: analysis.contexts.clone(),
        narrative_contract: NarrativeContract {
            sections: vec![
                "summary".to_string(),
                "priorities".to_string(),
                "actions".to_string(),
            ],
            point_text_policy:
                "Write concise Korean meeting language without ASCII digits; the renderer supplies authoritative numbers from evidence IDs."
                    .to_string(),
            evidence_policy:
                "Every point must cite one or more fact_id or context_id values returned here."
                    .to_string(),
            max_points_per_section: 4,
        },
    }
}
