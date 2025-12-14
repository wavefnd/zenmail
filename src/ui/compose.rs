use ratatui::{
    Frame,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, Paragraph, Wrap},
    style::{Style, Modifier},
};

use crate::app::{App, ComposeField};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1), Constraint::Length(2)])
        .split(f.size());

    let body_style = if app.compose.focus == ComposeField::Body {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    let header = Paragraph::new(format!(
        "To: {}\nSubject: {}\n\n(Tab to switch · Ctrl+S to send · Esc to cancel)",
        app.compose.to,
        app.compose.subject
    ))
        .block(Block::default().borders(Borders::ALL).title("Compose"));

    f.render_widget(header, chunks[0]);

    let body_text = if app.compose.quote.is_empty() {
        app.compose.body.clone()
    } else if app.compose.body.trim().is_empty() {
        app.compose.quote.clone()
    } else {
        format!("{}\n\n{}", app.compose.body, app.compose.quote)
    };

    let body = Paragraph::new(body_text)
        .block(Block::default().borders(Borders::ALL).title("Body"))
        .wrap(Wrap { trim: false })
        .style(body_style);

    f.render_widget(body, chunks[1]);

    let status = Paragraph::new(format!(
        "{}   Focus: {}",
        app.status,
        match app.compose.focus {
            ComposeField::To => "To",
            ComposeField::Subject => "Subject",
            ComposeField::Body => "Body",
        }
    ));
    f.render_widget(status, chunks[2]);
}
