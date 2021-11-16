use wiremock::{
  matchers::{method, path},
  Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[actix_rt::test]
async fn confirmations_without_token_are_rejected_with_a_badrequest() {
  let app = spawn_app().await;

  let resp = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
    .await
    .unwrap();

  assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[actix_rt::test]
async fn the_link_returned_by_subscribe_returns_an_ok_if_called() {
  let app = spawn_app().await;
  let body = "name=phil%20nadon&email=phil%40nadon.io";

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.email_server)
    .await;

  app.post_subscriptions(body.into()).await;
  let req = &app.email_server.received_requests().await.unwrap()[0];
  let confirmation_links = app.get_confirmation_links(&req);

  let resp = reqwest::get(confirmation_links.html).await.unwrap();

  assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[actix_rt::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
  let app = spawn_app().await;
  let body = "name=phil%20nadon&email=phil%40nadon.io";

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&app.email_server)
    .await;

  app.post_subscriptions(body.into()).await;
  let req = &app.email_server.received_requests().await.unwrap()[0];
  let confirmation_links = app.get_confirmation_links(&req);

  reqwest::get(confirmation_links.html)
    .await
    .unwrap()
    .error_for_status()
    .unwrap();

  let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
    .fetch_one(&app.db_pool)
    .await
    .expect("failed to fetch saved subscription");

  assert_eq!(saved.email, "phil@nadon.io");
  assert_eq!(saved.name, "phil nadon");
  assert_eq!(saved.status, "confirmed");
}
