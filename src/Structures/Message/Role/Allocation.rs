/*
此部分代码已经验证通过！！
禁止更改此部分代码，因为有手动保证的unwarp
请不要随便加pub/pub(crate)
用unsafe标记不安全的代码块，
之所以用unsafe是因为这和裸指针一样属于逻辑保证正确
*/

use std::sync::{LazyLock, Mutex};

use super::*;
use crate::{Imports::*};
use crate::Const::*;


#[derive(Debug,Default,Clone,PartialEq, Eq)]
struct Cell{
    id : ID,
    debug_name : String, // 日志如何打印
    json : JValue // 在http请求中如何发送
}

struct Allocation{
    dict : Map<ID,Cell>,
    new : ID,
}

impl Allocation {
    fn new_role(&mut self,
    debug_name : Option<String>,json : JValue
)->Role{
    let id = self.new;
    self.new += 1;
    let cell = Cell{
        id : id,
        debug_name : if debug_name.is_some(){debug_name.unwrap()} else {json.to_string()},
        json:json
    };
    self.dict.insert(id, cell);
    return Role{id};
}
}


pub static USER      : Role = unsafe{Role {id : Message_Role_Allocation_id_start + 1}};// 更改请对应
pub static ASSISTANT : Role = unsafe{Role {id : Message_Role_Allocation_id_start + 0}};// 更改请对应
pub static SYSTEM    : Role = unsafe{Role {id : Message_Role_Allocation_id_start + 2}};// 更改请对应


static allocation : LazyLock<Mutex<Allocation>> =
    LazyLock::new(||{
        unsafe {
        let mut a = Allocation{
            dict : Map::new(),
            new : Message_Role_Allocation_id_start
        };
        a.new_role(Some("assistant".into()), serde_json::json!("assistant"));// +0
        a.new_role(Some("user"     .into()), serde_json::json!("user"     ));// +1
        a.new_role(Some("system"   .into()), serde_json::json!("system"   ));// +2
        Mutex::new(a)
        }
    });

#[derive(Debug,Clone, Copy,PartialEq, Eq,Hash)]
pub struct Role{
    id : ID
}

impl Role {
    pub fn debug_name(&self) ->String{
        let mut ptr = allocation.lock().unwrap();
        ptr.dict.get(&self.id).unwrap().debug_name.clone()
    }

    pub fn json(&self) -> JValue{
        let mut ptr = allocation.lock().unwrap();
        ptr.dict.get(&self.id).unwrap().json.clone()
    }
}

pub fn new_role(
    debug_name : Option<String>,json : JValue
)->Role{
    let mut ptr = allocation.lock().unwrap();
    
    return ptr.new_role(debug_name, json);
}


