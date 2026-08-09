use std::env;
use std::fs;
use std::io;

use serde::Deserialize;

const DEFAULT_DB_PATH: &str = "./wapp_simple_stats_rust.db";

#[derive(Deserialize, Debug, Clone)]
struct Site {
    title: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Theme {
    auto: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct Database {
    path: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct BasicCredentials {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Auth {
    enabled: bool,
    basic: BasicCredentials,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
struct Privacy {
    anonymize_ip: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self { anonymize_ip: true }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct AppConfig {
    site: Site,
    theme: Theme,
    auth: Auth,
    #[serde(default)]
    privacy: Privacy,
    database: Option<Database>,
}

impl AppConfig {
    pub(crate) fn site_title(&self) -> &str {
        &self.site.title
    }

    pub(crate) fn theme_auto(&self) -> bool {
        self.theme.auto
    }

    pub(crate) fn auth_enabled(&self) -> bool {
        self.auth.enabled
    }

    pub(crate) fn credentials(&self) -> &BasicCredentials {
        &self.auth.basic
    }

    pub(crate) fn anonymize_ip(&self) -> bool {
        self.privacy.anonymize_ip
    }

    pub(crate) fn database_path(&self) -> &str {
        self.database
            .as_ref()
            .map_or(DEFAULT_DB_PATH, |database| database.path.as_str())
    }
}

pub(crate) fn load() -> io::Result<AppConfig> {
    let config_text = fs::read_to_string("config.yaml").map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot read config.yaml (copy config.example.yaml first): {error}"),
        )
    })?;

    let mut config: AppConfig = serde_yaml_ng::from_str(&config_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid config.yaml: {error}"),
        )
    })?;

    if let Ok(username) = env::var("WAPP_STATS_USERNAME") {
        config.auth.basic.username = username;
    }
    if let Ok(password) = env::var("WAPP_STATS_PASSWORD") {
        config.auth.basic.password = password;
    }

    validate(&config)?;
    if !config.auth.enabled {
        eprintln!("WARNING: statistics authentication is disabled; do not expose the app publicly");
    }

    Ok(config)
}

fn validate(config: &AppConfig) -> io::Result<()> {
    if config.site.title.trim().is_empty() {
        return Err(invalid("site.title must not be empty"));
    }
    if config.database_path().trim().is_empty() {
        return Err(invalid("database.path must not be empty"));
    }
    if config.auth.enabled && config.auth.basic.username.trim().is_empty() {
        return Err(invalid("auth.basic.username must not be empty"));
    }
    if config.auth.enabled && config.auth.basic.password.len() < 12 {
        return Err(invalid(
            "auth.basic.password must contain at least 12 bytes; WAPP_STATS_PASSWORD can override it",
        ));
    }
    if config.auth.enabled && config.auth.basic.password == "replace-with-a-random-password" {
        return Err(invalid(
            "replace the example auth password or set WAPP_STATS_PASSWORD",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
