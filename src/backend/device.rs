use std::path::Path;
use std::path::PathBuf;

pub fn scan_devices(tools_dir: &Path) -> Vec<String> {
    let update = if cfg!(target_arch = "x86_64") {
        tools_dir.join("linux-x86/update")
    } else {
        tools_dir.join("linux-arm/update")
    };

    let output = std::process::Command::new(&update)
        .arg("scan")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_default();

    let mut devices = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip lines that indicate no device or are informational headers
        if trimmed.starts_with("No") || trimmed.starts_with("no") {
            continue;
        }
        // Only count lines that contain numeric hub/port info
        if trimmed.contains("Hub") || trimmed.chars().any(|c| c.is_ascii_digit()) {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if let Some(first) = parts.first() {
                if *first != "Scan" && *first != "Device" && *first != "device" {
                    devices.push(format!("{first} | Connected | 0%"));
                }
            }
        }
    }

    if devices.is_empty() {
        if let Ok(lsusb) = std::process::Command::new("lsusb")
            .output().map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        {
            for line in lsusb.lines() {
                if line.contains("Amlogic") || line.contains("1b8e") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 6 {
                        let id = format!("{}:{}", parts[1], parts[3].trim_end_matches(':'));
                        let name = parts
                            .get(6..)
                            .map(|p| p.join(" "))
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| "Amlogic device".into());
                        devices.push(format!("{id} ({name}) | Connected | 0%"));
                    }
                }
            }
        }
    }

    devices
}

pub fn scan_devices_background(
    tools_dir: PathBuf,
    on_result: impl FnOnce(Vec<String>) + 'static,
) {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();
    std::thread::spawn(move || {
        let result = scan_devices(&tools_dir);
        let _ = tx.send(result);
    });
    let f = std::cell::RefCell::new(Some(on_result));
    let _ = glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        match rx.try_recv() {
            Ok(devices) => {
                if let Some(f) = f.borrow_mut().take() {
                    f(devices);
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}
