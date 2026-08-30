use rxls::{CellStyle, HAlign, Workbook};
use std::path::PathBuf;

const BASELINE: &[u8] = include_bytes!("../data/regional-sales-baseline.xlsx");

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/meeting-sales-demo.xlsx"));
    let source = Workbook::open(BASELINE).expect("open licensed baseline");
    let source_sheet = source.sheets.first().expect("baseline worksheet");

    let mut workbook = Workbook::new();
    let sheet = workbook.add_sheet("판매실적");
    let title = CellStyle::new()
        .bold()
        .size(15)
        .color([255, 255, 255])
        .fill([0x19, 0x1C, 0x1F])
        .align(HAlign::Left);
    let header = CellStyle::new()
        .bold()
        .color([0x19, 0x1C, 0x1F])
        .fill([0xE9, 0xED, 0xF2])
        .align(HAlign::Center);
    let quantity = CellStyle::new().num_fmt("#,##0");

    sheet.write_styled(0, 0, "2026 판매 회의 데이터", &title);
    sheet.merge(0, 0, 0, 3);
    for (column, label) in ["지역", "품목", "수량", "월"].iter().enumerate() {
        sheet.write_styled(1, column as u16, *label, &header);
    }

    let (_, _, last_row, _) = source_sheet.dimensions().expect("baseline dimensions");
    for source_row in 1..=last_row {
        let target_row = source_row + 1;
        let region = source_sheet.formatted(source_row, 0).expect("region");
        let item = source_sheet.formatted(source_row, 1).expect("item");
        let volume = source_sheet
            .cell(source_row, 2)
            .and_then(|cell| cell.get_float())
            .expect("volume");
        let month = source_sheet.formatted(source_row, 3).expect("month");
        sheet.write(target_row, 0, translate_region(region));
        sheet.write(target_row, 1, translate_item(item));
        sheet.write_styled(target_row, 2, volume, &quantity);
        sheet.write(target_row, 3, translate_month(month));
    }
    sheet.set_col_width(0, 14.0);
    sheet.set_col_width(1, 16.0);
    sheet.set_col_width(2, 14.0);
    sheet.set_col_width(3, 12.0);
    sheet.freeze_panes(2, 0);
    sheet.autofilter(1, 0, last_row + 1, 3);

    let source_info = workbook.add_sheet("데이터출처");
    source_info.write(0, 0, "원본");
    source_info.write(0, 1, "libxlsxwriter autofilter01.xlsx");
    source_info.write(1, 0, "라이선스");
    source_info.write(1, 1, "BSD-2-Clause");
    source_info.write(2, 0, "변경");
    source_info.write(
        2,
        1,
        "한국어 헤더와 범주명으로 번역한 SheetBrief 데모 파생본",
    );
    source_info.set_col_width(0, 14.0);
    source_info.set_col_width(1, 68.0);

    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    std::fs::write(&output, workbook.to_xlsx()).expect("write demo workbook");
    eprintln!("wrote {}", output.display());
}

fn translate_region(value: &str) -> &str {
    match value {
        "East" => "동부",
        "South" => "남부",
        "North" => "북부",
        "West" => "서부",
        other => other,
    }
}

fn translate_item(value: &str) -> &str {
    match value {
        "Grape" => "포도",
        "Apple" => "사과",
        "Orange" => "오렌지",
        "Pear" => "배",
        other => other,
    }
}

fn translate_month(value: &str) -> &str {
    match value {
        "January" => "1월",
        "February" => "2월",
        "March" => "3월",
        "April" => "4월",
        "May" => "5월",
        "June" => "6월",
        "July" => "7월",
        "August" => "8월",
        "September" => "9월",
        "October" => "10월",
        "November" => "11월",
        "December" => "12월",
        other => other,
    }
}
