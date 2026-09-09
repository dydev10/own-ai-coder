#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    Stop,
    ToolCall,
    Cancelled,
    Error,
}

#[derive(Debug)]
pub enum SessionUpdate {
    AgentMessageChunk {
        session: SessionId,
        text: String,
    },
    TurnEnd {
        session: SessionId,
        reason: StopReason,
    },
    Failed {
        session: SessionId,
        error: String,
    },
}
