#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    Stop,
    ToolCall,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallId(pub String);

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub enum ToolStatus {
    Pending,
    Running,
    Complete { output: String },
    Failed { error: String },
}

#[derive(Debug)]
pub enum SessionUpdate {
    AgentMessageChunk {
        session: SessionId,
        text: String,
    },
    ToolCallStarted {
        session: SessionId,
        call: ToolCall,
    },
    ToolCallUpdate {
        session: SessionId,
        id: ToolCallId,
        status: ToolStatus,
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
