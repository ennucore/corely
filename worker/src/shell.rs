use anyhow::Result;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;

pub async fn exec(command: &str, timeout_ms: u64, cwd: Option<&str>) -> Result<Value> {
    debug!("Executing command: {}", command);

    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };

    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut cmd = Command::new(shell);
    cmd.arg(shell_arg).arg(command);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let timeout_duration = Duration::from_millis(timeout_ms);

    let result = timeout(timeout_duration, async {
        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok::<_, anyhow::Error>(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        }))
    })
    .await;

    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(json!({
            "stdout": "",
            "stderr": "Command timed out",
            "exit_code": -1,
            "timed_out": true,
        })),
    }
}

pub async fn exec_stream(
    command: &str,
    cwd: Option<&str>,
    on_output: impl Fn(&str, &str),
) -> Result<Value> {
    debug!("Executing streaming command: {}", command);

    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };

    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut cmd = Command::new(shell);
    cmd.arg(shell_arg).arg(command);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut all_stdout = String::new();
    let mut all_stderr = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        on_output("stdout", &line);
                        all_stdout.push_str(&line);
                        all_stdout.push('\n');
                    }
                    Ok(None) => break,
                    Err(e) => {
                        on_output("stderr", &format!("Error reading stdout: {}", e));
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        on_output("stderr", &line);
                        all_stderr.push_str(&line);
                        all_stderr.push('\n');
                    }
                    Ok(None) => {}
                    Err(e) => {
                        on_output("stderr", &format!("Error reading stderr: {}", e));
                    }
                }
            }
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);

    Ok(json!({
        "stdout": all_stdout,
        "stderr": all_stderr,
        "exit_code": exit_code,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exec_simple_command() {
        let result = exec("echo hello", 5000, None).await.unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_exec_with_stderr() {
        let result = exec("echo error >&2", 5000, None).await.unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stderr"].as_str().unwrap().contains("error"));
    }

    #[tokio::test]
    async fn test_exec_nonzero_exit() {
        let result = exec("exit 42", 5000, None).await.unwrap();

        assert_eq!(result["exit_code"], 42);
    }

    #[tokio::test]
    async fn test_exec_with_cwd() {
        let result = exec("pwd", 5000, Some("/tmp")).await.unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("/tmp") ||
                result["stdout"].as_str().unwrap().contains("private/tmp"));
    }

    #[tokio::test]
    async fn test_exec_timeout() {
        let result = exec("sleep 10", 100, None).await.unwrap();

        assert!(result.get("timed_out").is_some());
        assert_eq!(result["timed_out"], true);
    }
}
