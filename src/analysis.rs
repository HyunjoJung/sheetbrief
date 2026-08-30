use crate::{Result, SheetBriefError, MAX_DATA_ROWS, MAX_WORKBOOK_BYTES};
use rxls::{Sheet, Workbook, WorkbookReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

const HEADER_SCAN_ROWS: u32 = 20;
const MAX_LABEL_CHARS: usize = 256;
const OLE2_MAGIC: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Analysis {
    pub schema_version: u32,
    pub input: InputMetadata,
    pub parser: ParserSummary,
    pub dataset: DatasetSummary,
    pub total: Aggregate,
    pub regions: Vec<Aggregate>,
    pub items: Vec<Aggregate>,
    pub months: Vec<Aggregate>,
    pub facts: Vec<Fact>,
    pub contexts: Vec<ContextStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputMetadata {
    pub file_name: String,
    pub sha256: String,
    pub byte_length: usize,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParserSummary {
    pub sheets: usize,
    pub cells: usize,
    pub formulas: usize,
    pub text_truncated: bool,
    pub partial: bool,
    pub warnings: Vec<Value>,
    pub diagnostics: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetSummary {
    pub sheet: String,
    pub header_row: u32,
    pub data_rows: usize,
    pub source_range: String,
    pub candidate_sheets: usize,
    pub columns: BTreeMap<String, ColumnBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnBinding {
    pub header: String,
    pub column: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Aggregate {
    pub fact_id: String,
    pub label: String,
    pub value: f64,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    pub fact_id: String,
    pub metric: String,
    pub dimensions: BTreeMap<String, String>,
    pub value: f64,
    pub row_count: usize,
    pub source: SourceRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    pub sheet: String,
    pub range: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextStatus {
    pub context_id: String,
    pub available: bool,
    pub description: String,
}

#[derive(Debug, Clone)]
struct HeaderMatch {
    row: u32,
    region: u16,
    item: u16,
    volume: u16,
    month: u16,
    bindings: BTreeMap<String, ColumnBinding>,
}

#[derive(Debug, Clone)]
struct Bucket {
    label: String,
    value: f64,
    row_count: usize,
}

pub fn analyze_workbook(bytes: &[u8], file_name: impl Into<String>) -> Result<Analysis> {
    if bytes.len() > MAX_WORKBOOK_BYTES {
        return Err(SheetBriefError::InputTooLarge {
            actual: bytes.len(),
            limit: MAX_WORKBOOK_BYTES,
        });
    }

    let format = detect_format(bytes)?;
    let workbook = Workbook::open(bytes)?;
    let diagnostics_text =
        WorkbookReport::from_workbook_with_package(&format, &workbook, bytes).to_json();
    let diagnostics: Value = serde_json::from_str(&diagnostics_text)?;
    let parser = parser_summary(diagnostics);

    if parser.partial || parser.text_truncated {
        return Err(SheetBriefError::ReportVerification(
            "partial or text-truncated workbook cannot produce a normal report".to_string(),
        ));
    }

    let candidates = workbook
        .sheets
        .iter()
        .filter(|sheet| sheet.is_worksheet)
        .filter_map(|sheet| match locate_headers(sheet) {
            Ok(Some(headers)) => Some(Ok((sheet, headers))),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>>>()?;

    let candidate_sheets = candidates.len();
    let Some((sheet, headers)) = candidates.into_iter().next() else {
        return Err(SheetBriefError::SchemaNotFound);
    };

    let (_, _, last_sheet_row, _) =
        sheet
            .dimensions()
            .ok_or_else(|| SheetBriefError::NoDataRows {
                sheet: sheet.name.clone(),
            })?;
    let mut regions = BTreeMap::new();
    let mut items = BTreeMap::new();
    let mut months = BTreeMap::new();
    let mut total = 0.0;
    let mut data_rows = 0usize;
    let mut last_data_row = None;

    for row in headers.row.saturating_add(1)..=last_sheet_row {
        let columns = [headers.region, headers.item, headers.volume, headers.month];
        if columns
            .iter()
            .all(|column| !cell_has_value(sheet, row, *column))
        {
            continue;
        }

        if data_rows == MAX_DATA_ROWS {
            return Err(SheetBriefError::TooManyRows {
                sheet: sheet.name.clone(),
                limit: MAX_DATA_ROWS,
            });
        }

        let region = required_label(sheet, row, headers.region, "Region")?;
        let item = required_label(sheet, row, headers.item, "Item")?;
        let month = required_label(sheet, row, headers.month, "Month")?;
        let volume_cell =
            sheet
                .cell(row, headers.volume)
                .ok_or_else(|| SheetBriefError::MissingValue {
                    sheet: sheet.name.clone(),
                    row: row + 1,
                    field: "Volume".to_string(),
                })?;
        let volume = volume_cell
            .get_float()
            .ok_or_else(|| SheetBriefError::InvalidVolume {
                sheet: sheet.name.clone(),
                row: row + 1,
            })?;
        if !volume.is_finite() {
            return Err(SheetBriefError::NonFiniteVolume {
                sheet: sheet.name.clone(),
                row: row + 1,
            });
        }

        add_bucket(&mut regions, region, volume)?;
        add_bucket(&mut items, item, volume)?;
        add_bucket(&mut months, month, volume)?;
        total += volume;
        if !total.is_finite() {
            return Err(SheetBriefError::NonFiniteVolume {
                sheet: sheet.name.clone(),
                row: row + 1,
            });
        }
        data_rows += 1;
        last_data_row = Some(row);
    }

    let Some(last_data_row) = last_data_row else {
        return Err(SheetBriefError::NoDataRows {
            sheet: sheet.name.clone(),
        });
    };

    let first_column = [headers.region, headers.item, headers.volume, headers.month]
        .into_iter()
        .min()
        .unwrap_or(0);
    let last_column = [headers.region, headers.item, headers.volume, headers.month]
        .into_iter()
        .max()
        .unwrap_or(0);
    let source_range = format!(
        "{}{}:{}{}",
        column_name(first_column),
        headers.row + 2,
        column_name(last_column),
        last_data_row + 1
    );
    let source = SourceRef {
        sheet: sheet.name.clone(),
        range: source_range.clone(),
    };

    let mut used_fact_ids = BTreeSet::new();
    used_fact_ids.insert("total.volume".to_string());
    let total = Aggregate {
        fact_id: "total.volume".to_string(),
        label: "All records".to_string(),
        value: total,
        row_count: data_rows,
    };
    let regions = build_aggregates("region", regions, &mut used_fact_ids, SortMode::Value)?;
    let items = build_aggregates("item", items, &mut used_fact_ids, SortMode::Value)?;
    let months = build_aggregates("month", months, &mut used_fact_ids, SortMode::Month)?;

    let mut facts = Vec::with_capacity(1 + regions.len() + items.len() + months.len());
    facts.push(fact_from_aggregate(&total, None, &source));
    facts.extend(
        regions
            .iter()
            .map(|aggregate| fact_from_aggregate(aggregate, Some("region"), &source)),
    );
    facts.extend(
        items
            .iter()
            .map(|aggregate| fact_from_aggregate(aggregate, Some("item"), &source)),
    );
    facts.extend(
        months
            .iter()
            .map(|aggregate| fact_from_aggregate(aggregate, Some("month"), &source)),
    );

    Ok(Analysis {
        schema_version: 1,
        input: InputMetadata {
            file_name: file_name.into(),
            sha256: sha256_hex(bytes),
            byte_length: bytes.len(),
            format,
        },
        parser,
        dataset: DatasetSummary {
            sheet: sheet.name.clone(),
            header_row: headers.row + 1,
            data_rows,
            source_range,
            candidate_sheets,
            columns: headers.bindings,
        },
        total,
        regions,
        items,
        months,
        facts,
        contexts: vec![
            ContextStatus {
                context_id: "context.target".to_string(),
                available: false,
                description: "목표치 미제공".to_string(),
            },
            ContextStatus {
                context_id: "context.comparison_period".to_string(),
                available: false,
                description: "비교 기간 미제공".to_string(),
            },
            ContextStatus {
                context_id: "context.revenue".to_string(),
                available: false,
                description: "가격 및 매출 데이터 미제공".to_string(),
            },
        ],
    })
}

fn detect_format(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(OLE2_MAGIC) {
        return Ok("xls".to_string());
    }
    if !bytes.starts_with(b"PK") {
        return Err(SheetBriefError::UnsupportedContainer(
            "neither OLE2 nor ZIP".to_string(),
        ));
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        SheetBriefError::UnsupportedContainer(format!("invalid ZIP package: {error}"))
    })?;
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            SheetBriefError::UnsupportedContainer(format!("invalid ZIP entry: {error}"))
        })?;
        names.insert(entry.name().replace('\\', "/"));
    }

    if names.contains("xl/workbook.bin") {
        return Ok("xlsb".to_string());
    }
    if names.contains("xl/workbook.xml") {
        return Ok(if names.contains("xl/vbaProject.bin") {
            "xlsm"
        } else {
            "xlsx"
        }
        .to_string());
    }
    if names.contains("content.xml") && names.contains("META-INF/manifest.xml") {
        return Ok("ods".to_string());
    }
    Err(SheetBriefError::UnsupportedContainer(
        "ZIP package is not XLSX, XLSB, or ODS".to_string(),
    ))
}

fn parser_summary(diagnostics: Value) -> ParserSummary {
    let usize_at = |pointer: &str| {
        diagnostics
            .pointer(pointer)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0)
    };
    let bool_at = |pointer: &str| {
        diagnostics
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let warnings = diagnostics
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ParserSummary {
        sheets: usize_at("/stats/sheets"),
        cells: usize_at("/stats/cells"),
        formulas: usize_at("/stats/formulas"),
        text_truncated: bool_at("/stats/text_truncated"),
        partial: bool_at("/provenance/partial"),
        warnings,
        diagnostics,
    }
}

fn locate_headers(sheet: &Sheet) -> Result<Option<HeaderMatch>> {
    let Some((first_row, first_col, last_row, last_col)) = sheet.dimensions() else {
        return Ok(None);
    };
    let scan_end = last_row.min(first_row.saturating_add(HEADER_SCAN_ROWS - 1));

    for row in first_row..=scan_end {
        let mut columns = BTreeMap::<&'static str, (u16, String)>::new();
        let mut duplicate = None;
        for col in first_col..=last_col {
            let Some(text) = sheet.formatted(row, col).map(str::trim) else {
                continue;
            };
            let Some(field) = canonical_header(text) else {
                continue;
            };
            if columns.insert(field, (col, text.to_string())).is_some() {
                duplicate = Some(field);
            }
        }
        if columns.len() == 4 {
            if let Some(field) = duplicate {
                return Err(SheetBriefError::DuplicateHeader {
                    sheet: sheet.name.clone(),
                    row: row + 1,
                    field: field.to_string(),
                });
            }
            let mut bindings = BTreeMap::new();
            for (field, (column, header)) in &columns {
                bindings.insert(
                    (*field).to_string(),
                    ColumnBinding {
                        header: header.clone(),
                        column: column_name(*column),
                    },
                );
            }
            return Ok(Some(HeaderMatch {
                row,
                region: columns["region"].0,
                item: columns["item"].0,
                volume: columns["volume"].0,
                month: columns["month"].0,
                bindings,
            }));
        }
    }
    Ok(None)
}

fn canonical_header(value: &str) -> Option<&'static str> {
    match normalize(value).as_str() {
        "region" | "area" | "지역" | "권역" => Some("region"),
        "item" | "product" | "상품" | "품목" => Some("item"),
        "volume" | "quantity" | "qty" | "salesvolume" | "수량" | "거래량" => Some("volume"),
        "month" | "월" | "월별" => Some("month"),
        _ => None,
    }
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn cell_has_value(sheet: &Sheet, row: u32, col: u16) -> bool {
    sheet.cell(row, col).is_some()
        || sheet
            .formatted(row, col)
            .is_some_and(|value| !value.trim().is_empty())
}

fn required_label(sheet: &Sheet, row: u32, col: u16, field: &str) -> Result<String> {
    let label = sheet
        .formatted(row, col)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SheetBriefError::MissingValue {
            sheet: sheet.name.clone(),
            row: row + 1,
            field: field.to_string(),
        })?;
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(SheetBriefError::MissingValue {
            sheet: sheet.name.clone(),
            row: row + 1,
            field: format!("{field} (label exceeds {MAX_LABEL_CHARS} characters)"),
        });
    }
    Ok(label.to_string())
}

fn add_bucket(map: &mut BTreeMap<String, Bucket>, label: String, value: f64) -> Result<()> {
    let key = label.to_lowercase();
    let bucket = map.entry(key).or_insert_with(|| Bucket {
        label,
        value: 0.0,
        row_count: 0,
    });
    bucket.value += value;
    if !bucket.value.is_finite() {
        return Err(SheetBriefError::ReportVerification(
            "aggregate exceeded the finite numeric range".to_string(),
        ));
    }
    bucket.row_count += 1;
    Ok(())
}

#[derive(Clone, Copy)]
enum SortMode {
    Value,
    Month,
}

fn build_aggregates(
    dimension: &str,
    buckets: BTreeMap<String, Bucket>,
    used_ids: &mut BTreeSet<String>,
    sort_mode: SortMode,
) -> Result<Vec<Aggregate>> {
    let mut values = buckets.into_values().collect::<Vec<_>>();
    match sort_mode {
        SortMode::Value => values.sort_by(|left, right| {
            right
                .value
                .total_cmp(&left.value)
                .then_with(|| left.label.cmp(&right.label))
        }),
        SortMode::Month => values.sort_by(|left, right| {
            month_rank(&left.label)
                .cmp(&month_rank(&right.label))
                .then_with(|| left.label.cmp(&right.label))
        }),
    }

    values
        .into_iter()
        .map(|bucket| {
            let base = format!("{dimension}.{}.volume", slug(&bucket.label));
            let fact_id = if used_ids.insert(base.clone()) {
                base
            } else {
                let collision_safe = format!(
                    "{dimension}.{}-{}.volume",
                    slug(&bucket.label),
                    &sha256_hex(bucket.label.as_bytes())[..8]
                );
                if !used_ids.insert(collision_safe.clone()) {
                    return Err(SheetBriefError::ReportVerification(
                        "fact ID collision could not be resolved".to_string(),
                    ));
                }
                collision_safe
            };
            Ok(Aggregate {
                fact_id,
                label: bucket.label,
                value: bucket.value,
                row_count: bucket.row_count,
            })
        })
        .collect()
}

fn fact_from_aggregate(aggregate: &Aggregate, dimension: Option<&str>, source: &SourceRef) -> Fact {
    let mut dimensions = BTreeMap::new();
    if let Some(dimension) = dimension {
        dimensions.insert(dimension.to_string(), aggregate.label.clone());
    }
    Fact {
        fact_id: aggregate.fact_id.clone(),
        metric: "volume_sum".to_string(),
        dimensions,
        value: aggregate.value,
        row_count: aggregate.row_count,
        source: source.clone(),
    }
}

fn month_rank(label: &str) -> (u8, String) {
    let normalized = normalize(label);
    let rank = match normalized.as_str() {
        "january" | "jan" | "1월" => 1,
        "february" | "feb" | "2월" => 2,
        "march" | "mar" | "3월" => 3,
        "april" | "apr" | "4월" => 4,
        "may" | "5월" => 5,
        "june" | "jun" | "6월" => 6,
        "july" | "jul" | "7월" => 7,
        "august" | "aug" | "8월" => 8,
        "september" | "sep" | "sept" | "9월" => 9,
        "october" | "oct" | "10월" => 10,
        "november" | "nov" | "11월" => 11,
        "december" | "dec" | "12월" => 12,
        _ => 255,
    };
    (rank, normalized)
}

fn slug(label: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            separator = true;
        }
    }
    if out.is_empty() {
        format!("value-{}", &sha256_hex(label.as_bytes())[..8])
    } else {
        out
    }
}

fn column_name(column: u16) -> String {
    let mut index = u32::from(column) + 1;
    let mut chars = Vec::new();
    while index > 0 {
        let remainder = (index - 1) % 26;
        chars.push(char::from_u32(u32::from(b'A') + remainder).unwrap_or('A'));
        index = (index - 1) / 26;
    }
    chars.iter().rev().collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
