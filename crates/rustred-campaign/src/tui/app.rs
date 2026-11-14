//! Interactive state of the dashboard (panel, selection, scroll, stop protocol) and the key handling. Pure: no
//! terminal, so the stop protocol and navigation are unit-testable with a fake [`Controls`].
//!
//! Drill-down data comes through a [`Fetcher`]: any `ProgressSource` answers at once (tests, `--once`); the live
//! dashboard's background reader answers later ([`Fetched::Pending`], the panel says "loading"), so a key never
//! waits for the file system (audit M1).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use super::clean::{Clean, cl};
use super::controls::Controls;
use super::model::{self, ItemView, PHASES, ViewModel};
use super::source::{ItemRow, ProgressSource};
use crate::signals::StopLevel;

/// An answer now, or later through [`App::apply_items`] / [`App::apply_detail`].
#[derive(Debug)]
pub enum Fetched<T> {
    Now(Result<T, String>),
    Pending,
}

/// Where the drill-down data comes from.
pub trait Fetcher {
    fn items(&self, node: &str) -> Fetched<Vec<ItemRow>>;
    fn detail(&self, item: &str) -> Fetched<Value>;
}

impl<S: ProgressSource + ?Sized> Fetcher for S {
    fn items(&self, node: &str) -> Fetched<Vec<ItemRow>> {
        Fetched::Now(self.node_items(node))
    }
    fn detail(&self, item: &str) -> Fetched<Value> {
        Fetched::Now(self.item_detail(item))
    }
}

/// A drill-down value being loaded or loaded.
#[derive(Clone, Debug, PartialEq)]
pub enum Slot<T> {
    Loading,
    Ready(T),
}

/// The node panel's items: loading, or the sanitised rows / error.
pub type ItemsSlot = Slot<Result<Vec<ItemView>, Clean>>;
/// The item panel's detail: loading, or the sanitised record / error.
pub type DetailSlot = Slot<Result<Value, Clean>>;

/// Minimum pause before the third Ctrl-C leaves the dashboard (a key-repeat burst must not; audit m8).
pub const THIRD_CTRL_C_PAUSE_S: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Summary,
    Node,
    Item,
    Jobs,
    Events,
    Attention,
    Help,
}

/// The one table shown at 80 columns (Tab cycles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactTable {
    Ladder,
    Nodes,
    Jobs,
    Events,
    Attention,
}

impl CompactTable {
    /// Nodes first (which nodes are closed is the first question), then the ladder, jobs, events, attention.
    pub fn next(self) -> Self {
        match self {
            CompactTable::Nodes => CompactTable::Ladder,
            CompactTable::Ladder => CompactTable::Jobs,
            CompactTable::Jobs => CompactTable::Events,
            CompactTable::Events => CompactTable::Attention,
            CompactTable::Attention => CompactTable::Nodes,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            CompactTable::Ladder => "Work list by ladder level",
            CompactTable::Nodes => "Nodes",
            CompactTable::Jobs => "Running jobs",
            CompactTable::Events => "Recent events",
            CompactTable::Attention => "Attention",
        }
    }
}

/// Why the dashboard returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExitReason {
    /// Watcher: `q` or Ctrl-C (the campaign is not affected).
    ViewerQuit,
    /// The supervisor reported that it stopped, without a stop request seen here.
    CampaignEnded,
    /// The supervisor stopped after stop requests (drain or force).
    StopRequested(StopLevel),
    /// Third Ctrl-C: the terminal was restored at once; the caller decides.
    ForcedExit,
    /// SIGHUP or the terminal went away (write error): continue in plain mode.
    Hangup,
    /// The terminal could not be set up (the caller falls back to plain mode).
    TerminalError(String),
}

/// What a key asks the loop to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    Quit(ExitReason),
}

#[derive(Clone, Debug)]
pub struct App {
    pub panel: Panel,
    pub stack: Vec<Panel>,
    pub compact: CompactTable,
    pub selected_node: usize,
    pub selected_item: usize,
    pub scroll: usize,
    /// Item phase filter of the node panel (index into [`PHASES`]).
    pub item_filter: Option<usize>,
    pub message: Option<String>,
    pub message_until_s: f64,
    pub stop_count: usize,
    pub stop_first_s: Option<f64>,
    pub grace_s: u64,
    pub hosts_campaign: bool,
    /// Drill-down: the node's items (sanitised) and the item detail (every string sanitised).
    pub node_items: Option<(Clean, ItemsSlot)>,
    pub item_detail: Option<(Clean, DetailSlot)>,
    /// Time of the last Ctrl-C key.
    pub last_ctrl_c_s: Option<f64>,
}

