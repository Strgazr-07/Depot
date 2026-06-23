//! Interactive terminal UI: browse what the scanner found — grouped by category
//! with subtotals — tick what you want gone, filter by category, and send the
//! selected items to the Recycle Bin / Trash.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::scan::{run_scan, Category, ScanItem, ScanMsg, ScanOptions};
use crate::util::{human_size, relative_age};

struct Row {
    item: ScanItem,
    selected: bool,
}

/// One rendered line: either a category header or a selectable item (by row idx).
enum Disp {
    Header(Category),
    Item(usize),
}

struct App {
    rows: Vec<Row>,
    disp: Vec<Disp>,
    state: ListState,
    filter: HashSet<Category>,
    scanning: bool,
    status: String,
    last_action: String,
}

fn cat_order(c: Category) -> usize {
    Category::ALL.iter().position(|x| *x == c).unwrap_or(99)
}

impl App {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            disp: Vec::new(),
            state: ListState::default(),
            filter: Category::ALL.iter().copied().collect(),
            scanning: true,
            status: "Starting…".into(),
            last_action: String::new(),
        }
    }

    fn push(&mut self, item: ScanItem) {
        self.rows.push(Row { item, selected: false });
        self.rebuild();
    }

    /// Re-sort rows (category order, then largest first) and lay out the
    /// display list with one header per visible, non-empty category.
    fn rebuild(&mut self) {
        self.rows.sort_by(|a, b| {
            cat_order(a.item.category)
                .cmp(&cat_order(b.item.category))
                .then(b.item.size.cmp(&a.item.size))
        });

        let prev = self.current_row();
        self.disp.clear();
        let mut last_cat: Option<Category> = None;
        for (i, row) in self.rows.iter().enumerate() {
            let cat = row.item.category;
            if !self.filter.contains(&cat) {
                continue;
            }
            if last_cat != Some(cat) {
                self.disp.push(Disp::Header(cat));
                last_cat = Some(cat);
            }
            self.disp.push(Disp::Item(i));
        }

        // Keep the cursor on the same row if it's still visible, else first item.
        let target = prev.and_then(|r| {
            self.disp.iter().position(|d| matches!(d, Disp::Item(i) if *i == r))
        });
        self.state.select(target.or_else(|| self.first_item()));
    }

    fn first_item(&self) -> Option<usize> {
        self.disp.iter().position(|d| matches!(d, Disp::Item(_)))
    }

    fn current_row(&self) -> Option<usize> {
        match self.state.selected().and_then(|i| self.disp.get(i)) {
            Some(Disp::Item(r)) => Some(*r),
            _ => None,
        }
    }

    fn step(&mut self, forward: bool) {
        if self.disp.is_empty() {
            return;
        }
        let mut i = self.state.selected().unwrap_or(0) as isize;
        let n = self.disp.len() as isize;
        for _ in 0..n {
            i += if forward { 1 } else { -1 };
            if i < 0 || i >= n {
                return; // hit an edge — stay put
            }
            if matches!(self.disp[i as usize], Disp::Item(_)) {
                self.state.select(Some(i as usize));
                return;
            }
        }
    }

    fn toggle(&mut self) {
        if let Some(r) = self.current_row() {
            self.rows[r].selected = !self.rows[r].selected;
        }
    }

    fn set_all(&mut self, value: bool) {
        for row in &mut self.rows {
            if self.filter.contains(&row.item.category) {
                row.selected = value;
            }
        }
    }

    fn toggle_filter(&mut self, c: Category) {
        if self.filter.contains(&c) {
            self.filter.remove(&c);
        } else {
            self.filter.insert(c);
        }
        self.rebuild();
    }

    fn visible_total(&self) -> u64 {
        self.rows
            .iter()
            .filter(|r| self.filter.contains(&r.item.category))
            .map(|r| r.item.size)
            .sum()
    }

    fn selected_size(&self) -> u64 {
        self.rows.iter().filter(|r| r.selected).map(|r| r.item.size).sum()
    }

    fn selected_count(&self) -> usize {
        self.rows.iter().filter(|r| r.selected).count()
    }

    fn subtotal(&self, c: Category) -> u64 {
        self.rows
            .iter()
            .filter(|r| r.item.category == c)
            .map(|r| r.item.size)
            .sum()
    }

    /// Send every path of every ticked item to the Recycle Bin / Trash.
    fn delete_selected(&mut self) {
        let paths: Vec<PathBuf> = self
            .rows
            .iter()
            .filter(|r| r.selected)
            .flat_map(|r| r.item.paths.clone())
            .collect();
        if paths.is_empty() {
            self.last_action = "Nothing selected".into();
            return;
        }
        let reclaimed = self.selected_size();
        if trash::delete_all(&paths).is_err() {
            for p in &paths {
                let _ = trash::delete(p);
            }
        }
        let before = self.rows.len();
        // Drop rows whose representative path is now gone.
        self.rows
            .retain(|r| !r.selected || r.item.primary().exists());
        let removed = before - self.rows.len();
        self.rebuild();
        self.last_action = format!(
            "Moved {removed} item(s) to Recycle Bin · freed ~{}",
            human_size(reclaimed)
        );
    }
}

