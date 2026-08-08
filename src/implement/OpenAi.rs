use std::time::Duration;
use crate::Structures::Trait::Provider_API_T;
use crate::Structures::Conversation::Conversation;
use crate::Structures::Message::{Behaviour, Behaviour_E, Message, ToolUse};
use crate::Structures::Message::Role::ASSISTANT;
use crate::Structures::Message::tools::Registry;
use crate::Imports::JValue;
use crate::Errors::{MyError, MyResult};
use crate::Structures::Message::tools::Structures::JValueType;

use serde_json::json;


/// OpenAI 工具类型映射
fn jvalue_type_to_openai(t : JValueType) -> &'static str {
    match t {
        JValueType::Number => "number",
        JValueType::Object => "object",
        JValueType::Bool => "boolean",
        JValueType::Array => "array",
        JValueType::String => "string",
        JValueType::Null => "null",
    }
}

/// 生成 OpenAI 格式的 tools 定义数组（JSON）
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
                    "type": jvalue_type_to_openai(v.Type),
                    "description": v.description,
                }),
            );
            if v.is_required.is_required() {
                required.push(json!(k));
            }
        }
        tools.push(json!({
            "type": "function",
            "function": {
                "name": t.name(),
                "description": t.description(),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required,
                }
            }
        }));
    }
    JValue::Array(tools)
}


/// OpenAI 兼容 Chat Completions Provider
#[derive(Debug)]
pub struct OpenAI {
    api_key: String,
    base_url: String,
    model: String,
    thinking: String,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    tool_choice: String, // "auto" | "none" | "required" | 具体工具名
    timeout: Duration,
    top_p: Option<f64>,
    presence_penalty: Option<f64>,
    frequency_penalty: Option<f64>,
    user: Option<String>,
    extra_headers: Vec<(String, String)>,
}

impl OpenAI {
    /// 新建 OpenAI provider
    /// api_key  : API key
    /// base_url : API 基础地址，如 "https://api.openai.com/v1"
    #[allow(non_snake_case)]
    pub fn New(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            model: "gpt-4o-mini".to_string(),
            thinking: "high".to_string(),
            temperature: None,
            max_tokens: None,
            tool_choice: "auto".to_string(),
            timeout: Duration::from_secs(120),
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            extra_headers: Vec::new(),
        }
    }

    /// 覆盖默认模型，如 DeepSeek 用 "deepseek-chat"
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// 覆盖思考强度：low / medium / high
    pub fn with_thinking(mut self, level: String) -> Self {
        self.thinking = level;
        self
    }

    pub fn with_temperature(mut self, v: f64) -> Self {
        self.temperature = Some(v);
        self
    }

    pub fn with_max_tokens(mut self, v: u32) -> Self {
        self.max_tokens = Some(v);
        self
    }

    /// tool_choice：auto / none / required / 具体工具名
    pub fn with_tool_choice(mut self, v: String) -> Self {
        self.tool_choice = v;
        self
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    pub fn with_top_p(mut self, v: f64) -> Self {
        self.top_p = Some(v);
        self
    }

    pub fn with_presence_penalty(mut self, v: f64) -> Self {
        self.presence_penalty = Some(v);
        self
    }

    pub fn with_frequency_penalty(mut self, v: f64) -> Self {
        self.frequency_penalty = Some(v);
        self
    }

    pub fn with_user(mut self, v: String) -> Self {
        self.user = Some(v);
        self
    }

    pub fn with_header(mut self, k: String, v: String) -> Self {
        self.extra_headers.push((k, v));
        self
    }

    /// 序列化：把 OpenAI 配置转成 json（仅 api 对象，无 type 包装）
    /// timeout 以秒数（整数）表示，extra_headers 以 {name: value} 对象表示
    pub fn to_json(&self) -> JValue {
        let mut body = json!({
            "api_key": self.api_key,
            "base_url": self.base_url,
            "model": self.model,
            "thinking": self.thinking,
            "tool_choice": self.tool_choice,
            "timeout": self.timeout.as_secs(),
        });
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = self.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if let Some(t) = self.top_p {
            body["top_p"] = json!(t);
        }
        if let Some(p) = self.presence_penalty {
            body["presence_penalty"] = json!(p);
        }
        if let Some(f) = self.frequency_penalty {
            body["frequency_penalty"] = json!(f);
        }
        if let Some(u) = &self.user {
            body["user"] = json!(u);
        }
        if !self.extra_headers.is_empty() {
            let mut headers = serde_json::Map::new();
            for (k, v) in &self.extra_headers {
                headers.insert(k.clone(), JValue::String(v.clone()));
            }
            body["extra_headers"] = JValue::Object(headers);
        }
        body
    }
}

