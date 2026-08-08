use std::fmt::format;
use std::str::FromStr;

use super::Imports::*;
use super::Expection::{MyResult,MyError,ResultWithWarning};
use super::Convernt::try_convert;



#[derive(Debug,Clone)]
pub enum IsEquired  {
    Equired,
    NotRequired(JValue)
}

impl IsEquired {
    pub fn is_required(&self) -> bool{
        match self {
            Self::Equired => true,
            Self::NotRequired(_) => false
        }
    }
}

#[derive(Debug,Clone,Copy,PartialEq, Eq)]
pub enum JValueType {
    Number,
    Object,
    Bool,
    Array,
    String,
    Null
}

impl JValueType {
    fn from_JValue(v : &JValue)->Self{
        match v {
            JValue::Array(_)  => Self::Array,
            JValue::Bool(_)   => Self::Bool,
            JValue::Number(_) => Self::Number,
            JValue::Null      => Self::Null,
            JValue::Object(_) => Self::Object,
            JValue::String(_) => Self::String
        }
    }
    
}


#[derive(Debug,Clone)]
pub struct  ArgumentsProperties {
    // 请自行确保Type与default的类型是一致的
    pub name : String,
    pub description : String,
    pub is_required : IsEquired,
    pub Type : JValueType
}

impl ArgumentsProperties {
    pub fn new(name: String, description: String, is_required: IsEquired, Type: JValueType) -> Self {
        Self { name, description, is_required, Type }
    }
}




#[allow(non_camel_case_types)]
#[derive(Debug,Clone)]
pub struct Arguments_Error_Missing_args{
    pub name : String,
    pub Error_desc : String
}


pub trait tools_T : Send + Sync {
    fn arguments(&self) -> Map<String,ArgumentsProperties>;
    /*
    需要的参数，分为以下内容:
    参数名称
    参数描述
    是否必须
    若不必须，默认值为
    */
    fn call(&self,args : JDict) -> ResultWithWarning<JValue>{
        //设计支持强制类型转化吗？感觉会增加复杂度，先放在这里吧
        // 是否支持被丢弃的参数？
        let mut args = args;
        let mut result = JMap::new();
        let mut errs = Vec::new();
        for (k , v) in self.arguments(){
            match args.remove_entry(&k) {
                None => {// 不存在这个参数
                    match v.is_required{
                        IsEquired::Equired=>{
                            let t = format!("arguments : {} is required but missing",&k);
                            errs.push(
                                Arguments_Error_Missing_args { name: 
                                    k, Error_desc: t
                                 }
                            );
                        },
                        IsEquired::NotRequired(N) => {
                            result.insert(k, N);
                        }
                    }
                },
                Some((args_k,args_v)) =>{
                    let r;
                    let t = JValueType::from_JValue(&args_v);
                    if t != v.Type{
                        match try_convert(args_v, v.Type){
                            Ok(O) => {r = O},
                            Err(e) => {
                                errs.push(Arguments_Error_Missing_args { 
                                    name: args_k, Error_desc: format!("arguments : {k} is type : {:?} but expect {:?} ,and cann't convernt to type : {:?} because {}",t,v.Type,v.Type,e
                                ) 
                                });
                                continue;
                            }
                        }
                    }else{
                        r = args_v
                    }
                    result.insert(args_k, r);
                }
            }
        }

        let W = if args.is_empty() {None}else{
            let mut s = String::from_str("extra Invalid parameters not passed :\n").unwrap();
            for (k,v) in args.into_iter(){
                s+= ( format!("\t {} : {}\n",k,v)).as_str();
            }
            Some(s)
        };

        if errs.len() > 0{
            Err(
                MyError::MissingArgs(errs)
            )
        }else{
            match self.execute(result) {
                Ok(O) => Ok((O,W)),
                Err(e) => Err(MyError::FunctionCallErr(e))
            }
        }
    }

    fn execute(&self,args : JDict) -> AnyResult<JValue>;

    fn name(&self) -> &str;
    fn description(&self) -> String { String::new() }
    fn parse_result(&self,result : JValue) -> ResultWithWarning<JValue>{ // 解析工具传回来的JSON
        Ok((result,None))
    }
}

// dyn_clone::clone_trait_object!(Arguments_T);
// #[derive(Debug,Clone)]
// enum Arguments_turnStatus {
//     UnConvernt(Box<dyn Arguments_T>),
//     Convernt(JDict)
// }

// pub struct Arguments{
//     data : Arguments_turnStatus,
// }

