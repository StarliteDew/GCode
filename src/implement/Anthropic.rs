use std::time::Duration;

use serde_json::json;

use crate::Errors::{MyError, MyResult};
use crate::Imports::JValue;
use crate::Structures::Conversation::Conversation;
use crate::Structures::Message::{Behaviour, Behaviour_E, Message, ToolUse};
use crate::Structures::Message::Role::ASSISTANT;
use crate::Structures::Message::tools::Structures::JValueType;
use crate::Structures::Trait::Provider_API_T;

/// Anthropic 工具类型映射（input_schema 使用 JSON Schema 类型名）
fn jvalue_type_to_anthropic(t: JValueType) -> &'static str {
    match t {
        JValueType::Number => "number",
        JValueType::Object => "object",
        JValueType::Bool => "boolean",
        JValueType::Array => "array",
        JValueType::String => "string",
        JValueType::Null => "null",
    }
}

/// 生成 Anthropic 格式的 tools 定义数组（JSON）
/// 与 OpenAI 不同：无 `type: "function"` 包装层，参数使用 `input_schema`
pub fn all_definitions() -> JValue {
    let ptr = crate::Structures::Message::tools::Registry::registry.read().unwrap();
    let mut tools = Vec::new();
    for t in ptr.dict.values() {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for (k, v) in t.arguments() {
            properties.insert(
                k.clone(),
                json!({
                    "type": jvalue_type_to_anthropic(v.Type),
                    "description": v.description,
                }),
            );
            if v.is_required.is_required() {
                required.push(json!(k));
            }
        }
        tools.push(json!({
            "name": t.name(),
            "description": t.description(),
            "input_schema": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        }));
    }
    JValue::Array(tools)
}

/// Anthropic Messages API Provider
#[derive(Debug)]
pub struct Anthropic {
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<u32>,
    thinking_budget_tokens: Option<u32>,
    tool_choice: String, // "auto" | "any" | 具体工具名
    timeout: Duration,
    stop_sequences: Vec<String>,
    extra_headers: Vec<(String, String)>,
}

