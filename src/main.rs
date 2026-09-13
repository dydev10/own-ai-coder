mod app;
mod backend;
mod cli;
mod config;
mod session;
mod tools;
mod ui;

use clap::Parser;
use dotenv::dotenv;

use crate::app::App;

use crate::config::Config;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short = 'p', long)]
    prompt: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();

    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let args = Args::parse();
    let app_config = Config::load()?;

    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    if let Some(arg_prompt) = args.prompt {
        // Prompt mode: read the prompt args and runs the agent loop once, then exits the program
        eprintln!("Gonna run prompt mode");
        rt.block_on(cli::run(app_config, arg_prompt));
    } else {
        // Chat mode: Launches the full chat TUI
        eprintln!("Gonna run Chat mode TUI");
        ratatui::run(|terminal| rt.block_on(App::new(app_config).run(terminal)))?;
    }

    Ok(())
}
