use crate::AiError;
use serde::{Deserialize, Serialize};

pub const ANTHROPIC_VERSION: &str = "2023-06-01";
const MESSAGE_TOKENS: u32 = 4096;
const TEST_TOKENS: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum AiProvider {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
}

impl AiProvider {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI-compatible",
        }
    }

    #[must_use]
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAi => "https://api.openai.com/v1",
        }
    }

    #[must_use]
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-haiku-5-5",
            Self::OpenAi => "gpt-5.4-mini",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: AiProvider,

    pub base_url: String,

    pub model: String,
    pub instructions: String,
    pub max_diff_bytes: u32,
}

impl AiConfig {
    #[must_use]
    pub fn base_url(&self) -> String {
        let url = self.base_url.trim().trim_end_matches('/');
        if url.is_empty() {
            self.provider.default_base_url().to_owned()
        } else {
            url.to_owned()
        }
    }

    #[must_use]
    pub fn model(&self) -> String {
        let model = self.model.trim();
        if model.is_empty() {
            self.provider.default_model().to_owned()
        } else {
            model.to_owned()
        }
    }

    pub fn host(&self) -> Result<String, AiError> {
        Ok(self
            .service()?
            .host()
            .ok_or(AiError::BaseUrl)?
            .to_ascii_lowercase())
    }

    pub fn service(&self) -> Result<ureq::http::Uri, AiError> {
        let base = self.base_url();
        let uri: ureq::http::Uri = base.parse().map_err(|_| AiError::BaseUrl)?;
        if !matches!(uri.scheme_str(), Some("http" | "https"))
            || uri
                .authority()
                .is_none_or(|authority| authority.as_str().contains('@'))
            || uri.host().is_none_or(str::is_empty)
            || base.contains(['?', '#', '\\'])
        {
            return Err(AiError::BaseUrl);
        }
        Ok(uri)
    }

    pub fn service_id(&self) -> Result<String, AiError> {
        let uri = self.service()?;
        let authority = uri
            .authority()
            .ok_or(AiError::BaseUrl)?
            .as_str()
            .to_ascii_lowercase();
        let scheme = uri.scheme_str().ok_or(AiError::BaseUrl)?;
        Ok(format!(
            "{scheme}://{authority}{}",
            uri.path().trim_end_matches('/')
        ))
    }

    pub fn needs_key(&self) -> Result<bool, AiError> {
        Ok(self.host()?
            == match self.provider {
                AiProvider::Anthropic => "api.anthropic.com",
                AiProvider::OpenAi => "api.openai.com",
            })
    }

    #[must_use]
    pub fn is_plain_http(&self) -> bool {
        self.base_url().to_lowercase().starts_with("http://")
    }

    pub fn endpoint(&self) -> Result<String, AiError> {
        let base = self.service_id()?;
        Ok(match self.provider {
            AiProvider::Anthropic if base.ends_with("/v1/messages") => base,
            AiProvider::Anthropic if base.ends_with("/v1") => format!("{base}/messages"),
            AiProvider::Anthropic => format!("{base}/v1/messages"),
            AiProvider::OpenAi if base.ends_with("/chat/completions") => base,
            AiProvider::OpenAi => format!("{base}/chat/completions"),
        })
    }
}

#[must_use]
pub fn takes_effort(model: &str) -> bool {
    const FAMILIES: &[&str] = &[
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-haiku-5",
        "claude-fable",
        "claude-mythos",
        "claude-opus-4-5",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-sonnet-4-6",
    ];
    let model = model.trim().to_lowercase();
    FAMILIES.iter().any(|family| model.starts_with(family))
}

