use std::collections::HashMap;
pub use std::fmt::Debug;

pub use serde_json::Value as JValue;
pub use serde_json::Map as JMap;
pub use anyhow::Result as AnyResult;
pub use anyhow::Error as AnyError;

pub type JDict = JMap<String,JValue>;
pub type Map<k,v> = crate::Const::DEFAULT_MAP<k,v>;

