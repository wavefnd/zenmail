use anyhow::Result;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};

use crate::config::{MailConfig, UserConfig};

fn is_localhost(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost"
}

pub fn send(cfg: &MailConfig, user: &UserConfig, to: &str, subject: &str, body: &str) -> Result<()> {
    let email = Message::builder()
        .from(user.email.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .body(body.to_string())?;

    let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());

    let mut tlsb = TlsParameters::builder(cfg.host.clone());
    if is_localhost(&cfg.host) {
        tlsb = tlsb
            .dangerous_accept_invalid_certs(true)
            .dangerous_accept_invalid_hostnames(true);
    }
    let tls = tlsb.build()?;

    let mailer = SmtpTransport::builder_dangerous(&cfg.host)
        .port(cfg.port)
        .credentials(creds)
        .tls(Tls::Required(tls))
        .build();

    mailer.send(&email)?;
    Ok(())
}
