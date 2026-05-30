use anyhow::{bail, Context, Result};
use configparser::ini::Ini;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub access: AccessConfig,
    /// Parsed for compatibility with the Python tool's `config.ini`, but not
    /// applied: grammers 0.9 no longer exposes `device_model`/`lang_code`/
    /// `timeout` through its high-level client API.
    #[allow(dead_code)]
    pub client: ClientConfig,
}

#[derive(Debug, Clone)]
pub struct AccessConfig {
    pub session: String,
    pub api_id: i32,
    pub api_hash: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClientConfig {
    pub timeout: i32,
    pub device_model: String,
    pub lang_code: String,
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let mut ini = Ini::new();
        ini.load(path)
            .map_err(|err| anyhow::anyhow!(err))
            .with_context(|| format!("failed to load config file: {path}"))?;

        let session = read_string(&ini, "Access", "session")?;
        let api_id = read_string(&ini, "Access", "id")?
            .parse::<i32>()
            .with_context(|| "Access.id must be an integer")?;
        let api_hash = read_string(&ini, "Access", "hash")?;

        let timeout = read_string(&ini, "Client", "timeout")?
            .parse::<i32>()
            .with_context(|| "Client.timeout must be an integer")?;
        let device_model = read_string(&ini, "Client", "device_model")?;
        let lang_code = read_string(&ini, "Client", "lang_code")?;

        Ok(Self {
            access: AccessConfig {
                session,
                api_id,
                api_hash,
            },
            client: ClientConfig {
                timeout,
                device_model,
                lang_code,
            },
        })
    }
}

fn read_string(ini: &Ini, section: &str, key: &str) -> Result<String> {
    match ini.get(section, key) {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => bail!("missing required config field {section}.{key}"),
    }
}
