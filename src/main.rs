use std::env;
use std::fs;
use std::io::{Write,self,BufRead};
// use std::io::{self, BufRead};
use std::process::exit;

use serde_json::{json, Value};

use AIApi_rs::Framework::Framework;
use AIApi_rs::Structures::Message::{Behaviour_E, Message};
use AIApi_rs::Structures::Trait::Provider_API_T;
use AIApi_rs::implement::OpenAi;
use AIApi_rs::implement::OpenAi::OpenAI;
use AIApi_rs::implement::tools;

const USAGE: &str = "\
用法:
  main.exe api_openai default <filePath>   将默认的 OpenAI 配置(含 type)写入 filePath
  main.exe -c <filePath>                   读取 filePath 配置，构建 fw 并进入对话
";

fn main() {
    let args: Vec<String> = env::args().collect();
    let get = |i: usize| args.get(i).map(|s| s.as_str());

    match get(1) {
        Some("api_openai") => {
            if get(2) != Some("default") {
                print!("{USAGE}");
                exit(1);
            }
            match get(3) {
                Some(path) => write_default_config(path),
                None => {
                    print!("{USAGE}");
                    exit(1);
                }
            }
        }
        Some("-c") => match get(2) {
            Some(path) => {
                let (system, openai) = openai_from_file(path);
                let mut fw: Framework<Box<dyn Provider_API_T>> =
                    Framework::new(Box::new(openai), None);
                // 注入配置中的系统提示词（作为第一条 system 消息）
                if let Some(s) = system {
                    fw.system(s);
                }
                
                run_interactive(&mut fw);
            }
            None => {
                print!("{USAGE}");
                exit(1);
            }
        },
        _ => print!("{USAGE}"),
    }
}

/// 把默认 OpenAI 配置以 { type, api } 完整对象写入文件
fn write_default_config(path: &str) {
    let openai = OpenAI::New(
        "sk-your-api-key".to_string(),
        "https://api.openai.com/v1".to_string(),
    );
    let cfg = json!({
        "type": "OpenAi",
        "system": "你是一个有帮助的AI助手，请使用中文简洁地回答。",
        "api": openai.to_json(),
    });
    fs::write(path, serde_json::to_string_pretty(&cfg).unwrap())
        .unwrap_or_else(|e| {
            eprintln!("写入失败: {e}");
            exit(1);
        });
    println!("已写入默认 OpenAI 配置到 {path}");
}

/// 从配置文件构建 OpenAI provider 与系统提示词：
/// 支持 { "type": "OpenAi", "system": "...", "api": {...} } 完整格式，也兼容直接就是 api 对象
/// 返回 (系统提示词, OpenAI)
fn openai_from_file(path: &str) -> (Option<String>, OpenAI) {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("读取失败: {e}");
        exit(1);
    });
    let cfg: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
        eprintln!("JSON 解析失败: {e}");
        exit(1);
    });

    // system 字段表示系统提示词，注入到对话开头
    let system = cfg
        .get("system")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let api = match cfg.get("type") {
        Some(t) => {
            if t.as_str() != Some("OpenAi") {
                eprintln!("暂不支持的类型: {t}");
                exit(1);
            }
            cfg.get("api").cloned().unwrap_or_else(|| {
                eprintln!("配置文件缺少 api 字段");
                exit(1);
            })
        }
        None => cfg,
    };

    let openai = OpenAi::from_json(api).unwrap_or_else(|e| {
        eprintln!("配置解析失败: {e}");
        exit(1);
    });
    (system, openai)
}

/// 把一条消息渲染成人类可读文本
fn message_to_text(m: &Message) -> String {
    let mut out = Vec::new();
    for b in &m.did {
        match &b.behaviour {
            Behaviour_E::Thinking(t) => out.push(format!("[思考] {t}\n[思考完成]\n")),
            Behaviour_E::Say(s) => out.push(format!("[回答] {s}")),
            Behaviour_E::Function_call_and_result(f) => {
                let mut args = String::new();
                for (k,v) in  &f.arguments {
                    args += format!("{} : {}," , k,v).as_str()
                }
                // println!("debug : {}",f.result.is_none());
                if f.result.is_none(){
                    out.push(format!("[调用工具] {}({})", f.function_name,args));
                }else {
                    let res = &f.result.as_ref().unwrap() ;
                    match res {
                    Ok((v, _w)) => out.push(format!("[工具结果] Ok({v})")),
                    Err(e) => out.push(format!("[工具错误] Err({e})")),
                    }
                    
                }
            }
            _ => {}
        }
    }
    out.join("\n")
}

/// 交互式对话：模型请求工具时自动执行并继续追问，直到模型直接回答
fn run_interactive(fw: &mut Framework<Box<dyn Provider_API_T>>) {
    tools::register_all(fw).unwrap_or_else(|e| {
        eprintln!("注册工具失败: {e}");
        exit(1);
    });

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    println!("已进入对话，输入 exit 退出。");
    print!(" > ");
    stdout.flush().unwrap();
    
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line == "exit" {
            break;
        }
        if line.is_empty() {
            continue;
        }

        let _ = fw.user_say(line).unwrap_or_else(|e| {
            eprintln!("请求失败: {e}");
            exit(1);
        });

        // 模型请求工具 -> 执行工具 -> 把结果发回 -> 直到模型直接回答
        while fw.last_is_functioncall() {
            if let Some(m) = fw.last_message() {
            println!("{}", message_to_text(m));
            }
            fw.parse_functionCall_and_execute();
            if let Some(m) = fw.last_message() {
            println!("{}", message_to_text(m));
            }
            let _ = fw.request().unwrap_or_else(|e| {
                eprintln!("请求失败: {e}");
                exit(1);
            });
            
        }

        if let Some(m) = fw.last_message() {
            println!("{}", message_to_text(m));
        }

        print!(" > ");
        stdout.flush().unwrap();
    }
}
