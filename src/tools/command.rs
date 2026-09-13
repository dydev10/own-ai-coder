use std::process::Command;

use serde::Deserialize;
use serde_json::json;

use crate::tools::{ToolKind, ToolResult, ToolSpec};

#[derive(Debug, Deserialize)]
struct BashArgs {
    command: String,
}

pub async fn bash(arguments: &str) -> ToolResult {
    let args = match serde_json::from_str::<BashArgs>(arguments) {
        Ok(a) => a,
        Err(e) => {
            return ToolResult {
                output: format!("invalid arguments: {e}"),
                success: false,
            };
        }
    };

    let res = match Command::new("sh").arg("-c").arg(&args.command).output() {
        Ok(o) => o,
        Err(e) => {
            // eprintln!("Failed to Execute on sh {}", arg.command);
            return ToolResult {
                output: format!("could not start shell: {e}"),
                success: false,
            };
        }
    };

    let mut text = String::new();
    let stdout = String::from_utf8_lossy(&res.stdout).to_string();
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();

    if !stdout.is_empty() {
        text.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
            text.push_str("stderr:\n");
        }
        text.push_str(&stderr);
    }

    if !res.status.success() {
        text.push('\n');
        text.push_str(&format!("exit code {}", res.status.code().unwrap_or(-1)));
    }

    if text.is_empty() {
        text.push_str("(no output)");
    }

    ToolResult {
        output: text,
        success: res.status.success(),
    }

    //let args = Value::from_str(arguments).ok()?;
    //let command = args["command"].as_str()?;

    //let res = Command::new("sh").arg("-c").arg(command).output().unwrap();

    //let stdout = String::from_utf8_lossy(&res.stdout).to_string();
    //let stderr = String::from_utf8_lossy(&res.stderr).to_string();

    //if res.status.success() {
    //    eprintln!("Command Executed on sh {:?}", command);
    //    Some(stdout)
    //} else {
    //    eprintln!("Failed to Execute on sh {:?}", command);
    //    Some(stderr)
    //}
}

// tool specs
pub fn bash_spec() -> ToolSpec {
    ToolSpec {
        name: ToolKind::Bash.name(),
        description: "Run a shell command. Each call is a fresh shell.",
        parameters: json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                }
            }
        }),
    }

    //let bash_tool_def = WireTool {
    //    kind: String::from("function"),
    //    function: ToolDefFunction {
    //        name: String::from("Bash"),
    //        description: String::from("Execute a shell command"),
    //        parameters: ToolDefParameters {
    //            kind: String::from("object"),
    //            required: vec![String::from("command")],
    //            properties: HashMap::from([(
    //                String::from("command"),
    //                ToolDefProperty {
    //                    kind: String::from("string"),
    //                    description: String::from("The command to execute"),
    //                },
    //            )]),
    //        },
    //    },
    //};
}
