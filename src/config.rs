use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::{fs, path::{Path, PathBuf}};

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub imap: MailConfig,
    pub smtp: MailConfig,
    pub user: UserConfig,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub starttls: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct UserConfig {
    pub name: String,
    pub email: String,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        Ok(dir.join("zenmail").join("config.toml"))
    }

    pub fn load_or_create() -> Result<(Self, bool, PathBuf)> {
        let path = Self::path()?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, DEFAULT_CONFIG)?;
            let cfg: Self = toml::from_str(DEFAULT_CONFIG)?;
            return Ok((cfg, true, path));
        }

        let data = fs::read_to_string(&path)?;
        let cfg = toml::from_str(&data)?;
        Ok((cfg, false, path))
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        fs::write(path, toml)?;
        Ok(())
    }
}

const DEFAULT_CONFIG: &str = r#"
[imap]
host = "127.0.0.1"
port = 1143
username = "you@email.ml"
password = "BRIDGE_PASSWORD"
starttls = true

[smtp]
host = "127.0.0.1"
port = 1025
username = "you@email.ml"
password = "BRIDGE_PASSWORD"
starttls = true

[user]
name = "Your Name"
email = "you@email.ml"
"#;