impl Anthropic {
    /// 新建 Anthropic provider
    /// api_key  : API key
    /// base_url : API 基础地址，如 "https://api.anthropic.com/v1"
    #[allow(non_snake_case)]
    pub fn New(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            model: "claude-3-5-sonnet".to_string(),
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking_budget_tokens: None,
            tool_choice: "auto".to_string(),
            timeout: Duration::from_secs(120),
            stop_sequences: Vec::new(),
            extra_headers: Vec::new(),
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// max_tokens 在 Anthropic 中为必填项
    pub fn with_max_tokens(mut self, v: u32) -> Self {
        self.max_tokens = v;
        self
    }

    pub fn with_temperature(mut self, v: f64) -> Self {
        self.temperature = Some(v);
        self
    }

    pub fn with_top_p(mut self, v: f64) -> Self {
        self.top_p = Some(v);
        self
    }

    pub fn with_top_k(mut self, v: u32) -> Self {
        self.top_k = Some(v);
        self
    }

    /// 开启扩展思考（extended thinking），budget_tokens 为思考 token 预算
    pub fn with_thinking_budget(mut self, budget_tokens: u32) -> Self {
        self.thinking_budget_tokens = Some(budget_tokens);
        self
    }

    /// tool_choice：auto / any / 具体工具名
    pub fn with_tool_choice(mut self, v: String) -> Self {
        self.tool_choice = v;
        self
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// 追加一个停止序列
    pub fn with_stop_sequence(mut self, s: String) -> Self {
        self.stop_sequences.push(s);
        self
    }

    pub fn with_header(mut self, k: String, v: String) -> Self {
        self.extra_headers.push((k, v));
        self
    }
}

impl Provider_API_T for Anthropic {
    fn response_to_Message(&self, resp: JValue) -> MyResult<Message> {
        /*
        resp : api返回的response的json
        Anthropic 响应中 content 是一个内容块（Content Blocks）数组：
            - { "type": "text", "text": "..." }             -> Say
            - { "type": "thinking", "thinking": "..." }      -> Thinking
            - { "type": "tool_use", "id", "name", "input" }  -> Function_call_and_result(result = None)
         */
        let content = resp["content"].clone();
        if !content.is_array() {
            return Err(MyError::new(
                "UnrecognizedResponse",
                format!("未找到 content 数组，原始响应 = {}", resp),
            ));
        }

        let mut did = Vec::new();
        for block in content.as_array().unwrap() {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(t) = block["text"].as_str() {
                        if !t.trim().is_empty() {
                            did.push(Behaviour {
                                meta: JValue::Null,
                                behaviour: Behaviour_E::Say(t.to_string()),
                            });
                        }
                    }
                }
                Some("thinking") => {
                    if let Some(t) = block["thinking"].as_str() {
                        if !t.trim().is_empty() {
                            did.push(Behaviour {
                                meta: JValue::Null,
                                behaviour: Behaviour_E::Thinking(t.to_string()),
                            });
                        }
                    }
                }
                Some("redacted_thinking") => {
                    if let Some(t) = block["data"].as_str() {
                        if !t.trim().is_empty() {
                            did.push(Behaviour {
                                meta: JValue::Null,
                                behaviour: Behaviour_E::Thinking(t.to_string()),
                            });
                        }
                    }
                }
                Some("tool_use") => {
                    let function_name = block["name"]
                        .as_str()
                        .ok_or_else(|| {
                            MyError::new(
                                "ToolCallMalformed",
                                format!("tool_use 缺少 name：{block}"),
                            )
                        })?
                        .to_string();
                    let function_call_id = block["id"]
                        .as_str()
                        .ok_or_else(|| {
                            MyError::new(
                                "ToolCallMalformed",
                                format!("tool_use 缺少 id：{block}"),
                            )
                        })?
                        .to_string();
                    let arguments = block["input"]
                        .as_object()
                        .cloned()
                        .unwrap_or_default();
                    did.push(Behaviour {
                        meta: JValue::Null,
                        behaviour: Behaviour_E::Function_call_and_result(ToolUse {
                            function_name,
                            function_call_id,
                            arguments,
                            result: None, // 只接收到 tool_use，result 为 None
                        }),
                    });
                }
                _ => {
                    // 其他内容块（如 search_result）暂不支持，忽略
                }
            }
        }

        if did.is_empty() {
            return Err(MyError::new(
                "EmptyMessage",
                "AI 返回了空消息（既无文本也无工具调用）",
            ));
        }

        let mut meta = serde_json::Map::new();
        meta.insert("raw".into(), resp);

        Ok(Message {
            who: ASSISTANT, // 因为是 AI 返回的，Role 一直为 assistant
            did,
            meta: JValue::Object(meta),
        })
    }

