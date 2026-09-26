//! Source size gate: no new source file may exceed SOURCE_LIMIT lines.
//! Run: cargo run --bin check-source-size
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SOURCE_LIMIT: usize = 300;
/// Existing debt is explicit and finite. New files must meet the limit; removing
/// an entry is the only way to retire debt, so the check never quietly regresses.
const GRANDFATHERED: &[&str] = &[];
const SOURCE_EXTENSIONS: &[&str] = &[
    ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".vue", ".rs", ".inc",
];
const IGNORED: &[&str] = &[
    ".agent",
    ".agents",
    ".dev",
    ".git",
    ".tmp",
    "build",
    "coverage",
    "dist",
    "dist-electron",
    "node_modules",
    "release",
    "target",
    "vendor",
];

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !IGNORED.contains(&name.to_string_lossy().as_ref()) {
                collect(&path, files);
            }
        } else if SOURCE_EXTENSIONS
            .iter()
            .any(|e| entry.file_name().to_string_lossy().ends_with(e))
        {
            files.push(path);
        }
    }
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect(&root, &mut files);

    let mut violations = Vec::new();
    let mut debt = Vec::new();
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let lines = line_count(&text);
        if lines <= SOURCE_LIMIT {
            continue;
        }
        let label = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        if GRANDFATHERED.contains(&label.as_str()) {
            debt.push(format!("{label}: {lines} lines"));
        } else {
            violations.push(format!("{label}: {lines} lines (max {SOURCE_LIMIT})"));
        }
    }

    if !violations.is_empty() {
        eprintln!("source size check failed ({} file(s))", violations.len());
        violations.sort();
        for v in &violations {
            eprintln!("{v}");
        }
        return ExitCode::FAILURE;
    }

    println!(
        "source size check passed ({} grandfathered file(s) over {SOURCE_LIMIT} lines)",
        debt.len()
    );
    if !debt.is_empty() {
        debt.sort();
        println!("baseline debt:");
        for d in &debt {
            println!("{d}");
        }
    }
    ExitCode::SUCCESS
}
