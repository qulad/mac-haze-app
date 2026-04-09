use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

/// A line of output from a child process, tagged by stream.
#[derive(Debug, Clone)]
pub enum ProcessOutput {
    Stdout(String),
    Stderr(String),
}

/// Handle to a running process.
/// Dropping this does NOT kill the process — call `.kill()` explicitly.
pub struct ProcessHandle {
    pub stdin_tx: mpsc::Sender<String>,
    pub output_rx: mpsc::Receiver<ProcessOutput>,
    pub exit_rx: oneshot::Receiver<i32>,
    kill_tx: Option<oneshot::Sender<()>>,
}

impl ProcessHandle {
    pub fn new(
        stdin_tx: mpsc::Sender<String>,
        output_rx: mpsc::Receiver<ProcessOutput>,
        exit_rx: oneshot::Receiver<i32>,
        kill_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            stdin_tx,
            output_rx,
            exit_rx,
            kill_tx: Some(kill_tx),
        }
    }

    pub async fn write_line(&self, line: &str) -> Result<(), ProcessError> {
        self.stdin_tx
            .send(format!("{}\n", line))
            .await
            .map_err(|_| ProcessError::StdinClosed)
    }

    pub fn kill(mut self) -> Result<(), ProcessError> {
        self.kill_tx
            .take()
            .ok_or(ProcessError::AlreadyExited)?
            .send(())
            .map_err(|_| ProcessError::AlreadyExited)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Executable not in allowlist: {0}")]
    NotAllowed(String),

    #[error("Failed to spawn process: {0}")]
    SpawnFailed(String),

    #[error("Stdin channel closed")]
    StdinClosed,

    #[error("Process already exited")]
    AlreadyExited,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Security contract:
/// - MUST NOT use shell=true or /bin/sh -c
/// - Executable MUST be in allowed_executables() before spawn is called
/// - User-controlled input goes ONLY into `args`, NEVER into the executable path
/// - Each argument is passed as a separate Vec element — no shell interpolation
#[async_trait]
pub trait ProcessExecutor: Send + Sync + 'static {
    /// Spawn a process. Fails immediately if `executable` is not in the allowlist.
    async fn spawn(
        &self,
        executable: &Path,
        args: &[&str],
        env_vars: &[(&str, &str)],
        working_dir: Option<&Path>,
    ) -> Result<ProcessHandle, ProcessError>;

    /// Returns the set of absolute paths this executor will allow.
    fn allowed_executables(&self) -> &[PathBuf];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_handle_write_line_formats_newline() {
        let (tx, mut rx) = mpsc::channel(1);
        let (out_tx, out_rx) = mpsc::channel(1);
        let (exit_tx, exit_rx) = oneshot::channel();
        let (kill_tx, _kill_rx) = oneshot::channel();

        let handle = ProcessHandle::new(tx, out_rx, exit_rx, kill_tx);
        let _ = exit_tx.send(0);
        drop(out_tx);

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            handle.write_line("hello").await.unwrap();
            let received = rx.recv().await.unwrap();
            assert_eq!(received, "hello\n");
        });
    }
}
