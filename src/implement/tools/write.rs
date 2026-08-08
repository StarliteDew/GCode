use std::fs;

use serde_json::json;

use crate::Structures::Message::tools::Imports::{AnyResult, JDict, JValue, Map};
use crate::Structures::Message::tools::{ArgumentsProperties, IsEquired, JValueType, tools_T};

pub struct WriteFile;

impl tools_T for WriteFile {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "path".into(),
            ArgumentsProperties::new(
                "path".into(),
                "要写入的文件路径".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "content".into(),
            ArgumentsProperties::new(
                "content".into(),
                "要写入的文本内容".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "append".into(),
            ArgumentsProperties::new(
                "append".into(),
                "是否追加到文件末尾，默认 false（覆盖）".into(),
                IsEquired::NotRequired(json!(false)),
                JValueType::Bool,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少参数 path"))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少参数 content"))?;
        let append = args
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if append {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| anyhow::anyhow!("打开文件 {path} 失败: {e}"))?;
            f.write_all(content.as_bytes())
                .map_err(|e| anyhow::anyhow!("写入文件 {path} 失败: {e}"))?;
        } else {
            fs::write(path, content)
                .map_err(|e| anyhow::anyhow!("写入文件 {path} 失败: {e}"))?;
        }

        Ok(json!({
            "path": path,
            "append": append,
            "bytes": content.len(),
        }))
    }

    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> String {
        "向本地文件写入或追加文本内容。".into()
    }
}
