//! Asking the model, and reading the answer as it arrives.
//!
//! The reply is streamed, always. That is not a nicety: the route a ChatGPT plan token
//! takes is the streaming one, so a request that asked for the whole answer at once
//! would be a different shape of request with nothing to gain. Streaming is also what
//! makes a slow answer bearable, since the first words arrive in about a second rather
//! than the whole thing arriving in thirty.
//!
//! As in `oauth`, the parts that can be decided without the network are plain functions:
//! cutting the stream into events, and saying what one event means. Those are where the
//! bugs would be, and they are the parts a test can reach.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::error::{ChatError, ChatErrorCode, ChatResult};

const API: &str = "https://api.openai.com/v1";

/// A model can think for a while before it says anything. Long enough to let it, short
/// enough that a connection which has quietly died does not hang the panel forever.
const TURN_TIMEOUT: Duration = Duration::from_secs(180);

/// A reply past this is not an answer to anything, and holding it costs memory that the
/// editor wants.
const MAX_REPLY: usize = 1024 * 1024;

/// One of the models this account may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub slug: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct ListedModel {
    slug: Option<String>,
    display_name: Option<String>,
    /// Models the account has but is not meant to be offered are marked otherwise.
    visibility: Option<String>,
}

/// What the account may use, in the order the service offers them.
///
/// The order is kept rather than sorted, because it is the service's own ranking and is
/// the only thing here that knows which model is the sensible default this month.
pub async fn list_models(token: &str) -> ChatResult<Vec<Model>> {
    let response = super::oauth::http()?
        .get(format!("{API}/models"))
        .bearer_auth(token)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| {
            ChatError::retryable(
                ChatErrorCode::Network,
                "ChatGPT could not be reached. Check your connection.",
            )
        })?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(api_error(&value, status));
    }
    Ok(read_models(&value))
}

/// Pick the listed models out of a catalogue, ignoring anything malformed.
///
/// Lenient on purpose. A catalogue that grows a field, or carries one entry this version
/// cannot read, is not a reason to refuse to talk to any model at all.
pub fn read_models(value: &Value) -> Vec<Model> {
    let Some(listed) = value.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    listed
        .iter()
        .filter_map(|entry| {
            let entry: ListedModel = serde_json::from_value(entry.clone()).ok()?;
            if entry.visibility.as_deref() != Some("list") {
                return None;
            }
            let slug = entry.slug.filter(|s| !s.trim().is_empty())?;
            let display_name = entry.display_name.unwrap_or_else(|| slug.clone());
            Some(Model { slug, display_name })
        })
        .collect()
}

/// What to ask.
pub struct Turn<'a> {
    pub model: &'a str,
    /// The conversation so far, as Responses API input items. A list of items rather
    /// than a list of messages because a turn that called a tool has items in it that
    /// are not messages, and they have to go back in the order they happened.
    pub input: &'a [Value],
    /// Standing guidance, resent every turn. Not a message: it is not part of the
    /// conversation and should not be answered as though somebody said it.
    pub instructions: &'a str,
}

pub fn request_body(turn: &Turn<'_>) -> Value {
    json!({
        "model": turn.model,
        "input": turn.input,
        "instructions": turn.instructions,
        // Nothing is kept on OpenAI's side. The conversation belongs to this window and
        // ends with it, which is also what makes it nobody else's to read.
        "store": false,
        "stream": true,
    })
}

/// What one event in the stream means.
#[derive(Debug, PartialEq)]
pub enum Step {
    /// More of the answer.
    Delta(String),
    /// The whole response object, once the model has finished.
    Completed(Value),
    Failed(ChatError),
    /// Something this version does not act on. Most events are this, and will stay that
    /// way: the stream describes far more than a reader has to care about.
    Ignored,
}

