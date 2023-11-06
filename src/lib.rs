use std::{any::Any, fmt::Debug, ops::ControlFlow};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, CrosstermBackend, Layout, Rect},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Tabs},
    Frame, Terminal,
};

#[derive(Debug)]
pub enum Retning {
    Up,
    Down,
    Left,
    Right,
}

impl TryFrom<KeyEvent> for Retning {
    type Error = ();

    fn try_from(value: KeyEvent) -> Result<Self, Self::Error> {
        match value.code {
            KeyCode::Left => Ok(Self::Left),
            KeyCode::Right => Ok(Self::Right),
            KeyCode::Up => Ok(Self::Up),
            KeyCode::Down => Ok(Self::Down),
            KeyCode::Char('k') => Ok(Self::Up),
            KeyCode::Char('j') => Ok(Self::Down),
            KeyCode::Char('h') => Ok(Self::Left),
            KeyCode::Char('l') => Ok(Self::Right),
            _ => Err(()),
        }
    }
}

type Term = ratatui::Terminal<Bakende>;
type Bakende = ratatui::backend::CrosstermBackend<std::io::Stderr>;

pub struct App<T> {
    app_state: T,
    terminal: Term,
    tab_idx: usize,
    tabs: Vec<Box<dyn Tab<AppState = T>>>,
}

impl<T> App<T> {
    pub fn new(app_data: T, tabs: Vec<Box<dyn Tab<AppState = T>>>) -> Self {
        let terminal = Terminal::new(CrosstermBackend::new(std::io::stderr())).unwrap();

        assert!(!tabs.is_empty());

        Self {
            terminal,
            app_state: app_data,
            tabs,
            tab_idx: 0,
        }
    }

