use async_openai::{Client, config::OpenAIConfig};
use clap::Parser;
use serde_json::{Value, json};
use std::{env, fs, process, str::FromStr};
use dotenv::dotenv;

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

fn tool_call(name: &str, arguments: &str) {
   match name {
      "Read" => read_tool(arguments),
      _ => println!("Unknown tool called") 
   } 
}

fn read_tool(arguments: &str) {
    match Value::from_str(arguments) {
        Ok(args) => match args["file_path"].as_str() {
            Some(file_path) => match fs::read_to_string(file_path) {
                Ok(content) => println!("{}", content),
                Err(_err) => println!("Cant read the file: {}", file_path),
            },
            None => (),
        },
        Err(_err) => println!("json parse error in args"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let args = Args::parse();

    let base_url = env::var("OPENROUTER_BASE_URL")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

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
        "nvidia/nemotron-3-super-120b-a12b:free"
    } else {
        "anthropic/claude-haiku-4.5"
    };

    #[allow(unused_variables)]
    let response: Value = client
        .chat()
        .create_byot(json!({
            "messages": [
                {
                    "role": "user",
                    "content": args.prompt
                }
            ],
            "model": model,
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "Read",
                        "description": "Read and return the contents of a file",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "file_path": {
                                    "type": "string",
                                    "description": "The path to the file to read"
                                }
                            },
                            "required": ["file_path"]
                        }
                    }
                }
            ],
        }))
        .await?;

    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    // Extract the response kind
    let response_kind: Option<ChatFinishKind> = match response["choices"][0]["finish_reason"].as_str() {
        Some(reason) => ChatFinishKind::from_reason(reason),
        None => None,
    };

    // Check if normal response or tool call
    match response_kind {
        Some(kind) => match kind {
            ChatFinishKind::Stop => {
                if let Some(content) = response["choices"][0]["message"]["content"].as_str() {
                println!("{}", content);
                }
            },
            ChatFinishKind::ToolCall => {
                if let Some(tool_calls) = response["choices"][0]["message"]["tool_calls"][0]["function"]["name"].as_str() {
                    // println!("Tool Call \"{}\"", tool_calls);
                    if let Some(arguments) = response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"].as_str()  {
                        tool_call(tool_calls, arguments);
                    }
                }
            },
        }
        None => println!("Unknown response type from LLM"),
    }

    Ok(())
}
