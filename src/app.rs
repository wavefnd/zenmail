use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::mail::{self, MessageSummary};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    List,
    Mail,
    Compose,
    Config,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComposeField {
    To,
    Subject,
    Body,
}

pub struct ComposeState {
    pub to: String,
    pub subject: String,
    pub body: String,   // editable (your reply text)
    pub quote: String,  // readonly quoted block (for Reply)
    pub focus: ComposeField,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    ImapHost,
    ImapPort,
    ImapUser,
    ImapPass,
    ImapStarttls,
    SmtpHost,
    SmtpPort,
    SmtpUser,
    SmtpPass,
    SmtpStarttls,
    UserName,
    UserEmail,
}

pub struct ConfigEditState {
    pub focus: ConfigField,

    pub imap_host: String,
    pub imap_port: String,
    pub imap_user: String,
    pub imap_pass: String,
    pub imap_starttls: bool,

    pub smtp_host: String,
    pub smtp_port: String,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub smtp_starttls: bool,

    pub user_name: String,
    pub user_email: String,
}

impl ConfigEditState {
    pub fn from_config(c: &Config) -> Self {
        Self {
            focus: ConfigField::ImapHost,

            imap_host: c.imap.host.clone(),
            imap_port: c.imap.port.to_string(),
            imap_user: c.imap.username.clone(),
            imap_pass: c.imap.password.clone(),
            imap_starttls: c.imap.starttls,

            smtp_host: c.smtp.host.clone(),
            smtp_port: c.smtp.port.to_string(),
            smtp_user: c.smtp.username.clone(),
            smtp_pass: c.smtp.password.clone(),
            smtp_starttls: c.smtp.starttls,

            user_name: c.user.name.clone(),
            user_email: c.user.email.clone(),
        }
    }
}

pub struct App {
    pub view: View,
    pub return_view: View,

    pub messages: Vec<MessageSummary>,
    pub selected: usize,

    pub current_header: Option<MessageSummary>,
    pub current_body: String,
    pub body_scroll: u16,

    pub compose: ComposeState,

    pub cfg_edit: ConfigEditState,
    pub config_path: PathBuf,

    pub status: String,

    pub config: Config,
}

enum AppMsg {
    MailList(Vec<MessageSummary>),
    MailBody { header: MessageSummary, body: String },
    Status(String),
}

fn clamp_dec(v: usize) -> usize {
    v.saturating_sub(1)
}

struct TuiGuard;
impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

