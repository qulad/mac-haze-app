use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use super::executor::{ProcessError, ProcessExecutor, ProcessHandle, ProcessOutput};

pub struct TokioProcessExecutor {
    allowed: Vec<PathBuf>,
}

impl TokioProcessExecutor {
    pub fn new(allowed_executables: Vec<PathBuf>) -> Self {
        Self {
            allowed: allowed_executables,
        }
    }
}

#[async_trait]
impl ProcessExecutor for TokioProcessExecutor {
    fn allowed_executables(&self) -> &[PathBuf] {
        &self.allowed
    }

    async fn spawn(
        &self,
        executable: &Path,
        args: &[&str],
        env_vars: &[(&str, &str)],
        working_dir: Option<&Path>,
    ) -> Result<ProcessHandle, ProcessError> {
        // SECURITY: canonicalize + allowlist check before any OS call
        let canonical = executable
            .canonicalize()
            .map_err(|e| ProcessError::SpawnFailed(e.to_string()))?;

        if !self.allowed.contains(&canonical) {
            return Err(ProcessError::NotAllowed(canonical.display().to_string()));
        }

        let mut cmd = Command::new(&canonical);
        cmd.args(args);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        for (k, v) in env_vars {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ProcessError::SpawnFailed(e.to_string()))?;

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);
        let (output_tx, output_rx) = mpsc::channel::<ProcessOutput>(256);
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let (exit_done_tx, exit_rx) = oneshot::channel::<i32>();

        // Stdin writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
            }
        });

        // Stdout reader task
        let out_tx = output_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if out_tx.send(ProcessOutput::Stdout(line)).await.is_err() {
                    break;
                }
            }
        });

        // Stderr reader task
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if output_tx.send(ProcessOutput::Stderr(line)).await.is_err() {
                    break;
                }
            }
        });

        // Wait/kill task
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    let code = status
                        .ok()
                        .and_then(|s| s.code())
                        .unwrap_or(-1);
                    let _ = exit_done_tx.send(code);
                }
                _ = kill_rx => {
                    let _ = child.kill().await;
                    let _ = exit_done_tx.send(-1);
                }
            }
        });

        Ok(ProcessHandle::new(stdin_tx, output_rx, exit_rx, kill_tx))
    }
}