    pub fn draw(&mut self) {
        let idx = self.tab_idx;

        self.terminal
            .draw(|f| {
                let (tab_area, remainder_area) = {
                    let chunks = Layout::default()
                        .direction(ratatui::prelude::Direction::Vertical)
                        .constraints(vec![Constraint::Length(3), Constraint::Min(0)])
                        .split(f.size())
                        .to_vec();
                    (chunks[0], chunks[1])
                };

                let tabs = Tabs::new(self.tabs.iter().map(|tab| tab.title()).collect())
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().white())
                    .highlight_style(Style::default().light_red())
                    .select(idx)
                    .divider(symbols::DOT);

                f.render_widget(tabs, tab_area);

                self.tabs[self.tab_idx].entry_render(f, &mut self.app_state, remainder_area);
            })
            .unwrap();
    }

    pub fn handle_key(&mut self) -> ControlFlow<()> {
        let key = event::read().unwrap();

        if let Event::Key(x) = key {
            if x.code == KeyCode::Tab {
                self.go_right()
            } else if x.code == KeyCode::BackTab {
                self.go_left()
            };
        }

        let tab = &mut self.tabs[self.tab_idx];

        tab.entry_keyhandler(key, &mut self.app_state);

        if tab.should_exit() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn go_right(&mut self) {
        self.tab_idx = std::cmp::min(self.tab_idx + 1, self.tabs.len() - 1);
    }

    fn go_left(&mut self) {
        if self.tab_idx != 0 {
            self.tab_idx -= 1;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

impl Pos {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl<T> TabData<T> {
    pub fn _debug_show_cursor(&self, f: &mut Frame) {
        f.set_cursor(self.cursor.x, self.cursor.y);
    }

    pub fn validate_pos(&mut self) {
        for area in self.areas.iter() {
            if self.is_selected(*area) {
                return;
            }
        }
        if !self.areas.is_empty() {
            self.move_to_area(self.areas[0]);
        }
    }

    pub fn move_to_area(&mut self, area: Rect) {
        let x = area.x + area.width / 2;
        let y = area.y + area.height / 2;
        self.cursor = Pos::new(x, y);
    }

    pub fn is_selected(&self, area: Rect) -> bool {
        Self::isitselected(area, &self.cursor)
    }

    fn is_valid_pos(&self, pos: &Pos) -> bool {
        for area in &self.areas {
            if Self::isitselected(*area, pos) {
                return true;
            }
        }
        false
    }

    fn current_area(&self) -> &Rect {
        self.areas
            .iter()
            .find(|area| Self::isitselected(**area, &self.cursor))
            .unwrap()
    }

    pub fn isitselected(area: Rect, cursor: &Pos) -> bool {
        cursor.x >= area.left()
            && cursor.x < area.right()
            && cursor.y >= area.top()
            && cursor.y < area.bottom()
    }

    pub fn move_right(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: current_area.right(),
            y: self.cursor.y,
        };
        if self.is_valid_pos(&new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn move_down(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            y: current_area.bottom(),
            x: self.cursor.x,
        };
        if self.is_valid_pos(&new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn move_up(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: self.cursor.x,
            y: current_area.top().saturating_sub(1),
        };
        if self.is_valid_pos(&new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn move_left(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: current_area.left().saturating_sub(1),
            y: self.cursor.y,
        };
        if self.is_valid_pos(&new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn navigate(&mut self, direction: Retning) {
        match direction {
            Retning::Up => self.move_up(),
            Retning::Down => self.move_down(),
            Retning::Left => self.move_left(),
            Retning::Right => self.move_right(),
        }
    }
}

pub enum PopUpState {
    Exit,
    Continue,
    Resolve(Box<dyn Any>),
}

impl Default for PopUpState {
    fn default() -> Self {
        Self::Continue
    }
}

impl Debug for PopUpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exit => write!(f, "Exit"),
            Self::Continue => write!(f, "Continue"),
            Self::Resolve(arg0) => f.debug_tuple("Resolve").field(arg0).finish(),
        }
    }
}

pub trait Widget {
    type AppData;

    fn keyhandler(&mut self, app_data: &mut Self::AppData, key: KeyEvent);
    fn main_render(
        &mut self,
        f: &mut Frame,
        app_data: &mut Self::AppData,
        is_selected: bool,
        cursor: Pos,
    ) {
        let rect = self.draw_titled_border(f, is_selected, cursor);
        self.render(f, app_data, rect);
    }

    fn render(&mut self, f: &mut Frame, app_data: &mut Self::AppData, area: Rect);
    fn area(&self) -> Rect;
    fn set_area(&mut self, area: Rect);
    fn title(&self) -> &str {
        ""
    }

    fn draw_titled_border(&self, f: &mut Frame, is_selected: bool, cursor: Pos) -> Rect {
        let block = Block::default().title(self.title()).borders(Borders::ALL);

        let block = if TabData::<Self::AppData>::isitselected(self.area(), &cursor) {
            if is_selected {
                block.border_style(Style {
                    fg: Some(ratatui::style::Color::Red),
                    ..Default::default()
                })
            } else {
                block.border_style(Style {
                    fg: Some(ratatui::style::Color::Black),
                    ..Default::default()
                })
            }
        } else {
            block.border_style(Style {
                fg: Some(ratatui::style::Color::White),
                ..Default::default()
            })
        };

        let rect = self.area();

        if rect.width < 3 || rect.height < 3 {
            return rect;
        }

        f.render_widget(block, rect);

        Rect {
            x: rect.x + 1,
            y: rect.y + 1,
            width: rect.width.saturating_sub(2),
            height: rect.height.saturating_sub(2),
        }
    }

    fn is_selected(&self, cursor: &Pos) -> bool {
        TabData::<Self::AppData>::isitselected(self.area(), cursor)
    }
}

#[derive(Default)]
pub struct TabData<T> {
    pub areas: Vec<Rect>,
    pub cursor: Pos,
    pub is_selected: bool,
    pub popup_state: PopUpState,
    pub popup: Option<Box<dyn Tab<AppState = T>>>,
}

impl<T> Debug for TabData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabData")
            .field("areas", &self.areas)
            .field("cursor", &self.cursor)
            .field("is_selected", &self.is_selected)
            .field("popup_state", &self.popup_state)
            .finish()
    }
}

pub trait Tab {
    type AppState;

    fn widgets(&mut self) -> Vec<&mut dyn Widget<AppData = Self::AppState>>;
    fn title(&self) -> &str;
    fn set_selection(&mut self, area: Rect);
    fn tabdata(&mut self) -> &mut TabData<Self::AppState>;

    fn resolve_tab(&mut self, value: Box<dyn Any>) {
        *self.popup_state() = PopUpState::Resolve(value);
    }

    fn exit_tab(&mut self) {
        *self.popup_state() = PopUpState::Exit;
    }

    fn set_popup(&mut self, pop: Box<dyn Tab<AppState = Self::AppState>>) {
        self.tabdata().popup = Some(pop);
    }

    fn pop_up(&mut self) -> Option<&mut Box<dyn Tab<AppState = Self::AppState>>> {
        self.tabdata().popup.as_mut()
    }

    fn get_popup_value(&mut self) -> Option<&mut PopUpState> {
        self.pop_up().map(|x| x.popup_state())
    }

    fn popup_state(&mut self) -> &mut PopUpState {
        &mut self.tabdata().popup_state
    }

    fn check_popup_value(&mut self, app_data: &mut Self::AppState) {
        let mut is_exit = false;
        let mut is_resolve = false;

        let Some(popval) = self.get_popup_value() else {
            return;
        };

        match popval {
            PopUpState::Exit => is_exit = true,
            PopUpState::Continue => return,
            PopUpState::Resolve(_) => is_resolve = true,
        }

        if is_exit {
            self.tabdata().popup = None;
            return;
        }

        // weird to do it like this but theres like double mutably borrow rules otherwise.
        if is_resolve {
            let PopUpState::Resolve(resolved_value) = std::mem::take(popval) else {
                panic!()
            };

            self.handle_popup_value(app_data, resolved_value);
            self.tabdata().popup = None;
        }
    }

    fn handle_popup_value(&mut self, _app_data: &mut Self::AppState, _return_value: Box<dyn Any>) {}

    fn entry_keyhandler(&mut self, key: Event, app_data: &mut Self::AppState) -> ControlFlow<()> {
        if let Some(popup) = self.pop_up() {
            return popup.entry_keyhandler(key, app_data);
        }

        let key = match key {
            Event::Key(x) => x,
            // todo find out why it doesnt work
            Event::Mouse(x) => {
                self.tabdata().cursor = Pos {
                    y: x.row,
                    x: x.column,
                };
                return ControlFlow::Continue(());
            }
            _ => {
                return ControlFlow::Continue(());
            }
        };

        if !self.selected() && key.code == KeyCode::Esc {
            self.exit_tab();
            return ControlFlow::Break(());
        } else if self.selected() && key.code == KeyCode::Esc {
            self.tabdata().is_selected = false;
            return ControlFlow::Continue(());
        } else if let Ok(ret) = Retning::try_from(key) {
            if !self.selected() {
                self.navigate(ret);
                return ControlFlow::Continue(());
            }
        }

        if self.tab_keyhandler(app_data, key) {
            if !self.selected() && key.code == KeyCode::Char(' ') || key.code == KeyCode::Enter {
                self.tabdata().is_selected = true;
            } else {
                self.widget_keyhandler(app_data, key);
            }
        }

        self.after_keyhandler(app_data);

        ControlFlow::Continue(())
    }

    // Keyhandling that requires the state of the object.
    // bool represents whether the tab 'captures' the key
    // or passes it onto the widget in focus
    fn tab_keyhandler(
        &mut self,
        _app_data: &mut Self::AppState,
        _key: crossterm::event::KeyEvent,
    ) -> bool {
        true
    }

    // Keyhandler that only mutates the widget itself.
    fn widget_keyhandler(
        &mut self,
        app_data: &mut Self::AppState,
        key: crossterm::event::KeyEvent,
    ) {
        let cursor = self.cursor();
        for widget in self.widgets() {
            if widget.is_selected(&cursor) {
                widget.keyhandler(app_data, key);
                return;
            }
        }
    }

    fn entry_render(&mut self, f: &mut Frame, app_data: &mut Self::AppState, area: Rect) {
        self.check_popup_value(app_data);

        match self.pop_up() {
            Some(pop_up) => pop_up.entry_render(f, app_data, area),
            None => {
                self.tabdata().areas.clear();
                self.set_selection(area);
                self.tabdata().validate_pos();
                self.render(f, app_data);
            }
        }
    }

    fn should_exit(&mut self) -> bool {
        matches!(self.popup_state(), PopUpState::Exit)
    }

    fn render(&mut self, f: &mut ratatui::Frame, app_data: &mut Self::AppState) {
        let is_selected = self.selected();
        let cursor = self.cursor();

        for widget in self.widgets() {
            widget.main_render(f, app_data, is_selected, cursor);
        }
    }

    fn after_keyhandler(&mut self, _app_data: &mut Self::AppState) {}

    fn cursor(&mut self) -> Pos {
        self.tabdata().cursor
    }

    fn selected(&mut self) -> bool {
        self.tabdata().is_selected
    }

    fn navigate(&mut self, dir: Retning) {
        self.tabdata().navigate(dir);
    }
}
