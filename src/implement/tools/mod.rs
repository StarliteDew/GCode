pub mod bash;
pub mod read;
pub mod ssh;
pub mod write;

pub use bash::Bash;
pub use read::ReadFile;
pub use ssh::{ListChannel, SshClose, SshCloseAll, SshExecute, SshKeyLogin, SshPasswordLogin};
pub use write::WriteFile;

use crate::Framework::Framework;
use crate::Structures::Trait::Provider_API_T;

/// 一次性把内置工具全部注册到框架
pub fn register_all<T: Provider_API_T>(framework: &mut Framework<T>) -> crate::Errors::MyResult<()> {
    framework.add_tool(Bash, false)?;
    framework.add_tool(ReadFile, false)?;
    framework.add_tool(WriteFile, false)?;
    framework.add_tool(SshPasswordLogin, false)?;
    framework.add_tool(SshKeyLogin, false)?;
    framework.add_tool(ListChannel, false)?;
    framework.add_tool(SshExecute, false)?;
    framework.add_tool(SshClose, false)?;
    framework.add_tool(SshCloseAll, false)?;
    Ok(())
}
