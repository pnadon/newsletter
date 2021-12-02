use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};

use uuid::Uuid;

use wiremock::matchers::{any, method, path};

use wiremock::{Mock, ResponseTemplate};

#[actix_rt::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
  let app = spawn_app().await;
  create_unconfirmed_subscriber(&app).await;

  Mock::given(any())
    .respond_with(ResponseTemplate::new(200))
    .expect(0)
    .mount(&app.email_server)
    .await;

  let body = serde_json::json!({
    "title": "Newsletter title",
    "content": {
      "text": "Newsletter body as plain text",
      "html": "<p>Newsletter body as HTML</p>",
    }
  });
  let resp = app.post_newsletters(body).await;

  assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[actix_rt::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
  let app = spawn_app().await;
  create_confirmed_subscriber(&app).await;

  Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .expect(1)
    .mount(&app.email_server)
    .await;

  let body = serde_json::json!({
    "title": "Newsletter title",
    "content": {
      "text": "Newsletter body as plain text",
      "html": "<p>Newsletter body as HTML</p>",
    }
  });
  let resp = app.post_newsletters(body).await;

  assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[actix_rt::test]
async fn newsletters_returns_badrequest_for_invalid_data() {
  let app = spawn_app().await;
  let test_cases = vec![
    (
      serde_json::json!({
        "content": {
          "text": "Newsletter body as plain text",
          "html": "<p>Newsletter body as HTML</p>",
        }
      }),
      "missing title",
    ),
    (
      serde_json::json!({"title": "Newsletter!"}),
      "missing content",
    ),
  ];

  for (body, msg) in test_cases {
    let resp = app.post_newsletters(body).await;

    assert_eq!(
      resp.status(),
      reqwest::StatusCode::BAD_REQUEST,
      "Expected API to respond with BAD_REQUEST, for {}",
      msg
    );
  }
}

#[actix_rt::test]
async fn requests_missing_authorization_are_rejected() {
  let app = spawn_app().await;

  let resp = reqwest::Client::new()
    .post(&format!("{}/newsletters", &app.address))
    .json(&serde_json::json!({
      "title": "Newsletter title",
      "content": {
        "text": "Newsletter body as plain text",
        "html": "<p>Newsletter body as HTML</p>",
      }
    }))
    .send()
    .await
    .expect("Failed to execute request.");

  assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
  assert_eq!(
    r#"Basic realm="publish""#,
    resp.headers()["WWW-Authenticate"]
  );
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
  let body = "name=phil%20nadon&email=phil%40nadon.io";

  let _mock_guard = Mock::given(path("/email"))
    .and(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .named("Create uncomfirmed subscriber")
    .expect(1)
    .mount_as_scoped(&app.email_server)
    .await;

  app
    .post_subscriptions(body.into())
    .await
    .error_for_status()
    .unwrap();

  let email_request = &app
    .email_server
    .received_requests()
    .await
    .unwrap()
    .pop()
    .unwrap();

  app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
  let confirmation_link = create_unconfirmed_subscriber(app).await;

  reqwest::get(confirmation_link.html)
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
}

#[actix_rt::test]
async fn non_existent_user_is_rejected() {
  let app = spawn_app().await;
  let username = Uuid::new_v4().to_string();
  let password = Uuid::new_v4().to_string();

  let response = reqwest::Client::new()
    .post(&format!("{}/newsletters", &app.address))
    .basic_auth(username, Some(password))
    .json(&serde_json::json!({
      "title": "Newsletter title",
      "content": {
        "text": "Newsletter body as plain text",
        "html": "<p>Newsletter body as HTML</p>",
      }
    }))
    .send()
    .await
    .expect("Failed to execute request.");

  assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
  assert_eq!(
    r#"Basic realm="publish""#,
    response.headers()["WWW-Authenticate"]
  );
}

#[actix_rt::test]
async fn invalid_password_is_rejected() {
  let app = spawn_app().await;
  let username = &app.test_user.username;
  let password = Uuid::new_v4().to_string();

  assert_ne!(app.test_user.password, password);

  let response = reqwest::Client::new()
    .post(&format!("{}/newsletters", &app.address))
    .basic_auth(username, Some(password))
    .json(&serde_json::json!({
      "title": "Newsletter title",
      "content": {
        "text": "Newsletter body as plain text",
        "html": "<p>Newsletter body as HTML</p>",
      }
    }))
    .send()
    .await
    .expect("Failed to execute request.");

  assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
  assert_eq!(
    r#"Basic realm="publish""#,
    response.headers()["WWW-Authenticate"]
  );
}
