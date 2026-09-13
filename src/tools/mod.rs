use crate::tools::{
    command::bash_spec,
    fs::{read_file_spec, write_file_spec},
};

mod command;
mod fs;

pub enum ToolKind {
    ReadFile,
    WriteFile,
    Bash,
}

impl ToolKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "read_file" => Some(ToolKind::ReadFile),
            "write_file" => Some(ToolKind::WriteFile),
            "bash" => Some(ToolKind::Bash),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::Bash => "bash",
        }
    }
}

pub struct ToolResult {
    pub output: String,
    pub success: bool,
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

pub fn specs() -> Vec<ToolSpec> {
    vec![read_file_spec(), write_file_spec(), bash_spec()]
}

pub async fn execute(name: &str, arguments: &str) -> ToolResult {
    let Some(tool_kind) = ToolKind::from_name(name) else {
        return ToolResult {
            output: format!("unknown tool : {name}"),
            success: false,
        };
    };

    match tool_kind {
        ToolKind::ReadFile => fs::read_file(arguments).await,
        ToolKind::WriteFile => fs::write_file(arguments).await,
        ToolKind::Bash => command::bash(arguments).await,
    }
}
