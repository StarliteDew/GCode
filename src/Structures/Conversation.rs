use super::Message::tools::Tool;
use super::Message::Message;

use crate::Const::DEFAULT_MAP as Map;
// use crate::Structures::Message::Role::Role;
use crate::Imports::*;
use super::Message::tools;
use super::Message::Role;
use super::Message::tools::Structures::tools_T;

#[derive(Debug)]
pub struct Conversation{
    pub  conversation : Vec<Message>,
    pub(crate)  meta : serde_json::Value,
    pub(crate)  tools : Map<String,Tool>,
    // pub(crate)  roles : Vec<Role::Role>
}


impl Conversation {
    pub fn add_tool<T : tools_T + 'static>(&mut self,tool : T) -> Tool{
        let t = tools::add_tool(tool);
        self.tools.insert(t.get_name(), t);
        t
    }

    // pub fn add_role(&mut self,debug_name : Option<String>,json : JValue)->Role::Role{
    //     let r = Role::new_role(debug_name, json);
    //     self.roles.push(r);
    //     r
    // }

}