pub fn interpret(event: &Value) -> Step {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "response.output_text.delta" => match event.get("delta").and_then(Value::as_str) {
            Some(text) => Step::Delta(text.to_string()),
            None => Step::Ignored,
        },
        "response.completed" => Step::Completed(
            event
                .get("response")
                .cloned()
                .unwrap_or_else(|| json!({ "output": [] })),
        ),
        "response.failed" => Step::Failed(api_error(
            event.get("response").unwrap_or(event),
            reqwest::StatusCode::OK,
        )),
        "error" => Step::Failed(api_error(event, reqwest::StatusCode::OK)),
        "response.incomplete" => Step::Failed(ChatError::retryable(
            ChatErrorCode::Provider,
            "ChatGPT stopped before finishing. You can try again.",
        )),
        _ => Step::Ignored,
    }
}

/// Cuts a byte stream into server-sent events.
///
/// Bytes rather than text, because a chunk can end in the middle of a character and
/// decoding it on its own would turn that character into a replacement. Events end at a
/// blank line, and a newline never appears inside a multi-byte character, so a complete
/// event is always safe to decode.
#[derive(Default)]
pub struct Events {
    buffer: Vec<u8>,
}

impl Events {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the next chunk, and return whatever events are now complete.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Value> {
        self.buffer.extend_from_slice(chunk);
        // Carriage returns are allowed by the format and are noise to everything below.
        self.buffer.retain(|b| *b != b'\r');

        let mut out = Vec::new();
        while let Some(end) = find(&self.buffer, b"\n\n") {
            let block = self.buffer.drain(..end + 2).collect::<Vec<u8>>();
            if let Some(value) = read_block(&block[..end]) {
                out.push(value);
            }
        }
        out
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One event: its `data:` lines joined, parsed as JSON.
fn read_block(block: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(block).ok()?;
    let mut data = String::new();
    for line in text.split('\n') {
        // Everything else the format allows (`event:`, `id:`, `retry:`, and comments,
        // which begin with a colon) says nothing the payload does not.
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        if !data.is_empty() {
            data.push('\n');
        }
        data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
    }
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    serde_json::from_str(&data).ok()
}

/// Ask, and hand each piece of the answer over as it arrives.
///
/// Returns the finished response object, which carries the output items the caller needs
/// to continue the conversation. The text is reported through `on_delta` rather than
/// returned twice.
pub async fn ask(
    token: &str,
    turn: &Turn<'_>,
    mut on_delta: impl FnMut(&str),
) -> ChatResult<Value> {
    let mut response = super::oauth::http()?
        .post(format!("{API}/responses"))
        .bearer_auth(token)
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .timeout(TURN_TIMEOUT)
        .json(&request_body(turn))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ChatError::retryable(ChatErrorCode::Network, "ChatGPT did not respond in time.")
            } else {
                ChatError::retryable(
                    ChatErrorCode::Network,
                    "ChatGPT could not be reached. Check your connection.",
                )
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        // A refusal arrives as JSON rather than a stream, so it is read whole.
        let value: Value = serde_json::from_slice(&response.bytes().await.unwrap_or_default())
            .unwrap_or(Value::Null);
        return Err(api_error(&value, status));
    }

    let mut events = Events::new();
    let mut length = 0usize;
    loop {
        let chunk = response.chunk().await.map_err(|_| {
            ChatError::retryable(
                ChatErrorCode::Network,
                "The answer was cut off. You can try again.",
            )
        })?;
        let Some(chunk) = chunk else { break };
        for event in events.feed(&chunk) {
            match interpret(&event) {
                Step::Delta(text) => {
                    length += text.len();
                    if length > MAX_REPLY {
                        return Err(ChatError::new(
                            ChatErrorCode::Provider,
                            "The answer was too long to show. Try asking for less at once.",
                        ));
                    }
                    on_delta(&text);
                }
                Step::Completed(response) => return Ok(response),
                Step::Failed(error) => return Err(error),
                Step::Ignored => {}
            }
        }
    }