impl Provider_API_T for OpenAI {
    fn response_to_Message(&self, resp: JValue) -> MyResult<Message> {
        /*
        resp : api返回的response的json
        因为是ai返回的，所以Role一直为assistant，

        若ai返回的是文本
            1. 提取<thinking>标签(如果有的话)
            2. 提取正文
        若返回的是function_call
            1. 使用ToolUse的result = None
        ai 不会返回其他的格式，若遇到，则panic
         */
        let message = resp["choices"][0]["message"].clone();
        if message.is_null() {
            return Err(MyError::new(
                "UnrecognizedResponse",
                format!("未找到 choices[0].message，原始响应 = {}", resp),
            ));
        }

        let mut did = Vec::new();

        // 厂商把思考过程放在独立的 reasoning_content / reasoning 字段里（如 DeepSeek、Kimi）
        for field in ["reasoning_content", "reasoning"] {
            if let Some(r) = message[field].as_str() {
                if !r.trim().is_empty() {
                    did.push(Behaviour {
                        meta: JValue::Null,
                        behaviour: Behaviour_E::Thinking(r.to_string()),
                    });
                    break;
                }
            }
        }

        // 若 AI 返回的是文本：1. 提取 <thinking> 标签  2. 提取正文
        if let Some(content) = message["content"].as_str() {
            match extract_thinking(content) {
                Some((thinking, rest)) => {
                    did.push(Behaviour {
                        meta: JValue::Null,
                        behaviour: Behaviour_E::Thinking(thinking),
                    });
                    if !rest.trim().is_empty() {
                        did.push(Behaviour {
                            meta: JValue::Null,
                            behaviour: Behaviour_E::Say(rest),
                        });
                    }
                }
                None => {
                    if !content.trim().is_empty() {
                        did.push(Behaviour {
                            meta: JValue::Null,
                            behaviour: Behaviour_E::Say(content.to_string()),
                        });
                    }
                }
            }
        }

        // 若 AI 返回的是 function_call：1. 使用 ToolUse 的 result = None
        if let Some(tool_calls) = message["tool_calls"].as_array() {
            for tc in tool_calls {
                let function_name = tc["function"]["name"]
                    .as_str()
                    .ok_or_else(|| {
                        MyError::new("ToolCallMalformed", format!("tool_call 缺少 function.name：{tc}"))
                    })?
                    .to_string();
                let function_call_id = tc["id"]
                    .as_str()
                    .ok_or_else(|| {
                        MyError::new("ToolCallMalformed", format!("tool_call 缺少 id：{tc}"))
                    })?
                    .to_string();
                let args_str = tc["function"]["arguments"].as_str().ok_or_else(|| {
                    MyError::new(
                        "ToolCallMalformed",
                        format!("tool_call 缺少 function.arguments：{tc}"),
                    )
                })?;
                let arguments: serde_json::Map<String, JValue> = serde_json::from_str(args_str)
                    .map_err(|e| {
                        MyError::new(
                            "ToolCallMalformed",
                            format!("tool_call arguments 不是合法 JSON：{e}"),
                        )
                    })?;
                did.push(Behaviour {
                    meta: JValue::Null,
                    behaviour: Behaviour_E::Function_call_and_result(ToolUse {
                        function_name,
                        function_call_id,
                        arguments,
                        result: None, // 只接收到 function_call，result 为 None
                    }),
                });
            }
        }

        // AI 不会返回其他格式，若空消息则报错
        if did.is_empty() {
            return Err(MyError::new(
                "EmptyMessage",
                "AI 返回了空消息（既无文本也无工具调用）",
            ));
        }

        // meta 习惯上是 dict，raw 字段存储原生返回
        let mut meta = serde_json::Map::new();
        meta.insert("raw".into(), resp);

        Ok(Message {
            who: ASSISTANT, // 因为是 AI 返回的，Role 一直为 assistant
            did,
            meta: JValue::Object(meta),
        })
    }