impl App {
    pub fn new(hosts_campaign: bool, grace_s: u64) -> Self {
        App {
            panel: Panel::Summary,
            stack: Vec::new(),
            compact: CompactTable::Nodes,
            selected_node: 0,
            selected_item: 0,
            scroll: 0,
            item_filter: None,
            message: None,
            message_until_s: 0.0,
            stop_count: 0,
            stop_first_s: None,
            grace_s,
            hosts_campaign,
            node_items: None,
            item_detail: None,
            last_ctrl_c_s: None,
        }
    }

    /// An answer of the background reader for the node panel (ignored when another node is open by now).
    pub fn apply_items(&mut self, node: &str, r: Result<Vec<ItemRow>, String>) {
        if let Some((id, slot)) = &mut self.node_items
            && id == node
        {
            *slot = Slot::Ready(model::item_views(r));
        }
    }

    /// An answer of the background reader for the item panel.
    pub fn apply_detail(&mut self, item: &str, r: Result<Value, String>) {
        if let Some((id, slot)) = &mut self.item_detail
            && id == item
        {
            *slot = Slot::Ready(model::item_detail_view(r));
        }
    }

    pub fn say(&mut self, text: impl AsRef<str>, now_s: f64) {
        self.message = Some(cl(text).into_string());
        self.message_until_s = now_s + 8.0;
    }

    /// Message still shown at `now_s`.
    pub fn message_at(&self, now_s: f64) -> Option<&str> {
        self.message
            .as_deref()
            .filter(|_| now_s <= self.message_until_s)
    }

    /// Syncs with stop requests that did not come through a key (signals).
    pub fn observe_stops(&mut self, count: usize, now_s: f64) {
        if count > self.stop_count {
            if self.stop_first_s.is_none() {
                self.stop_first_s = Some(now_s);
            }
            self.stop_count = count;
        }
    }

    /// Seconds left of the drain grace (None when no stop was requested).
    pub fn grace_left(&self, vm: &ViewModel, now_s: f64) -> Option<f64> {
        let first = self.stop_first_s?;
        Some(
            vm.grace_left_s
                .unwrap_or_else(|| (self.grace_s as f64 - (now_s - first)).max(0.0)),
        )
    }

