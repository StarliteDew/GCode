use crate::Structures::Message::Behaviour_E::Say;
use crate::Structures::Message::Message;
use crate::Structures::Message::tools::Registry::registry;
use crate::Structures::Message::{Behaviour, Behaviour_E, ToolUse};
use crate::Structures::Message::tools::{Registry, Tool, add_tool, tools_T};
use crate::Structures::Message::Role::{Role, new_role,SYSTEM,USER,ASSISTANT};
use crate::Structures::Conversation::Conversation;
use crate::Structures::Trait::Provider_API_T;
use crate::Imports::JValue;
use crate::Errors::{MyError, MyResult};
use crate::Structures::Message::tools::Expection::MyError as OldErr;
use reqwest::Client;
use crate::Const::DEFAULT_MAP as Map;


#[derive(Debug)]
pub struct Framework<T : Provider_API_T> {
    pub conversation : Conversation,
    pub Provider_api : T, // apikey 请求头  - 请求体 / 格式自己处理
    pub client : Client,
    pub runtime : tokio::runtime::Runtime
}


impl<T : Provider_API_T> Framework<T>{
    pub fn new(Provider_api : T,conversation : Option<Conversation>)->Self{
        Self {
            conversation : if conversation.is_none() { Conversation { conversation: Vec::new(), meta: JValue::Null, tools: Map::new() }}else {conversation.unwrap()},
            Provider_api : Provider_api,
            client : Client::new(),
            runtime : tokio::runtime::Runtime::new().unwrap()
        }
    }

    pub fn total_tools() -> Map<String,Tool>{
        let ptr = Registry::registry.read().unwrap();
        let mut result = Map::new();
        let _ = (*ptr).dict_name.iter().map(|(s,i)| {result.insert(s.clone(), Tool(*i))});
        drop(ptr);
        result
    }
    
    pub fn add_tool_by_Tool(&mut self,
        t :Tool, // 工具
        cover : bool // 是否覆盖，若为true则总是返回Ok
                    ) -> MyResult<()>{
        let name =t.get_name();
        if cover || self.conversation.tools.get(&name).is_none(){
            let _ = self.conversation.tools.insert(name, t);
            Ok(())
        }else{
            Err(
                MyError::new("ExistedTool", format!("the tool : {} is existed.",name))
            )
        }
    }

    pub fn add_tool(&mut self,t : impl tools_T + 'static,cover : bool) -> MyResult<()>{
        let name = t.name().to_string();
        let mut ptr = registry.read().unwrap();
        if ptr.dict_name.get(&name).is_some() {
            Err(
                MyError::new("ExistedTool", format!("the tool : {} is existed.",name))
            )
        }else{
            drop(ptr);
            let t = add_tool(t);

            self.add_tool_by_Tool(t, cover)
        }
    }

    pub fn system(&mut self,txt : String){
        self.conversation.conversation.push(Message { who: SYSTEM, did: vec![Behaviour_E::Say(txt).into_Behavior()], meta: JValue::Null });
    }

    pub fn last_message(&self) -> Option<&Message>{
        self.conversation.conversation.last()
    }

    pub fn messages(&self) ->&[Message]{
        &self.conversation.conversation
    }

    pub fn user_say(&mut self,txt : String) -> MyResult<&Message>{
        self.conversation.conversation.push(
            Message { who: USER, did: vec![Behaviour_E::Say(txt).into_Behavior()], meta: JValue::Null }
        );
        self.request()
    }

    pub fn conversation_mut(&mut self) -> &mut Conversation{
        &mut self.conversation
    }

    pub fn request(&mut self) -> MyResult<&Message>{
        let resp = self.Provider_api.request(&self.conversation);
        let rt = &self.runtime;

        
        let result = match rt.block_on(async {self.client.execute(resp).await}){
            Ok(r) =>{
                match rt.block_on(r.json::<JValue>())
                {
                    Ok(O) => {O},
                    Err(e) => {return Err(MyError::new("ParseJsonFailed", e));}
                }
            },
            Err(e) => {
                return Err(MyError::new("RequestError", e));
            }
        };


        let msg = self.Provider_api.response_to_Message(result)?;
        self.conversation.conversation.push(msg);
        Ok(self.last_message().unwrap())
    }

    pub fn last_is_functioncall(&self) -> bool{
        self.last_message()
            .map(|x| {
                x.did.iter().any(|b| {
                    matches!(&b.behaviour, Behaviour_E::Function_call_and_result(f) if f.result.is_none())
                })
            })
            .unwrap_or(false)
    }

    pub fn parse_functionCall_and_execute(&mut self) -> Option<&Message>{

        /*行为逻辑
         * 解析最后一条message，
         * 只有在该message存在且包含Function_call_and_result且result为None时才会调用工具
         * 否则返回None
         * 
         * 对message里每个待执行的functionUse依次调用工具:
         * 1. 查找工具
         * 若不存在工具将结果存储在functionUse的result里，为ResultWithWarning::Err(Other("cann't find tools : {} ."))
         * 若存在则调用 tools.call_result() // D:\vscode-project\AIApi-rs\src\Structures\Message\tools\Registry.rs:50
         * 将结果存储在 functionuse
         * 
         * 到此更新完成。
         * 
         * 截取最后的message（即处理完的functionuse）
         * 并且返回引用
         * 注意全程要使用&mut 来操作 
         * 
         */
        let last = self.conversation.conversation.last_mut()?;

        let mut executed = false;
        for b in last.did.iter_mut() {
            if let Behaviour_E::Function_call_and_result(f) = &mut b.behaviour {
                if f.result.is_none() {
                    match self.conversation.tools.get(&f.function_name) {
                        Some(tool) => {
                            f.result = Some(tool.call_result(f.arguments.clone()));
                        }
                        None => {
                            f.result = Some(Err(OldErr::Other(anyhow::anyhow!(
                                "cann't find tools : {} .",
                                f.function_name
                            ))));
                        }
                    }
                    executed = true;
                }
            }
        }

        if !executed {
            return None;
        }

        self.last_message()
    }
}