#[must_use]
pub fn request_body(config: &AiConfig, system: &str, user: &str, test: bool) -> serde_json::Value {
    let model = config.model();
    let max_tokens = if test { TEST_TOKENS } else { MESSAGE_TOKENS };
    match config.provider {
        AiProvider::Anthropic => {
            let mut body = serde_json::json!({
                "model": model,
                "max_tokens": max_tokens,
                "stream": true,
                "system": system,
                "messages": [{ "role": "user", "content": user }],
            });
            if takes_effort(&model) {
                body["output_config"] = serde_json::json!({ "effort": "low" });
            }
            body
        }
        AiProvider::OpenAi => {
            let mut body = serde_json::json!({
                "model": model,
                "stream": true,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user },
                ],
            });
            let hosted = config
                .host()
                .is_ok_and(|host| host == "api.openai.com" || host.ends_with(".openai.azure.com"));
            if hosted {
                body["max_completion_tokens"] = max_tokens.into();
                if model.starts_with("gpt-5.4-") {
                    body["reasoning_effort"] = "none".into();
                }
            } else {
                body["max_tokens"] = max_tokens.into();
            }
            body
        }
    }
}

#[must_use]
pub fn headers(provider: AiProvider, key: Option<&str>) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("content-type", "application/json".to_owned()),
        ("accept", "text/event-stream".to_owned()),
    ];
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    match provider {
        AiProvider::Anthropic => {
            headers.push(("anthropic-version", ANTHROPIC_VERSION.to_owned()));
            if let Some(key) = key {
                headers.push(("x-api-key", key.to_owned()));
            }
        }
        AiProvider::OpenAi => {
            if let Some(key) = key {
                headers.push(("authorization", format!("Bearer {key}")));
            }
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(provider: AiProvider, base_url: &str) -> AiConfig {
        AiConfig {
            provider,
            base_url: base_url.to_owned(),
            model: String::new(),
            instructions: String::new(),
            max_diff_bytes: 40_000,
        }
    }

    #[test]
    fn endpoints_follow_each_providers_base_url_convention() {
        let cases = [
            (
                AiProvider::Anthropic,
                "",
                "https://api.anthropic.com/v1/messages",
            ),
            (
                AiProvider::Anthropic,
                "https://gw.example/v1/",
                "https://gw.example/v1/messages",
            ),
            (
                AiProvider::Anthropic,
                "https://gw.example/v1/messages",
                "https://gw.example/v1/messages",
            ),
            (
                AiProvider::OpenAi,
                "",
                "https://api.openai.com/v1/chat/completions",
            ),
            (
                AiProvider::OpenAi,
                "http://localhost:11434/v1",
                "http://localhost:11434/v1/chat/completions",
            ),
            (
                AiProvider::OpenAi,
                "https://res.openai.azure.com/openai/v1",
                "https://res.openai.azure.com/openai/v1/chat/completions",
            ),
        ];
        for (provider, base, endpoint) in cases {
            assert_eq!(config(provider, base).endpoint().unwrap(), endpoint);
        }
    }

    #[test]
    fn a_base_url_must_be_http() {
        assert!(matches!(
            config(AiProvider::OpenAi, "ftp://x").endpoint(),
            Err(AiError::BaseUrl)
        ));
        assert_eq!(
            config(AiProvider::OpenAi, "http://LOCALHOST:1234/v1")
                .host()
                .unwrap(),
            "localhost"
        );
    }

    #[test]
    fn effort_is_sent_only_to_models_that_take_it() {
        assert!(takes_effort("claude-opus-5-5"));
        assert!(takes_effort("claude-sonnet-4-6"));
        assert!(!takes_effort("claude-haiku-4-5"));
        assert!(!takes_effort("claude-3-7-sonnet-latest"));
        let body = request_body(&config(AiProvider::Anthropic, ""), "s", "u", true);
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["stream"], true);
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn the_openai_body_carries_the_system_prompt_as_a_message() {
        let body = request_body(&config(AiProvider::OpenAi, ""), "sys", "usr", true);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "usr");
        assert_eq!(body["max_completion_tokens"], 1024);
    }

    #[test]
    fn the_key_goes_only_into_the_auth_header() {
        let anthropic = headers(AiProvider::Anthropic, Some("sk-ant"));
        assert!(anthropic.contains(&("x-api-key", "sk-ant".to_owned())));
        assert!(anthropic.contains(&("anthropic-version", ANTHROPIC_VERSION.to_owned())));
        let openai = headers(AiProvider::OpenAi, Some("sk-x"));
        assert!(openai.contains(&("authorization", "Bearer sk-x".to_owned())));
        assert_eq!(headers(AiProvider::OpenAi, None).len(), 2);
    }
}
