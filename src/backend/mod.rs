pub mod llm;

use crate::{backend::llm::LlmBackend, session::SessionId};

pub enum Backend {
    Llm(LlmBackend),
    //Acp(AcpBackend),    // to be implemented: runs agent loop out of process
}

impl Backend {
    pub fn prompt(&mut self, session: SessionId, text: String) {
        match self {
            Backend::Llm(llm) => llm.prompt(session, text),
            // more backend will be matched here
        }
    }
}
