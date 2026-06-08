use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct SuggestionsBox {
    pub suggestions: Vec<String>,
    pub state: ListState,
    pub is_active: bool,
}

impl SuggestionsBox {
    pub fn new() -> Self {
        Self {
            suggestions: vec![],
            state: ListState::default(),
            is_active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active && !self.suggestions.is_empty()
    }

    pub fn set_suggestions(&mut self, suggs: Vec<String>) {
        self.suggestions = suggs;
        self.is_active = !self.suggestions.is_empty();
        if self.is_active {
            self.state.select(Some(0));
        } else {
            self.state.select(None);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Down => self.next(),
            KeyCode::Up => self.previous(),
            KeyCode::Right | KeyCode::Tab => {
                if let Some(i) = self.state.selected() {
                    let val = self.suggestions[i].clone();
                    self.is_active = false;
                    return Some(val);
                }
            }
            KeyCode::Esc => {
                self.is_active = false;
            }
            _ => {}
        }
        None
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.suggestions.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.suggestions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn draw(&mut self, f: &mut ratatui::Frame, area: Rect) {
        if !self.is_active() {
            return;
        }

        let items: Vec<ListItem> = self
            .suggestions
            .iter()
            .map(|s| ListItem::new(s.clone()))
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Suggestions "))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        // We render using state to keep track of selection
        f.render_stateful_widget(list, area, &mut self.state);
    }
}
