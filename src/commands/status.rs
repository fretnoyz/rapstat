use anyhow::{Context, Result};
use colored::Colorize;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let path = cwd.join("STATUS.md");

    if !path.exists() {
        println!("{}", "No STATUS.md found. Run `rapstat scan` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    print_status(&content);
    Ok(())
}

fn print_status(content: &str) {
    // Split frontmatter from body at the second `---` delimiter.
    let mut parts = content.splitn(3, "---\n");
    let _ = parts.next(); // empty before first ---
    let frontmatter = parts.next().unwrap_or("").trim();
    let body = parts.next().unwrap_or("").trim();

    // Print frontmatter fields with color.
    println!("{}", "=== rapstat STATUS ===".bold().cyan());
    for line in frontmatter.lines() {
        if let Some((key, val)) = line.split_once(':') {
            println!("{}: {}", key.trim().bold(), val.trim().green());
        }
    }
    println!();

    // Print body sections, highlighting headers and flagging drift.
    let mut in_discrepancies = false;
    for line in body.lines() {
        if line.starts_with("## ") {
            in_discrepancies = false;
            println!("{}", line.bold().cyan());
        } else if line.contains("drift_detected") || line.contains("missing") {
            in_discrepancies = false;
            println!("{}", line.yellow());
        } else if line.contains("Discrepancies:") && !line.contains("none") {
            in_discrepancies = true;
            println!("{}", line.yellow());
        } else if line.starts_with("  - ") && in_discrepancies {
            // Discrepancy bullet under a flagged section.
            println!("{}", line.red());
        } else {
            if line.starts_with("- ") {
                in_discrepancies = false;
            }
            println!("{}", line);
        }
    }
}
