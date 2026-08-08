use std::fmt;

use anyhow::Error as AnyError;

use crate::Structures::Message::tools::Expection::MyError as LegacyMyError;

/// 项目内部统一错误类型。
///
/// 相比旧工具框架的 `Structures::Message::tools::Expection::MyError` 枚举，
/// 采用「分类名 name + 底层 anyhow 错误」的单一形状，全项目统一使用。
///
/// # 错误名（name）注册表
///
/// 新增错误时必须先在下面登记：错误名 / 使用位置（文件） / 用途。
///
/// | name                 | 使用位置                           | 用途                                            |
/// |----------------------|------------------------------------|-------------------------------------------------|
/// | `anyhow`             | `Errors.rs`（`From<AnyError>`）    | 任意 anyhow::Error 的兜底包装                    |
/// | `message`            | `Errors.rs`（`From<String>`/`From<&str>`） | 字符串消息直接转为错误                    |
/// | `MissingArgs`        | `Errors.rs`（`from_legacy_tools_error`）    | 旧工具框架迁移：必填参数缺失             |
/// | `FunctionCallErr`    | `Errors.rs`（`from_legacy_tools_error`）    | 旧工具框架迁移：execute 层调用失败        |
/// | `Other`              | `Errors.rs`（`from_legacy_tools_error`）    | 旧工具框架迁移：其他任意错误              |
/// | `UnrecognizedResponse` | `Structures/Message.rs:100`        | OpenAI 响应缺 `choices[0].message`，格式无法识别 |
/// | `ToolCallMalformed`  | `Structures/Message.rs:139/145/150/157`     | tool_call 缺 `function.name`/`id`/`function.arguments`，或 `arguments` 不是合法 JSON |
/// | `EmptyMessage`       | `Structures/Message.rs:176`        | AI 返回空消息（既无文本也无工具调用）             |
/// | `RequestError`       | `Framework.rs`（`say`）            | HTTP 请求发送失败或状态码错误                     |
/// | `RequestParseError`  | `Framework.rs`（`say`）            | 响应 JSON 解析失败                                 |
/// | `MissingConfigField` | `implement/OpenAi.rs`（`from_json`）| 反序列化时 api 对象缺少必填字段（如 api_key/base_url） |
/// | `InvalidConfigField` | `implement/OpenAi.rs`（`from_json`）| 反序列化时字段类型/取值不符合预期（如 max_tokens 超范围） |
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct MyError {
    name: String,
    Error: AnyError,
}

impl MyError {
    pub fn new(
        name: impl Into<String>,
        error: impl fmt::Display + fmt::Debug + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            Error: AnyError::msg(error),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn error(&self) -> &AnyError {
        &self.Error
    }

    pub fn into_error(self) -> AnyError {
        self.Error
    }
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.Error)
    }
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.Error.source()
    }
}

impl From<AnyError> for MyError {
    fn from(e: AnyError) -> Self {
        MyError::new("anyhow", e)
    }
}

impl From<String> for MyError {
    fn from(s: String) -> Self {
        MyError::new("message", AnyError::msg(s))
    }
}

impl From<&str> for MyError {
    fn from(s: &str) -> Self {
        MyError::new("message", AnyError::msg(s.to_string()))
    }
}

/// 便捷类型别名：全项目统一返回结果
pub type MyResult<T> = Result<T, MyError>;

// ==================== 旧工具框架迁移（不改动原有代码） ====================

/// 迁移：旧 `Expection::MyError` -> 统一 `MyError`
pub fn from_legacy_tools_error(e: LegacyMyError) -> MyError {
    match e {
        LegacyMyError::MissingArgs(args) => {
            let mut s = String::from("arguments Error:\n");
            for a in &args {
                s += &format!("\t{} : {}\n", a.name, a.Error_desc);
            }
            MyError::new("MissingArgs", AnyError::msg(s.trim_end().to_string()))
        }
        LegacyMyError::FunctionCallErr(e) => MyError::new("FunctionCallErr", e),
        LegacyMyError::Other(e) => MyError::new("Other", e),
    }
}

/// 迁移：旧 `ResultWithWarning<T>` -> `MyResult<(T, Option<String>)>`
pub fn from_legacy_result_with_warning<T>(
    r: crate::Structures::Message::tools::Expection::ResultWithWarning<T>,
) -> MyResult<(T, Option<String>)> {
    match r {
        Ok((v, w)) => Ok((v, w)),
        Err(e) => Err(from_legacy_tools_error(e)),
    }
}

/// 迁移：旧错误可通过 `?` 直接转成统一 `MyError`
impl From<LegacyMyError> for MyError {
    fn from(e: LegacyMyError) -> Self {
        from_legacy_tools_error(e)
    }
}