pub async fn run() -> Result<()> {
    let (config, created, config_path) = Config::load_or_create()?;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppMsg>();

    let mut app = App {
        view: if created { View::Config } else { View::List },
        return_view: View::List,

        messages: vec![],
        selected: 0,

        current_header: None,
        current_body: String::new(),
        body_scroll: 0,

        compose: ComposeState {
            to: String::new(),
            subject: String::new(),
            body: String::new(),
            quote: String::new(),
            focus: ComposeField::To,
        },

        cfg_edit: ConfigEditState::from_config(&config),
        config_path,

        status: if created {
            "config.toml created. Fill your credentials and press Ctrl+S to save.".to_string()
        } else {
            "Starting...".to_string()
        },

        config: config.clone(),
    };

    if !created {
        spawn_refresh_list(app.config.clone(), tx.clone());
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, crossterm::cursor::Hide)?;
    let _guard = TuiGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMsg::MailList(list) => {
                    app.messages = list;
                    if app.messages.is_empty() {
                        app.selected = 0;
                    } else {
                        app.selected = app.selected.min(app.messages.len() - 1);
                    }
                    app.status = format!("Loaded {} messages", app.messages.len());
                }
                AppMsg::MailBody { header, body } => {
                    app.current_header = Some(header);
                    app.current_body = body;
                    app.body_scroll = 0;
                    app.status = "Mail loaded".to_string();
                }
                AppMsg::Status(s) => app.status = s,
            }
        }

        terminal.draw(|f| crate::ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Resize(_, _) => {
                    terminal.clear()?;
                    continue;
                }
                Event::Key(k) => {
                    if k.kind != KeyEventKind::Press {
                        continue;
                    }

                    if k.code == KeyCode::Char('q') {
                        break;
                    }

                    if k.code == KeyCode::Char('g') && app.view != View::Config {
                        app.return_view = app.view;
                        app.cfg_edit = ConfigEditState::from_config(&app.config);
                        app.view = View::Config;
                        app.status = "Config".to_string();
                        continue;
                    }

                    match app.view {
                        View::List => handle_list_keys(&mut app, k.code, k.modifiers, &tx),
                        View::Mail => handle_mail_keys(&mut app, k.code, k.modifiers, &tx),
                        View::Compose => handle_compose_keys(&mut app, k.code, k.modifiers, &tx),
                        View::Config => handle_config_keys(&mut app, k.code, k.modifiers, &tx, &mut terminal),
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn spawn_refresh_list(config: Config, tx: mpsc::UnboundedSender<AppMsg>) {
    let _ = tx.send(AppMsg::Status("Fetching mail list...".to_string()));
    tokio::task::spawn_blocking(move || match mail::imap::fetch_summaries(&config.imap, 50) {
        Ok(list) => {
            let _ = tx.send(AppMsg::MailList(list));
        }
        Err(e) => {
            let _ = tx.send(AppMsg::Status(format!("IMAP list error: {e}")));
        }
    });
}

fn spawn_fetch_body(config: Config, header: MessageSummary, tx: mpsc::UnboundedSender<AppMsg>) {
    let _ = tx.send(AppMsg::Status(format!("Fetching body (uid={})...", header.uid)));
    tokio::task::spawn_blocking(move || match mail::imap::fetch_body_plain(&config.imap, header.uid) {
        Ok(body) => {
            let _ = tx.send(AppMsg::MailBody { header, body });
        }
        Err(e) => {
            let _ = tx.send(AppMsg::Status(format!("IMAP body error: {e}")));
        }
    });
}

fn spawn_send_mail(
    config: Config,
    to: String,
    subject: String,
    body: String,
    tx: mpsc::UnboundedSender<AppMsg>,
) {
    let _ = tx.send(AppMsg::Status("Sending...".to_string()));
    tokio::task::spawn_blocking(move || match mail::smtp::send(&config.smtp, &config.user, &to, &subject, &body) {
        Ok(_) => {
            let _ = tx.send(AppMsg::Status("Sent".to_string()));
        }
        Err(e) => {
            let _ = tx.send(AppMsg::Status(format!("SMTP error: {e}")));
        }
    });
}

fn reset_compose_new(app: &mut App) {
    app.compose.to.clear();
    app.compose.subject.clear();
    app.compose.body.clear();
    app.compose.quote.clear();
    app.compose.focus = ComposeField::To;
}

fn compose_full_body(c: &ComposeState) -> String {
    let body = c.body.trim_end().to_string();
    let quote = c.quote.trim_end().to_string();

    if quote.is_empty() {
        body
    } else if body.is_empty() {
        quote
    } else {
        format!("{body}\n\n{quote}")
    }
}

fn extract_reply_to(from: &str) -> String {
    let s = from.trim();

    if let Some(l) = s.find('<') {
        if let Some(r) = s[l + 1..].find('>') {
            let addr = s[l + 1..l + 1 + r].trim();
            if !addr.is_empty() {
                return addr.to_string();
            }
        }
    }

    for tok in s.split_whitespace() {
        let t = tok.trim_matches(|c: char| c == '<' || c == '>' || c == ',' || c == ';');
        if t.contains('@') {
            return t.to_string();
        }
    }

    s.to_string()
}

fn make_reply_subject(subject: &str) -> String {
    let s = subject.trim();
    if s.is_empty() {
        "Re:".to_string()
    } else if s.to_ascii_lowercase().starts_with("re:") {
        s.to_string()
    } else {
        format!("Re: {s}")
    }
}

fn quote_lines(text: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.replace("\r\n", "\n").lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str("> ");
        out.push_str(line);
    }
    if out.is_empty() {
        out.push_str("> ");
    }
    out
}

fn make_reply_quote(header: &MessageSummary, body: &str) -> String {
    let from = if header.from.is_empty() { "(unknown)" } else { header.from.as_str() };
    let date = if header.date.is_empty() { "(unknown date)" } else { header.date.as_str() };
    let intro = format!("On {date}, {from} wrote:");
    let quoted = quote_lines(body);
    format!("{intro}\n{quoted}")
}

fn start_reply(app: &mut App) {
    let Some(h) = app.current_header.clone() else {
        app.status = "No mail selected".to_string();
        return;
    };

    if app.current_body.trim().is_empty() || app.current_body.trim() == "Loading..." {
        app.status = "Mail is still loading".to_string();
        return;
    }

    app.compose.to = extract_reply_to(&h.from);
    app.compose.subject = make_reply_subject(&h.subject);

    app.compose.body.clear(); // user writes reply here (top)
    app.compose.quote = make_reply_quote(&h, &app.current_body); // quote below
    app.compose.focus = ComposeField::Body;

    app.view = View::Compose;
    app.status = "Reply".to_string();
}

fn handle_list_keys(app: &mut App, code: KeyCode, _mods: KeyModifiers, tx: &mpsc::UnboundedSender<AppMsg>) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.messages.is_empty() {
                app.selected = (app.selected + 1).min(app.messages.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.selected = clamp_dec(app.selected);
        }
        KeyCode::Enter => {
            if let Some(m) = app.messages.get(app.selected).cloned() {
                app.view = View::Mail;
                app.current_header = Some(m.clone());
                app.current_body = "Loading...".to_string();
                spawn_fetch_body(app.config.clone(), m, tx.clone());
            }
        }
        KeyCode::Char('o') => {
            spawn_refresh_list(app.config.clone(), tx.clone());
        }
        KeyCode::Char('c') => {
            reset_compose_new(app);
            app.view = View::Compose;
            app.status = "Compose".to_string();
        }
        _ => {}
    }
}

fn handle_mail_keys(app: &mut App, code: KeyCode, _mods: KeyModifiers, tx: &mpsc::UnboundedSender<AppMsg>) {
    match code {
        KeyCode::Esc => {
            app.view = View::List;
            app.status = "Back".to_string();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.body_scroll = app.body_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.body_scroll = app.body_scroll.saturating_sub(1);
        }
        KeyCode::Char('c') => {
            reset_compose_new(app);
            app.view = View::Compose;
            app.status = "Compose".to_string();
        }
        KeyCode::Char('r') => {
            start_reply(app);
        }
        KeyCode::Char('o') => {
            // optional: refresh list while reading
            spawn_refresh_list(app.config.clone(), tx.clone());
            app.status = "Refreshing...".to_string();
        }
        _ => {}
    }
}

fn handle_compose_keys(app: &mut App, code: KeyCode, mods: KeyModifiers, tx: &mpsc::UnboundedSender<AppMsg>) {
    if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('s')) {
        if app.compose.to.trim().is_empty() {
            app.status = "To is empty".to_string();
            return;
        }
        if app.compose.subject.trim().is_empty() {
            app.status = "Subject is empty".to_string();
            return;
        }

        let full_body = compose_full_body(&app.compose);

        spawn_send_mail(
            app.config.clone(),
            app.compose.to.clone(),
            app.compose.subject.clone(),
            full_body,
            tx.clone(),
        );
        return;
    }

    match code {
        KeyCode::Esc => {
            app.view = View::List;
            app.status = "Compose canceled".to_string();
        }
        KeyCode::Tab => {
            app.compose.focus = match app.compose.focus {
                ComposeField::To => ComposeField::Subject,
                ComposeField::Subject => ComposeField::Body,
                ComposeField::Body => ComposeField::To,
            };
        }
        KeyCode::Backspace => match app.compose.focus {
            ComposeField::To => {
                app.compose.to.pop();
            }
            ComposeField::Subject => {
                app.compose.subject.pop();
            }
            ComposeField::Body => {
                app.compose.body.pop();
            }
        },
        KeyCode::Enter => {
            if app.compose.focus == ComposeField::Body {
                app.compose.body.push('\n');
            } else {
                app.compose.focus = match app.compose.focus {
                    ComposeField::To => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::Body,
                };
            }
        }
        KeyCode::Char(ch) => match app.compose.focus {
            ComposeField::To => app.compose.to.push(ch),
            ComposeField::Subject => app.compose.subject.push(ch),
            ComposeField::Body => app.compose.body.push(ch),
        },
        _ => {}
    }
}

