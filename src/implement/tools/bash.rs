use std::process::Command;

use serde_json::json;

use crate::Structures::Message::tools::Imports::{AnyResult, JDict, JValue, Map};
use crate::Structures::Message::tools::{ArgumentsProperties, IsEquired, JValueType, tools_T};

pub struct Bash;

impl tools_T for Bash {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "command".into(),
            ArgumentsProperties::new(
                "command".into(),
                "要执行的 shell 命令".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "cwd".into(),
            ArgumentsProperties::new(
                "cwd".into(),
                "工作目录，默认当前目录".into(),
                IsEquired::NotRequired(json!(".")),
                JValueType::String,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少参数 command"))?;
        let cwd = args.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");

        let mut cmd = Command::new(if cfg!(windows) {
            "powershell.exe"
        } else {
            "bash"
        });
        if cfg!(windows) {
            // Windows 用 PowerShell：NoProfile/NonInteractive 避免交互提示，并把输出编码设为 UTF-8
            cmd.args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(format!(
                    "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; {}",
                    command
                ));
        } else {
            cmd.arg("-c").arg(command);
        }

        let output = cmd
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| anyhow::anyhow!("执行命令失败: {e}"))?;

        Ok(json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "status": output.status.code(),
            "success": output.status.success(),
        }))
    }

    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> String {
        "执行 shell 命令并返回 stdout、stderr 与退出码。可用于运行程序、查看目录结构等。".into()
    }
}
