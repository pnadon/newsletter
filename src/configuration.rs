use crate::domain::SubscriberEmail;
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::{
  postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
  ConnectOptions, PgPool,
};

#[derive(Deserialize, Debug)]
pub struct Settings {
  pub database: DatabaseSettings,
  pub application: ApplicationSettings,
  pub email_client: EmailClientSettings,
}

#[derive(Deserialize, Debug)]
pub struct DatabaseSettings {
  pub username: String,
  pub password: String,
  pub host: String,
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub port: u16,
  pub database_name: String,
  pub require_ssl: bool,
}

#[derive(Deserialize, Debug)]
pub struct ApplicationSettings {
  pub host: String,
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub port: u16,
  pub base_url: String,
}

impl DatabaseSettings {
  pub fn without_db(&self) -> PgConnectOptions {
    let ssl_mode = if self.require_ssl {
      PgSslMode::Require
    } else {
      PgSslMode::Prefer
    };
    PgConnectOptions::new()
      .host(&self.host)
      .username(&self.username)
      .password(&self.password)
      .port(self.port)
      .ssl_mode(ssl_mode)
  }

  pub fn with_db(&self) -> PgConnectOptions {
    let mut opts = self.without_db().database(&self.database_name);

    opts.log_statements(log::LevelFilter::Trace);

    opts
  }

  pub fn get_db_pool(&self) -> PgPool {
    PgPoolOptions::new()
      .connect_timeout(std::time::Duration::from_secs(2))
      .connect_lazy_with(self.with_db())
  }
}

#[derive(Deserialize, Debug)]
pub struct EmailClientSettings {
  pub base_url: String,
  pub sender_email: String,
  pub authorization_token: String,
  pub default_timeout: std::time::Duration,
}

impl EmailClientSettings {
  pub fn sender(&self) -> Result<SubscriberEmail, String> {
    SubscriberEmail::parse(self.sender_email.clone())
  }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
  let mut settings = config::Config::default();

  let base_path = std::env::current_dir().expect("failed to determine current directory");
  let configuration_directory = base_path.join("configuration");

  settings.merge(config::File::from(configuration_directory.join("base")).required(true))?;

  let environment: Environment = std::env::var("APP_ENVIRONMENT")
    .unwrap_or_else(|_| "local".into())
    .try_into()
    .expect("failed to parse APP_ENVIRONMENT");

  settings
    .merge(config::File::from(configuration_directory.join(environment.as_str())).required(true))?;

  settings.merge(config::Environment::with_prefix("app").separator("__"))?;

  settings.try_into()
}

pub enum Environment {
  Local,
  Production,
}

impl Environment {
  pub fn as_str(&self) -> &'static str {
    match self {
      Environment::Local => "local",
      Environment::Production => "production",
    }
  }
}

impl TryFrom<String> for Environment {
  type Error = String;

  fn try_from(s: String) -> Result<Self, Self::Error> {
    match s.to_lowercase().as_str() {
      "local" => Ok(Self::Local),
      "production" => Ok(Self::Production),
      other => Err(format!(
        "{} is not a supported environment. Use `local` or `production`",
        other,
      )),
    }
  }
}
