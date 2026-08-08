use std::collections::HashMap;
type Map<k,v> = crate::Const::DEFAULT_MAP<k,v>;
pub type ID = i64;
mod Allocation;

pub use Allocation::{Role,new_role,USER,SYSTEM,ASSISTANT};

