use actix_http::StatusCode;
// warning: sqlx's transactions implementation performs a rollback
// if the transaction object is dropped before a commit is called.
// since this code is asynchronous, when the transaction goes out
// of scope and a rollback operation is enqueued, that rollback
// operation won't execute immediately.
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use chrono::Utc;

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};

use uuid::Uuid;

use crate::{
  domain::{NewSubscriber, SubscriberEmail, SubscriberName},
  email_client::EmailClient,
  startup::ApplicationBaseUrl,
};

#[derive(serde::Deserialize)]
pub struct SubscribeFormData {
  email: String,
  name: String,
}

impl TryFrom<SubscribeFormData> for NewSubscriber {
  type Error = String;

  fn try_from(form: SubscribeFormData) -> Result<Self, Self::Error> {
    let name = SubscriberName::parse(form.name);
    let email = SubscriberEmail::parse(form.email).map_err(|e| vec![e]);

    match (name, email) {
      (Ok(name), Ok(email)) => Ok(NewSubscriber { name, email }),
      (Err(es), Ok(_)) => Err(es),
      (Ok(_), Err(es)) => Err(es),
      (Err(mut name_es), Err(mut email_es)) => {
        name_es.append(&mut email_es);
        Err(name_es)
      }
    }
    .map_err(|es| es.join(", "))
  }
}

/// Marks a user as a potential subscriber, and sends them a confirmation email.
/// Only after clicking the link in that email will they be confirmed subscribers.
/// (Handling the confirmation is done by another endpoint)
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name,
    )
)]
#[allow(clippy::async_yields_async)]
pub async fn subscribe(
  form: web::Form<SubscribeFormData>,
  pool: web::Data<PgPool>,
  email_client: web::Data<EmailClient>,
  base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
  let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;
  let mut transaction = pool
    .begin()
    .await
    .context("Failed to acquire a Postgres connection from the pool")?;
  let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
    .await
    .context("Failed to insert new subscriber in the database.")?;
  let token = generate_subcription_token();
  store_token(&mut transaction, subscriber_id, &token)
    .await
    .context("Failed to store the confirmation token for a new subscriber.")?;
  transaction
    .commit()
    .await
    .context("Failed to commit SQL transaction to store a new subscriber.")?;
  send_confirmation_email(&email_client, &new_subscriber, &base_url, &token)
    .await
    .context("Failed to send a confirmation email.")?;
  Ok(HttpResponse::Ok().finish())
}

/// Store a token which uniquelly identifies a subscriber.
/// This is so that we know who is confirming their subscription
/// when they click the link in the email and reach the confirmation endpoint.
#[tracing::instrument(
  name = "Store subscription token in the database",
  skip(subscription_token, transaction)
)]
pub async fn store_token(
  transaction: &mut Transaction<'_, Postgres>,
  subscriber_id: Uuid,
  subscription_token: &str,
) -> Result<(), StoreTokenError> {
  sqlx::query!(
    r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
    VALUES ($1, $2)"#,
    subscription_token,
    subscriber_id,
  )
  .execute(transaction)
  .await
  .map_err(StoreTokenError)?;
  Ok(())
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
  #[error("{0}")]
  ValidationError(String),
  #[error(transparent)]
  UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

/// Maps the internal errors to HTTP error codes that the end user sees.
impl ResponseError for SubscribeError {
  fn status_code(&self) -> StatusCode {
    match self {
      SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
      SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl std::fmt::Debug for StoreTokenError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl std::error::Error for StoreTokenError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.0)
  }
}

pub fn error_chain_fmt(
  e: &impl std::error::Error,
  f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
  writeln!(f, "{}\n", e)?;
  let mut current = e.source();
  while let Some(cause) = current {
    writeln!(f, "Caused by:\n\t{}", cause)?;
    current = cause.source();
  }
  Ok(())
}

/// Sends a confirmation email so that a user can confirm they wish to subscribe.
#[tracing::instrument(
  name = "Send a confirmation email to a new subscriber",
  skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
  email_client: &EmailClient,
  new_subscriber: &NewSubscriber,
  base_url: &ApplicationBaseUrl,
  subscription_token: &str,
) -> Result<(), reqwest::Error> {
  let confirmation_link = format!(
    "{}/subscriptions/confirm?subscription_token={}",
    base_url.as_ref(),
    subscription_token,
  );

  let text_body = format!(
    "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
    confirmation_link,
  );

  let html_body = format!(
    "Welcome to our newsletter!<br />\
    Click <a href=\"{}\">here</a> to confirm your subscription.",
    confirmation_link,
  );

  email_client
    .send_email(
      &new_subscriber.email,
      &format!("Welcome {}!", new_subscriber.name.as_ref()),
      &html_body,
      &text_body,
    )
    .await
}

/// Stores a potential subscriber into the database.
#[tracing::instrument(
  name = "Saving new subscriber details in the database",
  skip(transaction, new_subscriber)
)]
#[allow(clippy::async_yields_async)]
pub async fn insert_subscriber(
  transaction: &mut Transaction<'_, Postgres>,
  new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
  let subscriber_id = Uuid::new_v4();
  sqlx::query!(
    r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending_confirmation')
    "#,
    subscriber_id,
    new_subscriber.email.as_ref(),
    new_subscriber.name.as_ref(),
    Utc::now(),
  )
  .execute(transaction)
  .await?;
  Ok(subscriber_id)
}

fn generate_subcription_token() -> String {
  let mut rng = thread_rng();
  std::iter::repeat_with(|| rng.sample(Alphanumeric))
    .map(char::from)
    .take(25)
    .collect()
}
