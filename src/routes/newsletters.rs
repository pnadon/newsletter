use actix_http::{
  header::{HeaderMap, HeaderValue},
  StatusCode,
};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use reqwest::header;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
  domain::SubscriberEmail, email_client::EmailClient, routes::error_chain_fmt,
  telemetry::spawn_blocking_with_tracing,
};

/// Data contained in the body of the request.
/// The request is for Postmark's API, and thus title corresponds
/// to the email subject, and content corresponds to the email's body.
#[derive(Deserialize)]
pub struct BodyData {
  title: String,
  content: Content,
}

/// Content of the email, which is in plaintext and/or html.
#[derive(Deserialize)]
pub struct Content {
  html: String,
  text: String,
}

/// Errors which may occur during the publishing step.
#[derive(thiserror::Error)]
pub enum PublishError {
  #[error("Authentication failed.")]
  AuthError(#[source] anyhow::Error),
  #[error(transparent)]
  UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl ResponseError for PublishError {
  fn status_code(&self) -> StatusCode {
    match self {
      PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
      PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    let status_code = self.status_code();
    match self {
      PublishError::AuthError(_) => {
        let mut resp = HttpResponse::new(status_code);

        let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();

        resp
          .headers_mut()
          .insert(header::WWW_AUTHENTICATE, header_value);

        resp
      }
      PublishError::UnexpectedError(_) => HttpResponse::new(status_code),
    }
  }
}

/// Publishes a newsletter to subscribers.
/// This endpoint requires authentication due to the risk of abuse.
#[tracing::instrument(
  name = "Publishing newsletter to confirmed subscribers",
  skip(body, pool, email_client, request),
  fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
  body: web::Json<BodyData>,
  pool: web::Data<PgPool>,
  email_client: web::Data<EmailClient>,
  request: web::HttpRequest,
) -> Result<HttpResponse, PublishError> {
  let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

  let _user_id = validate_credentials(credentials, &pool).await?;

  let subscribers = get_confirmed_subscribers(&pool).await?;
  for subscriber in subscribers {
    match subscriber {
      Ok(subscriber) => {
        email_client
          .send_email(
            &subscriber.email,
            &body.title,
            &body.content.html,
            &body.content.text,
          )
          .await
          .with_context(|| {
            format!(
              "Failed to send newsletter issue to {}",
              subscriber.email.as_ref()
            )
          })?;
      }
      Err(e) => {
        tracing::warn!(
          error.cause_chain = ?e,
          "Skipping a confirmed subscriber. \
          Their stored contact details are invalid",
        );
      }
    }
  }
  Ok(HttpResponse::Ok().finish())
}

struct Credentials {
  username: String,
  password: String,
}

/// Parses the header into user credentials, using Basic Authentication.
/// https://en.wikipedia.org/wiki/Basic_access_authentication.
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
  let header_value = headers
    .get("Authorization")
    .context("'Authorization' header is missing")?
    .to_str()
    .context("'Authorization' header is not a valid UTF8 encoded string.")?;

  let encoded_segment = header_value
    .strip_prefix("Basic ")
    .context("Authorization scheme is not Basic.")?;

  let decoded_bytes = base64::decode_config(encoded_segment, base64::STANDARD)
    .context("Failed to decode Credentials using base64.")?;

  let decoded_credentials = String::from_utf8(decoded_bytes)
    .context("Decoded credential data is not a valid UTF8 encoded string.")?;

  let mut credentials = decoded_credentials.splitn(2, ':');

  let username = credentials
    .next()
    .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
    .to_string();

  let password = credentials
    .next()
    .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
    .to_string();

  Ok(Credentials { username, password })
}

struct ConfirmedSubscriber {
  email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
  pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
  Ok(
    sqlx::query!(
      r#"
      SELECT email
      FROM subscriptions
      WHERE status = 'confirmed'
      "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
      Ok(email) => Ok(ConfirmedSubscriber { email }),
      Err(e) => Err(anyhow::anyhow!(e)),
    })
    .collect(),
  )
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
async fn validate_credentials(
  credentials: Credentials,
  pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
  let (user_id, expected_password_hash) = get_stored_credentials(&credentials.username, pool)
    .await
    .map_err(PublishError::UnexpectedError)?
    .map(|(u, p)| (Some(u), p))
    .unwrap_or((None, "$argon2id$v=19$m=15000,t=2,p=1$gZiV/M1gPc22ElAH/Jh1Hw$CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno".to_string()));

  spawn_blocking_with_tracing(move || {
    verify_password_hash(expected_password_hash, credentials.password)
  })
  .await
  .context("Failed to spawn blocking task.")
  .map_err(PublishError::UnexpectedError)??;

  user_id.ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Unknown username.")))
}

#[tracing::instrument(
  name = "Verify password hash",
  skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
  expected_password_hash: String,
  password_candidate: String,
) -> Result<(), PublishError> {
  let expected_password_hash = PasswordHash::new(&expected_password_hash)
    .context("Failed to parse hash in PHC string format.")
    .map_err(PublishError::UnexpectedError)?;

  Argon2::default()
    .verify_password(password_candidate.as_bytes(), &expected_password_hash)
    .context("Invalid password.")
    .map_err(PublishError::AuthError)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
  username: &str,
  pool: &PgPool,
) -> Result<Option<(uuid::Uuid, String)>, anyhow::Error> {
  Ok(
    sqlx::query!(
      r#"
      SELECT user_id, password_hash
      FROM users
      WHERE username = $1
      "#,
      username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|row| (row.user_id, row.password_hash)),
  )
}