    fn conversation_to_Json(&self, conversation: &Conversation) -> JValue {
        // 把框架可读的 Conversation 转成 OpenAI Chat Completions 的 messages 数组
        let mut messages = Vec::new();
        for msg in &conversation.conversation {
            let role = msg.who.json();
            let role_is_tool = role.as_str() == Some("tool");

            let mut content = String::new();
            let mut tool_calls: Vec<JValue> = Vec::new();
            let mut tool_results: Vec<(String, String)> = Vec::new();

            for b in &msg.did {
                match &b.behaviour {
                    Behaviour_E::Say(s) | Behaviour_E::Thinking(s) => {
                        // Thinking 在 OpenAI 兼容层也并入 content 发送
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(s);
                    }
                    Behaviour_E::File_local(_) | Behaviour_E::File_clould(_) => {
                        // 文件/URL 内容：OpenAI 兼容层暂不转换，忽略
                    }
                    Behaviour_E::Function_call_and_result(ToolUse {
                        function_name,
                        function_call_id,
                        arguments,
                        result,
                    }) => {
                        // 无论是否已执行，都保留 assistant 的 tool_calls 声明，
                        // 这样已执行的 fc 结果可以紧跟其后的 tool 角色消息发回模型
                        tool_calls.push(json!({
                            "id": function_call_id,
                            "type": "function",
                            "function": {
                                "name": function_name,
                                "arguments": serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string())
                            }
                        }));
                        if let Some(res) = result {
                            // 工具执行结果
                            let c = crate::Structures::Message::tools::Expection::Warning_to_JValue(res).to_string();

                            // let c = match res {
                            //     Ok((v, w)) => match w {
                            //         Some(w) => format!("{}\n(warning : {})", v, w),
                            //         None => format!("{}", v),
                            //     },
                            //     Err(e) => format!("{}", e),
                            // };
                            tool_results.push((function_call_id.clone(), c));
                        }
                    }
                    Behaviour_E::Other(_) => {
                        // 未来拓展用，目前不支持
                    }
                }
            }

            // tool 角色的结果消息
            if role_is_tool {
                for (call_id, c) in tool_results {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": c
                    }));
                }
                continue;
            }

            // assistant 带 tool_calls 的消息（其后紧跟 tool 结果）
            if !tool_calls.is_empty() {
                let mut m = json!({
                    "role": role,
                    "content": null,
                    "tool_calls": tool_calls,
                });
                if !content.is_empty() {
                    m["content"] = json!(content);
                }
                messages.push(m);
                for (call_id, c) in tool_results {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": c
                    }));
                }
                continue;
            }

            // 普通文本消息
            messages.push(json!({
                "role": role,
                "content": content,
            }));
        }

        JValue::Array(messages)
    }

    fn request(&self, conversation: &Conversation) -> reqwest::Request {
        let messages = self.conversation_to_Json(conversation);
        // 请求体：OpenAI Chat Completions 格式
        let mut body = json!({
            "model": &self.model,
            "messages": messages,
            "reasoning_effort": &self.thinking,
            "stream": false,
        });
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = self.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if !self.tool_choice.is_empty() {
            match self.tool_choice.as_str() {
                "auto" | "none" | "required" => {
                    body["tool_choice"] = json!(self.tool_choice);
                }
                name => {
                    body["tool_choice"] = json!({
                        "type": "function",
                        "function": { "name": name },
                    });
                }
            }
        }
        if let Some(t) = self.top_p {
            body["top_p"] = json!(t);
        }
        if let Some(p) = self.presence_penalty {
            body["presence_penalty"] = json!(p);
        }
        if let Some(f) = self.frequency_penalty {
            body["frequency_penalty"] = json!(f);
        }
        if let Some(u) = &self.user {
            body["user"] = json!(u);
        }
        // 带上全局注册的工具定义，AI 才知道有哪些工具可调用
        let tools = all_definitions();
        if !tools.as_array().map_or(true, |a| a.is_empty()) {
            body["tools"] = tools;
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut req = reqwest::Request::new(
            reqwest::Method::POST,
            reqwest::Url::parse(&url).expect("base_url 不是合法 URL"),
        );
        // 请求头：OpenAI 标准认证
        req.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.api_key).parse().unwrap(),
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

/// 提取 <thinking>...</thinking> 标签
/// 返回 (思考内容, 剩余正文)；没有标签则返回 None
fn extract_thinking(content: &str) -> Option<(String, String)> {
    const OPEN: &str = "<thinking>";
    const CLOSE: &str = "</thinking>";
    let start = content.find(OPEN)?;
    let rest = &content[start + OPEN.len()..];
    let end = rest.find(CLOSE)?;
    let thinking = rest[..end].to_string();
    let before = &content[..start];
    let after = &rest[end + CLOSE.len()..];
    Some((thinking, format!("{before}{after}")))
}



/*
json 字段详细解释
type : "OpenAi" , // 表示这是openAI格式
api : { // Openai字段
    "api_key"            : "sk-xxx",                          // 必填
    "base_url"           : "https://api.openai.com/v1",       // 必填
    "model"              : "gpt-4o-mini",                     // 可选
    "thinking"           : "high",                            // 可选，low / medium / high
    "temperature"        : 0.7,                               // 可选
    "max_tokens"         : 4096,                              // 可选
    "tool_choice"        : "auto",                            // 可选，auto / none / required / 工具名
    "timeout"            : 120,                               // 可选，秒
    "top_p"              : 0.9,                               // 可选
    "presence_penalty"   : 0.0,                               // 可选
    "frequency_penalty"  : 0.0,                               // 可选
    "user"               : "user-123",                        // 可选
    "extra_headers"      : { "X-Custom": "value" },           // 可选
},
*/

// 反序列化：传入 type:"OpenAi" 后面的 api 对象（仅 api 对象，无 type 包装）
pub fn from_json(s: JValue) -> MyResult<OpenAI> {
    let get_required = |key: &str| -> MyResult<String> {
        s.get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| {
                MyError::new(
                    "MissingConfigField",
                    format!("OpenAI 配置缺少必填字段 `{key}`"),
                )
            })
    };

    let mut openai = OpenAI::New(get_required("api_key")?, get_required("base_url")?);

    macro_rules! str_field {
        ($method:ident, $key:literal) => {
            if let Some(v) = s.get($key).and_then(|v| v.as_str()) {
                openai = openai.$method(v.to_string());
            }
        };
    }
    str_field!(with_model, "model");
    str_field!(with_thinking, "thinking");
    str_field!(with_tool_choice, "tool_choice");
    str_field!(with_user, "user");

    macro_rules! f64_field {
        ($method:ident, $key:literal) => {
            if let Some(v) = s.get($key).and_then(|v| v.as_f64()) {
                openai = openai.$method(v);
            }
        };
    }
    f64_field!(with_temperature, "temperature");
    f64_field!(with_top_p, "top_p");
    f64_field!(with_presence_penalty, "presence_penalty");
    f64_field!(with_frequency_penalty, "frequency_penalty");

    if let Some(v) = s.get("max_tokens").and_then(|v| v.as_u64()) {
        openai = openai.with_max_tokens(u32::try_from(v).map_err(|_| {
            MyError::new(
                "InvalidConfigField",
                format!("OpenAI 配置 `max_tokens` 超出 u32 范围：{v}"),
            )
        })?);
    }
    if let Some(v) = s.get("timeout").and_then(|v| v.as_u64()) {
        openai = openai.with_timeout(Duration::from_secs(v));
    }
    if let Some(headers) = s.get("extra_headers").and_then(|v| v.as_object()) {
        for (k, v) in headers {
            if let Some(v) = v.as_str() {
                openai = openai.with_header(k.clone(), v.to_string());
            }
        }
    }

    Ok(openai)
}





#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip() {
        let cfg = json!({
            "api_key": "sk-test",
            "base_url": "https://api.openai.com/v1",
            "model": "gpt-4o",
            "temperature": 0.7,
            "max_tokens": 4096,
            "tool_choice": "auto",
            "timeout": 60,
            "top_p": 0.9,
            "user": "u1",
            "extra_headers": { "X-Test": "yes" },
        });
        let o = from_json(cfg).unwrap();
        let back = o.to_json();
        assert_eq!(back["api_key"], "sk-test");
        assert_eq!(back["timeout"], 60);
        assert_eq!(back["extra_headers"]["X-Test"], "yes");
        assert_eq!(back["max_tokens"], 4096);

        let o2 = from_json(back).unwrap();
        assert_eq!(o2.base_url, "https://api.openai.com/v1");
        assert_eq!(o2.timeout, Duration::from_secs(60));
        assert_eq!(o2.extra_headers, vec![("X-Test".into(), "yes".into())]);
    }

    #[test]
    fn missing_required() {
        let err = from_json(json!({"api_key": "sk-test"})).unwrap_err();
        assert_eq!(err.name(), "MissingConfigField");
    }
}
