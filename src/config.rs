use std::{env, error::Error};

use crate::backend::llm::ProviderConfig;

pub struct Config {
    pub provider: ProviderConfig,
    // more later like theme, keymaps
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let base_url = env::var("LOCAL_OLLAMA_URL").unwrap_or(
            env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
        );
        eprintln!("Using Base URL: {}", base_url);

        let api_key = env::var("OPENROUTER_API_KEY").ok();

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

        Ok(Self {
            provider: ProviderConfig {
                base_url,
                api_key,
                model,
                streaming: true,
            },
        })
    }
}
