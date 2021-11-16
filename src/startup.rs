use actix_web::dev::Server;
use actix_web::{
  web::{get, post, Data},
  App, HttpServer,
};

use sqlx::PgPool;
use std::net::{SocketAddr, TcpListener};
use tracing_actix_web::TracingLogger;

use crate::configuration::Settings;
use crate::email_client::EmailClient;
use crate::routes::{self, publish_newsletter};

#[derive(Debug, Clone)]
pub struct ApplicationBaseUrl(String);

impl AsRef<str> for ApplicationBaseUrl {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

pub struct ServerBuilder {
  listener: TcpListener,
  db_pool: PgPool,
  email_client: EmailClient,
  base_url: ApplicationBaseUrl,
}

impl ServerBuilder {
  pub fn build(configuration: Settings) -> Result<Self, std::io::Error> {
    let db_pool = configuration.database.get_db_pool();

    let email_client = EmailClient::try_from(configuration.email_client)
      .expect("failed to parse EmailClientSettings");

    let address = format!(
      "{}:{}",
      configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;

    let base_url = ApplicationBaseUrl(configuration.application.base_url);
    Ok(Self {
      listener,
      db_pool,
      email_client,
      base_url,
    })
  }

  pub fn run(self) -> Result<Server, std::io::Error> {
    let Self {
      listener,
      db_pool,
      email_client,
      base_url,
    } = self;
    let db_pool = Data::new(db_pool);
    let email_client = Data::new(email_client);
    let base_url = Data::new(base_url);

    Ok(
      HttpServer::new(move || {
        App::new()
          .wrap(TracingLogger::default())
          .route("/health_check", get().to(routes::health))
          .route("/newsletters", post().to(publish_newsletter))
          .route("/subscriptions", post().to(routes::subscribe))
          .route("/subscriptions/confirm", get().to(routes::confirm))
          .app_data(db_pool.clone())
          .app_data(email_client.clone())
          .app_data(base_url.clone())
      })
      .listen(listener)?
      .run(),
    )
  }

  pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
    self.listener.local_addr()
  }
}
