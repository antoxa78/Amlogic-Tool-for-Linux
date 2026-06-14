pub fn parse_progress(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains("......") || trimmed.starts_with("..") {
        return None;
    }

    if let Some(pct) = trimmed.strip_suffix('%') {
        if let Ok(val) = pct.trim().parse::<f64>() {
            return Some(val);
        }
    }

    if trimmed.contains("[OK]") {
        return Some(100.0);
    }

    if trimmed.contains("[KO]") || trimmed.contains("ERR") || trimmed.contains("error") {
        return None;
    }

    None
}

#[allow(dead_code)]
pub fn classify_line(line: &str) -> LineType {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return LineType::Empty;
    }
    if trimmed.contains("[OK]") {
        return LineType::Success;
    }
    if trimmed.contains("[KO]") || trimmed.contains("ERR") {
        return LineType::Error;
    }
    if trimmed.starts_with("..") || trimmed.contains("......") {
        return LineType::ProgressDots;
    }
    LineType::Info
}

#[allow(dead_code)]
pub enum LineType {
    Empty,
    Success,
    Error,
    ProgressDots,
    Info,
}