    pub fn selected_node_id<'a>(&self, vm: &'a ViewModel) -> Option<&'a str> {
        vm.nodes
            .get(self.selected_node.min(vm.nodes.len().saturating_sub(1)))
            .map(|n| n.id.as_str())
    }

    /// Items of the node panel after the filter.
    pub fn visible_items(&self) -> Vec<&ItemView> {
        match &self.node_items {
            Some((_, Slot::Ready(Ok(v)))) => v
                .iter()
                .filter(|i| self.item_filter.is_none_or(|k| i.state == PHASES[k]))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn push(&mut self, p: Panel) {
        if self.panel != p {
            self.stack.push(self.panel);
            self.panel = p;
            self.scroll = 0;
        }
    }

    fn pop(&mut self) {
        self.panel = self.stack.pop().unwrap_or(Panel::Summary);
        self.scroll = 0;
    }

    fn open_node(&mut self, vm: &ViewModel, source: &dyn Fetcher) {
        if let Some(id) = self.selected_node_id(vm).map(cl) {
            self.node_items = Some((id.clone(), Slot::Loading));
            if let Fetched::Now(r) = source.items(id.as_str()) {
                self.apply_items(id.as_str(), r);
            }
            self.selected_item = 0;
            self.item_filter = None;
            self.push(Panel::Node);
        }
    }

    fn open_item(&mut self, source: &dyn Fetcher, now_s: f64) {
        let item = self
            .visible_items()
            .get(self.selected_item)
            .map(|i| i.item.clone());
        match item {
            Some(id) => {
                self.item_detail = Some((id.clone(), Slot::Loading));
                if let Fetched::Now(r) = source.detail(id.as_str()) {
                    self.apply_detail(id.as_str(), r);
                }
                self.push(Panel::Item);
            }
            None => self.say(
                "w / Enter: no item selected (open a node with Enter first)",
                now_s,
            ),
        }
    }

    fn move_sel(&mut self, delta: isize, vm: &ViewModel) {
        let step = |v: usize, n: usize| -> usize {
            if n == 0 {
                0
            } else {
                (v as isize + delta).clamp(0, n as isize - 1) as usize
            }
        };
        match self.panel {
            Panel::Summary => self.selected_node = step(self.selected_node, vm.nodes.len()),
            Panel::Node => {
                let n = self.visible_items().len();
                self.selected_item = step(self.selected_item, n);
            }
            _ => self.scroll = (self.scroll as isize + delta).max(0) as usize,
        }
    }

    /// Handles one key.
    pub fn on_key(
        &mut self,
        key: KeyEvent,
        vm: &ViewModel,
        source: &dyn Fetcher,
        controls: &dyn Controls,
        now_s: f64,
    ) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
            return self.ctrl_c(vm, controls, now_s);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.panel != Panel::Summary {
                    self.pop();
                } else if !self.hosts_campaign {
                    return Action::Quit(ExitReason::ViewerQuit);
                } else {
                    self.say(
                        "q does not stop the run: Ctrl-C stops it safely (twice: now)",
                        now_s,
                    );
                }
            }
            KeyCode::Tab => self.compact = self.compact.next(),
            KeyCode::Char('n') | KeyCode::Down => self.move_sel(1, vm),
            KeyCode::Char('N') | KeyCode::Up => self.move_sel(-1, vm),
            KeyCode::PageDown => self.move_sel(10, vm),
            KeyCode::PageUp => self.move_sel(-10, vm),
            KeyCode::Home => {
                self.selected_item = 0;
                self.scroll = 0;
                if self.panel == Panel::Summary {
                    self.selected_node = 0;
                }
            }
            KeyCode::End => self.move_sel(1_000_000, vm),
            KeyCode::Enter => match self.panel {
                Panel::Summary => self.open_node(vm, source),
                Panel::Node => self.open_item(source, now_s),
                _ => {}
            },
            KeyCode::Char('f') | KeyCode::Char('i') => {
                if self.panel == Panel::Summary {
                    self.open_node(vm, source)
                }
            }
            KeyCode::Char('w') => {
                if self.panel == Panel::Node {
                    self.open_item(source, now_s)
                } else {
                    self.say("w: open a node (Enter), select an item, then w", now_s)
                }
            }
            KeyCode::Char('j') => self.push(Panel::Jobs),
            KeyCode::Char('e') => self.push(Panel::Events),
            KeyCode::Char('a') => self.push(Panel::Attention),
            KeyCode::Char('?') | KeyCode::Char('h') => self.push(Panel::Help),
            // 1..9 select the first nine phases, 0 the tenth, * all (every phase reachable, audit m6)
            KeyCode::Char(c @ '0'..='9') if self.panel == Panel::Node => {
                let k = c as usize - '0' as usize;
                let idx = if k == 0 { 9 } else { k - 1 };
                self.item_filter = (idx < PHASES.len()).then_some(idx);
                self.selected_item = 0;
            }
            KeyCode::Char('*') if self.panel == Panel::Node => {
                self.item_filter = None;
                self.selected_item = 0;
            }
            KeyCode::Char(c @ ('p' | 'r' | 'd' | '+' | '-' | 'R' | 'S' | 'T' | 'k' | '/')) => {
                let verb = match c {
                    'p' => "pause",
                    'r' => "resume",
                    'd' => "drain --reason TEXT",
                    '+' | '-' => "workers --max N",
                    'R' => "retry <item> --reason TEXT",
                    'S' => "skip <item> --reason TEXT",
                    'T' => "terminal <item> --reason TEXT",
                    'k' => "key <node> '<key>'",
                    _ => "events --kind ...",
                };
                self.say(
                    format!("not bound here: type `rr_campaign {verb}` in another pane"),
                    now_s,
                );
            }
            _ => return Action::None,
        }
        Action::Redraw
    }

    fn ctrl_c(&mut self, vm: &ViewModel, controls: &dyn Controls, now_s: f64) -> Action {
        if !controls.hosts_campaign() {
            controls.request_stop();
            return Action::Quit(ExitReason::ViewerQuit);
        }
        // the third press leaves the dashboard only after a pause (a burst / key repeat forces the stop, no more)
        if self.stop_count >= 2
            && self
                .last_ctrl_c_s
                .is_some_and(|t| now_s - t < THIRD_CTRL_C_PAUSE_S)
        {
            self.last_ctrl_c_s = Some(now_s);
            self.say(
                "forced stop already requested; Ctrl-C again after a pause leaves the dashboard",
                now_s,
            );
            return Action::Redraw;
        }
        self.last_ctrl_c_s = Some(now_s);
        let level = controls.request_stop();
        let count = controls.stop_count();
        if self.stop_first_s.is_none() {
            self.stop_first_s = Some(now_s);
        }
        self.stop_count = count.max(self.stop_count + 1);
        if self.stop_count >= 3 {
            return Action::Quit(ExitReason::ForcedExit);
        }
        let _ = (level, vm);
        Action::Redraw
    }
}