fn next_field(f: ConfigField) -> ConfigField {
    use ConfigField::*;
    match f {
        ImapHost => ImapPort,
        ImapPort => ImapUser,
        ImapUser => ImapPass,
        ImapPass => ImapStarttls,
        ImapStarttls => SmtpHost,
        SmtpHost => SmtpPort,
        SmtpPort => SmtpUser,
        SmtpUser => SmtpPass,
        SmtpPass => SmtpStarttls,
        SmtpStarttls => UserName,
        UserName => UserEmail,
        UserEmail => ImapHost,
    }
}

fn prev_field(f: ConfigField) -> ConfigField {
    use ConfigField::*;
    match f {
        ImapHost => UserEmail,
        ImapPort => ImapHost,
        ImapUser => ImapPort,
        ImapPass => ImapUser,
        ImapStarttls => ImapPass,
        SmtpHost => ImapStarttls,
        SmtpPort => SmtpHost,
        SmtpUser => SmtpPort,
        SmtpPass => SmtpUser,
        SmtpStarttls => SmtpPass,
        UserName => SmtpStarttls,
        UserEmail => UserName,
    }
}

fn field_is_port(f: ConfigField) -> bool {
    matches!(f, ConfigField::ImapPort | ConfigField::SmtpPort)
}

fn field_is_toggle(f: ConfigField) -> bool {
    matches!(f, ConfigField::ImapStarttls | ConfigField::SmtpStarttls)
}

