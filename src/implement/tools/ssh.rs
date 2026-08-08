use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use anyhow::anyhow;
use serde_json::json;

use russh::client::{self, Handle};
use russh::keys::{load_secret_key, PrivateKey, PrivateKeyWithHashAlg};
use russh::ChannelMsg;

use crate::Structures::Message::tools::Imports::{AnyResult, JDict, JValue, Map};
use crate::Structures::Message::tools::{ArgumentsProperties, IsEquired, JValueType, tools_T};

/// 连接 / 认证的默认超时（秒）
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// 简易客户端 Handler：不校验 known_hosts，接受所有主机密钥
#[derive(Debug)]
struct SshHandler;

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// 一个已登录并保活的 SSH 会话
struct SshChannel {
    host: String,
    port: u16,
    user: String,
    handle: Handle<SshHandler>,
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime, // 保存 runtime 让后台连接任务不退出
}

type ChannelTable = HashMap<String, SshChannel>;

/// 全局 Channel 表：名称 -> 已登录会话
static CHANNELS: LazyLock<RwLock<ChannelTable>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 认证方式
enum AuthKind {
    Password(String),
    Key { key: Arc<PrivateKey> },
}

fn new_runtime() -> AnyResult<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new().map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))
}

fn get_str(args: &JDict, key: &str) -> AnyResult<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("缺少参数: {key}"))
}

/// 检查 Channel 名称是否已存在
fn ensure_name_free(name: &str) -> AnyResult<()> {
    let guard = CHANNELS.read().unwrap();
    if guard.contains_key(name) {
        return Err(anyhow!("Channel 名称 '{name}' 已存在，请提供新的名称"));
    }
    Ok(())
}

/// 建立连接并完成认证，返回会话句柄与承载它的 runtime（runtime 必须随句柄一起保存）
fn connect_and_auth(
    host: &str,
    port: u16,
    user: &str,
    auth: AuthKind,
) -> AnyResult<(Handle<SshHandler>, tokio::runtime::Runtime)> {
    let rt = new_runtime()?;
    let handle = rt.block_on(async move {
        let mut config = client::Config::default();
        config.inactivity_timeout = Some(Duration::from_secs(600));
        config.keepalive_interval = Some(Duration::from_secs(30));
        let config = Arc::new(config);

        let mut session = tokio::time::timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            client::connect(config, (host, port), SshHandler),
        )
        .await
        .map_err(|_| anyhow!("连接 {host}:{port} 超时（{CONNECT_TIMEOUT_SECS} 秒）"))?
        .map_err(|e| anyhow!("连接 {host}:{port} 失败: {e}"))?;

        match auth {
            AuthKind::Password(pwd) => {
                let r = tokio::time::timeout(
                    Duration::from_secs(CONNECT_TIMEOUT_SECS),
                    session.authenticate_password(user, pwd),
                )
                .await
                .map_err(|_| anyhow!("密码认证超时"))?
                .map_err(|e| anyhow!("密码认证失败: {e}"))?;
                if !r.success() {
                    return Err(anyhow!("密码认证被拒绝，请检查用户名或密码"));
                }
            }
            AuthKind::Key { key } => {
                // RSA 密钥需要和服务器协商签名哈希算法，其他类型忽略
                let hash = if key.algorithm().is_rsa() {
                    session.best_supported_rsa_hash().await.unwrap_or(None).flatten()
                } else {
                    None
                };
                let pk = PrivateKeyWithHashAlg::new(key, hash);
                let r = tokio::time::timeout(
                    Duration::from_secs(CONNECT_TIMEOUT_SECS),
                    session.authenticate_publickey(user, pk),
                )
                .await
                .map_err(|_| anyhow!("密钥认证超时"))?
                .map_err(|e| anyhow!("密钥认证失败: {e}"))?;
                if !r.success() {
                    return Err(anyhow!("密钥认证被拒绝，请检查用户名或私钥"));
                }
            }
        }
        Ok::<Handle<SshHandler>, anyhow::Error>(session)
    })?;
    Ok((handle, rt))
}

// ==================== 工具 1：账号/密码登录 ====================

pub struct SshPasswordLogin;

