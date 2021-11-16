use crate::{configuration::EmailClientSettings, domain::SubscriberEmail};
use reqwest::{Client, Url};
use serde::Serialize;

const POSTMARK_SERVER_TOKEN_HEADER: &str = "X-Postmark-Server-Token";

pub struct EmailClient {
  http_client: Client,
  base_url: String,
  sender: SubscriberEmail,
  authorization_token: String,
}

impl EmailClient {
  pub fn new(
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: String,
    default_timeout: std::time::Duration,
  ) -> Self {
    let http_client = Client::builder().timeout(default_timeout).build().unwrap();

    Self {
      http_client,
      base_url,
      sender,
      authorization_token,
    }
  }

  pub async fn send_email(
    &self,
    recipient: &SubscriberEmail,
    subject: &str,
    html_body: &str,
    text_body: &str,
  ) -> Result<(), reqwest::Error> {
    let url = Url::parse(&self.base_url).unwrap().join("email").unwrap();

    let request_body = SendEmailRequest {
      from: &self.sender,
      to: recipient,
      subject,
      html_body,
      text_body,
    };

    self
      .http_client
      .post(url)
      .header(POSTMARK_SERVER_TOKEN_HEADER, &self.authorization_token)
      .json(&request_body)
      .send()
      .await?
      .error_for_status()?;

    Ok(())
  }
}

impl TryFrom<EmailClientSettings> for EmailClient {
  type Error = String;

  fn try_from(settings: EmailClientSettings) -> Result<Self, Self::Error> {
    Ok(EmailClient::new(
      settings.base_url.clone(),
      settings.sender()?,
      settings.authorization_token,
      settings.default_timeout,
    ))
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
  from: &'a SubscriberEmail,
  to: &'a SubscriberEmail,
  subject: &'a str,
  html_body: &'a str,
  text_body: &'a str,
}

#[cfg(test)]
mod tests {
  use fake::{
    faker::{
      internet::en::SafeEmail,
      lorem::en::{Paragraph, Sentence},
    },
    Fake, Faker,
  };
  use wiremock::{
    matchers::{any, header, header_exists, method, path},
    Match, Mock, MockServer, Request, ResponseTemplate,
  };

  use claim::{assert_err, assert_ok};

  use crate::domain::SubscriberEmail;

  use super::{EmailClient, POSTMARK_SERVER_TOKEN_HEADER};

  struct SendEmailBodyMatcher;

  impl Match for SendEmailBodyMatcher {
    fn matches(&self, request: &Request) -> bool {
      let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);

      result
        .map(|body| {
          [
            body.get("From").is_some(),
            body.get("To").is_some(),
            body.get("Subject").is_some(),
            body.get("HtmlBody").is_some(),
            body.get("TextBody").is_some(),
          ]
          .iter()
          .all(|v| *v)
        })
        .unwrap_or(false)
    }
  }

  fn subject() -> String {
    Sentence(1..2).fake()
  }

  fn content() -> String {
    Paragraph(1..10).fake()
  }

  fn email() -> SubscriberEmail {
    SubscriberEmail::parse(SafeEmail().fake()).unwrap()
  }

  fn email_client(base_url: String) -> EmailClient {
    EmailClient::new(
      base_url,
      email(),
      Faker.fake(),
      std::time::Duration::from_secs(10),
    )
  }

  #[tokio::test]
  async fn send_email_sends_the_expected_request() {
    let mock_server = MockServer::start().await;
    let email_client = email_client(mock_server.uri());

    Mock::given(header_exists(POSTMARK_SERVER_TOKEN_HEADER))
      .and(header("Content-Type", "application/json"))
      .and(path("/email"))
      .and(method("POST"))
      .and(SendEmailBodyMatcher)
      .respond_with(ResponseTemplate::new(200))
      .expect(1)
      .mount(&mock_server)
      .await;

    let _ = email_client
      .send_email(&email(), &subject(), &content(), &content())
      .await;
  }

  #[tokio::test]
  async fn send_email_succeeds_if_the_server_returns_200() {
    let mock_server = MockServer::start().await;
    let email_client = email_client(mock_server.uri());

    Mock::given(any())
      .respond_with(ResponseTemplate::new(200))
      .expect(1)
      .mount(&mock_server)
      .await;

    let outcome = email_client
      .send_email(&email(), &subject(), &content(), &content())
      .await;

    assert_ok!(outcome);
  }

  #[tokio::test]
  async fn send_email_fails_if_the_server_returns_500() {
    let mock_server = MockServer::start().await;
    let email_client = email_client(mock_server.uri());

    Mock::given(any())
      .respond_with(ResponseTemplate::new(500))
      .expect(1)
      .mount(&mock_server)
      .await;

    let outcome = email_client
      .send_email(&email(), &subject(), &content(), &content())
      .await;

    assert_err!(outcome);
  }
}
