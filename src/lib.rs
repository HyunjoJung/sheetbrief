#![forbid(unsafe_code)]

mod agent;
mod analysis;
mod narrative;
mod report;

pub use agent::{
    build_agent_context, AgentContext, AgentDataset, AgentInput, AgentParser, AgentSignals,
    NarrativeContract,
};
pub use analysis::{
    analyze_workbook, Aggregate, Analysis, ColumnBinding, ContextStatus, DatasetSummary, Fact,
    InputMetadata, ParserSummary, SourceRef,
};
pub use narrative::{default_narrative, validate_narrative, Narrative, NarrativePoint};
pub use report::{build_report, build_report_bundle, ReportBundle};

pub const MAX_WORKBOOK_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_DATA_ROWS: usize = 100_000;

pub type Result<T> = std::result::Result<T, SheetBriefError>;

#[derive(Debug, thiserror::Error)]
pub enum SheetBriefError {
    #[error("workbook is {actual} bytes; the prototype limit is {limit} bytes")]
    InputTooLarge { actual: usize, limit: usize },
    #[error("unsupported workbook container: {0}")]
    UnsupportedContainer(String),
    #[error("spreadsheet parsing failed: {0}")]
    Spreadsheet(#[from] rxls::Error),
    #[error("spreadsheet diagnostics could not be decoded: {0}")]
    Diagnostics(#[from] serde_json::Error),
    #[error("no worksheet contains the required Region, Item, Volume, and Month headers")]
    SchemaNotFound,
    #[error("worksheet {sheet:?} row {row} maps more than one column to {field:?}")]
    DuplicateHeader {
        sheet: String,
        row: u32,
        field: String,
    },
    #[error("worksheet {sheet:?} row {row} is missing {field:?}")]
    MissingValue {
        sheet: String,
        row: u32,
        field: String,
    },
    #[error("worksheet {sheet:?} row {row} has a non-numeric Volume value")]
    InvalidVolume { sheet: String, row: u32 },
    #[error("worksheet {sheet:?} row {row} has a non-finite Volume value")]
    NonFiniteVolume { sheet: String, row: u32 },
    #[error("worksheet {sheet:?} contains more than {limit} data rows")]
    TooManyRows { sheet: String, limit: usize },
    #[error("worksheet {sheet:?} contains no data rows below the selected header")]
    NoDataRows { sheet: String },
    #[error("invalid narrative: {0}")]
    InvalidNarrative(String),
    #[error("Word report generation failed: {0}")]
    Word(#[from] rwml::Error),
    #[error("generated report failed verification: {0}")]
    ReportVerification(String),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("usage: {0}")]
    Usage(String),
}
