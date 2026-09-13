use std::io::Result;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui_textarea::TextArea;
use tokio::{select, sync::mpsc};
use tokio_stream::StreamExt;

use crate::{
    backend::{Backend, llm::LlmBackend},
    config::Config,
    session::{SessionId, SessionUpdate, ToolCall, ToolCallId, ToolStatus},
    ui,
};

pub enum Status {
    Idle,
    Streaming,
    //Cancelling,
}

pub struct ToolItem {
    pub call: ToolCall,
    pub status: ToolStatus,
    pub expanded: bool,
}

pub enum Item {
    User(String),
    Assistant(String),
    //Thought(String),
    Tool(ToolItem),
    Error(String),
}

pub enum Action {
    Quit,
    Cancel,
    Submit,
    Input(KeyEvent),
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
}

pub struct ScrollState {
    pub offset: u16,
    pub pinned: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            pinned: true,
        }
    }
}

pub struct App {
    backend: Backend,
    updates: mpsc::Receiver<SessionUpdate>,
    session: SessionId,
    pub transcript: Vec<Item>,
    pub input: TextArea<'static>,
    pub scroll: ScrollState,
    pub status: Status,
    //pub pending: Option<PermissionRequest>,
    //pub keymap: KeyMap,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::channel(256);
        App {
            backend: Backend::Llm(LlmBackend::new(config.provider, tx)),
            updates: rx,
            session: SessionId(1), // only one active session for now, hardcoded to id 1
            //transcript: vec![],
            transcript: createMockItems(),
            input: TextArea::default(),
            scroll: ScrollState::default(),
            status: Status::Idle,
            should_quit: false,
        }
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut tui_events = crossterm::event::EventStream::new();
        while !self.should_quit {
            terminal.draw(|frame| ui::draw(frame, &self))?;

            select! {
                Some(Ok(event)) = tui_events.next() => {
                    if let Some(action) = self.handle_event(event) {
                        self.update(action);
                    }
                }
                Some(update) = self.updates.recv() => {
                    self.apply_session_update(update);
                }
            }
        }
        Ok(())
    }

    pub fn update(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Submit => {
                self.submit();
            }
            Action::Cancel => {
                eprintln!("Cancel will be triggered here");
            }
            Action::Input(key) => {
                self.input.input(key);
            }
            Action::ScrollUp => {
                let amount = self.page_height() / 2;
                self.scroll_up(amount);
            }
            Action::ScrollDown => {
                let amount = self.page_height() / 2;
                self.scroll_down(amount);
            }
            Action::PageUp => {
                let amount = self.page_height();
                self.scroll_up(amount);
            }
            Action::PageDown => {
                let amount = self.page_height();
                self.scroll_down(amount);
            }
        }
    }

    fn handle_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else { return None };
        if key.kind != KeyEventKind::Press {
            return None;
        }
        self.handle_key(key)
    }

    fn handle_key(&self, key: KeyEvent) -> Option<Action> {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Action::Quit),
            (KeyModifiers::NONE, KeyCode::Enter) => Some(Action::Submit),
            // scroll
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => Some(Action::ScrollUp),
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => Some(Action::ScrollDown),
            (KeyModifiers::NONE, KeyCode::PageUp) => Some(Action::PageUp),
            (KeyModifiers::NONE, KeyCode::PageDown) => Some(Action::PageDown),
            // text input fallthrough
            _ => Some(Action::Input(key)),
        }
    }

    // scroll handling
    fn max_scroll_offset(&self) -> u16 {
        ui::transcript::max_offset(&self.transcript, ui::main_area(self))
    }

    fn page_height(&self) -> u16 {
        ui::transcript::page_height(ui::main_area(self))
    }

    fn scroll_follow(&mut self) {
        if self.scroll.pinned {
            self.scroll.offset = self.max_scroll_offset();
        }
    }

    fn scroll_up(&mut self, amount: u16) {
        self.scroll.pinned = false;
        self.scroll.offset = self.scroll.offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        let max_scroll = self.max_scroll_offset();
        self.scroll.offset = self.scroll.offset.saturating_add(amount).min(max_scroll);
        self.scroll.pinned = self.scroll.offset == max_scroll;
    }

    fn submit(&mut self) {
        let text = self.input.lines().join("\n");
        if text.trim().is_empty() {
            return;
        }

        self.transcript.push(Item::User(text.clone()));
        self.input = TextArea::default();
        self.status = Status::Streaming;

        // submit the prompt to backend to start agent loop
        self.backend.prompt(self.session, text);

        // also pin scroll to bottom to show latest streamed content on Submit
        self.scroll.pinned = true;
        self.scroll_follow();
    }

    fn find_tool_item_mut(&mut self, id: &ToolCallId) -> Option<&mut ToolItem> {
        self.transcript
            .iter_mut()
            .rev()
            .find_map(|item| match item {
                Item::Tool(tool) if tool.call.id == *id => Some(tool),
                _ => None,
            })
    }

    fn apply_session_update(&mut self, update: SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk { text, .. } => match self.transcript.last_mut() {
                Some(Item::Assistant(s)) => s.push_str(&text),
                Some(_) | None => self.transcript.push(Item::Assistant(text)),
            },
            SessionUpdate::ToolCallStarted { call, .. } => {
                self.transcript.push(Item::Tool(ToolItem {
                    call,
                    status: ToolStatus::Pending,
                    expanded: false,
                }));
            }
            SessionUpdate::ToolCallUpdate { id, status, .. } => {
                if let Some(tool) = self.find_tool_item_mut(&id) {
                    tool.status = status;
                }
            }
            SessionUpdate::TurnEnd { .. } => self.status = Status::Idle,
            SessionUpdate::Failed { error, .. } => {
                self.transcript.push(Item::Error(error));
            }
        }
    }
}

fn createMockItems() -> Vec<Item> {
    vec![
        Item::User("what is ownership in rust".into()),
        Item::Assistant(
            "Every value in Rust has a single owner. When the owner goes out of \
             scope, the value is dropped. You can move ownership to another \
             binding, or lend it out temporarily with a reference — but there \
             is only ever one owner at a time, and the compiler checks this \
             statically rather than at runtime."
                .into(),
        ),
        Item::User("借用とムーブの違いを教えて".into()),
        Item::Assistant(
            "ムーブは所有権そのものを渡します。渡した側の変数はもう使えません。\
             借用は参照を貸すだけなので、元の変数は有効なままです。可変借用は\
             同時に一つしか存在できません。"
                .into(),
        ),
        Item::User("short".into()),
        Item::Assistant("ok".into()),
        Item::Error("connection reset by peer".into()),
    ]
}
