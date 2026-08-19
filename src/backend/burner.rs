use crate::backend::types::BurnConfig;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub enum BurnEvent {
    Line(String, bool),
    Exited(i32),
    Error(String),
}

pub struct BurnProcess {
    child: Child,
    rx: Receiver<BurnEvent>,
    done: bool,
}

impl BurnProcess {
    pub fn start(config: &BurnConfig, script_dir: &Path) -> Result<Self, String> {
        let img_path = config
            .image_path
            .as_ref()
            .ok_or("No image path configured")?;

        let (child, stdout, stderr) = if config.adnl {
            let script = script_dir.join("tools").join("adnl_flash_tool.sh");
            if !script.exists() {
                return Err(format!(
                    "adnl_flash_tool.sh not found: {}",
                    script.display()
                ));
            }

            let mut cmd = Command::new("stdbuf");
            cmd.args(["-oL", "bash", &script.to_string_lossy()])
                .arg(format!("--img={}", img_path.display()))
                .arg(format!("--parts={}", config.parts))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if config.wipe {
                cmd.arg("--wipe");
            }
            if config.erase_bootloader {
                cmd.arg("--destroy");
            }
            cmd.arg(format!("--reset={}", if config.reset { "y" } else { "n" }));
            cmd.arg(format!("--soc={}", config.soc));

            #[cfg(unix)]
            cmd.process_group(0);

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn adnl_flash_tool.sh: {e}"))?;

            let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
            let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
            (child, stdout, stderr)
        } else {
            const SUPPORTED_SOCS: &[&str] = &["m8", "gxl", "axg", "txlx", "g12a"];
            if !SUPPORTED_SOCS.contains(&config.soc.as_str()) {
                return Err(format!(
                    "SoC '{}' is not supported by the classic burn engine (supported: m8, gxl, axg, txlx, g12a)",
                    config.soc
                ));
            }

            let script = script_dir.join("tools").join("aml_flash_tool.sh");
            if !script.exists() {
                return Err(format!("aml_flash_tool.sh not found: {}", script.display()));
            }

            let mut cmd = Command::new("stdbuf");
            cmd.args(["-oL", "bash", &script.to_string_lossy()])
                .arg(format!("--img={}", img_path.display()))
                .arg(format!("--parts={}", config.parts))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if config.wipe {
                cmd.arg("--wipe");
            }
            if config.erase_bootloader {
                cmd.arg("--destroy");
            }
            cmd.arg(format!("--reset={}", if config.reset { "y" } else { "n" }));
            cmd.arg(format!("--soc={}", config.soc));
            if let Some(pw) = &config.password_path {
                cmd.arg(format!("--password={}", pw.display()));
            }
            if let Some(ef) = &config.efuse_path {
                cmd.arg(format!("--efuse-file={}", ef.display()));
            }

            #[cfg(unix)]
            cmd.process_group(0);

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn aml_flash_tool.sh: {e}"))?;

            let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
            let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
            (child, stdout, stderr)
        };

        let (tx, rx) = mpsc::channel();

        let tx_stdout = tx.clone();
        std::thread::Builder::new()
            .name("burn-stdout".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(l) => {
                            if tx_stdout.send(BurnEvent::Line(l, false)).is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx_stdout.send(BurnEvent::Error(e.to_string()));
                            return;
                        }
                    }
                }
            })
            .expect("spawn stdout reader");

        std::thread::Builder::new()
            .name("burn-stderr".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    match line {
                        Ok(l) => {
                            if tx.send(BurnEvent::Line(l, true)).is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(BurnEvent::Error(e.to_string()));
                            return;
                        }
                    }
                }
            })
            .expect("spawn stderr reader");

        Ok(Self {
            child,
            rx,
            done: false,
        })
    }

    pub fn read_line(&mut self) -> Option<BurnEvent> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => {
                if !self.done {
                    match self.child.try_wait() {
                        Ok(Some(status)) => {
                            self.done = true;
                            return Some(BurnEvent::Exited(status.code().unwrap_or(-1)));
                        }
                        Ok(None) => {}
                        Err(e) => return Some(BurnEvent::Error(e.to_string())),
                    }
                }
                None
            }
            Err(TryRecvError::Disconnected) => {
                if !self.done {
                    if let Ok(Some(status)) = self.child.try_wait() {
                        self.done = true;
                        return Some(BurnEvent::Exited(status.code().unwrap_or(-1)));
                    }
                    self.done = true;
                    return Some(BurnEvent::Exited(-1));
                }
                None
            }
        }
    }

    pub fn kill(&mut self) {
        #[cfg(unix)]
        if self.child.try_wait().ok().flatten().is_none() {
            let pid = self.child.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        let _ = self.child.kill();
    }
}
