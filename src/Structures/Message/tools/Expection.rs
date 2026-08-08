use super::Imports::*;
use super::Structures::Arguments_Error_Missing_args;
use std::fmt;
use serde_json::json;

#[derive(Debug)]
pub enum MyError {
    MissingArgs(Vec<Arguments_Error_Missing_args>),
    FunctionCallErr(AnyError),
    Other(AnyError),
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyError::MissingArgs(args) => {
                writeln!(f, "arguments Error:")?;
                for arg in args {
                    writeln!(f, "\t{} : {}", arg.name, arg.Error_desc)?;
                }
                Ok(())
            }
            MyError::FunctionCallErr(e) => write!(f, "function call error: {}", e),
            MyError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MyError::MissingArgs(_) => None,
            MyError::FunctionCallErr(e) => Some(&**e as &(dyn std::error::Error + 'static)),
            MyError::Other(e) => Some(&**e as &(dyn std::error::Error + 'static)),
        }
    }
}

impl From<AnyError> for MyError {
    fn from(e: AnyError) -> Self {
        MyError::Other(e)
    }
}


pub type ResultWithWarning<T> = Result<(T,Option<String>),MyError>;
pub type MyResult<T> = Result<T,MyError>;

pub fn Warning_to_Result<T>(r : ResultWithWarning<T>) -> MyResult<T>{
    match r{
        Ok(O) => Ok(O.0),
        Err(v) => Err(v)
    }
}


pub fn Warning_to_JValue(r : &ResultWithWarning<JValue>) -> JValue{
    let mut result : JDict = JDict::new();
        let call_result = r;
        match call_result {
            Ok((v,w)) => {
                match w{
                    Some(w) => {
                        result.insert("status".into(), json!("Ok(Warning)"));
                        result.insert("warning".into(), JValue::String(w.to_string()));
                        result.insert("result".into(), v.clone());
                    },
                    None => {
                        result.insert("status".into(), json!("Ok"));
                        // result.insert("warning".into(), JValue::String(w));
                        result.insert("result".into(), v.clone());
                    }
                }
            },
            Err(e) => {
                result.insert("status".into(), json!("Error"));
                result.insert("result".into(),JValue::String(format!("{}",e)));
            }
        }


        JValue::Object(result)
}
