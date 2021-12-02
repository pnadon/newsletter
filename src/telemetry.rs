use actix_web::rt::task::JoinHandle;
use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

/// Get a subscriber which can then be used to initialize a logger.
pub fn get_subscriber(
  name: String,
  env_filter: String,
  sink: impl for<'a> MakeWriter<'a> + Send + Sync + 'static,
) -> impl Subscriber + Sync + Send {
  let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

  let formatting_layer = BunyanFormattingLayer::new(name, sink);

  Registry::default()
    .with(env_filter)
    .with(JsonStorageLayer)
    .with(formatting_layer)
}

/// Initialize the global logger with the given subscriber.
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
  LogTracer::init().expect("failed to set logger");
  set_global_default(subscriber).expect("failed to set subscriber");
}

/// A convenient wrapper which does the following:
/// 1. Spawn a task inside of a thread used for CPU-intensive or other tasks which are allowed to block.
///    We want to move CPU-intensive tasks to this thread so that it does not cause delays in other threads,
///    which are primarily IO-bound.
/// 2. Create a span for the above process.
pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
  F: FnOnce() -> R + Send + 'static,
  R: Send + 'static,
{
  let current_span = tracing::Span::current();
  actix_web::rt::task::spawn_blocking(move || current_span.in_scope(f))
}