fn apply_cfg_edit(app: &mut App) -> Result<()> {
    let imap_port: u16 = app.cfg_edit.imap_port.parse()?;
    let smtp_port: u16 = app.cfg_edit.smtp_port.parse()?;

    app.config.imap.host = app.cfg_edit.imap_host.clone();
    app.config.imap.port = imap_port;
    app.config.imap.username = app.cfg_edit.imap_user.clone();
    app.config.imap.password = app.cfg_edit.imap_pass.clone();
    app.config.imap.starttls = app.cfg_edit.imap_starttls;

    app.config.smtp.host = app.cfg_edit.smtp_host.clone();
    app.config.smtp.port = smtp_port;
    app.config.smtp.username = app.cfg_edit.smtp_user.clone();
    app.config.smtp.password = app.cfg_edit.smtp_pass.clone();
    app.config.smtp.starttls = app.cfg_edit.smtp_starttls;

    app.config.user.name = app.cfg_edit.user_name.clone();
    app.config.user.email = app.cfg_edit.user_email.clone();

    Ok(())
}

fn open_in_editor(path: &std::path::Path) -> Result<()> {
    use crossterm::cursor::{Hide, MoveTo, Show};
    use crossterm::terminal::{Clear, ClearType};

    disable_raw_mode()?;
    execute!(stdout(), Show, LeaveAlternateScreen)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

    let status = if editor.split_whitespace().count() > 1 {
        let cmd = format!("{} {}", editor, path.display());
        Command::new("sh").arg("-c").arg(cmd).status()?
    } else {
        Command::new(editor).arg(path).status()?
    };

    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), Hide, Clear(ClearType::All), MoveTo(0, 0))?;
    enable_raw_mode()?;

    while event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }

    if !status.success() {
        return Err(anyhow!("editor exited with non-zero"));
    }
    Ok(())
}

fn reload_config_from_file(app: &mut App) -> Result<()> {
    let data = std::fs::read_to_string(&app.config_path)?;
    let cfg: Config = toml::from_str(&data)?;
    app.config = cfg.clone();
    app.cfg_edit = ConfigEditState::from_config(&cfg);
    Ok(())
}

