# AIApi-rs

一个用 Rust 编写的大模型对话框架：**对话抽象与工具调用彻底独立于具体的大模型 API**。

> ⚠️ **项目状态：早期开发阶段（v0.1.0）**
> 核心架构已就绪，但接口、行为与模块划分仍可能发生破坏性变更，暂不建议在生产环境使用。

## 核心理念

市面上各家 LLM（OpenAI、Anthropic、DeepSeek、Kimi……）的请求格式各不相同，而应用层真正关心的是「一段对话」和「模型可用的工具」，而不是某个厂商的 JSON 结构。

因此本项目把二者彻底分离：

```
┌───────────────────────── 应用层（你的程序 / main.rs）────────────────────────┐
│   只关心：Conversation（对话）、工具、Behaviour（行为流）                        │
└──────────────────────────────────┬───────────────────────────────────────────┘
                                   │ Framework<T: Provider_API_T>
┌──────────────────────────────────▼───────────────────────────────────────────┐
│   适配层（trait Provider_API_T）：把「通用对话」翻译成「厂商请求」               │
│     OpenAI    → /v1/chat/completions                                          │
│     Anthropic → /v1/messages                                                  │
│     你写的任意 provider → 只需实现 3 个方法                                    │
└───────────────────────────────────────────────────────────────────────────────┘
```

- **上层**（`Structures`、`Framework`）只处理与厂商无关的抽象：`Message` / `Behaviour_E`（思考、回答、文件、工具调用）、`Conversation`、`Role`。
- **下层**（`implement/`）通过 `Provider_API_T` trait 把抽象对话翻译成各厂商的请求体，再把厂商返回解析回统一结构。

接入一个新的模型厂商，**只需要实现 `Provider_API_T` trait 的三个方法**，其余一切（对话管理、工具执行、循环追问）都由框架完成。

## 特性

- **厂商无关的对话抽象**：`Message` 由一组 `Behaviour_E` 行为组成（思考 / 回答 / 本地文件 / 云端文件 / 工具调用 / 其他），天然支持思考型模型（DeepSeek、Claude extended thinking 等）。
- **内置 Provider**：OpenAI Chat Completions、Anthropic Messages，均支持工具调用与思考内容解析。
- **工具即插即用**：任何类型只要实现 `tools_T` trait，注册后即可被模型调用，自动完成参数校验、类型转换与结果回传。
- **内置工具集**：执行 shell 命令（`bash`）、读写本地文件（`read` / `write`）、SSH 会话管理（密码/密钥登录、执行远程命令、关闭连接）。
- **统一错误处理**：所有错误收敛为带分类名的 `MyError`。

## 快速上手

```bash
# 1. 生成一份默认 OpenAI 配置
cargo run -- api_openai default config.json
# 然后编辑 config.json，填入你的 api_key / base_url / model

# 2. 读取配置，进入交互式对话
cargo run -- -c config.json
```

交互式对话中，模型可以请求调用工具，框架会自动执行并把结果发回模型，直到模型直接给出回答。

## 添加自己的工具

只需实现 `tools_T`：

```rust
use serde_json::{json, Value};
use AIApi_rs::Structures::Message::tools::*;

pub struct MyTool;

impl tools_T for MyTool {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "text".into(),
            ArgumentsProperties::new(
                "text".into(),
                "要处理的文本".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({ "upper": text.to_uppercase() }))
    }

    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> String { "把文本转成大写。".into() }
}
```

然后在任意 `Framework` 上注册即可：

```rust
framework.add_tool(MyTool, false)?;
```

## 添加自己的 Provider

实现 `Provider_API_T` 的三个方法：

```rust
pub trait Provider_API_T {
    /// 把 API 返回的原始 JSON 解析成框架可读的 Message
    fn response_to_Message(&self, resp: JValue) -> MyResult<Message>;
    /// 把框架可读的 Conversation 翻译成 API 需要的对话内容
    fn conversation_to_Json(&self, conversation: &Conversation) -> JValue;
    /// 最终请求（含请求头与请求体）
    fn request(&self, conversation: &Conversation) -> reqwest::Request;
}
```

## 目录结构

```
src/
├── main.rs                 # 一个使用框架的实例：交互式 CLI（main 只是用法之一）
├── lib.rs
├── Framework.rs            # 框架核心：对话状态 + 请求 + 工具执行循环
├── Structures/
│   ├── Trait.rs            # Provider_API_T：厂商适配层契约
│   ├── Conversation.rs     # 对话容器
│   ├── Message.rs          # Message / Behaviour_E / ToolUse 抽象
│   ├── Message/tools/      # 工具框架：tools_T trait、注册表、参数校验、类型转换
│   └── Message/Role/       # 角色系统（user / assistant / system，可扩展）
├── implement/
│   ├── OpenAi.rs           # OpenAI Chat Completions provider
│   ├── Anthropic.rs        # Anthropic Messages provider
│   └── tools/              # 内置工具：bash / read / write / ssh
├── Errors.rs               # 统一错误类型
└── Const.rs                # 全局常量
```

## Roadmap

- [ ] 完善 `Behaviour_E` 中的文件（本地 / 云端）多模态支持
- [ ] 增加更多 Provider（DeepSeek、Kimi、Responses API 等）
- [ ] 流式（SSE）输出
- [ ] 对话持久化
- [ ] 稳定公开 API，进入可用阶段
