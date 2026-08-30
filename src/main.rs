use sheetbrief::{
    analyze_workbook, build_report, build_report_bundle, default_narrative, Result, SheetBriefError,
};
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("sheetbrief: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return Err(usage());
    };
    match (command, arguments.as_slice()) {
        ("analyze", [_, input, output]) => {
            let analysis = analyze_path(Path::new(input))?;
            write_json(Path::new(output), &analysis)?;
            println!("analysis: {}", Path::new(output).display());
        }
        ("report", [_, input, output]) => {
            let analysis = analyze_path(Path::new(input))?;
            let narrative = default_narrative(&analysis)?;
            let report = build_report(&analysis, &narrative)?;
            write_bytes(Path::new(output), &report)?;
            println!("report: {}", Path::new(output).display());
        }
        ("run", [_, input, output_directory]) => {
            let output_directory = PathBuf::from(output_directory);
            std::fs::create_dir_all(&output_directory)?;
            let analysis = analyze_path(Path::new(input))?;
            let narrative = default_narrative(&analysis)?;
            let report = build_report_bundle(&analysis, &narrative)?;
            let analysis_path = output_directory.join("analysis.json");
            let docx_path = output_directory.join("report.docx");
            let pdf_path = output_directory.join("report.pdf");
            write_json(&analysis_path, &analysis)?;
            write_bytes(&docx_path, &report.docx)?;
            write_bytes(&pdf_path, &report.pdf)?;
            println!("analysis: {}", analysis_path.display());
            println!("docx: {}", docx_path.display());
            println!("pdf: {}", pdf_path.display());
        }
        _ => return Err(usage()),
    }
    Ok(())
}

fn analyze_path(path: &Path) -> Result<sheetbrief::Analysis> {
    let bytes = std::fs::read(path)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workbook")
        .to_string();
    analyze_workbook(&bytes, file_name)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn usage() -> SheetBriefError {
    SheetBriefError::Usage(
        "sheetbrief analyze INPUT OUTPUT_JSON | report INPUT OUTPUT_DOCX | run INPUT OUTPUT_DIR"
            .to_string(),
    )
}
