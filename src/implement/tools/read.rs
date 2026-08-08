use std::fs;

use serde_json::json;

use crate::Structures::Message::tools::Imports::{AnyResult, JDict, JValue, Map};
use crate::Structures::Message::tools::{ArgumentsProperties, IsEquired, JValueType, tools_T};

pub struct ReadFile;

impl tools_T for ReadFile {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "path".into(),
            ArgumentsProperties::new(
                "path".into(),
                "要读取的文件路径".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "offset".into(),
            ArgumentsProperties::new(
                "offset".into(),
                "起始行号（从 1 开始），默认 0".into(),
                IsEquired::NotRequired(json!(0)),
                JValueType::Number,
            ),
        );
        m.insert(
            "limit".into(),
            ArgumentsProperties::new(
                "limit".into(),
                "最多读取多少行，默认 2000".into(),
                IsEquired::NotRequired(json!(2000)),
                JValueType::Number,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少参数 path"))?;
        let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0);
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(2000);

        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取文件 {path} 失败: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        let start = offset.max(1) as usize - 1;
        let end = (start + limit.max(0) as usize).min(lines.len());

        Ok(json!({
            "path": path,
            "lines": lines[start..end].join("\n"),
            "line_count": lines.len(),
        }))
    }

    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> String {
        "读取本地文本文件，可指定起始行与读取行数。".into()
    }
}
