use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
  #[allow(unused)]
  subscription_token: String,
}

/// Endpoint is used for confirming that a potential subscriber wishes to receive newsletters.
/// This endpoint is accessed by a user who clicked a confirmation link in an email we sent.
#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
#[allow(clippy::async_yields_async)]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
  match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
    Ok(Some(subscriber_id)) => match confirm_subscriber(&pool, subscriber_id).await {
      Ok(_) => HttpResponse::Ok(),
      Err(_) => HttpResponse::InternalServerError(),
    },
    Ok(None) => HttpResponse::Unauthorized(),
    Err(_) => HttpResponse::InternalServerError(),
  }
  .finish()
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
  match sqlx::query!(
    r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
    subscriber_id,
  )
  .execute(pool)
  .await
  {
    Ok(_) => Ok(()),
    Err(e) => {
      tracing::error!("Failed to execute query: {:?}", e);
      Err(e)
    }
  }
}

/// Token is used to identify which user wishes to confirm their subscription.
#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
  pool: &PgPool,
  subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
  match sqlx::query!(
    r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
    subscription_token,
  )
  .fetch_optional(pool)
  .await
  {
    Ok(maybe_v) => Ok(maybe_v.map(|r| r.subscriber_id)),
    Err(e) => {
      tracing::error!(error = %e, "failed to execute query");
      Err(e)
    }
  }
}
