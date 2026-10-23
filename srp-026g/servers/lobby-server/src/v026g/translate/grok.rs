//! The xAI Grok backend (the owner's choice, 2026-10-23; the default): one
//! chat completions call a line, for every language its readers need,
//! answered as JSON (structured output, `response_format` `json_schema`,
//! strict).
//!
//! - Model `grok-4.20-0309-non-reasoning` by default
//!   (`SRP_CHAT_TRANSLATE_MODEL`): xAI's one non-reasoning text model on the
//!   public API, $1.25 / $0.20 cached / $2.50 a million tokens
//!   (docs.x.ai, 2026-10-23).
//! - The key is `XAI_API_KEY`, from the environment only: never in the
//!   repository, never logged.
//! - The same instructions, user message and answer as Claude's (`claude`):
//!   the prompt's framing, the glossary's terms, the source language.
//! - A refusal (`refusal` set, `finish_reason` other than `stop`) or an answer
//!   that is no JSON is declined: the readers get the original.

use std::time::Duration;

use serde_json::{Value, json};

use super::claude::{SYSTEM, parse_answer, schema, user_message};
use super::{Backend, Lang, Line, Translated};

pub const DEFAULT_MODEL: &str = "grok-4.20-0309-non-reasoning";
const URL: &str = "https://api.x.ai/v1/chat/completions";
const KEY_VAR: &str = "XAI_API_KEY";

/// The request's body for `line`.
pub fn request_body(model: &str, line: &Line) -> Value {
    json!({
        "model": model,
        // A line is at most 255 bytes, so each translation is short.
        "max_tokens": 1024,
        "temperature": 0,
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "chat_translation", "schema": schema(), "strict": true }
        },
        "messages": [
            { "role": "system", "content": SYSTEM },
            { "role": "user", "content": user_message(line) }
        ]
    })
}

/// The answer's translations (only the languages asked for).
pub fn parse_response(body: &str, to: &[Lang]) -> Result<Translated, String> {
    let response: Value = serde_json::from_str(body)
        .map_err(|error| format!("an answer that is no JSON: {error}"))?;
    let choice = &response["choices"][0];
    if choice.is_null() {
        return Err("an answer with no choice".to_owned());
    }
    let declined = Translated {
        refused: true,
        ..Translated::default()
    };
    let refusal = choice["message"]["refusal"]
        .as_str()
        .is_some_and(|refusal| !refusal.is_empty());
    let finish = choice["finish_reason"].as_str().unwrap_or("stop");
    if refusal || finish != "stop" {
        return Ok(declined);
    }
    match choice["message"]["content"].as_str() {
        Some(text) => Ok(parse_answer(text, to)),
        None => Ok(declined),
    }
}

pub struct Grok {
    agent: ureq::Agent,
    key: String,
    model: String,
}

impl Grok {
    /// With the key of `XAI_API_KEY`; an HTTP call gives up after `timeout`.
    pub fn from_env(model: &str, timeout: Duration) -> Result<Self, String> {
        let key = std::env::var(KEY_VAR)
            .ok()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| format!("{KEY_VAR} is not set"))?;
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .user_agent("skhid-srp chat translation")
            .build();
        Ok(Self {
            agent,
            key,
            model: model.to_owned(),
        })
    }
}

impl Backend for Grok {
    fn translate(&self, line: &Line) -> Result<Translated, String> {
        let body = request_body(&self.model, line).to_string();
        let response = self
            .agent
            .post(URL)
            .set("authorization", &format!("Bearer {}", self.key))
            .set("content-type", "application/json")
            .send_string(&body);
        match response {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|error| format!("the answer was not read: {error}"))?;
                parse_response(&body, &line.to)
            }
            Err(ureq::Error::Status(code, response)) => {
                let detail = response.into_string().unwrap_or_default();
                let detail: String = detail.chars().take(300).collect();
                Err(format!("HTTP {code}: {detail}"))
            }
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Line {
        Line {
            text: "го на базу".into(),
            from: Some(Lang::Ru),
            to: vec![Lang::En, Lang::Pl],
            glossary: Vec::new(),
        }
    }

    #[test]
    fn the_request_is_one_strict_json_call_with_the_shared_framing() {
        let body = request_body(DEFAULT_MODEL, &line());
        assert_eq!(body["model"], "grok-4.20-0309-non-reasoning");
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(
            body["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("video game's chat")
        );
        let sent: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(sent["text"], "го на базу");
        assert_eq!(sent["into"], json!(["en", "pl"]));
    }

    #[test]
    fn an_answer_a_refusal_and_garbage() {
        let answer = json!({ "source": "ru", "translations": [
            { "lang": "en", "text": "go to the base" },
            { "lang": "de", "text": "not asked for" }
        ]});
        let response = json!({ "choices": [{
            "finish_reason": "stop",
            "message": { "role": "assistant", "content": answer.to_string() }
        }]});
        let translated = parse_response(&response.to_string(), &[Lang::En]).unwrap();
        assert_eq!(translated.source, Some(Lang::Ru));
        assert_eq!(
            translated.lines,
            vec![(Lang::En, "go to the base".to_owned())]
        );
        assert!(!translated.refused);

        let refusal = json!({ "choices": [{
            "finish_reason": "stop",
            "message": { "role": "assistant", "content": null, "refusal": "I can't help" }
        }]});
        assert!(
            parse_response(&refusal.to_string(), &[Lang::En])
                .unwrap()
                .refused
        );
        let filtered = json!({ "choices": [{
            "finish_reason": "content_filter",
            "message": { "role": "assistant", "content": "" }
        }]});
        assert!(
            parse_response(&filtered.to_string(), &[Lang::En])
                .unwrap()
                .refused
        );
        let no_json = json!({ "choices": [{
            "finish_reason": "stop",
            "message": { "role": "assistant", "content": "Sure! Here it is" }
        }]});
        assert!(
            parse_response(&no_json.to_string(), &[Lang::En])
                .unwrap()
                .refused
        );
        assert!(parse_response("<html>", &[Lang::En]).is_err());
        assert!(parse_response("{}", &[Lang::En]).is_err());
    }
}
