use newsletter::configuration::get_configuration;
use newsletter::startup::ServerBuilder;
use newsletter::telemetry::{get_subscriber, init_subscriber};
use tracing::warn;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let subscriber = get_subscriber("newsletter".into(), "info".into(), std::io::stdout);
  init_subscriber(subscriber);

  let configuration = get_configuration().expect("failed to read configuration");
  warn!(config = ?configuration); // For debugging purposes, will eventually be removed.
  ServerBuilder::build(configuration)?.run()?.await
}