impl tools_T for SshPasswordLogin {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "name".into(),
            ArgumentsProperties::new(
                "name".into(),
                "该 Channel 的命名（需唯一，用于后续 list_channel / ssh_execute）".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "host".into(),
            ArgumentsProperties::new(
                "host".into(),
                "SSH 服务器 IP 或主机名".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "port".into(),
            ArgumentsProperties::new(
                "port".into(),
                "SSH 端口，默认 22".into(),
                IsEquired::NotRequired(json!(22)),
                JValueType::Number,
            ),
        );
        m.insert(
            "user".into(),
            ArgumentsProperties::new(
                "user".into(),
                "登录用户名".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "password".into(),
            ArgumentsProperties::new(
                "password".into(),
                "登录密码".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let name = get_str(&args, "name")?;
        let host = get_str(&args, "host")?;
        let port = args.get("port").and_then(|v| v.as_i64()).unwrap_or(22) as u16;
        let user = get_str(&args, "user")?;
        let password = get_str(&args, "password")?;

        ensure_name_free(&name)?;

        let (handle, runtime) =
            connect_and_auth(&host, port, &user, AuthKind::Password(password))?;

        {
            let mut guard = CHANNELS.write().unwrap();
            if guard.contains_key(&name) {
                return Err(anyhow!("Channel 名称 '{name}' 已存在，请提供新的名称"));
            }
            guard.insert(
                name.clone(),
                SshChannel {
                    host: host.clone(),
                    port,
                    user: user.clone(),
                    handle,
                    runtime,
                },
            );
        }

        Ok(json!({
            "name": name,
            "host": host,
            "port": port,
            "user": user,
            "message": "密码登录成功，Channel 已保存",
        }))
    }

    fn name(&self) -> &str {
        "ssh_password_login"
    }

    fn description(&self) -> String {
        "使用账号/密码登录 SSH 服务器，并将连接保存为一个命名的 Channel。若名称已存在则报错。".into()
    }
}

// ==================== 工具 2：密钥登录 ====================

pub struct SshKeyLogin;

impl tools_T for SshKeyLogin {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "name".into(),
            ArgumentsProperties::new(
                "name".into(),
                "该 Channel 的命名（需唯一，用于后续 list_channel / ssh_execute）".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "host".into(),
            ArgumentsProperties::new(
                "host".into(),
                "SSH 服务器 IP 或主机名".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "port".into(),
            ArgumentsProperties::new(
                "port".into(),
                "SSH 端口，默认 22".into(),
                IsEquired::NotRequired(json!(22)),
                JValueType::Number,
            ),
        );
        m.insert(
            "user".into(),
            ArgumentsProperties::new(
                "user".into(),
                "登录用户名".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "key_path".into(),
            ArgumentsProperties::new(
                "key_path".into(),
                "私钥文件路径（支持 ed25519 / RSA 等 OpenSSH 格式）".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "key_passphrase".into(),
            ArgumentsProperties::new(
                "key_passphrase".into(),
                "私钥口令，若私钥无口令可省略".into(),
                IsEquired::NotRequired(json!("")),
                JValueType::String,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let name = get_str(&args, "name")?;
        let host = get_str(&args, "host")?;
        let port = args.get("port").and_then(|v| v.as_i64()).unwrap_or(22) as u16;
        let user = get_str(&args, "user")?;
        let key_path = get_str(&args, "key_path")?;
        let passphrase = args.get("key_passphrase").and_then(|v| v.as_str());

        ensure_name_free(&name)?;

        // 私钥在同步上下文加载（读文件 + 解析，非阻塞异步操作）
        let pass = if passphrase.map_or(true, |p| p.is_empty()) {
            None
        } else {
            Some(passphrase.unwrap())
        };
        let key = load_secret_key(&key_path, pass)
            .map_err(|e| anyhow!("加载私钥 {key_path} 失败: {e}"))?;

        let (handle, runtime) =
            connect_and_auth(&host, port, &user, AuthKind::Key { key: Arc::new(key) })?;

        {
            let mut guard = CHANNELS.write().unwrap();
            if guard.contains_key(&name) {
                return Err(anyhow!("Channel 名称 '{name}' 已存在，请提供新的名称"));
            }
            guard.insert(
                name.clone(),
                SshChannel {
                    host: host.clone(),
                    port,
                    user: user.clone(),
                    handle,
                    runtime,
                },
            );
        }

        Ok(json!({
            "name": name,
            "host": host,
            "port": port,
            "user": user,
            "key_path": key_path,
            "message": "密钥登录成功，Channel 已保存",
        }))
    }

    fn name(&self) -> &str {
        "ssh_key_login"
    }

    fn description(&self) -> String {
        "使用私钥登录 SSH 服务器，并将连接保存为一个命名的 Channel。若名称已存在则报错。".into()
    }
}

// ==================== 工具 3：列出 Channel ====================

pub struct ListChannel;

impl tools_T for ListChannel {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        Map::new()
    }

    fn execute(&self, _args: JDict) -> AnyResult<JValue> {
        let guard = CHANNELS.read().unwrap();
        let mut channels = Vec::new();
        for (name, ch) in guard.iter() {
            channels.push(json!({
                "name": name,
                "host": ch.host,
                "port": ch.port,
                "user": ch.user,
            }));
        }
        Ok(json!({
            "count": channels.len(),
            "channels": channels,
        }))
    }

    fn name(&self) -> &str {
        "list_channel"
    }

    fn description(&self) -> String {
        "列出所有已登录的 SSH Channel，包括名称、服务器 IP/端口与登录用户名。".into()
    }
}

/// 关闭一个已登录的会话：发送 SSH_MSG_DISCONNECT 并终止其后台 runtime
fn close_session(ch: SshChannel) {
    let SshChannel { handle, runtime, .. } = ch;
    let _ = runtime.block_on(async move {
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "closed by user", "")
            .await;
    });
}

// ==================== 工具 4：按名称关闭 Channel ====================

pub struct SshClose;

impl tools_T for SshClose {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "name".into(),
            ArgumentsProperties::new(
                "name".into(),
                "要关闭的 Channel 名称（由 ssh_password_login / ssh_key_login 创建）".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let name = get_str(&args, "name")?;

        let ch = {
            let mut guard = CHANNELS.write().unwrap();
            guard.remove(&name)
        };
        let ch = ch.ok_or_else(|| anyhow!("Channel '{name}' 不存在，请先登录"))?;

        let SshChannel {
            host,
            port,
            user,
            ..
        } = &ch;
        let result = json!({
            "name": name,
            "host": host,
            "port": port,
            "user": user,
            "message": "Channel 已关闭",
        });

        close_session(ch);
        Ok(result)
    }

    fn name(&self) -> &str {
        "ssh_close"
    }

    fn description(&self) -> String {
        "按名称关闭一个指定的 SSH Channel，释放连接。若 Channel 不存在则报错。".into()
    }
}

// ==================== 工具 5：关闭全部 Channel ====================

pub struct SshCloseAll;

impl tools_T for SshCloseAll {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        Map::new()
    }

    fn execute(&self, _args: JDict) -> AnyResult<JValue> {
        let closed: Vec<(String, SshChannel)> = {
            let mut guard = CHANNELS.write().unwrap();
            guard.drain().collect()
        };
        let count = closed.len();
        for (_name, ch) in closed {
            close_session(ch);
        }
        Ok(json!({
            "count": count,
            "message": "已全部关闭",
        }))
    }

    fn name(&self) -> &str {
        "ssh_close_all"
    }

    fn description(&self) -> String {
        "一次关闭所有已登录的 SSH Channel，并释放全部连接。".into()
    }
}

// ==================== 工具 6：在指定 Channel 上执行命令 ====================

pub struct SshExecute;

impl tools_T for SshExecute {
    fn arguments(&self) -> Map<String, ArgumentsProperties> {
        let mut m = Map::new();
        m.insert(
            "channel".into(),
            ArgumentsProperties::new(
                "channel".into(),
                "要使用的 Channel 名称（由 ssh_password_login / ssh_key_login 创建）".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "command".into(),
            ArgumentsProperties::new(
                "command".into(),
                "要在远程服务器上执行的 shell 命令".into(),
                IsEquired::Equired,
                JValueType::String,
            ),
        );
        m.insert(
            "timeout".into(),
            ArgumentsProperties::new(
                "timeout".into(),
                "执行超时时间（秒），默认 30".into(),
                IsEquired::NotRequired(json!(30)),
                JValueType::Number,
            ),
        );
        m
    }

    fn execute(&self, args: JDict) -> AnyResult<JValue> {
        let name = get_str(&args, "channel")?;
        let command = get_str(&args, "command")?;
        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_i64())
            .unwrap_or(30)
            .max(1) as u64;

        let rt = new_runtime()?;
        rt.block_on(async move {
            let guard = CHANNELS.read().unwrap();
            let ch = guard
                .get(&name)
                .ok_or_else(|| anyhow!("Channel '{name}' 不存在，请先登录"))?;

            let (stdout, stderr, exit_status) = tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                async {
                    let mut chan = ch
                        .handle
                        .channel_open_session()
                        .await
                        .map_err(|e| anyhow!("打开 SSH 通道失败: {e}"))?;
                    chan.exec(true, command.as_bytes().to_vec())
                        .await
                        .map_err(|e| anyhow!("发送命令失败: {e}"))?;

                    let mut stdout: Vec<u8> = Vec::new();
                    let mut stderr: Vec<u8> = Vec::new();
                    let mut exit_status: Option<u32> = None;
                    while let Some(msg) = chan.wait().await {
                        match msg {
                            ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                            ChannelMsg::ExtendedData { data, .. } => {
                                stderr.extend_from_slice(&data)
                            }
                            ChannelMsg::ExitStatus { exit_status: s } => exit_status = Some(s),
                            _ => {}
                        }
                    }
                    Ok::<_, anyhow::Error>((stdout, stderr, exit_status))
                },
            )
            .await
            .map_err(|_| anyhow!("命令执行超时（{timeout_secs} 秒），可适当调大 timeout 参数"))??;

            Ok(json!({
                "channel": name,
                "command": command,
                "stdout": String::from_utf8_lossy(&stdout),
                "stderr": String::from_utf8_lossy(&stderr),
                "exit_status": exit_status,
                "success": exit_status == Some(0),
            }))
        })
    }

    fn name(&self) -> &str {
        "ssh_execute"
    }

    fn description(&self) -> String {
        "在指定 Channel 上执行远程命令，并返回 stdout、stderr 与退出码；支持自定义超时（秒）。".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_channel_returns_empty() {
        let t = ListChannel;
        let (value, warning) = t.call(JDict::new()).unwrap();
        assert!(warning.is_none());
        assert_eq!(value["count"], 0);
        assert!(value["channels"].is_array());
    }

    #[test]
    fn ssh_execute_requires_channel() {
        let t = SshExecute;
        let mut args = JDict::new();
        args.insert("command".into(), json!("ls"));
        let err = t.call(args).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("channel"), "应提示缺少 channel 参数，实际: {msg}");
    }

    #[test]
    fn ssh_execute_channel_not_found() {
        let t = SshExecute;
        let mut args = JDict::new();
        args.insert("channel".into(), json!("不存在的名称"));
        args.insert("command".into(), json!("ls"));
        let err = t.call(args).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("不存在"), "应提示 Channel 不存在，实际: {msg}");
    }

    #[test]
    fn password_login_requires_all_args() {
        let t = SshPasswordLogin;
        let err = t.call(JDict::new()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("arguments Error"), "应提示缺失必填参数，实际: {msg}");
    }

    #[test]
    fn ssh_close_requires_name() {
        let t = SshClose;
        let err = t.call(JDict::new()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("name"), "应提示缺少 name 参数，实际: {msg}");
    }

    #[test]
    fn ssh_close_channel_not_found() {
        let t = SshClose;
        let mut args = JDict::new();
        args.insert("name".into(), json!("不存在的名称"));
        let err = t.call(args).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("不存在"), "应提示 Channel 不存在，实际: {msg}");
    }

    #[test]
    fn ssh_close_all_returns_empty() {
        let t = SshCloseAll;
        let (value, warning) = t.call(JDict::new()).unwrap();
        assert!(warning.is_none());
        assert_eq!(value["count"], 0);
    }
}
