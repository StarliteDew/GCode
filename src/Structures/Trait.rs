use crate::Imports::*;
use crate::Errors::{MyError, MyResult};

use super::Message::*;
use super::Conversation::Conversation;

pub trait Provider_API_T {
    fn response_to_Message(&self,resp :JValue) -> MyResult<Message>;// 将api返回变成 框架可读结构

    fn conversation_to_Json(&self,conversation : &Conversation) -> JValue; // 将可读的对话变成api中的对话内容

    fn request(&self,conversation : &Conversation) -> reqwest::Request;// 最终请求，包含请求头 / 和json
}

/// 让 Box<dyn Provider_API_T> 也能当作 Provider_API_T 使用
impl Provider_API_T for Box<dyn Provider_API_T> {
    fn response_to_Message(&self, resp: JValue) -> MyResult<Message> {
        (**self).response_to_Message(resp)
    }

    fn conversation_to_Json(&self, conversation: &Conversation) -> JValue {
        (**self).conversation_to_Json(conversation)
    }

    fn request(&self, conversation: &Conversation) -> reqwest::Request {
        (**self).request(conversation)
    }
}



/*
 * 注意：
 * 整个架构分为:
 *     对话
 *     请求
 *     Provider_api_T
 * 
 *  流程为:
 *  框架初始化 空对话 默认请求属性
 *  
 *  
 */

