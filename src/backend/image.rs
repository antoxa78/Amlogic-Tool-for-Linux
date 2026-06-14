use crate::backend::types::{ImageInfo, PartitionInfo};
use std::path::Path;
use std::path::PathBuf;

fn tmpdir() -> PathBuf {
    let base = std::env::var("AML_FLASH_TOOL_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".cache/aml-flash-tool")
        });
    std::fs::create_dir_all(&base).ok();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    base.join(format!("extract-{suffix}"))
}

pub fn cleanup_stale_tmp() {
    let base = std::env::var("AML_FLASH_TOOL_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".cache/aml-flash-tool")
        });
    if base.exists() {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().map_or(false, |n| n.to_string_lossy().starts_with("extract-")) {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }
    }
}

fn packer_candidates(tools_dir: &Path) -> Vec<PathBuf> {
    let subdir = if cfg!(target_arch = "x86_64") {
        "linux-x86"
    } else {
        "linux-arm"
    };
    let mut candidates = Vec::new();
    let base = tools_dir.join(subdir);
    // Prefer the khadas variant (different build) if present
    let khadas = base.join("aml_image_v2_packer_khadas");
    if khadas.exists() {
        candidates.push(khadas);
    }
    candidates.push(base.join("aml_image_v2_packer"));
    candidates
}



fn parse_image_output(tmp_dir: &Path) -> Result<ImageInfo, String> {
    let cfg_path = tmp_dir.join("image.cfg");
    let cfg_content =
        std::fs::read_to_string(&cfg_path).map_err(|e| format!("Failed to read image.cfg: {e}"))?;

    let mut partitions = Vec::new();
    let mut soc = "gxl".to_string();

    for line in cfg_content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("main_type=\"PARTITION\"") {
            let file = extract_value(trimmed, "file=");
            let name = extract_value(trimmed, "sub_type=");
            let ftype = extract_value(trimmed, "file_type=");
            if let (Some(file), Some(name)) = (file, name) {
                partitions.push(PartitionInfo {
                    file,
                    name,
                    file_type: ftype.unwrap_or("normal".into()),
                });
            }
        }
    }

    // Read platform.conf for SoC type
    let plat_path = tmp_dir.join("platform.conf");
    if let Ok(content) = std::fs::read_to_string(&plat_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("soctype:") {
                soc = val.trim().to_lowercase();
                break;
            }
        }
    }

    Ok(ImageInfo {
        partitions,
        tmp_dir: tmp_dir.to_path_buf(),
        soc,
        secured: false,
    })
}

fn extract_value<'a>(line: &'a str, key: &str) -> Option<String> {
    let pos = line.find(key)?;
    let after_key = &line[pos + key.len()..];
    let start = after_key.find('"')? + 1;
    let end = after_key[start..].find('"')?;
    Some(after_key[start..start + end].to_string())
}

#[allow(dead_code)]
pub fn cleanup_tmp(tmp_dir: &Path) {
    let _ = std::fs::remove_dir_all(tmp_dir);
}

fn count_files(dir: &Path) -> u32 {
    std::fs::read_dir(dir).map(|e| e.count() as u32).unwrap_or(0)
}

fn latest_file_name(dir: &Path) -> Option<String> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    entries
        .iter()
        .filter_map(|e| {
            let path = e.path();
            let meta = path.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((modified, e.file_name().to_string_lossy().to_string()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, name)| name)
}

enum UnpackMsg {
    Progress { item: u32, name: String },
    Done(Result<ImageInfo, String>),
}

fn unpack_image_streaming<F>(img_path: &Path, tools_dir: &Path, send_progress: &F) -> Result<ImageInfo, String>
where F: Fn(u32, String),
{
    let candidates = packer_candidates(tools_dir);
    let tmp_dir = tmpdir();
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create tmp dir {tmp_dir:?}: {e}"))?;

    let mut last_err = String::new();
    for packer in &candidates {
        if !packer.exists() || !packer.is_file() {
            last_err = format!("Tool not found: {}", packer.display());
            continue;
        }

        let help_check = std::process::Command::new(packer).arg("--help").output();
        if let Err(e) = &help_check {
            last_err = format!("Cannot execute {}: {e}", packer.display());
            continue;
        }

        let mut child = match std::process::Command::new(packer)
            .arg("-d")
            .arg(img_path)
            .arg(&tmp_dir)
            .current_dir(tools_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                last_err = format!("Failed to spawn {}: {e}", packer.display());
                continue;
            }
        };

        // Poll the temp directory for new files
        let mut prev = count_files(&tmp_dir);
        loop {
            let is_dead = child.try_wait().ok().flatten().is_some();
            let cur = count_files(&tmp_dir);
            if cur != prev {
                prev = cur;
                let name = latest_file_name(&tmp_dir).unwrap_or_else(|| format!("file {cur}"));
                send_progress(cur, name);
            }
            if is_dead {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        let status = child.wait().unwrap_or_default();
        let cfg_path = tmp_dir.join("image.cfg");
        let exit_ok = status.success();
        let cfg_exists = cfg_path.exists();

        if exit_ok && cfg_exists {
            return parse_image_output(&tmp_dir);
        }

        let detail = if !exit_ok {
            format!("exit code: {:?}", status.code())
        } else {
            "no image.cfg found".into()
        };
        last_err = format!("{} failed. {detail}", packer.display());
    }

    Err(last_err)
}

pub fn unpack_image_async<F, P>(img_path: PathBuf, tools_dir: PathBuf, on_complete: F, on_progress: P)
where
    F: FnOnce(Result<ImageInfo, String>) + 'static,
    P: Fn(u32, String) + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<UnpackMsg>();

    let tx_done = tx.clone();
    std::thread::spawn(move || {
        let prog = move |item: u32, name: String| {
            let _ = tx.send(UnpackMsg::Progress { item, name });
        };
        let result = unpack_image_streaming(&img_path, &tools_dir, &prog);
        let _ = tx_done.send(UnpackMsg::Done(result));
    });

    let f = std::cell::RefCell::new(Some(on_complete));
    let p = std::cell::RefCell::new(Some(on_progress));
    let _ = glib::idle_add_local(move || {
        match rx.try_recv() {
            Ok(UnpackMsg::Progress { item, name }) => {
                if let Some(ref cb) = *p.borrow() {
                    cb(item, name);
                }
                glib::ControlFlow::Continue
            }
            Ok(UnpackMsg::Done(result)) => {
                if let Some(f) = f.borrow_mut().take() {
                    f(result);
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}
