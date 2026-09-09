mod app;
mod backend;
mod session;
mod ui;

use async_openai::{Client, config::OpenAIConfig};
use clap::Parser;
use dotenv::dotenv;
use std::{env, process};

use crate::app::App;

use crate::backend::llm::{ChatMessage, ChatMessageKind, run_agent_loop};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short = 'p', long)]
    prompt: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let args = Args::parse();

    let base_url = env::var("LOCAL_OLLAMA_URL").unwrap_or(
        env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
    );
    eprintln!("Using Base URL: {}", base_url);

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
    eprintln!("Using Model: {}", model);

    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    if let Some(arg_prompt) = args.prompt {
        // Prompt mode: read the prompt args and runs the agent loop once, then exits the program
        eprintln!("Gonna run prompt mode");
        let messages: Vec<ChatMessage> = vec![ChatMessage {
            kind: ChatMessageKind::User,
            role: String::from("user"),
            content: Some(arg_prompt),
        }];
        rt.block_on(run_agent_loop(client, model, messages))?;
    } else {
        // Chat mode: Launches the full chat TUI
        eprintln!("Gonna run Chat mode TUI");
        ratatui::run(|terminal| rt.block_on(App::new().run(terminal)))?;
    }

    // println!("{:?}", messages);

    Ok(())
}