    fn conversation_to_Json(&self, conversation: &Conversation) -> JValue {
        /*
        Anthropic 格式：
        - system 是独立顶层字段，不在 messages 数组中
        - messages 的 role 只有 user / assistant
        - content 是内容块数组
        - 工具调用：assistant 消息中为 tool_use 块，紧跟的 user 消息中为 tool_result 块

        返回结构：{ "system": "...", "messages": [...] }，供 request 使用
         */
        let mut system_parts = Vec::new();
        let mut messages = Vec::new();

        for msg in &conversation.conversation {
            let role = msg.who.json();
            let role_str = role.as_str().unwrap_or("");

            // system 消息单独提取到顶层 system 字段
            if role_str == "system" {
                for b in &msg.did {
                    if let Behaviour_E::Say(s) = &b.behaviour {
                        system_parts.push(s.clone());
                    }
                }
                continue;
            }

            let mut blocks: Vec<JValue> = Vec::new();
            let mut tool_results: Vec<JValue> = Vec::new();

            for b in &msg.did {
                match &b.behaviour {
                    Behaviour_E::Say(s) | Behaviour_E::Thinking(s) => {
                        // text 块必须位于 tool_use 块之前（保持 did 顺序即可）
                        blocks.push(json!({ "type": "text", "text": s }));
                    }
                    Behaviour_E::File_local(_) | Behaviour_E::File_clould(_) => {
                        // 文件/URL 内容：Anthropic 兼容层暂不转换，忽略
                    }
                    Behaviour_E::Function_call_and_result(ToolUse {
                        function_name,
                        function_call_id,
                        arguments,
                        result,
                    }) => {
                        // assistant 消息中的 tool_use 声明
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": function_call_id,
                            "name": function_name,
                            "input": arguments,
                        }));
                        if let Some(res) = result {
                            // 已执行的工具结果 -> 紧跟的 user 消息中的 tool_result 块
                            let content = match res {
                                Ok((v, w)) => match w {
                                    Some(w) => format!("{}\n(warning: {})", v, w),
                                    None => format!("{}", v),
                                },
                                Err(e) => format!("{}", e),
                            };
                            tool_results.push(json!({
                                "type": "tool_result",
                                "tool_use_id": function_call_id,
                                "content": content,
                            }));
                        }
                    }
                    Behaviour_E::Other(_) => {
                        // 未来拓展用，目前不支持
                    }
                }
            }

            if blocks.is_empty() && tool_results.is_empty() {
                continue;
            }

            messages.push(json!({
                "role": role,
                "content": blocks,
            }));

            // tool_result 必须紧跟对应的 tool_use 所在消息
            if !tool_results.is_empty() {
                messages.push(json!({
                    "role": "user",
                    "content": tool_results,
                }));
            }
        }

        let mut result = json!({
            "messages": messages,
        });
        if !system_parts.is_empty() {
            result["system"] = json!(system_parts.join("\n"));
        }
        result
    }

    fn request(&self, conversation: &Conversation) -> reqwest::Request {
        let data = self.conversation_to_Json(conversation);
        // 请求体：Anthropic Messages API 格式
        let mut body = json!({
            "model": &self.model,
            "max_tokens": self.max_tokens,
            "messages": data["messages"],
        });
        if let Some(system) = data["system"].as_str() {
            if !system.is_empty() {
                body["system"] = json!(system);
            }
        }
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(p) = self.top_p {
            body["top_p"] = json!(p);
        }
        if let Some(k) = self.top_k {
            body["top_k"] = json!(k);
        }
        if let Some(budget) = self.thinking_budget_tokens {
            body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
        }
        if !self.stop_sequences.is_empty() {
            body["stop_sequences"] = json!(self.stop_sequences);
        }
        if !self.tool_choice.is_empty() {
            body["tool_choice"] = match self.tool_choice.as_str() {
                "auto" => json!({ "type": "auto" }),
                "any" | "required" => json!({ "type": "any" }),
                name => json!({ "type": "tool", "name": name }),
            };
        }
        // 带上全局注册的工具定义，AI 才知道有哪些工具可调用
        let tools = all_definitions();
        if !tools.as_array().map_or(true, |a| a.is_empty()) {
            body["tools"] = tools;
        }

        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let mut req = reqwest::Request::new(
            reqwest::Method::POST,
            reqwest::Url::parse(&url).expect("base_url 不是合法 URL"),
        );
        // 请求头：Anthropic 标准认证
        req.headers_mut().insert(
            "x-api-key",
            self.api_key.parse().unwrap(),
        );
        req.headers_mut().insert(
            "anthropic-version",
            "2023-06-01".parse().unwrap(),
        );
        req.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        for (k, v) in &self.extra_headers {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            let value = reqwest::header::HeaderValue::from_str(v).unwrap();
            req.headers_mut().insert(name, value);
        }
        *req.timeout_mut() = Some(self.timeout);
        *req.body_mut() = Some(body.to_string().into());
        req
    }
}
