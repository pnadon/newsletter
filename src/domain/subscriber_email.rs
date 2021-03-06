use serde::Serialize;
use validator::validate_email;

/// Represents a valid email.
#[derive(Debug, Clone, Serialize)]
pub struct SubscriberEmail(String);

impl SubscriberEmail {
  pub fn parse(s: String) -> Result<Self, String> {
    if validate_email(&s) {
      Ok(Self(s))
    } else {
      Err(format!("{} is not a valid subscriber email", s))
    }
  }
}

impl AsRef<str> for SubscriberEmail {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

#[cfg(test)]
mod tests {
  use super::SubscriberEmail;
  use claim::assert_err;
  use fake::faker::internet::en::SafeEmail; //  struct representing a safe email
  use fake::Fake; // <- import trait for SafeEmail

  #[test]
  fn empty_string_is_rejected() {
    let email = "".to_string();
    assert_err!(SubscriberEmail::parse(email));
  }

  #[test]
  fn email_missing_at_symbol_is_rejected() {
    let email = "nadon.io".to_string();
    assert_err!(SubscriberEmail::parse(email));
  }

  #[test]
  fn email_missing_subject_is_rejected() {
    let email = "@nadon.io".to_string();
    assert_err!(SubscriberEmail::parse(email));
  }

  #[derive(Debug, Clone)]
  struct ValidEmailFixture(pub String);

  impl quickcheck::Arbitrary for ValidEmailFixture {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
      let email = SafeEmail().fake_with_rng(g);
      Self(email)
    }
  }

  #[quickcheck_macros::quickcheck]
  fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture) -> bool {
    SubscriberEmail::parse(valid_email.0).is_ok()
  }
}
