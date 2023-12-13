use std::{
    any::Any,
    collections::BTreeMap,
    fmt::{Debug, Display},
    ops::ControlFlow,
};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};
use ratatui::{
    prelude::{Constraint, CrosstermBackend, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Tabs},
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

pub fn with_modifier(value: KeyEvent) -> Option<Retning> {
    if value.modifiers.contains(KeyModifiers::ALT) {
        return Retning::try_from(value).ok();
    }
    None
}

type Term = ratatui::Terminal<Bakende>;
type Bakende = ratatui::backend::CrosstermBackend<std::io::Stderr>;

pub struct App<T> {
    app_state: T,
    terminal: Term,
    tab_idx: usize,
    tabs: Vec<Box<dyn Tab<AppState = T>>>,
    widget_area: Rect,
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
            widget_area: Rect::default(),
        }
    }

    pub fn run(&mut self) {
        crossterm::terminal::enable_raw_mode().unwrap();
        crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::EnterAlternateScreen,
            Show
        )
        .unwrap();

        loop {
            self.draw();

            match self.handle_key() {
                ControlFlow::Continue(_) => continue,
                ControlFlow::Break(_) => break,
            }
        }

        crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen).unwrap();
        crossterm::terminal::disable_raw_mode().unwrap();
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
                self.widget_area = remainder_area;
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

        if !tab.tabdata().is_selected && tab.tabdata().popup.is_none() {
            if let Event::Key(k) = key {
                if k.code == KeyCode::Char('Q') {
                    return ControlFlow::Break(());
                }
            }
        }

        tab.entry_keyhandler(key, &mut self.app_state, self.widget_area);

        ControlFlow::Continue(())
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

#[derive(Default)]
pub struct TabData<T> {
    pub cursor: Pos,
    pub is_selected: bool,
    pub popup_state: PopUpState,
    pub proxy: Option<Box<dyn Tab<AppState = T>>>,
    pub popup: Option<Box<dyn Tab<AppState = T>>>,
    pub state_modifier: Option<Box<dyn FnMut(&Box<dyn Any>)>>,
    pub area_map: BTreeMap<String, Rect>,
    pub first_pass: bool,
    pub key_history: Vec<KeyCode>,
}

pub struct Wrapper(KeyCode);

impl From<KeyCode> for Wrapper {
    fn from(value: KeyCode) -> Self {
        Self(value)
    }
}

impl From<char> for Wrapper {
    fn from(c: char) -> Self {
        Self(KeyCode::Char(c))
    }
}

impl From<Wrapper> for KeyCode {
    fn from(value: Wrapper) -> Self {
        value.0
    }
}

impl<T> Debug for TabData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabData")
            .field("cursor", &self.cursor)
            .field("is_selected", &self.is_selected)
            .field("popup_state", &self.popup_state)
            .finish()
    }
}

impl<T> TabData<T> {
    pub fn _debug_show_cursor(&self, f: &mut Frame) {
        f.set_cursor(self.cursor.x, self.cursor.y);
    }

    pub fn is_selected(&self, area: Rect) -> bool {
        Self::isitselected(area, self.cursor)
    }

    pub fn char_match(&self, keys: &str) -> bool {
        let keys: Vec<KeyCode> = keys.chars().map(KeyCode::Char).collect();
        self.key_match(keys)
    }

    pub fn key_match(&self, keys: Vec<KeyCode>) -> bool {
        if self.key_history.len() < keys.len() {
            return false;
        }

        self.key_history.ends_with(keys.as_slice())
    }

    fn insert_key(&mut self, key: KeyCode) {
        let max_buffer = 30;
        let min_buffer = 10;

        if self.key_history.len() > max_buffer {
            self.key_history.drain(..(max_buffer - min_buffer));
        }

        self.key_history.push(key);
    }

    fn is_valid_pos(&self, pos: Pos) -> bool {
        for area in self.area_map.values() {
            if Self::isitselected(*area, pos) {
                return true;
            }
        }
        false
    }

    pub fn move_right(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: current_area.right(),
            y: self.cursor.y,
        };
        if self.is_valid_pos(new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn move_down(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            y: current_area.bottom(),
            x: self.cursor.x,
        };
        if self.is_valid_pos(new_pos) {
            self.cursor = new_pos;
        }
    }

    fn current_area(&self) -> Rect {
        let cursor = self.cursor;
        for (_, area) in self.area_map.iter() {
            if TabData::<()>::isitselected(*area, cursor) {
                return *area;
            }
        }
        panic!("omg: {:?}", cursor);
    }

    pub fn isitselected(area: Rect, cursor: Pos) -> bool {
        cursor.x >= area.left()
            && cursor.x < area.right()
            && cursor.y >= area.top()
            && cursor.y < area.bottom()
    }

