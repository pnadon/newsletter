use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;
use newsletter::{
  configuration::{get_configuration, DatabaseSettings},
  startup::ServerBuilder,
  telemetry::{get_subscriber, init_subscriber},
};

// This ensures that the global logger is only configured and initialized once.
pub static TRACING: Lazy<()> = Lazy::new(|| {
  let default_filter_level = "info".to_string();
  let subscriber_name = "test".to_string();
  if std::env::var("TEST_LOG").is_ok() {
    let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
    init_subscriber(subscriber);
  } else {
    let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
    init_subscriber(subscriber);
  }
});

/// Represents a collection of both a plaintext and html version of a confirmation link.
pub struct ConfirmationLinks {
  pub html: reqwest::Url,
  pub plain_text: reqwest::Url,
}

/// A convenient struct for testing the application.
pub struct TestApp {
  pub address: String,
  pub port: u16,
  pub db_pool: PgPool,
  pub email_server: MockServer,
  pub test_user: TestUser,
}

impl TestApp {
  /// POST to the /subscriptions endpoint.
  pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
    reqwest::Client::new()
      .post(&format!("{}/subscriptions", &self.address))
      .header("Content-Type", "application/x-www-form-urlencoded")
      .body(body)
      .send()
      .await
      .expect("failed to execute request")
  }

  /// POST to the /newsletters endpoint.
  pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
    reqwest::Client::new()
      .post(&format!("{}/newsletters", &self.address))
      .basic_auth(&self.test_user.username, Some(&self.test_user.password))
      .json(&body)
      .send()
      .await
      .expect("Failed to execute request.")
  }

  /// Parse the confirmation links from the given mock request.
  pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
    let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

    let get_link = |s: &str| {
      let links: Vec<_> = linkify::LinkFinder::new()
        .links(s)
        .filter(|l| *l.kind() == linkify::LinkKind::Url)
        .collect();

      assert_eq!(links.len(), 1);
      let raw_link = links[0].as_str();

      let mut confirmation_link = reqwest::Url::parse(raw_link).unwrap();
      // Want to make sure we are testing locally and aren't firing off requests to random servers.
      assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
      // Hack to ensure that we are using the same port as the test application.
      confirmation_link.set_port(Some(self.port)).unwrap();
      confirmation_link
    };

    let html = get_link(&body["HtmlBody"].as_str().unwrap());
    let plain_text = get_link(&body["TextBody"].as_str().unwrap());

    ConfirmationLinks { html, plain_text }
  }
}

pub async fn spawn_app() -> TestApp {
  Lazy::force(&TRACING);

  let email_server = MockServer::start().await;

  let configuration = {
    let mut c = get_configuration().expect("failed to read configuration");
    // Random database name so that tests don't clash.
    c.database.database_name = Uuid::new_v4().to_string();
    c.email_client.base_url = email_server.uri();
    c.application.port = 0;
    c
  };

  configure_database(&configuration.database).await;
  let db_pool = configuration.database.get_db_pool();

  let application = ServerBuilder::build(configuration).expect("could not create server builder");
  let port = application.local_addr().unwrap().port();
  let address = format!(
    "http://127.0.0.1:{}",
    application
      .local_addr()
      .expect("could not retrieve local address")
      .port()
  );
  let _ = tokio::spawn(application.run().expect("failed to start http server"));

  add_test_user(&db_pool).await;

  let test_app = TestApp {
    address,
    port,
    db_pool,
    email_server,
    test_user: TestUser::new(),
  };

  test_app.test_user.store(&test_app.db_pool).await;

  test_app
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
  let mut connection = PgConnection::connect_with(&config.without_db())
    .await
    .expect("failed to connect to Postgres");

  connection
    .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
    .await
    .expect("failed to create database");

  let connection_pool = PgPool::connect_with(config.with_db())
    .await
    .expect("failed to connect to Postgres");

  sqlx::migrate!("./migrations")
    .run(&connection_pool)
    .await
    .expect("failed to migrate the database");

  connection_pool
}

async fn add_test_user(pool: &PgPool) {
  sqlx::query!(
    "INSERT INTO users (user_id, username, password_hash)
    VALUES ($1, $2, $3)",
    Uuid::new_v4(),
    Uuid::new_v4().to_string(),
    Uuid::new_v4().to_string(),
  )
  .execute(pool)
  .await
  .expect("Failed to create test users.");
}

pub struct TestUser {
  pub user_id: Uuid,
  pub username: String,
  pub password: String,
}

impl TestUser {
  pub fn new() -> Self {
    Self {
      user_id: Uuid::new_v4(),
      username: Uuid::new_v4().to_string(),
      password: Uuid::new_v4().to_string(),
    }
  }

  async fn store(&self, pool: &PgPool) {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
      Algorithm::Argon2id,
      Version::V0x13,
      Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(self.password.as_bytes(), &salt)
    .unwrap()
    .to_string();

    sqlx::query!(
      "INSERT INTO users (user_id, username, password_hash)
      VALUES ($1, $2, $3)",
      self.user_id,
      self.username,
      password_hash,
    )
    .execute(pool)
    .await
    .expect("Failed to store test user.");
  }
}
