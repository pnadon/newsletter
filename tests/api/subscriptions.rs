use wiremock::{
  matchers::{method, path},
  Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[actix_rt::test]
async fn subscribe_returns_ok_for_valid_form_data() {
  let app = spawn_app().await;

  let body = "name=phil%20nadon&email=phil%40nadon.io";

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.email_server)
    .await;

  let resp = app.post_subscriptions(body.into()).await;

  assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[actix_rt::test]
async fn subscribe_persists_the_new_subscriber() {
  let app = spawn_app().await;

  let body = "name=phil%20nadon&email=phil%40nadon.io";

  app.post_subscriptions(body.into()).await;

  let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
    .fetch_one(&app.db_pool)
    .await
    .expect("failed to fetch saved subscription");

  assert_eq!(saved.email, "phil@nadon.io");
  assert_eq!(saved.name, "phil nadon");
  assert_eq!(saved.status, "pending_confirmation");
}

#[actix_rt::test]
async fn subscribe_returns_badrequest_when_data_is_missing() {
  let app = spawn_app().await;

  let test_cases = vec![
    ("name=phil%20nadon", "missing email address"),
    ("email=phil%40nadon.io", "missing the name"),
    ("", "missing both name and email"),
  ];

  for (body, msg) in test_cases {
    let resp = app.post_subscriptions(body.into()).await;

    assert_eq!(
      resp.status(),
      reqwest::StatusCode::BAD_REQUEST,
      "expected api to fail with a 400, with {}",
      msg,
    );
  }
}

#[actix_rt::test]
async fn subscribe_returns_badrequest_when_fields_are_present_but_invalid() {
  let app = spawn_app().await;

  let test_cases = vec![
    ("name=phil%20nadon&email=", "empty email address"),
    ("name=&email=phil%40nadon.io", "empty name"),
    ("name=phil%20nadon&email=not-an-email", "invalid email"),
  ];

  for (body, msg) in test_cases {
    let resp = app.post_subscriptions(body.into()).await;

    assert_eq!(
      resp.status(),
      reqwest::StatusCode::BAD_REQUEST,
      "expected api to fail with a 400, with {}",
      msg,
    );
  }
}

#[actix_rt::test]
async fn subscriber_sends_a_confirmation_email_for_valid_data() {
  let app = spawn_app().await;
  let body = "name=phil%20nadon&email=phil%40nadon.io";

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .expect(1)
    .mount(&app.email_server)
    .await;

  app.post_subscriptions(body.into()).await;
}

#[actix_rt::test]
async fn subscriber_sends_a_confirmation_email_with_a_link() {
  let app = spawn_app().await;
  let body = "name=phil%20nadon&email=phil%40nadon.io";

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.email_server)
    .await;

  app.post_subscriptions(body.into()).await;

  let email_request = &app.email_server.received_requests().await.unwrap()[0];

  let confirmation_links = app.get_confirmation_links(&email_request);
  assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

#[actix_rt::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
  let app = spawn_app().await;
  let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

  sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;",)
    .execute(&app.db_pool)
    .await
    .unwrap();

  let response = app.post_subscriptions(body.into()).await;

  assert_eq!(
    response.status(),
    reqwest::StatusCode::INTERNAL_SERVER_ERROR
  );
}
