use std::path::PathBuf;
use uuid::Uuid;

/// Configuration for managing large tool output spills to disk.
#[derive(Debug, Clone)]
pub struct SpillConfig {
    /// Maximum allowed output size in bytes before spilling to disk (default 32KB).
    pub max_bytes: usize,
    /// Directory where full spill files are stored.
    pub spill_dir: PathBuf,
}

impl Default for SpillConfig {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024,
            spill_dir: std::env::temp_dir().join("dsh-spill"),
        }
    }
}

/// Helper that inspects output text and writes to a spill file if it exceeds max_bytes.
pub fn process_output_spill(output: &str, config: &SpillConfig) -> std::io::Result<String> {
    if output.len() <= config.max_bytes {
        return Ok(output.to_string());
    }

    std::fs::create_dir_all(&config.spill_dir)?;

    let file_id = Uuid::new_v4().simple();
    let spill_path = config.spill_dir.join(format!("spill_{}.txt", file_id));
    std::fs::write(&spill_path, output)?;

    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    let head_lines = 25.min(total_lines);
    let tail_lines = 25.min(total_lines.saturating_sub(head_lines));

    let mut preview = String::new();
    preview.push_str(&format!(
        "[Tool output exceeded {} bytes ({} lines). Full raw output preserved in {}]\n\n--- Output Preview (First {} lines) ---\n",
        config.max_bytes, total_lines, spill_path.display(), head_lines
    ));

    for line in &lines[..head_lines] {
        preview.push_str(line);
        preview.push('\n');
    }

    if total_lines > head_lines + tail_lines {
        preview.push_str(&format!(
            "\n... [Truncated {} lines. Read {} for full content] ...\n\n",
            total_lines - head_lines - tail_lines,
            spill_path.display()
        ));
    }

    if tail_lines > 0 {
        preview.push_str(&format!("--- Output Preview (Last {} lines) ---\n", tail_lines));
        for line in &lines[total_lines - tail_lines..] {
            preview.push_str(line);
            preview.push('\n');
        }
    }

    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spill_threshold() {
        let config = SpillConfig {
            max_bytes: 100,
            spill_dir: std::env::temp_dir().join("dsh-test-spill"),
        };

        let short_text = "hello world";
        assert_eq!(process_output_spill(short_text, &config).unwrap(), short_text);

        let long_text = "line\n".repeat(50);
        let spilled = process_output_spill(&long_text, &config).unwrap();
        assert!(spilled.contains("[Tool output exceeded 100 bytes"));
        assert!(spilled.contains("Full raw output preserved in"));
    }
}

