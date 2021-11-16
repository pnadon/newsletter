use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

const FORBIDDEN_CHARACTERS: [char; 9] = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];

impl SubscriberName {
  pub fn parse(s: String) -> Result<Self, Vec<String>> {
    let validations = [
      (!s.trim().is_empty(), "name cannot be empty!"),
      (
        s.graphemes(true).count() <= 256,
        "name cannot be more than 256 characters!",
      ),
      (
        !s.chars().any(|c| FORBIDDEN_CHARACTERS.contains(&c)),
        "name cannot contain special characters!",
      ),
    ]
    .into_iter()
    .filter_map(|(is_valid, error_msg)| Self::is_valid_to_maybe_err(is_valid, error_msg))
    .collect::<Vec<String>>();

    if validations.is_empty() {
      Ok(Self(s))
    } else {
      Err(validations)
    }
  }

  fn is_valid_to_maybe_err(is_valid: bool, error_msg: &str) -> Option<String> {
    if is_valid {
      None
    } else {
      Some(error_msg.to_owned())
    }
  }

  #[allow(dead_code)]
  fn get_forbidden_characters() -> &'static [char] {
    &FORBIDDEN_CHARACTERS
  }
}

impl AsRef<str> for SubscriberName {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

#[cfg(test)]
mod tests {
  use crate::domain::SubscriberName;
  use claim::{assert_err, assert_ok};

  #[test]
  fn a_256_grapheme_long_name_is_valid() {
    let name = "a".repeat(256);
    assert_ok!(SubscriberName::parse(name));
  }

  #[test]
  fn a_name_longer_than_256_graphemes_is_rejected() {
    let name = "a".repeat(257);
    assert_err!(SubscriberName::parse(name));
  }

  #[test]
  fn whitespace_only_names_are_rejected() {
    let name = " ".to_string();
    assert_err!(SubscriberName::parse(name));
  }

  #[test]
  fn empty_string_is_rejected() {
    let name = "".to_string();
    assert_err!(SubscriberName::parse(name));
  }

  #[test]
  fn name_containing_invalid_characters_is_rejected() {
    for invalid_chr in SubscriberName::get_forbidden_characters() {
      assert_err!(SubscriberName::parse(invalid_chr.to_string()));
    }
  }

  #[test]
  fn a_valid_name_is_parsed_successfully() {
    let name = "Phil Nadon".to_string();
    assert_ok!(SubscriberName::parse(name));
  }
}
