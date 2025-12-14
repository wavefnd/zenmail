pub mod imap;
pub mod smtp;

#[derive(Clone, Debug)]
pub struct MessageSummary {
    pub uid: u32,
    pub from: String,
    pub date: String,
    pub subject: String,
}
