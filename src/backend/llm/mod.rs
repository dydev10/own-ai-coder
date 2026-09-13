use async_openai::{Client, config::OpenAIConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::{
    session::{self, SessionId, SessionUpdate, StopReason},
    tools,
};

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
    tools: &'a [WireTool],
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
struct ChatMessage {
    // #[serde(skip_serializing)]
    #[serde(flatten)]
    pub kind: ChatMessageKind,
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ChatMessageKind {
    User,
    Assistant,
    Tool { tool_call_id: String },
    ToolCalls { tool_calls: Vec<WireToolCall> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireToolCall {
    id: String,
    r#type: String,
    function: WireToolCallArgs,
}

impl From<WireToolCall> for session::ToolCall {
    fn from(value: WireToolCall) -> Self {
        session::ToolCall {
            id: session::ToolCallId(value.id),
            name: value.function.name,
            arguments: value.function.arguments,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireToolCallArgs {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireTool {
    #[serde(rename = "type")]
    kind: String,
    function: WireToolFunction,
}

impl From<&tools::ToolSpec> for WireTool {
    fn from(s: &tools::ToolSpec) -> Self {
        Self {
            kind: "function".into(),
            function: WireToolFunction {
                name: s.name.into(),
                description: s.description.into(),
                parameters: s.parameters.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

fn print_tool_reasoning(tool_name: &str, reasoning: &str) {
    return;
    eprintln!("/********\\");
    eprintln!("Reasoning to call tool: {tool_name}");
    eprintln!("------",);
    eprintln!("{reasoning}");
    eprintln!("---");
    eprintln!("\\********/");
    eprintln!("\n");
}

async fn run_tool(
    tool_call: WireToolCall,
    session: SessionId,
    tx: &mpsc::Sender<SessionUpdate>,
) -> ChatMessage {
    let session_tool_call: session::ToolCall = tool_call.clone().into();

    // Notify app about tool call before it starts
    let _ = tx
        .send(SessionUpdate::ToolCallStarted {
            session,
            call: session_tool_call.clone(),
        })
        .await;

    let res = tools::execute(&tool_call.function.name, &tool_call.function.arguments).await;

    let tool_status = if res.success {
        session::ToolStatus::Complete {
            output: res.output.clone(),
        }
    } else {
        session::ToolStatus::Failed {
            error: res.output.clone(),
        }
    };

    // Notify app about tool finish with status
    let _ = tx
        .send(SessionUpdate::ToolCallUpdate {
            session,
            id: session_tool_call.id,
            status: tool_status,
        })
        .await;

    ChatMessage {
        kind: ChatMessageKind::Tool {
            tool_call_id: tool_call.id.to_string(),
        },
        role: "tool".to_string(),
        content: Some(res.output),
    }
}

async fn agent_loop_step(
    client: &Client<OpenAIConfig>,
    tools: &Vec<WireTool>,
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
                        let tool_name = tool["function"]["name"]
                            .as_str()
                            .expect("tool_name not found in json");

                        // print reasoning for every tool call
                        if let Some(reasoning) =
                            response["choices"][0]["message"]["reasoning"].as_str()
                        {
                            print_tool_reasoning(tool_name, reasoning);
                        }

                        let tool_call_i = serde_json::from_value::<WireToolCall>(
                            response["choices"][0]["message"]["tool_calls"][i].clone(),
                        )
                        .expect("tool_call_i not found in json");
                        messages.push(ChatMessage {
                            kind: ChatMessageKind::ToolCalls {
                                tool_calls: vec![tool_call_i.clone()],
                            },
                            role: String::from("assistant"),
                            content: None,
                        });

                        let tool_output_message = run_tool(tool_call_i, session, tx).await;
                        messages.push(tool_output_message);
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
    let tools = tools::specs().iter().map(WireTool::from).collect();

    loop {
        match agent_loop_step(client, &tools, &config.model, history, session, tx).await? {
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
