use crate::AiProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseReader {
    event: String,
    data: Vec<String>,
}

impl SseReader {
    pub fn line(&mut self, line: &str) -> Option<SseEvent> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return self.finish();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => value.clone_into(&mut self.event),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }

    pub fn finish(&mut self) -> Option<SseEvent> {
        if self.data.is_empty() {
            self.event.clear();
            return None;
        }
        let event = SseEvent {
            event: std::mem::take(&mut self.event),
            data: std::mem::take(&mut self.data).join("\n"),
        };
        Some(event)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Stop(String),
    Done,

    Failed(String),
    Nothing,
}

#[must_use]
pub fn piece(provider: AiProvider, event: &SseEvent) -> Piece {
    match provider {
        AiProvider::Anthropic => anthropic(event),
        AiProvider::OpenAi => openai(event),
    }
}

fn anthropic(event: &SseEvent) -> Piece {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) else {
        return if event.event == "error" {
            Piece::Failed(event.data.clone())
        } else {
            Piece::Nothing
        };
    };
    let kind = json["type"].as_str().unwrap_or(event.event.as_str());
    match kind {
        "content_block_delta" if json["delta"]["type"] == "text_delta" => json["delta"]["text"]
            .as_str()
            .map_or(Piece::Nothing, |text| Piece::Text(text.to_owned())),
        "message_delta" => json["delta"]["stop_reason"]
            .as_str()
            .map_or(Piece::Nothing, |reason| Piece::Stop(reason.to_owned())),
        "message_stop" => Piece::Done,
        "error" => Piece::Failed(event.data.clone()),
        _ => Piece::Nothing,
    }
}

fn openai(event: &SseEvent) -> Piece {
    let data = event.data.trim();
    if data == "[DONE]" {
        return Piece::Done;
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
        return Piece::Nothing;
    };
    if json.get("error").is_some_and(|error| !error.is_null()) || event.event == "error" {
        return Piece::Failed(data.to_owned());
    }
    let choice = &json["choices"][0];
    if let Some(text) = choice["delta"]["content"]
        .as_str()
        .filter(|text| !text.is_empty())
    {
        return Piece::Text(text.to_owned());
    }
    choice["finish_reason"]
        .as_str()
        .map_or(Piece::Nothing, |reason| Piece::Stop(reason.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Vec<SseEvent> {
        let mut reader = SseReader::default();
        let mut found: Vec<SseEvent> = text
            .split_inclusive('\n')
            .filter_map(|line| reader.line(line))
            .collect();
        found.extend(reader.finish());
        found
    }

    #[test]
    fn events_end_at_a_blank_line_and_join_their_data_lines() {
        let found = events(": comment\r\nevent: a\r\ndata: one\r\ndata: two\r\n\r\ndata:three\n\n");
        assert_eq!(
            found,
            [
                SseEvent {
                    event: "a".into(),
                    data: "one\ntwo".into()
                },
                SseEvent {
                    event: String::new(),
                    data: "three".into()
                },
            ]
        );
    }

    #[test]
    fn a_last_event_without_a_blank_line_still_counts() {
        assert_eq!(events("data: [DONE]").len(), 1);
    }
}
