use config::{ConfigError, Config, File, Environment};

#[derive(Deserialize)]
pub struct Settings {
    pub listen_addr: String,
    pub logging: String,
    pub redis_addr: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();
        s.merge(File::with_name("Settings"))?;
        s.merge(Environment::with_prefix("AUTHSRV"))?;
        s.try_into()
    }
}
