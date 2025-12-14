use ratatui::{
    Frame,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, ConfigField};

fn mask(s: &str) -> String {
    if s.is_empty() { "".to_string() } else { "********".to_string() }
}

fn line(app: &App, field: ConfigField, label: &str, value: &str) -> String {
    let prefix = if app.cfg_edit.focus == field { "▶ " } else { "  " };
    format!("{prefix}{label:<10} {value}")
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(f.size());

    let mut s = String::new();

    s.push_str("IMAP\n");
    s.push_str(&line(app, ConfigField::ImapHost, "host", &app.cfg_edit.imap_host)); s.push('\n');
    s.push_str(&line(app, ConfigField::ImapPort, "port", &app.cfg_edit.imap_port)); s.push('\n');
    s.push_str(&line(app, ConfigField::ImapUser, "username", &app.cfg_edit.imap_user)); s.push('\n');
    s.push_str(&line(app, ConfigField::ImapPass, "password", &mask(&app.cfg_edit.imap_pass))); s.push('\n');
    s.push_str(&line(app, ConfigField::ImapStarttls, "starttls", if app.cfg_edit.imap_starttls { "true" } else { "false" })); s.push('\n');

    s.push('\n');
    s.push_str("SMTP\n");
    s.push_str(&line(app, ConfigField::SmtpHost, "host", &app.cfg_edit.smtp_host)); s.push('\n');
    s.push_str(&line(app, ConfigField::SmtpPort, "port", &app.cfg_edit.smtp_port)); s.push('\n');
    s.push_str(&line(app, ConfigField::SmtpUser, "username", &app.cfg_edit.smtp_user)); s.push('\n');
    s.push_str(&line(app, ConfigField::SmtpPass, "password", &mask(&app.cfg_edit.smtp_pass))); s.push('\n');
    s.push_str(&line(app, ConfigField::SmtpStarttls, "starttls", if app.cfg_edit.smtp_starttls { "true" } else { "false" })); s.push('\n');

    s.push('\n');
    s.push_str("USER\n");
    s.push_str(&line(app, ConfigField::UserName, "name", &app.cfg_edit.user_name)); s.push('\n');
    s.push_str(&line(app, ConfigField::UserEmail, "email", &app.cfg_edit.user_email)); s.push('\n');

    let body = Paragraph::new(s)
        .block(Block::default().borders(Borders::ALL).title("Config"))
        .wrap(Wrap { trim: false });

    f.render_widget(body, chunks[0]);

    let help = Paragraph::new(format!(
        "{}   {}",
        app.status,
        "Tab/Shift+Tab navigate · Space toggle · Ctrl+S save · e editor · Esc back"
    ));
    f.render_widget(help, chunks[1]);
}
