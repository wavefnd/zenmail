use ratatui::{
    Frame,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1), Constraint::Length(2)])
        .split(f.size());

    let header_text = if let Some(h) = &app.current_header {
        format!(
            "From    {}\nDate    {}\nSubject {}\nUID     {}",
            if h.from.is_empty() { "(unknown)" } else { &h.from },
            if h.date.is_empty() { "" } else { &h.date },
            if h.subject.is_empty() { "(no subject)" } else { &h.subject },
            h.uid
        )
    } else {
        "Loading...".to_string()
    };

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Mail"));

    let body = Paragraph::new(app.current_body.clone())
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((app.body_scroll, 0));

    f.render_widget(header, chunks[0]);
    f.render_widget(body, chunks[1]);

    let help = Paragraph::new(format!(
        "{}   {}",
        app.status,
        "j/k or ↑↓ scroll · Esc back · r reply · c compose · g config · q quit"
    ));
    f.render_widget(help, chunks[2]);
}
