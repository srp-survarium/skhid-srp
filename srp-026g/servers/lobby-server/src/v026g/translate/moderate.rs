//! The gate before the translation backend (`docs/notes/chat-translation.md`,
//! "What never reaches the model"): a tiny local classifier answers one
//! question per line, **may it go?** It runs as a small service of ours
//! (`deploy/chat-moderation/`), never a cloud one: the point is that a "no"
//! line leaves nothing behind it.
//!
//! - `SRP_CHAT_MODERATION_URL`: the service (`POST {"text", "lang"}`,
//!   answered `{"score": 0..1}`, the likelihood the line must not go).
//!   `none`: srp's word list alone (`swear`; for a test, not for players).
//!   Unset: chat translation stays off.
//! - `SRP_CHAT_MODERATION_THRESHOLDS`: `en=0.5,ru=0.5,...`; a score at or
//!   over its language's threshold is a "no". A language not listed:
//!   [`DEFAULT_THRESHOLD`].
//! - `SRP_CHAT_MODERATION_TIMEOUT_MS` (default 300): longer, or any error,
//!   is a "no" for the line (fail closed): its readers get the original.

use std::time::Duration;

use serde_json::{Value, json};

use super::Lang;

/// The threshold of a language the settings do not name.
pub const DEFAULT_THRESHOLD: f32 = 0.5;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(300);

/// A local classifier.
pub trait Moderator: Send + Sync {
    /// The likelihood (0 to 1) that `text`, guessed to be in `lang`, must
    /// not go to translation. Called on a worker thread.
    fn score(&self, text: &str, lang: Option<Lang>) -> Result<f32, String>;
}

/// The gate's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    /// `None`: the word list alone.
    pub url: Option<String>,
    pub thresholds: Vec<(Lang, f32)>,
    pub timeout: Duration,
}

impl Gate {
    /// The threshold of `lang`.
    pub fn threshold(&self, lang: Option<Lang>) -> f32 {
        lang.and_then(|lang| {
            self.thresholds
                .iter()
                .find(|(other, _)| *other == lang)
                .map(|(_, threshold)| *threshold)
        })
        .unwrap_or(DEFAULT_THRESHOLD)
    }

    /// `SRP_CHAT_MODERATION_*`; `Err` when translation must stay off.
    pub fn from_env() -> Result<Self, String> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let url = match var("SRP_CHAT_MODERATION_URL") {
            None => {
                return Err("SRP_CHAT_MODERATION_URL is not set (the local gate before \
                            translation; `none` for the word list alone)"
                    .to_owned());
            }
            Some(none) if none == "none" => None,
            Some(url) if url.starts_with("http://") || url.starts_with("https://") => Some(url),
            Some(other) => {
                return Err(format!(
                    "SRP_CHAT_MODERATION_URL is `{other}`; it must be a URL or `none`"
                ));
            }
        };
        let thresholds = match var("SRP_CHAT_MODERATION_THRESHOLDS") {
            None => Vec::new(),
            Some(value) => parse_thresholds(&value).ok_or_else(|| {
                format!(
                    "SRP_CHAT_MODERATION_THRESHOLDS is `{value}`; it must be like en=0.5,ru=0.6"
                )
            })?,
        };
        let timeout = match var("SRP_CHAT_MODERATION_TIMEOUT_MS") {
            None => DEFAULT_TIMEOUT,
            Some(value) => match value.parse::<u64>() {
                Ok(ms @ 20..=2000) => Duration::from_millis(ms),
                _ => {
                    return Err(format!(
                        "SRP_CHAT_MODERATION_TIMEOUT_MS is `{value}`; it must be 20 to 2000"
                    ));
                }
            },
        };
        Ok(Self {
            url,
            thresholds,
            timeout,
        })
    }

    /// The service's client, if there is one.
    pub fn moderator(&self) -> Option<Http> {
        self.url.as_ref().map(|url| Http::new(url, self.timeout))
    }
}

/// `en=0.5,ru=0.6`: each threshold over 0 and at most 1.
pub fn parse_thresholds(value: &str) -> Option<Vec<(Lang, f32)>> {
    value
        .split(',')
        .map(|pair| {
            let (code, threshold) = pair.trim().split_once('=')?;
            let lang = Lang::from_code(code.trim())?;
            let threshold: f32 = threshold.trim().parse().ok()?;
            (threshold > 0.0 && threshold <= 1.0).then_some((lang, threshold))
        })
        .collect()
}

/// The service over HTTP.
pub struct Http {
    agent: ureq::Agent,
    url: String,
}

impl Http {
    pub fn new(url: &str, timeout: Duration) -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(timeout).build(),
            url: url.to_owned(),
        }
    }
}

/// The score in the service's answer.
pub fn parse_score(body: &str) -> Result<f32, String> {
    let answer: Value = serde_json::from_str(body)
        .map_err(|error| format!("an answer that is no JSON: {error}"))?;
    let score = answer["score"].as_f64().ok_or("an answer with no score")?;
    if !(0.0..=1.0).contains(&score) {
        return Err(format!("a score of {score}"));
    }
    Ok(score as f32)
}

impl Moderator for Http {
    fn score(&self, text: &str, lang: Option<Lang>) -> Result<f32, String> {
        let body = json!({ "text": text, "lang": lang.map(Lang::code) }).to_string();
        match self
            .agent
            .post(&self.url)
            .set("content-type", "application/json")
            .send_string(&body)
        {
            Ok(response) => parse_score(
                &response
                    .into_string()
                    .map_err(|error| format!("the answer was not read: {error}"))?,
            ),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_and_scores_are_read() {
        assert_eq!(
            parse_thresholds("en=0.5, ru=0.62"),
            Some(vec![(Lang::En, 0.5), (Lang::Ru, 0.62)])
        );
        assert_eq!(parse_thresholds("xx=0.5"), None);
        assert_eq!(parse_thresholds("en=0"), None);
        let gate = Gate {
            url: None,
            thresholds: vec![(Lang::Ru, 0.7)],
            timeout: DEFAULT_TIMEOUT,
        };
        assert_eq!(gate.threshold(Some(Lang::Ru)), 0.7);
        assert_eq!(gate.threshold(Some(Lang::Pl)), DEFAULT_THRESHOLD);
        assert_eq!(gate.threshold(None), DEFAULT_THRESHOLD);
        assert_eq!(parse_score(r#"{"score": 0.25}"#), Ok(0.25));
        assert!(parse_score(r#"{"score": 3}"#).is_err());
        assert!(parse_score("<html>").is_err());
    }
}
