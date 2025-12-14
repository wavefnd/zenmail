use ratatui::Frame;
use crate::app::{App, View};

mod list;
mod view;
mod compose;
mod config;

pub fn draw(f: &mut Frame, app: &App) {
    match app.view {
        View::List => list::draw(f, app),
        View::Mail => view::draw(f, app),
        View::Compose => compose::draw(f, app),
        View::Config => config::draw(f, app),
    }
}
