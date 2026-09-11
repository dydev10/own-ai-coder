use tokio::sync::mpsc;

use crate::{
    backend::llm::LlmBackend,
    config::Config,
    session::{SessionId, SessionUpdate},
};

pub async fn run(config: Config, text: String) {
    let (tx, mut rx) = mpsc::channel(256);
    let mut backned = LlmBackend::new(config.provider, tx);
    let session = SessionId(1);

    // spawns the agent loop task
    backned.prompt(session, text);

    while let Some(update) = rx.recv().await {
        match update {
            SessionUpdate::AgentMessageChunk { text, .. } => print!("{text}"),
            SessionUpdate::TurnEnd { .. } => {
                println!(); // print new line because chunk print without new line
                break;
            }
            SessionUpdate::Failed { error, .. } => {
                eprintln!("error: {error}");
                break;
            }
        }
    }
}
