use async_openai::{Client, config::OpenAIConfig};
use clap::Parser;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, env, fs, process, str::FromStr};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short = 'p', long)]
    prompt: String,
}

#[derive(Debug)]
enum ChatFinishKind {
    Stop,
    ToolCall,
}

impl ChatFinishKind {
    fn from_reason(reason: &str) -> Option<ChatFinishKind> {
        match reason {
            "stop" => Some(ChatFinishKind::Stop),
            "tool_calls" => Some(ChatFinishKind::ToolCall),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolDefProperty {
    #[serde(rename = "type")]
    kind: String,
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolDefParameters {
    #[serde(rename = "type")]
    kind: String,
    required: Vec<String>,
    properties: HashMap<String, ToolDefProperty>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolDefFunction {
    name: String,
    description: String,
    parameters: ToolDefParameters,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolDef {
    #[serde(rename = "type")]
    kind: String,
    function: ToolDefFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCallArgs {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    r#type: String,
    function: ToolCallArgs,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ChatMessageKind {
    User,
    Assistant,
    Tool { tool_call_id: String },
    ToolCalls { tool_calls: Vec<ToolCall> },
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    // #[serde(skip_serializing)]
    #[serde(flatten)]
    kind: ChatMessageKind,
    role: String,
    content: Option<String>,
}

fn tool_call(id: &str, name: &str, arguments: &str) -> Option<ChatMessage> {
    match name {
        "Read" => {
            let content = read_tool(arguments);
            content.map(|text| {
                Some(ChatMessage {
                    kind: ChatMessageKind::Tool {
                        tool_call_id: String::from(id),
                    },
                    role: String::from("tool"),
                    content: Some(text),
                })
            })?
            // match content {
            //    Some(text) => Some(ChatMessage {
            //            kind: ChatMessageKind::Tool {
            //            tool_call_id: String::from(id),
            //        },
            //        role: String::from("tool"),
            //        content: Some(text),
            //    }),
            //    None => None,
            //}
        }
        _ => {
            println!("Unknown tool called");
            None
        }
    }
}

fn read_tool(arguments: &str) -> Option<String> {
    match Value::from_str(arguments) {
        Ok(args) => match args["file_path"].as_str() {
            Some(file_path) => match fs::read_to_string(file_path) {
                Ok(content) => Some(content),
                Err(_err) => {
                    println!("Cant read the file: {}", file_path);
                    None
                }
            },
            None => None,
        },
        Err(_err) => {
            println!("json parse error in args");
            None
        }
    }
}

async fn agent_loop_step(
    client: &Client<OpenAIConfig>,
    model: &String,
    messages: &mut Vec<ChatMessage>,
) -> Result<Option<ChatFinishKind>, Box<dyn std::error::Error>> {
    // println!("Model: {}", model);

    let read_tool_def = ToolDef {
        kind: String::from("function"),
        function: ToolDefFunction {
            name: String::from("Read"),
            description: String::from("Read and return the contents of a file"),
            parameters: ToolDefParameters {
                kind: String::from("object"),
                required: vec![String::from("file_path")],
                properties: HashMap::from([(
                    String::from("file_path"),
                    ToolDefProperty {
                        kind: String::from("string"),
                        description: String::from("The path to the file to read"),
                    },
                )]),
            },
        },
    };

    let write_tool_def = ToolDef {
        kind: String::from("function"),
        function: ToolDefFunction {
            name: String::from("Write"),
            description: String::from("Write content to a file"),
            parameters: ToolDefParameters {
                kind: String::from("object"),
                required: vec![String::from("file_path"), String::from("content")],
                properties: HashMap::from([
                    (
                        String::from("file_path"),
                        ToolDefProperty {
                            kind: String::from("string"),
                            description: String::from("The path of the file to write to"),
                        },
                    ),
                    (
                        String::from("content"),
                        ToolDefProperty {
                            kind: String::from("string"),
                            description: String::from("The content to write to the file"),
                        },
                    ),
                ]),
            },
        },
    };

    let response: Value = client
        .chat()
        .create_byot(json!({
            "messages": messages,
            "model": model,
            "tools": [
                read_tool_def,
                write_tool_def,
            ],
        }))
        .await?;

    // Extract the response kind
    let response_kind: Option<ChatFinishKind> =
        match response["choices"][0]["finish_reason"].as_str() {
            Some(reason) => ChatFinishKind::from_reason(reason),
            None => None,
        };

    // Check if normal response or tool call
    match &response_kind {
        Some(kind) => match kind {
            ChatFinishKind::Stop => {
                if let Some(content) = response["choices"][0]["message"]["content"].as_str() {
                    println!("{}", content);
                    let message = ChatMessage {
                        kind: ChatMessageKind::Assistant,
                        role: String::from("assistant"),
                        content: Some(String::from(content)),
                    };
                    messages.push(message);
                }
            }
            ChatFinishKind::ToolCall => {
                let tool_call_data = response["choices"][0]["message"]["tool_calls"].as_array();
                if let Some(tools) = tool_call_data {
                    for tool in tools {
                        let tool_call_id = tool["id"].as_str().expect("tool_call_id found in json");
                        let tool_name = tool["function"]["name"]
                            .as_str()
                            .expect("tool_name not found in json");
                        let tool_args = tool["function"]["arguments"]
                            .as_str()
                            .expect("tool_args not found in json");

                        let tool_call_0 = serde_json::from_value::<ToolCall>(
                            response["choices"][0]["message"]["tool_calls"][0].clone(),
                        )
                        .expect("tool_args not found in json");
                        messages.push(ChatMessage {
                            kind: ChatMessageKind::ToolCalls {
                                tool_calls: vec![tool_call_0],
                            },
                            role: String::from("assistant"),
                            content: None,
                        });

                        if let Some(message) = tool_call(tool_call_id, tool_name, tool_args) {
                            messages.push(message);
                        }
                    }
                }
            }
        },
        None => println!("Unknown response type from LLM"),
    };

    Ok(response_kind)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let args = Args::parse();

    let base_url = env::var("LOCAL_OLLAMA_URL").unwrap_or(
        env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
    );
    eprintln!("Using Base URL: {}", &base_url);

    let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
        eprintln!("OPENROUTER_API_KEY is not set");
        process::exit(1);
    });

    let config = OpenAIConfig::new()
        .with_api_base(base_url)
        .with_api_key(api_key);

    let client = Client::with_config(config);

    // switch model so that tests pass on codecrafter
    let is_local = env::var("LOCAL")
        .map(|local| local == "true")
        .unwrap_or(false);
    let model = if is_local {
        env::var("LOCAL_MODEL")
            .unwrap_or_else(|_| String::from("nvidia/nemotron-3-super-120b-a12b:free"))
    } else {
        String::from("anthropic/claude-haiku-4.5")
    };
    eprintln!("Using Model: {}", &model);

    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    let mut messages: Vec<ChatMessage> = Vec::new();
    messages.push(ChatMessage {
        kind: ChatMessageKind::User,
        role: String::from("user"),
        content: Some(args.prompt),
    });

    loop {
        match agent_loop_step(&client, &model, &mut messages).await? {
            Some(ChatFinishKind::Stop) => break,
            Some(ChatFinishKind::ToolCall) => (),
            None => {
                println!("Unexpected agent loop break");
                break;
            }
        }
    }

    // println!("{:?}", messages);

    Ok(())
}
