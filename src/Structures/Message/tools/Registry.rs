// 注册工具 Registry : 
use std::{collections::HashMap as Map, sync::{Arc, LazyLock, RwLock}};
use serde_json::json;
use anyhow::anyhow;

use super::Structures::tools_T;
pub(crate) type ID = i64;
use crate::Const::Tools_id_start;
use super::Expection::{ResultWithWarning,Warning_to_JValue};
use super::Imports::JDict;
use super::Imports::JValue;
use super::Expection::MyError;


pub(crate) struct Registry {
    pub(crate) dict : Map<ID,Arc<dyn tools_T>>,
    pub(crate) dict_name : Map<String,ID>,
    next : ID,
}
#[derive(Debug,Clone, Copy,Hash,PartialEq, Eq)]
pub struct Tool(pub(crate) ID);

pub(crate) static registry : LazyLock<RwLock<Registry>> = LazyLock::new(||{
    RwLock::new(Registry { dict: Map::new(), dict_name: Map::new(), next: Tools_id_start })
});

impl Registry {
    fn add_tool<T : tools_T + 'static>(&mut self,tool : T) -> Tool{
        let name = tool.name().to_string();
        let arc = Arc::new(tool);
        let id = self.next;
        self.next += 1;
        self.dict_name.insert(name, id);
        self.dict.insert(id, arc);
        Tool(id)
    }
}

impl Tool {
    pub fn get_name(&self) -> String{
        let ptr = registry.read().unwrap();
        let name = ptr.dict.get(&self.0).unwrap().clone();
        let n = name.name().to_string();
        drop(name);
        drop(ptr);
        n
    }

    pub fn call_result(&self,args : JDict) -> ResultWithWarning<JValue> {
        let ptr = registry.read().unwrap();
        let t = ptr.dict.get(&self.0).unwrap().clone();
        let result1 = t.call(args);
        result1
    }

    pub fn call(&self,args : JDict) -> JValue{
        
        let call_result = self.call_result(args);
        Warning_to_JValue(&call_result)
    }

}


pub fn add_tool<T : tools_T + 'static>(tool : T) -> Tool{
    let mut ptr = registry.write().unwrap();
    let t = ptr.add_tool(tool);
    drop(ptr);
    t
}

/// 按名称调用已注册的工具；未注册则返回错误
pub fn call_by_name(name : &str, args : JDict) -> ResultWithWarning<JValue> {
    let ptr = registry.read().unwrap();
    match ptr.dict_name.get(name) {
        Some(id) => {
            let t = ptr.dict.get(id).unwrap().clone();
            t.call(args)
        }
        None => Err(MyError::Other(anyhow!("工具 {name} 未注册"))),
    }
}
