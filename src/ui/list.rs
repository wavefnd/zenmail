use ratatui::{
    Frame,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    style::{Style, Modifier},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(f.size());

    let items = if app.messages.is_empty() {
        vec![ListItem::new("Loading... (press o to refresh)")]
    } else {
        app.messages.iter().map(|m| {
            let subject = if m.subject.is_empty() { "(no subject)" } else { m.subject.as_str() };
            let from = if m.from.is_empty() { "(unknown)" } else { m.from.as_str() };
            let date = if m.date.is_empty() { "" } else { m.date.as_str() };
            ListItem::new(format!("{subject}\n  {from}  {date}"))
        }).collect::<Vec<_>>()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Inbox"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !app.messages.is_empty() {
        state.select(Some(app.selected.min(app.messages.len().saturating_sub(1))));
    }

    f.render_stateful_widget(list, chunks[0], &mut state);

    let help = Paragraph::new(format!(
        "{}   {}",
        app.status,
        "j/k or ↑↓ move · Enter open · o refresh · c compose · g config · q quit"
    ))
        .wrap(Wrap { trim: true });

    f.render_widget(help, chunks[1]);
}