    // The stream ended without saying it was finished, which is not the same as an
    // answer. Reporting it as one would show a truncated reply as though it were whole.
    Err(ChatError::retryable(
        ChatErrorCode::Network,
        "The answer ended before it was finished. You can try again.",
    ))
}

/// The items the model produced, to be carried back into the next turn.
///
/// All of them, verbatim, including the ones this app does not read. A reasoning model
/// produces items that are not messages and that it expects to see again next turn;
/// dropping them because they are not text makes it think from scratch every time, and
/// with `store: false` there is no copy on the other side to make up for it.
pub fn output_items(response: &Value) -> Vec<Value> {
    response
        .get("output")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The part of the answer meant to be read.
pub fn output_text(response: &Value) -> String {
    let mut text = String::new();
    for item in output_items(response) {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(parts) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) == Some("output_text") {
                if let Some(said) = part.get("text").and_then(Value::as_str) {
                    text.push_str(said);
                }
            }
        }
    }
    text
}

/// Turn a refusal from the API into something worth showing.
fn api_error(value: &Value, status: reqwest::StatusCode) -> ChatError {
    let detail = value.get("error").unwrap_or(value);
    let described = detail
        .get("message")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty());

    // 401 means the token is not good any more, whatever it looked like locally. The
    // answer is to sign in again, not to retry with the same credentials.
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return ChatError::new(
            ChatErrorCode::Expired,
            "ChatGPT no longer accepts this sign-in. Please sign in again.",
        );
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ChatError::retryable(
            ChatErrorCode::Provider,
            described
                .unwrap_or("You have reached your ChatGPT plan's limit for now. Try again later."),
        );
    }
    ChatError {
        code: ChatErrorCode::Provider,
        message: described
            .map(str::to_string)
            .unwrap_or_else(|| format!("ChatGPT could not answer ({status}).")),
        retryable: status.is_server_error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Value {
        json!({ "models": [
            { "slug": "gpt-5.1", "display_name": "GPT-5.1", "visibility": "list" },
            { "slug": "hidden", "display_name": "Hidden", "visibility": "hidden" },
            { "slug": "gpt-5.1-mini", "display_name": "GPT-5.1 mini", "visibility": "list" },
        ]})
    }

    #[test]
    fn only_the_models_meant_to_be_offered_are_offered() {
        let models = read_models(&catalogue());
        assert_eq!(
            models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
            ["gpt-5.1", "gpt-5.1-mini"],
            "the service's own order is the ranking"
        );
    }

    #[test]
    fn one_unreadable_entry_does_not_cost_the_rest() {
        let mut value = catalogue();
        value["models"]
            .as_array_mut()
            .unwrap()
            .push(json!("nonsense"));
        value["models"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "slug": "", "visibility": "list" }));
        assert_eq!(read_models(&value).len(), 2);
    }

    #[test]
    fn a_catalogue_that_makes_no_sense_offers_nothing_rather_than_failing() {
        assert!(read_models(&json!({ "data": [] })).is_empty());
        assert!(read_models(&Value::Null).is_empty());
    }

    #[test]
    fn a_model_with_no_name_is_shown_by_its_slug() {
        let value = json!({ "models": [{ "slug": "gpt-5.1", "visibility": "list" }] });
        assert_eq!(read_models(&value)[0].display_name, "gpt-5.1");
    }

    #[test]
    fn nothing_asked_is_kept_on_the_other_side() {
        let input = vec![json!({ "role": "user", "content": "hello" })];
        let body = request_body(&Turn {
            model: "gpt-5.1",
            input: &input,
            instructions: "be useful",
        });
        assert_eq!(body["store"], json!(false), "the conversation stays here");
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["instructions"], json!("be useful"));
        assert_eq!(body["input"], json!(input));
    }

    #[test]
    fn a_stream_is_cut_at_the_blank_lines() {
        let mut events = Events::new();
        let found = events.feed(
            b"event: response.output_text.delta\ndata: {\"type\":\"a\"}\n\ndata: {\"type\":\"b\"}\n\n",
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0]["type"], json!("a"));
        assert_eq!(found[1]["type"], json!("b"));
    }

    #[test]
    fn an_event_split_across_chunks_is_still_one_event() {
        let mut events = Events::new();
        assert!(
            events.feed(b"data: {\"ty").is_empty(),
            "nothing is complete yet"
        );
        assert!(events.feed(b"pe\":\"a\"}").is_empty());
        let found = events.feed(b"\n\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["type"], json!("a"));
    }

    #[test]
    fn a_character_split_across_chunks_survives() {
        // Decoding each chunk on its own would turn the emoji into a replacement
        // character, and the person would see the damage in their answer.
        let text = "ok 🎬";
        let event = format!(
            "data: {}\n\n",
            json!({ "type": "response.output_text.delta", "delta": text })
        );
        let bytes = event.as_bytes();
        let split = bytes.len() - 6;

        let mut events = Events::new();
        events.feed(&bytes[..split]);
        let found = events.feed(&bytes[split..]);
        assert_eq!(found.len(), 1);
        assert_eq!(interpret(&found[0]), Step::Delta(text.to_string()));
    }

    #[test]
    fn carriage_returns_are_not_part_of_the_answer() {
        let mut events = Events::new();
        let found = events.feed(b"data: {\"type\":\"a\"}\r\n\r\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["type"], json!("a"));
    }

    #[test]
    fn the_bookkeeping_lines_are_not_mistaken_for_answers() {
        let mut events = Events::new();
        let found = events.feed(b": a comment\nid: 1\nretry: 50\n\ndata: [DONE]\n\n");
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_data_payload_written_over_several_lines_is_joined() {
        let mut events = Events::new();
        let found = events.feed(b"data: {\ndata: \"type\": \"a\"\ndata: }\n\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["type"], json!("a"));
    }

    #[test]
    fn the_answer_is_read_out_of_the_stream() {
        assert_eq!(
            interpret(&json!({ "type": "response.output_text.delta", "delta": "hi" })),
            Step::Delta("hi".into())
        );
        let completed = interpret(&json!({
            "type": "response.completed",
            "response": { "output": [{ "type": "message" }] },
        }));
        assert!(matches!(completed, Step::Completed(r) if r["output"].is_array()));
    }

    #[test]
    fn most_of_the_stream_is_not_addressed_to_us() {
        for kind in [
            "response.created",
            "response.in_progress",
            "response.output_item.added",
            "response.reasoning_summary_text.delta",
            "something.invented.later",
        ] {
            assert_eq!(interpret(&json!({ "type": kind })), Step::Ignored, "{kind}");
        }
        assert_eq!(interpret(&json!({})), Step::Ignored);
    }

    #[test]
    fn a_failure_in_the_stream_is_reported_in_the_provider_s_own_words() {
        let step = interpret(&json!({
            "type": "response.failed",
            "response": { "error": { "message": "the model is overloaded" } },
        }));
        match step {
            Step::Failed(error) => assert_eq!(error.message, "the model is overloaded"),
            other => panic!("{other:?}"),
        }
    }

    fn finished() -> Value {
        json!({ "output": [
            { "type": "reasoning", "id": "rs_1", "summary": [] },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "Two clips" },
                { "type": "refusal", "refusal": "no" },
            ]},
        ]})
    }

    #[test]
    fn the_readable_part_of_the_answer_is_pulled_out() {
        assert_eq!(output_text(&finished()), "Two clips");
        assert_eq!(output_text(&json!({})), "");
    }

    #[test]
    fn everything_the_model_produced_goes_back_next_turn() {
        // Including the reasoning item, which is not text and is not shown, but which a
        // reasoning model expects to see again. Nothing is stored on the other side to
        // make up for dropping it.
        let items = output_items(&finished());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], json!("reasoning"));
    }

    #[test]
    fn a_dead_token_asks_for_a_sign_in_rather_than_a_retry() {
        let error = api_error(&Value::Null, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(error.code, ChatErrorCode::Expired);
        assert!(!error.retryable);
    }

    #[test]
    fn reaching_a_plan_limit_is_said_plainly() {
        let error = api_error(
            &json!({ "error": { "message": "You have hit your usage limit." } }),
            reqwest::StatusCode::TOO_MANY_REQUESTS,
        );
        assert_eq!(error.message, "You have hit your usage limit.");
        assert!(error.retryable, "a limit passes with time");
    }
}
