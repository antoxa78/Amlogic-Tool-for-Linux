use chrono::Local;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::io::Write;

pub struct LogPanel {
    pub text_view: gtk4::TextView,
    buffer: gtk4::TextBuffer,
    log_file: std::cell::RefCell<Option<PathBuf>>,
}

impl LogPanel {
    pub fn new() -> Self {
        let buffer = gtk4::TextBuffer::new(None);
        let text_view = gtk4::TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .monospace(true)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .css_classes(["log-view"])
            .build();

        Self {
            text_view,
            buffer,
            log_file: std::cell::RefCell::new(None),
        }
    }

    pub fn append_log(&self, msg: &str) {
        let ts = Local::now().format("%H:%M:%S").to_string();
        let line = format!("[{}] {}\n", ts, msg);
        let mut end = self.buffer.end_iter();
        self.buffer.insert(&mut end, &line);
        self.text_view.scroll_to_iter(&mut end, 0.0, false, 0.0, 0.0);

        if let Some(ref path) = *self.log_file.borrow() {
            if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
                let _ = writeln!(file, "{line}");
            }
        }
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
