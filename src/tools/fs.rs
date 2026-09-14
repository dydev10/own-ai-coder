use serde::Deserialize;
use serde_json::json;

use crate::tools::{ToolKind, ToolResult, ToolSpec};

#[derive(Debug, Deserialize)]
struct ReadFileArgs {
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

pub async fn read_file(arguments: &str) -> ToolResult {
    let args = match serde_json::from_str::<ReadFileArgs>(arguments) {
        Ok(a) => a,
        Err(e) => {
            return ToolResult {
                output: format!("invalid arguments: {e}"),
                success: false,
            };
        }
    };

    match tokio::fs::read_to_string(&args.file_path).await {
        Ok(content) => ToolResult {
            output: content,
            success: true,
        },
        Err(e) => {
            //eprintln!("Cant read the file: {}", args.file_path);
            ToolResult {
                output: e.to_string(),
                success: false,
            }
        }
    }
}

pub async fn write_file(arguments: &str) -> ToolResult {
    let args = match serde_json::from_str::<WriteFileArgs>(arguments) {
        Ok(a) => a,
        Err(e) => {
            return ToolResult {
                output: format!("invalid arguments: {e}"),
                success: false,
            };
        }
    };

    match tokio::fs::write(&args.file_path, &args.content).await {
        Ok(()) => {
            //eprintln!("Write Successful to the file: {}", args.file_path);
            ToolResult {
                output: format!("wrote {} bytes to {}", args.content.len(), args.file_path),
                success: true,
            }
        }
        Err(e) => {
            //eprintln!("Can not write to the file: {}", args.file_path);
            ToolResult {
                output: format!("failed to write {}: {e}", args.file_path),
                success: false,
            }
        }
    }
}

// tool specs
pub fn read_file_spec() -> ToolSpec {
    ToolSpec {
        name: ToolKind::ReadFile.name(),
        description: "Read contents of a file",
        parameters: json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to read",
                }
            }
        }),
    }
}

pub fn write_file_spec() -> ToolSpec {
    ToolSpec {
        name: ToolKind::WriteFile.name(),
        description: "Write content to a file. The file is created if missing and overwritten if not.",
        parameters: json!({
            "type": "object",
            "required": ["file_path", "content"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the file to write to",
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file",
                }
            }
        }),
    }
}

// primary arg
pub fn read_file_primary_arg(arguments: &str) -> Option<String> {
    serde_json::from_str::<ReadFileArgs>(arguments)
        .ok()
        .map(|a| a.file_path)
}

pub fn write_file_primary_arg(arguments: &str) -> Option<String> {
    serde_json::from_str::<WriteFileArgs>(arguments)
        .ok()
        .map(|a| a.file_path)
}