fn handle_config_keys(
    app: &mut App,
    code: KeyCode,
    mods: KeyModifiers,
    tx: &mpsc::UnboundedSender<AppMsg>,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('s')) {
        match apply_cfg_edit(app) {
            Ok(_) => {
                if let Err(e) = app.config.save_to(&app.config_path) {
                    app.status = format!("Save error: {e}");
                    return;
                }
                app.status = "Saved config.toml".to_string();
                app.view = app.return_view;
                spawn_refresh_list(app.config.clone(), tx.clone());
            }
            Err(e) => app.status = format!("Config invalid: {e}"),
        }
        return;
    }

    match code {
        KeyCode::Esc => {
            app.view = app.return_view;
            app.status = "Back".to_string();
        }
        KeyCode::Tab => app.cfg_edit.focus = next_field(app.cfg_edit.focus),
        KeyCode::BackTab => app.cfg_edit.focus = prev_field(app.cfg_edit.focus),
        KeyCode::Char('e') => {
            if let Err(e) = open_in_editor(&app.config_path) {
                app.status = format!("Editor error: {e}");
                return;
            }
            let _ = terminal.clear();

            match reload_config_from_file(app) {
                Ok(_) => {
                    app.status = "Reloaded config".to_string();
                    spawn_refresh_list(app.config.clone(), tx.clone());
                }
                Err(e) => app.status = format!("Reload failed: {e}"),
            }
        }
        KeyCode::Char(' ') => {
            if field_is_toggle(app.cfg_edit.focus) {
                match app.cfg_edit.focus {
                    ConfigField::ImapStarttls => app.cfg_edit.imap_starttls = !app.cfg_edit.imap_starttls,
                    ConfigField::SmtpStarttls => app.cfg_edit.smtp_starttls = !app.cfg_edit.smtp_starttls,
                    _ => {}
                }
            }
        }
        KeyCode::Backspace => {
            match app.cfg_edit.focus {
                ConfigField::ImapHost => { app.cfg_edit.imap_host.pop(); }
                ConfigField::ImapPort => { app.cfg_edit.imap_port.pop(); }
                ConfigField::ImapUser => { app.cfg_edit.imap_user.pop(); }
                ConfigField::ImapPass => { app.cfg_edit.imap_pass.pop(); }

                ConfigField::SmtpHost => { app.cfg_edit.smtp_host.pop(); }
                ConfigField::SmtpPort => { app.cfg_edit.smtp_port.pop(); }
                ConfigField::SmtpUser => { app.cfg_edit.smtp_user.pop(); }
                ConfigField::SmtpPass => { app.cfg_edit.smtp_pass.pop(); }

                ConfigField::UserName => { app.cfg_edit.user_name.pop(); }
                ConfigField::UserEmail => { app.cfg_edit.user_email.pop(); }

                _ => {}
            }
        }
        KeyCode::Char(ch) => {
            if field_is_toggle(app.cfg_edit.focus) {
                return;
            }
            if field_is_port(app.cfg_edit.focus) && !ch.is_ascii_digit() {
                return;
            }

            match app.cfg_edit.focus {
                ConfigField::ImapHost => app.cfg_edit.imap_host.push(ch),
                ConfigField::ImapPort => app.cfg_edit.imap_port.push(ch),
                ConfigField::ImapUser => app.cfg_edit.imap_user.push(ch),
                ConfigField::ImapPass => app.cfg_edit.imap_pass.push(ch),
                ConfigField::SmtpHost => app.cfg_edit.smtp_host.push(ch),
                ConfigField::SmtpPort => app.cfg_edit.smtp_port.push(ch),
                ConfigField::SmtpUser => app.cfg_edit.smtp_user.push(ch),
                ConfigField::SmtpPass => app.cfg_edit.smtp_pass.push(ch),
                ConfigField::UserName => app.cfg_edit.user_name.push(ch),
                ConfigField::UserEmail => app.cfg_edit.user_email.push(ch),
                _ => {}
            }
        }
        _ => {}
    }
}
