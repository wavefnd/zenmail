use anyhow::{Result, anyhow};
use native_tls::TlsConnector;
use std::net::TcpStream;

use crate::config::MailConfig;
use crate::mail::MessageSummary;

fn is_localhost(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost"
}

fn tls_connector_for(cfg: &MailConfig) -> Result<TlsConnector> {
    let mut b = TlsConnector::builder();

    if is_localhost(&cfg.host) {
        b.danger_accept_invalid_certs(true);
        b.danger_accept_invalid_hostnames(true);
    }

    Ok(b.build()?)
}

fn connect(cfg: &MailConfig) -> Result<imap::Session<native_tls::TlsStream<TcpStream>>> {
    let tls = tls_connector_for(cfg)?;

    // STARTTLS / TLS 선택
    let client = if cfg.starttls {
        imap::connect_starttls((cfg.host.as_str(), cfg.port), &cfg.host, &tls)?
    } else {
        imap::connect((cfg.host.as_str(), cfg.port), &cfg.host, &tls)?
    };

    let session = client
        .login(&cfg.username, &cfg.password)
        .map_err(|e| e.0)?;

    Ok(session)
}

fn bytes_opt_to_string(v: Option<&[u8]>) -> String {
    v.map(|b| String::from_utf8_lossy(b).trim().to_string())
        .unwrap_or_default()
}

fn addr_to_string(name: Option<&[u8]>, mailbox: Option<&[u8]>, host: Option<&[u8]>) -> String {
    let name = name.map(|b| String::from_utf8_lossy(b).trim().to_string()).unwrap_or_default();
    let mailbox = mailbox.map(|b| String::from_utf8_lossy(b).to_string());
    let host = host.map(|b| String::from_utf8_lossy(b).to_string());

    match (name.is_empty(), mailbox, host) {
        (false, Some(m), Some(h)) => format!("{} <{}@{}>", name, m, h),
        (_, Some(m), Some(h)) => format!("<{}@{}>", m, h),
        _ => name,
    }
}

pub fn fetch_summaries(cfg: &MailConfig, limit: usize) -> Result<Vec<MessageSummary>> {
    let mut sess = connect(cfg)?;
    sess.select("INBOX")?;

    let mut uids: Vec<u32> = sess.uid_search("ALL")?.into_iter().collect();
    if uids.is_empty() {
        let _ = sess.logout();
        return Ok(vec![]);
    }

    uids.sort_unstable();

    let mut picked: Vec<u32> = uids.into_iter().rev().take(limit).collect();
    picked.reverse();

    let mut out = Vec::with_capacity(picked.len());

    for uid in picked {
        let fetches = sess.uid_fetch(uid.to_string(), "ENVELOPE")?;
        let f = fetches.iter().next().ok_or_else(|| anyhow!("no fetch result"))?;
        let env = f.envelope().ok_or_else(|| anyhow!("no envelope"))?;

        let from = if let Some(froms) = &env.from {
            if let Some(a) = froms.get(0) {
                addr_to_string(a.name.as_deref(), a.mailbox.as_deref(), a.host.as_deref())
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let date = bytes_opt_to_string(env.date.as_deref());
        let subject = bytes_opt_to_string(env.subject.as_deref());

        out.push(MessageSummary { uid, from, date, subject });
    }

    let _ = sess.logout();
    Ok(out)
}

pub fn fetch_body_plain(cfg: &MailConfig, uid: u32) -> Result<String> {
    let mut sess = connect(cfg)?;
    sess.select("INBOX")?;

    let fetches = sess.uid_fetch(uid.to_string(), "BODY.PEEK[]")?;
    let f = fetches.iter().next().ok_or_else(|| anyhow!("no fetch result"))?;
    let raw = f.body().ok_or_else(|| anyhow!("no body"))?;

    let parsed = mailparse::parse_mail(raw)?;
    let text = extract_text_plain(&parsed);

    let _ = sess.logout();
    Ok(text)
}

fn extract_text_plain(m: &mailparse::ParsedMail) -> String {
    if !m.subparts.is_empty() {
        let mut out = String::new();
        for sp in &m.subparts {
            let t = extract_text_plain(sp);
            if !t.trim().is_empty() {
                if !out.is_empty() { out.push_str("\n\n"); }
                out.push_str(&t);
            }
        }
        return out;
    }

    let ctype = m.ctype.mimetype.to_lowercase();
    if ctype == "text/plain" {
        if let Ok(body) = m.get_body() {
            return body;
        }
    }

    String::new()
}
