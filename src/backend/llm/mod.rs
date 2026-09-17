use async_openai::{Client, config::OpenAIConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

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
    cancel: Option<CancellationToken>,
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
            cancel: None,
        }
    }
    pub fn prompt(&mut self, session: SessionId, text: String) {
        let cancel_token = CancellationToken::new();
        self.cancel = Some(cancel_token.clone());

        let history = self.history.clone();
        let (client, config, tx) = (self.client.clone(), self.config.clone(), self.tx.clone());

        tokio::spawn(async move {
            let mut history_guard = history.lock().await;
            history_guard.push(ChatMessage {
                kind: ChatMessageKind::User,
                role: String::from("user"),
                content: Some(text),
            });

            if let Err(e) = run_agent_loop(
                &client,
                &config,
                &mut history_guard,
                session,
                &tx,
                &cancel_token,
            )
            .await
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

    pub fn cancel(&mut self) {
        if let Some(token) = self.cancel.take() {
            token.cancel();
        }
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
struct ChatChunk {
    choices: Vec<ChoiceChunk>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChoiceChunk {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    //reasoning: Option<String>,
    tool_calls: Option<Vec<ToolCallChunk>>,
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

#[derive(Debug, Deserialize)]
struct ToolCallChunk {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ToolFunctionChunk>,
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

#[derive(Debug, Deserialize)]
struct ToolFunctionChunk {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl PartialToolCall {
    fn push_chunk(&mut self, chunk: ToolCallChunk) {
        if let Some(id) = chunk.id {
            self.id = id;
        }

        if let Some(function) = chunk.function {
            if let Some(name) = function.name {
                self.name = name;
            }

            if let Some(arguments) = function.arguments {
                self.arguments.push_str(&arguments);
            }
        }
    }
}

impl From<PartialToolCall> for WireToolCall {
    fn from(value: PartialToolCall) -> Self {
        WireToolCall {
            id: value.id,
            r#type: "function".into(),
            function: WireToolCallArgs {
                name: value.name,
                arguments: value.arguments,
            },
        }
    }
}

#[derive(Debug, Default)]
struct ChatStreamState {
    text: String,
    finish_kind: Option<ChatFinishKind>,
    tool_calls: BTreeMap<usize, PartialToolCall>,
}

impl ChatStreamState {
    fn push_chunk(&mut self, chunk: ChatChunk, session: SessionId) -> Vec<SessionUpdate> {
        let mut updates = Vec::new();

        // read the first choice only, ignore multiple choice
        let Some(choice) = chunk.choices.into_iter().next() else {
            return updates;
        };

        if let Some(text) = choice.delta.content {
            self.text.push_str(&text);

            updates.push(SessionUpdate::AgentMessageChunk { session, text });
        }

        // tool calls
        if let Some(chunk_calls) = choice.delta.tool_calls {
            for call in chunk_calls {
                let partial_entry = self.tool_calls.entry(call.index).or_default();
                partial_entry.push_chunk(call);
            }
        }

        if let Some(reason) = choice.finish_reason.as_deref() {
            self.finish_kind =
                Some(ChatFinishKind::from_reason(reason).unwrap_or(ChatFinishKind::Stop));
        }

        updates
    }
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
    config: &ProviderConfig,
    messages: &mut Vec<ChatMessage>,
    session: SessionId,
    tx: &mpsc::Sender<SessionUpdate>,
    cancel_token: &CancellationToken,
) -> Result<Option<ChatFinishKind>, Box<dyn std::error::Error + Send + Sync>> {
    // println!("Model: {}", model);

    let request_body = ChatRequest {
        model: config.model.clone(),
        messages,
        tools,
        stream: config.streaming,
    };

    if config.streaming {
        let mut stream = client.chat().create_stream_byot(request_body).await?;
        let mut state = ChatStreamState::default();

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => return Ok(None),
                maybe_chunk = stream.next() => {
                    let Some(next_chunk) = maybe_chunk else { break };
                    let chunk: ChatChunk = next_chunk?;
                    let updates = state.push_chunk(chunk, session);
                    for update in updates {
                        tx.send(update).await?
                    }
                }
            }
        }

        match &state.finish_kind {
            Some(ChatFinishKind::ToolCall) => {
                let calls: Vec<WireToolCall> = state
                    .tool_calls
                    .into_values()
                    .map(WireToolCall::from)
                    .collect();

                messages.push(ChatMessage {
                    kind: ChatMessageKind::ToolCalls {
                        tool_calls: calls.clone(),
                    },
                    role: "assistant".into(),
                    content: (!state.text.is_empty()).then_some(state.text),
                });

                for call in calls {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    let tool_message = run_tool(call, session, tx).await;
                    messages.push(tool_message);
                }
            }
            Some(ChatFinishKind::Stop) => {
                messages.push(ChatMessage {
                    kind: ChatMessageKind::Assistant,
                    role: "assistant".into(),
                    content: Some(state.text),
                });
            }
            _ => {}
        }

        return Ok(state.finish_kind);
    }

    let response: Value = client.chat().create_byot(request_body).await?;

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

async fn run_agent_loop(
    client: &Client<OpenAIConfig>,
    config: &ProviderConfig,
    history: &mut Vec<ChatMessage>,
    session: SessionId,
    tx: &mpsc::Sender<SessionUpdate>,
    cancel_token: &CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // todo: move this outside of run_agent_loop
    let tools = tools::specs().iter().map(WireTool::from).collect();

    let turn_checkpoint = history.len();

    loop {
        if cancel_token.is_cancelled() {
            history.truncate(turn_checkpoint);
            tx.send(SessionUpdate::TurnEnd {
                session,
                reason: StopReason::Cancelled,
            })
            .await?;
            break;
        }

        match agent_loop_step(client, &tools, config, history, session, tx, cancel_token).await? {
            Some(ChatFinishKind::Stop) => {
                tx.send(SessionUpdate::TurnEnd {
                    session,
                    reason: StopReason::Stop,
                })
                .await?;
                break;
            }
            Some(ChatFinishKind::ToolCall) => continue,
            None if cancel_token.is_cancelled() => continue, // aborts on next iteration
            None => {
                tx.send(SessionUpdate::Failed {
                    session,
                    error: "Unexpected agent loop break: Unknown finish_reason".into(),
                })
                .await?;
                //println!("Unexpected agent loop break");
                break;
            }
        }
    }
    Ok(())
}
