use std::io::Result;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui_textarea::TextArea;

use crate::ui;

pub enum Status {
    Idle,
    //Streaming,
    //Cancelling,
}

pub enum Item {
    User(String),
    Assistant(String),
    //Thought(String),
    //Tool(ToolCall, ToolStatus),
    Error(String),
}

pub enum Action {
    Quit,
    Cancel,
    Submit,
    Input(KeyEvent),
}

pub struct App {
    pub transcript: Vec<Item>,
    pub input: TextArea<'static>,
    //pub scroll: ScrollState,
    pub status: Status,
    //pub pending: Option<PermissionRequest>,
    //pub keymap: KeyMap,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            //transcript: vec![],
            transcript: createMockItems(),
            input: TextArea::default(),
            status: Status::Idle,
            should_quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::draw(frame, &self))?;

            if let Some(action) = self.handle_event(event::read()?) {
                self.update(action);
            }
        }
        Ok(())
    }

    pub fn update(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Submit => {
                let text = self.input.lines().join("\n");
                if !text.trim().is_empty() {
                    self.transcript.push(Item::User(text));
                    self.input = TextArea::default();
                }
            }
            Action::Cancel => {
                eprintln!("Cancel will be triggered here");
            }
            Action::Input(key) => {
                self.input.input(key);
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
            _ => Some(Action::Input(key)),
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
