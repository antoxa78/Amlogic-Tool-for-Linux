use chrono::Local;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;

/// Upper bound on lines kept in the on-screen log so very long burns do not
/// make the text view sluggish. The optional log file always gets everything.
const MAX_LINES: i32 = 5000;

pub struct LogPanel {
    pub text_view: gtk4::TextView,
    buffer: gtk4::TextBuffer,
    log_file: RefCell<Option<PathBuf>>,
}

impl LogPanel {
    pub fn new() -> Self {
        let buffer = gtk4::TextBuffer::new(None);
        // Colors are applied per theme via `update_tag_colors`.
        for name in ["time", "error", "success", "warning"] {
            buffer.tag_table().add(&gtk4::TextTag::new(Some(name)));
        }

        let text_view = gtk4::TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(12)
            .right_margin(12)
            .css_classes(["log-view"])
            .build();

        let panel = Self {
            text_view,
            buffer,
            log_file: RefCell::new(None),
        };
        panel.update_tag_colors(libadwaita::StyleManager::default().is_dark());
        panel
    }

    /// Re-tint the semantic tags for light or dark mode.
    pub fn update_tag_colors(&self, dark: bool) {
        let (err, ok, warn, dim) = if dark {
            ("#ff7b63", "#8ff0a4", "#f8e45c", "#9a9996")
        } else {
            ("#c01c28", "#1b8553", "#9c6e03", "#77767b")
        };
        let table = self.buffer.tag_table();
        for (name, color) in [("error", err), ("success", ok), ("warning", warn), ("time", dim)] {
            if let Some(tag) = table.lookup(name) {
                tag.set_foreground(Some(color));
            }
        }
    }

    pub fn append_log(&self, msg: &str) {
        let msg = strip_ansi(msg);
        let ts = Local::now().format("%H:%M:%S").to_string();

        let mut end = self.buffer.end_iter();
        self.buffer
            .insert_with_tags_by_name(&mut end, &format!("{ts}  "), &["time"]);
        match classify(&msg) {
            Some(tag) => self
                .buffer
                .insert_with_tags_by_name(&mut end, &format!("{msg}\n"), &[tag]),
            None => self.buffer.insert(&mut end, &format!("{msg}\n")),
        }

        // Trim the oldest lines once the buffer gets large.
        let extra = self.buffer.line_count() - MAX_LINES;
        if extra > 0 {
            let mut start = self.buffer.start_iter();
            if let Some(mut cut) = self.buffer.iter_at_line(extra) {
                self.buffer.delete(&mut start, &mut cut);
            }
        }

        self.scroll_to_end();

        if let Some(ref path) = *self.log_file.borrow() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
            {
                let _ = writeln!(file, "[{ts}] {msg}");
            }
        }
    }

    /// Keep the newest line visible. Done after layout (idle) through the
    /// scroll adjustment: scrolling straight away — before the window has its
    /// final size — could leave the view offset so the first line is cut off
    /// under the header. Only follows when the user is already at the bottom.
    fn scroll_to_end(&self) {
        let Some(adj) = self.text_view.vadjustment() else {
            return;
        };
        let at_bottom = adj.value() + adj.page_size() >= adj.upper() - 40.0;
        if !at_bottom {
            return;
        }
        glib::idle_add_local_once(move || {
            let overflow = adj.upper() - adj.page_size();
            adj.set_value(overflow.max(0.0));
        });
    }

    pub fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), false)
            .to_string()
    }

    pub fn set_log_file(&self, path: PathBuf) {
        *self.log_file.borrow_mut() = Some(path);
    }

    pub fn clear_log_file(&self) {
        *self.log_file.borrow_mut() = None;
    }

    pub fn clear(&self) {
        self.buffer.set_text("");
    }
}

fn classify(msg: &str) -> Option<&'static str> {
    let lower = msg.to_lowercase();
    if msg.contains("[KO]")
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("[stderr]")
    {
        Some("error")
    } else if msg.contains("[OK]") || lower.contains("completed") || lower.contains("success") {
        Some("success")
    } else if lower.contains("warn") || lower.contains("cancel") || lower.contains("stop") {
        Some("warning")
    } else {
        None
    }
}

/// Remove ANSI escape sequences (the burn scripts print colored output).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // CSI: parameters/intermediates until a final byte in @..~
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colors() {
        assert_eq!(strip_ansi("\x1b[0;32m[OK]\x1b[m done"), "[OK] done");
        assert_eq!(strip_ansi("plain\r"), "plain");
    }

    #[test]
    fn classifies() {
        assert_eq!(classify("ERROR: x"), Some("error"));
        assert_eq!(classify("Writing boot [OK]"), Some("success"));
        assert_eq!(classify("Loading image"), None);
    }
}