pub fn run(opts: ScanOptions) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, opts);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut DefaultTerminal, opts: ScanOptions) -> Result<()> {
    let mut app = App::new();

    let (tx, mut rx) = mpsc::channel();
    let scan_opts = opts.clone();
    let mut handle = Some(thread::spawn(move || run_scan(scan_opts, tx)));

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        loop {
            match rx.try_recv() {
                Ok(ScanMsg::Item(it)) => app.push(it),
                Ok(ScanMsg::Status(s)) => app.status = s,
                Ok(ScanMsg::Done) => app.scanning = false,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.scanning = false;
                    break;
                }
            }
        }

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => app.step(true),
                    KeyCode::Up | KeyCode::Char('k') => app.step(false),
                    KeyCode::Char(' ') => app.toggle(),
                    KeyCode::Char('a') => app.set_all(true),
                    KeyCode::Char('n') => app.set_all(false),
                    KeyCode::Char('d') => app.delete_selected(),
                    KeyCode::Char('1') => app.toggle_filter(Category::Node),
                    KeyCode::Char('2') => app.toggle_filter(Category::Python),
                    KeyCode::Char('3') => app.toggle_filter(Category::Model),
                    KeyCode::Char('4') => app.toggle_filter(Category::Build),
                    KeyCode::Char('r') => {
                        app.rows.clear();
                        app.rebuild();
                        app.scanning = true;
                        app.status = "Rescanning…".into();
                        app.last_action.clear();
                        let (ntx, nrx) = mpsc::channel();
                        rx = nrx;
                        let o = opts.clone();
                        handle = Some(thread::spawn(move || run_scan(o, ntx)));
                    }
                    _ => {}
                }
            }
        }
    }

    drop(handle.take());
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(4),
        ])
        .split(f.area());

    // ── Header ────────────────────────────────────────────────────────────
    let scan_state = if app.scanning {
        format!("scanning… {}", app.status)
    } else {
        "scan complete".to_string()
    };
    let mut header_spans = vec![
        Span::styled(
            " depot ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  filters: "),
    ];
    for (i, c) in Category::ALL.iter().enumerate() {
        let on = app.filter.contains(c);
        header_spans.push(Span::styled(
            format!("[{}]{} ", i + 1, c.label()),
            if on {
                Style::default().fg(category_color(*c)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));
    }
    header_spans.push(Span::styled(
        format!("  [{scan_state}]"),
        Style::default().fg(Color::Cyan),
    ));
    let header = Paragraph::new(Line::from(header_spans))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // ── Grouped list ──────────────────────────────────────────────────────
    let items: Vec<ListItem> = app
        .disp
        .iter()
        .map(|d| match d {
            Disp::Header(cat) => ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {} ", cat.title()),
                    Style::default()
                        .fg(category_color(*cat))
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    format!("— {}", human_size(app.subtotal(*cat))),
                    Style::default().fg(Color::DarkGray),
                ),
            ])),
            Disp::Item(i) => {
                let r = &app.rows[*i];
                let check = if r.selected { "[x]" } else { "[ ]" };
                let detail = if r.item.count() > 1 {
                    r.item.label.clone()
                } else {
                    format!("{:<13} {}", r.item.kind, r.item.primary().display())
                };
                ListItem::new(Line::from(vec![
                    Span::raw(format!("   {check} ")),
                    Span::styled(
                        format!("{:>9} ", human_size(r.item.size)),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!("{:<8} ", relative_age(r.item.modified)),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(truncate(&detail, 70)),
                ]))
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " {} items · {} ",
            app.rows.len(),
            human_size(app.visible_total())
        )))
        .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶");
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // ── Footer ────────────────────────────────────────────────────────────
    let mut status_line = vec![
        Span::styled(
            format!(" {} visible ", human_size(app.visible_total())),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("· "),
        Span::styled(
            format!(
                "{} in {} selected ",
                human_size(app.selected_size()),
                app.selected_count()
            ),
            Style::default().fg(Color::Green),
        ),
    ];
    if !app.last_action.is_empty() {
        status_line.push(Span::raw("· "));
        status_line.push(Span::styled(
            app.last_action.clone(),
            Style::default().fg(Color::Magenta),
        ));
    }
    let footer = Paragraph::new(vec![
        Line::from(status_line),
        Line::from(Span::styled(
            " ↑↓ move · space tick · a/n all/none · 1-4 filter · d delete→Recycle Bin · r rescan · q quit",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn category_color(c: Category) -> Color {
    match c {
        Category::Node => Color::Red,
        Category::Python => Color::Blue,
        Category::Model => Color::Magenta,
        Category::Build => Color::Green,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
