pub mod Role;
pub mod tools;

use crate::Imports::*;
use crate::Errors::{MyError, MyResult};

#[derive(Debug)]
pub struct ToolUse {
    pub function_name : String,
    pub function_call_id : String,
    pub arguments : serde_json::Map<String,JValue>,
    pub result : Option<tools::ResultWithWarning<JValue>>
    // 只接收到functioncall 时候result 为None
}
#[derive(Debug)]
#[non_exhaustive]
pub enum Behaviour_E {
    Thinking(String),
    Say(String),
    File_local(String),//文件路径
    File_clould(String),//URL
    Function_call_and_result(ToolUse),
    Other(JValue) // 未来拓展用，目前不支持
}

impl Behaviour_E {
    pub fn say(s : String) -> Self{
        Self::Say(s)
    }

    pub fn File_clould(f :String) -> Self{
        Self::File_clould(f)
    }

    pub fn File_local(f : String) -> Self{
        Self::File_local(f)
    }

    pub fn into_Behavior(self) -> Behaviour{
        Behaviour { meta: JValue::Null, behaviour: self }
    }
}


#[derive(Debug)]
pub struct Behaviour {
    pub meta : JValue, // 额外元数据，不重要，框架填写
    pub behaviour : Behaviour_E
}
#[derive(Debug)]
pub struct Message{
    pub who : Role::Role,
    pub did : Vec<Behaviour>,
    pub meta : JValue // 额外元数据
    /*
    习惯上
        meta是一个dict

        - raw 字段存储原生返回
     */
}