    pub fn move_up(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: self.cursor.x,
            y: current_area.top().saturating_sub(1),
        };
        if self.is_valid_pos(new_pos) {
            self.cursor = new_pos;
        }
    }

    pub fn move_left(&mut self) {
        let current_area = self.current_area();
        let new_pos = Pos {
            x: current_area.left().saturating_sub(1),
            y: self.cursor.y,
        };
        if self.is_valid_pos(new_pos) {
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
    fn render(&mut self, f: &mut Frame, app_data: &mut Self::AppData, area: Rect);

    fn id(&self) -> String {
        format!("{:p}", self)
    }

    fn main_render(
        &mut self,
        f: &mut Frame,
        app_data: &mut Self::AppData,
        is_selected: bool,
        cursor: Pos,
        area: Rect,
    ) {
        let rect = self.draw_titled_border(f, is_selected, cursor, area);
        self.render(f, app_data, rect);
    }

    fn title(&self) -> &str {
        ""
    }

    fn draw_titled_border(
        &self,
        f: &mut Frame,
        is_selected: bool,
        cursor: Pos,
        area: Rect,
    ) -> Rect {
        let block = Block::default().title(self.title()).borders(Borders::ALL);

        let block = if TabData::<Self::AppData>::isitselected(area, cursor) {
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

        if area.width < 3 || area.height < 3 {
            return area;
        }

        f.render_widget(block, area);

        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    }
}

pub trait Tab {
    type AppState;

    /* USER DEFINED */

    fn widgets(&mut self, area: Rect) -> Vec<(&mut dyn Widget<AppData = Self::AppState>, Rect)>;
    fn tabdata(&mut self) -> &mut TabData<Self::AppState>;
    fn tabdata_ref(&self) -> &TabData<Self::AppState>;
    fn title(&self) -> &str;
    fn remove_popup_hook(&mut self) {}

    /* USER CAN CALL */

    fn resolve_tab(&mut self, value: Box<dyn Any>) {
        if let Some(mut fun) = std::mem::take(&mut self.tabdata().state_modifier) {
            fun(&value);
        }

        *self.popup_state() = PopUpState::Resolve(value);
    }

    fn exit_tab(&mut self) {
        *self.popup_state() = PopUpState::Exit;
    }

    fn set_proxy(&mut self, proxy: Box<dyn Tab<AppState = Self::AppState>>) {
        self.tabdata().proxy = Some(proxy);
    }

    fn set_popup(&mut self, pop: Box<dyn Tab<AppState = Self::AppState>>) {
        self.tabdata().popup = Some(pop);
    }

    fn set_popup_with_modifier(
        &mut self,
        mut pop: Box<dyn Tab<AppState = Self::AppState>>,
        f: Box<dyn FnMut(&Box<dyn Any>)>,
    ) {
        pop.tabdata().state_modifier = Some(f);
        self.tabdata().popup = Some(pop);
    }

    fn move_to_widget(&mut self, w: &dyn Widget<AppData = Self::AppState>) {
        let id = w.id();
        self.move_to_id(id.as_str());
    }

    fn move_to_id(&mut self, id: &str) {
        let area = self.tabdata().area_map[id];
        self.move_to_area(area);
    }

    fn move_to_area(&mut self, area: Rect) {
        let x = area.x + area.width / 2;
        let y = area.y + area.height / 2;
        self.tabdata().cursor = Pos::new(x, y);
    }

    fn handle_popup_value(&mut self, _app_data: &mut Self::AppState, _return_value: Box<dyn Any>) {}

    /// Keyhandler stuff that should run if no widgets are selected.
    fn tab_keyhandler_deselected(
        &mut self,
        _app_data: &mut Self::AppState,
        _key: crossterm::event::KeyEvent,
    ) -> bool {
        true
    }

    /// Keyhandler stuff that should run if a widget is selected.
    /// Think of this as widget specific stuff that requires the state of the tab,
    /// and therefore the logic cannot be handled by the widget directly.
    fn tab_keyhandler_selected(
        &mut self,
        _app_data: &mut Self::AppState,
        _key: crossterm::event::KeyEvent,
    ) -> bool {
        true
    }

    fn is_selected(&self, w: &dyn Widget<AppData = Self::AppState>) -> bool {
        let id = w.id();
        let Some(area) = self.tabdata_ref().area_map.get(&id) else {
            return false;
        };

        TabData::<()>::isitselected(*area, self.tabdata_ref().cursor)
    }

    fn pre_render_hook(&mut self, _app_data: &mut Self::AppState) {}

    fn phantom(&mut self) -> Option<&mut Box<dyn Tab<AppState = Self::AppState>>> {
        None
    }

    fn render(&mut self, f: &mut ratatui::Frame, app_data: &mut Self::AppState, area: Rect) {
        let is_selected = self.selected();
        let cursor = self.cursor();

        for (widget, area) in self.widgets(area) {
            widget.main_render(f, app_data, is_selected, cursor, area);
        }
    }

    fn after_keyhandler(&mut self, _app_data: &mut Self::AppState) {}

    /* INTERNAL */

    fn pop_up(&mut self) -> Option<&mut Box<dyn Tab<AppState = Self::AppState>>> {
        self.tabdata().popup.as_mut()
    }

    fn proxy(&mut self) -> Option<&mut Box<dyn Tab<AppState = Self::AppState>>> {
        self.tabdata().proxy.as_mut()
    }

    fn get_popup_value(&mut self) -> Option<&mut PopUpState> {
        self.pop_up().map(|x| x.popup_state())
    }

    fn popup_state(&mut self) -> &mut PopUpState {
        &mut self.tabdata().popup_state
    }

    fn validate_pos(&mut self, area: Rect) {
        let cursor = self.tabdata().cursor;
        for (_, area) in self.widgets(area) {
            if TabData::<()>::isitselected(area, cursor) {
                return;
            }
        }
        let the_area = self.widgets(area)[0].1;
        self.move_to_area(the_area);
    }

    /// its a function so that it can be overriden if needed.
    fn remove_popup(&mut self) {
        self.tabdata().popup = None;
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
            self.remove_popup();
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

    fn pre_keyhandler_hook(&mut self, _key: KeyEvent) {}

    fn pre_popup_hook(&mut self, _app_data: &mut Self::AppState) {}

    fn entry_keyhandler(&mut self, event: Event, app_data: &mut Self::AppState, area: Rect) {
        let Event::Key(key) = event else {
            return;
        };

        if let Some(proxy) = self.proxy() {
            proxy.entry_keyhandler(event, app_data, area);
            return;
        }

        if self.pop_up().is_none() {
            self.pre_popup_hook(app_data);
        }

        if let Some(popup) = self.pop_up() {
            popup.entry_keyhandler(event, app_data, area);
            return;
        }

        self.pre_keyhandler_hook(key);

        if self.selected() {
            if key.code == KeyCode::Esc {
                self.tabdata().is_selected = false;
            } else if self.tab_keyhandler(app_data, key) {
                self.widget_keyhandler(app_data, key, area);
            }
        } else {
            if key.code == KeyCode::Enter {
                self.tabdata().is_selected = true;
            } else if key.code == KeyCode::Esc {
                self.exit_tab();
            } else if let Ok(ret) = Retning::try_from(key) {
                self.navigate(ret);
            } else {
                self.tab_keyhandler(app_data, key);
            }
        }

        self.after_keyhandler(app_data);
    }

    // Keyhandling that requires the state of the object.
    // bool represents whether the tab 'captures' the key
    // or passes it onto the widget in focus
    fn tab_keyhandler(
        &mut self,
        app_data: &mut Self::AppState,
        key: crossterm::event::KeyEvent,
    ) -> bool {
        self.tabdata().insert_key(key.code);

        if self.tabdata().is_selected {
            self.tab_keyhandler_selected(app_data, key)
        } else {
            self.tab_keyhandler_deselected(app_data, key)
        }
    }

    // Keyhandler that only mutates the widget itself.
    fn widget_keyhandler(
        &mut self,
        app_data: &mut Self::AppState,
        key: crossterm::event::KeyEvent,
        area: Rect,
    ) {
        let cursor = self.cursor();
        for (widget, area) in self.widgets(area) {
            if TabData::<Self::AppState>::isitselected(area, cursor) {
                widget.keyhandler(app_data, key);
                return;
            }
        }
    }

    fn set_map(&mut self, area: Rect) {
        let mut map = BTreeMap::new();
        for (widget, area) in self.widgets(area) {
            map.insert(widget.id(), area);
        }
        self.tabdata().area_map = map;
    }

    fn entry_render(&mut self, f: &mut Frame, app_data: &mut Self::AppState, area: Rect) {
        if let Some(proxy) = self.proxy() {
            proxy.entry_render(f, app_data, area);
        }

        self.check_popup_value(app_data);

        if self.pop_up().is_none() {
            self.pre_popup_hook(app_data);
        }

        match self.pop_up() {
            Some(pop_up) => pop_up.entry_render(f, app_data, area),
            None => {
                self.set_map(area);
                self.validate_pos(area);
                self.pre_render_hook(app_data);
                self.render(f, app_data, area);
            }
        }

        self.tabdata().first_pass = true;
    }

    fn should_exit(&mut self) -> bool {
        matches!(self.popup_state(), PopUpState::Exit)
    }

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
