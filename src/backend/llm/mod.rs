use async_openai::{Client, config::OpenAIConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs, process::Command, str::FromStr, sync::Arc};
use tokio::sync::{Mutex, mpsc};

use crate::session::{SessionId, SessionUpdate, StopReason};

#[derive(Debug)]
pub struct LlmBackend {
    client: Client<OpenAIConfig>,
    config: ProviderConfig,
    history: Arc<Mutex<Vec<ChatMessage>>>,
    tx: mpsc::Sender<SessionUpdate>,
}

impl LlmBackend {
    pub fn new(config: ProviderConfig, tx: mpsc::Sender<SessionUpdate>) -> Self {
        let client_config = OpenAIConfig::new()
            .with_api_base(&config.base_url)
            .with_api_key(config.api_key.as_deref().unwrap_or("unused"));
        let client = Client::with_config(client_config);
        Self {
            client,
            config,
            history: Arc::new(Mutex::new(Vec::new())),
            tx,
        }
    }
    pub fn prompt(&mut self, session: SessionId, text: String) {
        let history = self.history.clone();
        let (client, config, tx) = (self.client.clone(), self.config.clone(), self.tx.clone());

        tokio::spawn(async move {
            let mut history_guard = history.lock().await;
            history_guard.push(ChatMessage {
                kind: ChatMessageKind::User,
                role: String::from("user"),
                content: Some(text),
            });

            if let Err(e) = run_agent_loop(&client, &config, &mut history_guard, session, &tx).await
            {
                let _ = tx
                    .send(SessionUpdate::Failed {
                        session,
                        error: e.to_string(),
                    })
                    .await;
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub streaming: bool,
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

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: String,
    messages: &'a [ChatMessage],
    tools: &'a [ToolDef],
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    // #[serde(skip_serializing)]
    #[serde(flatten)]
    pub kind: ChatMessageKind,
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageKind {
    User,
    Assistant,
    Tool { tool_call_id: String },
    ToolCalls { tool_calls: Vec<ToolCall> },
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    r#type: String,
    function: ToolCallArgs,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCallArgs {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolDef {
    #[serde(rename = "type")]
    kind: String,
    function: ToolDefFunction,
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

fn print_tool_reasoning(tool_name: &str, reasoning: &str) {
    eprintln!("/********\\");
    eprintln!("Reasoning to call tool: {tool_name}");
    eprintln!("------",);
    eprintln!("{reasoning}");
    eprintln!("---");
    eprintln!("\\********/");
    eprintln!("\n");
}

fn tool_call(id: &str, name: &str, arguments: &str) -> Option<ChatMessage> {
    let mut content = None;
    match name {
        "Read" => {
            content = read_tool(arguments);
        }
        "Write" => {
            content = write_tool(arguments);
        }
        "Bash" => {
            content = bash_tool(arguments);
        }
        _ => {
            println!("Unknown tool called: {:?}", name);
        }
    }
    content.map(|text| ChatMessage {
        kind: ChatMessageKind::Tool {
            tool_call_id: String::from(id),
        },
        role: String::from("tool"),
        content: Some(text),
    })
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

fn write_tool(arguments: &str) -> Option<String> {
    let args = Value::from_str(arguments).ok()?;
    let file_path = args["file_path"].as_str()?;
    let content = args["content"].as_str()?;

    fs::write(file_path, content).ok()?;
    eprintln!("Write Successful to the file: {}", file_path);
    Some(String::from("Done."))
}

fn bash_tool(arguments: &str) -> Option<String> {
    let args = Value::from_str(arguments).ok()?;
    let command = args["command"].as_str()?;

    let res = Command::new("sh").arg("-c").arg(command).output().unwrap();

    let stdout = String::from_utf8_lossy(&res.stdout).to_string();
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();

    if res.status.success() {
        eprintln!("Command Executed on sh {:?}", command);
        Some(stdout)
    } else {
        eprintln!("Failed to Execute on sh {:?}", command);
        Some(stderr)
    }
}

fn build_tool_defs() -> Vec<ToolDef> {
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

    let bash_tool_def = ToolDef {
        kind: String::from("function"),
        function: ToolDefFunction {
            name: String::from("Bash"),
            description: String::from("Execute a shell command"),
            parameters: ToolDefParameters {
                kind: String::from("object"),
                required: vec![String::from("command")],
                properties: HashMap::from([(
                    String::from("command"),
                    ToolDefProperty {
                        kind: String::from("string"),
                        description: String::from("The command to execute"),
                    },
                )]),
            },
        },
    };

    vec![read_tool_def, write_tool_def, bash_tool_def]
}

async fn agent_loop_step(
    client: &Client<OpenAIConfig>,
    tools: &Vec<ToolDef>,
    model: &String,
    messages: &mut Vec<ChatMessage>,
    session: SessionId,
    tx: &mpsc::Sender<SessionUpdate>,
) -> Result<Option<ChatFinishKind>, Box<dyn std::error::Error + Send + Sync>> {
    // println!("Model: {}", model);

    let response: Value = client
        .chat()
        .create_byot(ChatRequest {
            model: model.clone(),
            messages,
            tools,
            stream: false,
        })
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
                    //println!("{}", content);

                    // Update backend history
                    let message = ChatMessage {
                        kind: ChatMessageKind::Assistant,
                        role: String::from("assistant"),
                        content: Some(String::from(content)),
                    };
                    messages.push(message);

                    // Send event for App UI Update
                    tx.send(SessionUpdate::AgentMessageChunk {
                        session,
                        text: String::from(content),
                    })
                    .await?;
                }
            }
            ChatFinishKind::ToolCall => {
                let tool_call_data = response["choices"][0]["message"]["tool_calls"].as_array();
                if let Some(tools) = tool_call_data {
                    for (i, tool) in tools.iter().enumerate() {
                        let tool_call_id = tool["id"].as_str().expect("tool_call_id found in json");
                        let tool_name = tool["function"]["name"]
                            .as_str()
                            .expect("tool_name not found in json");
                        let tool_args = tool["function"]["arguments"]
                            .as_str()
                            .expect("tool_args not found in json");

                        // print reasoning for every tool call
                        if let Some(reasoning) =
                            response["choices"][0]["message"]["reasoning"].as_str()
                        {
                            print_tool_reasoning(tool_name, reasoning);
                        }

                        let tool_call_i = serde_json::from_value::<ToolCall>(
                            response["choices"][0]["message"]["tool_calls"][i].clone(),
                        )
                        .expect("tool_call_i not found in json");
                        messages.push(ChatMessage {
                            kind: ChatMessageKind::ToolCalls {
                                tool_calls: vec![tool_call_i],
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

pub async fn run_agent_loop(
    client: &Client<OpenAIConfig>,
    config: &ProviderConfig,
    history: &mut Vec<ChatMessage>,
    session: SessionId,
    tx: &mpsc::Sender<SessionUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // todo: move this outside of run_agent_loop
    let tools = build_tool_defs();

    loop {
        match agent_loop_step(&client, &tools, &config.model, history, session, tx).await? {
            Some(ChatFinishKind::Stop) => {
                tx.send(SessionUpdate::TurnEnd {
                    session,
                    reason: StopReason::Stop,
                })
                .await?;
                break;
            }
            Some(ChatFinishKind::ToolCall) => (),
            None => {
                println!("Unexpected agent loop break");
                break;
            }
        }
    }
    Ok(())
}
