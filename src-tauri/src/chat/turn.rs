//! One turn: the question, whatever the model reaches for, and the answer.
//!
//! A turn is not one request. The model answers, and its answer may be "call this
//! tool"; the tool runs, its result goes back, and it answers again. That repeats until
//! it has nothing left to ask for. So the conversation is a list of items rather than a
//! list of messages: a call and its result are part of the record, in the order they
//! happened, and the next turn is only coherent if they are all still there.
//!
//! Two things shape the rest of this file.
//!
//! **Our names do not survive the crossing.** The catalogue names tools `area.verb`,
//! which is what makes a list of them readable. The provider only accepts letters,
//! digits, underscores and dashes, so the dot has to go. The mapping is built from the
//! catalogue and checked both ways, rather than done with string surgery at each end,
//! because a name that does not map back is a tool call that cannot be run.
//!
//! **A turn can stop in the middle.** A tool that can lose work is not run on the
//! model's say-so. The turn stops, the panel asks, and the answer resumes it. That is
//! why the queue of outstanding calls is kept here rather than on a stack: there is no
//! stack, because the turn has gone back to the window and may not come back at all.

use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use studio_agent::{Policy, Surface, Tool};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::client;
use super::error::{ChatError, ChatErrorCode, ChatResult};
use crate::state::AppState;

/// How many times round the loop before giving up.
///
/// A model that has called tools this many times without arriving at an answer is not
/// about to. The cost of being wrong here is an answer that needed a ninth call; the
/// cost of having no limit is a conversation that spends somebody's plan in a circle.
const MAX_STEPS: usize = 8;

pub const EVENT_DELTA: &str = "chat://delta";
pub const EVENT_TOOL: &str = "chat://tool";

/// Emitted as `chat://delta` while an answer is being written.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatDelta {
    pub text: String,
}

/// Emitted as `chat://tool` when the assistant has done something.
///
/// Sent as it happens rather than listed at the end, because what the assistant did to
/// the document is the part worth watching, and by the time the answer arrives the
/// timeline has already moved.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatToolRun {
    /// The catalogue name, such as `timeline.move_clip`.
    pub tool: String,
    /// What it is, in words.
    pub title: String,
    pub ok: bool,
    /// Why not, when it did not work.
    pub detail: Option<String>,
}

/// How a turn ended.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum TurnEnd {
    /// The model has said what it has to say.
    Answered { text: String },
    /// It wants to do something that can lose work, and is waiting to be told whether
    /// to. Nothing has been done yet.
    NeedsApproval {
        tool: String,
        title: String,
        /// What the tool does, from the catalogue, so the question is answerable by
        /// somebody who has never heard of the tool.
        description: String,
        /// Exactly what it would be called with.
        #[ts(type = "unknown")]
        arguments: Value,
    },
}

/// A tool call the model asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub call_id: String,
    /// As the provider sent it, which is not the catalogue's spelling.
    pub name: String,
    pub arguments: Value,
}

/// The conversation, for as long as the app is running.
///
/// Deliberately not written to disk: nothing here asked to be kept, and a transcript of
/// somebody's work sitting in a file is a thing to be read by whoever finds it.
static CONVERSATION: Mutex<Vec<Value>> = Mutex::new(Vec::new());

/// Calls still to be run, the first of which is waiting to be approved.
///
/// The whole queue, not just the one call: the provider requires every call it made to
/// come back with a result before the conversation can go on, so the ones behind an
/// unapproved call cannot simply be forgotten.
static PENDING: Mutex<Option<Waiting>> = Mutex::new(None);

#[derive(Debug, Clone)]
struct Waiting {
    queue: Vec<Call>,
    /// The document as it was when the model decided to make these calls.
    revision: Option<u64>,
    /// What the model has said so far this turn.
    ///
    /// Carried through the pause because a turn can speak before it acts ("let me look
    /// at the timeline"), and that half of the answer would otherwise be thrown away
    /// while somebody decided whether to approve the other half.
    said: String,
}

fn conversation() -> std::sync::MutexGuard<'static, Vec<Value>> {
    CONVERSATION.lock().unwrap_or_else(|e| e.into_inner())
}

fn pending() -> std::sync::MutexGuard<'static, Option<Waiting>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn forget() {
    conversation().clear();
    *pending() = None;
}

/// Whether a turn is sitting waiting to be told yes or no.
pub fn is_waiting() -> bool {
    pending().is_some()
}

// ---------------------------------------------------------------------------
// Names, and what the provider is offered
// ---------------------------------------------------------------------------

/// The catalogue name as the provider will accept it.
///
/// Only the dot is a problem: every name here is otherwise lower-case letters and
/// underscores, which is already inside what the provider allows.
pub fn wire_name(name: &str) -> String {
    name.replace('.', "_")
}

/// Back from a provider name to the catalogue's, by looking it up rather than guessing.
///
/// Reversing the substitution by hand would be wrong: an underscore in the result could
/// have come from either side of the dot, and there is no way to tell which from the
/// string alone.
pub fn catalogue_name(offered: &[Tool], wire: &str) -> Option<String> {
    offered
        .iter()
        .find(|tool| wire_name(&tool.name) == wire)
        .map(|tool| tool.name.clone())
}

/// The tools, in the shape the Responses API takes them.
pub fn tools_payload(offered: &[Tool]) -> Vec<Value> {
    offered
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": wire_name(&tool.name),
                "description": tool.description,
                "parameters": tool.input_schema,
                // Said rather than left out. Strict mode would require every property to
                // be required and every object closed, and these schemas have genuinely
                // optional fields; omitting the flag lets the provider decide, and what
                // it decides is not something to find out from a failed call.
                "strict": false,
            })
        })
        .collect()
}

/// The tools the assistant may use.
fn offered() -> Vec<Tool> {
    studio_agent::surface(&Policy::new(Surface::InApp))
}

/// The calls in a finished response.
pub fn calls_in(response: &Value) -> Vec<Call> {
    client::output_items(response)
        .into_iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter_map(|item| {
            Some(Call {
                call_id: item.get("call_id").and_then(Value::as_str)?.to_string(),
                name: item.get("name").and_then(Value::as_str)?.to_string(),
                // Arguments arrive as a JSON string. One that will not parse is still a
                // call that has to be answered, so it becomes null here and is refused
                // below with a reason the model can act on.
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(Value::Null),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Running the turn
// ---------------------------------------------------------------------------

/// Say something and let it run until it needs an answer or has one.
pub async fn send(app: &AppHandle, token: &str, model: &str, message: &str) -> ChatResult<TurnEnd> {
    conversation().push(json!({ "role": "user", "content": message }));
    match drive(app, token, model, String::new()).await {
        Ok(end) => Ok(end),
        Err(error) => {
            // The question comes back out. One nobody answered would otherwise be asked
            // again, silently, alongside the next one.
            rewind_to_question();
            Err(error)
        }
    }
}

/// Answer the question the turn stopped on, and carry on.
pub async fn resume(
    app: &AppHandle,
    token: &str,
    model: &str,
    approved: bool,
) -> ChatResult<TurnEnd> {
    let waiting = pending().take().ok_or_else(|| {
        ChatError::new(
            ChatErrorCode::NotWaiting,
            "There is nothing waiting to be approved.",
        )
    })?;
    let mut queue = waiting.queue.clone();
    let head = queue.remove(0);

    let revision = if approved {
        let (output, revision) = perform(app, &head, waiting.revision);
        answer(&head, output);
        revision
    } else {
        answer(
            &head,
            json!({
                "refused": true,
                "reason": "The person was asked and did not approve this. Do not try it \
                           again; say what you would have done and stop.",
            }),
        );
        waiting.revision
    };

    if let Some(mut rest) = work_through(app, queue, revision) {
        rest.said = waiting.said;
        return Ok(waiting_on(rest));
    }
    drive(app, token, model, waiting.said).await
}

/// Ask, run what it asks for, ask again, until there is an answer.
///
/// `said` is what the model has already said this turn, which is not always nothing: a
/// turn that spoke, called a tool and is now being resumed brings its first half back
/// with it.
async fn drive(app: &AppHandle, token: &str, model: &str, said: String) -> ChatResult<TurnEnd> {
    let offered = offered();
    let tools = tools_payload(&offered);
    let mut said = said;

    for _ in 0..MAX_STEPS {
        let effort = super::settings::load(app).effort;
        let input = conversation().clone();
        let instructions = instructions(app);
        let before = revision(app);

        let response = client::ask(
            token,
            &client::Turn {
                model,
                input: &input,
                instructions: &instructions,
                tools: &tools,
                // Read each time round rather than once: somebody who changes it
                // mid-conversation means the next thing it does, not the one after.
                effort: effort.as_deref(),
            },
            |text| {
                let _ = app.emit(
                    EVENT_DELTA,
                    ChatDelta {
                        text: text.to_string(),
                    },
                );
            },
        )
        .await?;

        conversation().extend(client::output_items(&response));

        // Every step's words are kept, not only the last step's. A model often says
        // what it is about to do before doing it, and that sentence is the one that
        // explains the row of tool calls underneath it.
        let text = client::output_text(&response);
        if !text.is_empty() {
            if !said.is_empty() {
                said.push_str("\n\n");
            }
            said.push_str(&text);
        }

        let calls = calls_in(&response);
        if calls.is_empty() {
            return Ok(TurnEnd::Answered { text: said });
        }
        if let Some(mut rest) = work_through(app, calls, before) {
            rest.said = said;
            return Ok(waiting_on(rest));
        }
    }

    rewind_to_question();
    Err(ChatError::new(
        ChatErrorCode::Provider,
        "The assistant kept working without reaching an answer, so Studio stopped it. \
         Try asking for one thing at a time.",
    ))
}

/// Run calls in order until one needs approving. Returns what is left when so.
fn work_through(app: &AppHandle, queue: Vec<Call>, revision: Option<u64>) -> Option<Waiting> {
    let offered = offered();
    let mut revision = revision;
    let mut queue = queue;

    while !queue.is_empty() {
        let call = queue[0].clone();
        let known = catalogue_name(&offered, &call.name)
            .and_then(|name| studio_agent::find(&name).map(|tool| (name, tool)));

        let Some((name, tool)) = known else {
            // A name that is not on offer is answered rather than ignored: the provider
            // will not take the next turn until every call has a result, and the model
            // can pick a real tool once it is told this one is not there.
            queue.remove(0);
            answer(
                &call,
                json!({
                    "error": "no such tool",
                    "detail": format!("{} is not available.", call.name),
                }),
            );
            continue;
        };

        if tool.annotations.destructive {
            return Some(Waiting {
                queue,
                revision,
                // Filled in by the caller, which is the one that knows what has been
                // said: this function only runs things.
                said: String::new(),
            });
        }

        queue.remove(0);
        let (output, after) = perform(app, &call, revision);
        revision = after;
        let _ = app.emit(
            EVENT_TOOL,
            ChatToolRun {
                tool: name,
                title: tool.title.clone(),
                ok: output.get("error").is_none(),
                detail: output
                    .get("remedy")
                    .or_else(|| output.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
        );
        answer(&call, output);
    }
    None
}

/// Actually run one call, and say where the document is afterwards.
fn perform(app: &AppHandle, call: &Call, revision: Option<u64>) -> (Value, Option<u64>) {
    let offered = offered();
    let Some(name) = catalogue_name(&offered, &call.name) else {
        return (
            json!({ "error": "no such tool", "detail": call.name }),
            revision,
        );
    };
    let state = app.state::<AppState>();
    match crate::agent::run(&name, call.arguments.clone(), revision, app, &state) {
        Ok(reply) => {
            let after = reply
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.history.revision)
                .or(revision);
            (reply.result, after)
        }
        // A refusal is not a failure of the turn. It is information, and the remedy on
        // it is written for whoever reads it: the model can act on "name the sequence"
        // in a way it cannot act on an exception.
        Err(error) => (
            serde_json::to_value(&error).unwrap_or_else(|_| json!({ "error": "failed" })),
            revision,
        ),
    }
}

/// Put a call's result back into the conversation.
///
/// The output goes in as a string because that is what the provider takes. It is JSON
/// inside that string, which the model reads perfectly well, and it keeps the shape the
/// tool actually returned rather than flattening it into prose.
fn answer(call: &Call, output: Value) {
    conversation().push(json!({
        "type": "function_call_output",
        "call_id": call.call_id,
        "output": serde_json::to_string(&output).unwrap_or_else(|_| "{}".into()),
    }));
}

fn waiting_on(waiting: Waiting) -> TurnEnd {
    let call = waiting.queue[0].clone();
    let described = catalogue_name(&offered(), &call.name)
        .and_then(|name| studio_agent::find(&name))
        .map(|tool| (tool.name, tool.title, tool.description))
        .unwrap_or_else(|| (call.name.clone(), call.name.clone(), String::new()));
    let arguments = call.arguments.clone();
    *pending() = Some(waiting);
    TurnEnd::NeedsApproval {
        tool: described.0,
        title: described.1,
        description: described.2,
        arguments,
    }
}

/// Drop everything back to before the last question was asked.
fn rewind_to_question() {
    *pending() = None;
    let mut conversation = conversation();
    while let Some(last) = conversation.pop() {
        if last.get("role").and_then(Value::as_str) == Some("user") {
            break;
        }
    }
}

fn revision(app: &AppHandle) -> Option<u64> {
    let state = app.state::<AppState>();
    let mut slot = state.session();
    slot.as_mut().map(|session| session.editor.revision())
}

/// Standing guidance, sent with every turn rather than put in the conversation.
///
/// Not a message, because it is not something anybody said and should not be answered as
/// though it were. Resent each turn because nothing is kept on the provider's side,
/// which is the same reason the conversation itself is carried in full.
const INSTRUCTIONS: &str = "\
You are the assistant inside Studio, a desktop video editor. The person you are talking \
to has the project open in front of them and can see the timeline change as you work.

Be brief and concrete. Answer in plain sentences, not lists, unless a list is genuinely \
the shape of the answer. Use timecodes the way an editor does.

You have tools for reading the project and for changing it. Prefer looking to guessing: \
the summary below says how many clips and tracks a sequence has, not what is on them, \
so use the query tools before you act on anything specific. Never state a clip, time or \
name you have not read.

Make the edit that was asked for and stop. Do not tidy up, rename things or add tracks \
that nobody asked about. If a request is ambiguous in a way that changes what you would \
do, ask rather than choosing.

Some tools can lose work, and the person is asked before those run. If one is declined, \
do not look for another way round it: say what you would have done and stop.

When a tool refuses, read the remedy it gives you and act on it. A refusal usually means \
you named something imprecisely, not that the thing cannot be done.";

/// The guidance plus what is open right now.
///
/// Rebuilt every turn rather than once per conversation, so an answer is never given
/// against a project that has since been closed, saved as something else, or edited by
/// hand while the conversation sat idle.
fn instructions(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let mut slot = state.session();
    let Some(session) = slot.as_mut() else {
        return format!("{INSTRUCTIONS}\n\nNo project is open, so nothing can be read or changed.");
    };
    match studio_agent::queries::overview(&session.editor) {
        Ok(overview) => format!(
            "{INSTRUCTIONS}\n\nThe open project:\n{}",
            serde_json::to_string_pretty(&overview).unwrap_or_default()
        ),
        Err(_) => format!("{INSTRUCTIONS}\n\nNo project is open."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_name_crosses_and_comes_back() {
        let offered = studio_agent::catalogue();
        assert!(!offered.is_empty());
        for tool in &offered {
            let wire = wire_name(&tool.name);
            assert!(
                wire.len() <= 64
                    && !wire.is_empty()
                    && wire
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
                "{} becomes {wire}, which the provider will not accept",
                tool.name
            );
            assert_eq!(
                catalogue_name(&offered, &wire).as_deref(),
                Some(tool.name.as_str())
            );
        }
    }

    #[test]
    fn no_two_tools_arrive_under_the_same_name() {
        // If two did, one of them could never be run: the name coming back would resolve
        // to whichever was found first, and the other's calls would go to the wrong tool.
        let offered = studio_agent::catalogue();
        let mut seen: Vec<String> = offered.iter().map(|t| wire_name(&t.name)).collect();
        let total = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), total);
    }

    #[test]
    fn a_name_that_is_not_offered_resolves_to_nothing() {
        let offered = studio_agent::catalogue();
        assert_eq!(catalogue_name(&offered, "timeline_invent_clip"), None);
        assert_eq!(
            catalogue_name(&offered, "query.overview"),
            None,
            "our own spelling"
        );
    }

    #[test]
    fn what_is_offered_is_curated_rather_than_everything() {
        // The whole catalogue in every request is how a model gets worse rather than
        // better: each entry costs it attention on every turn.
        let all = studio_agent::catalogue();
        let offered = offered();
        assert!(!offered.is_empty());
        assert!(
            offered.len() < all.len(),
            "{} of {}",
            offered.len(),
            all.len()
        );
    }

    #[test]
    fn the_tools_are_described_in_the_shape_the_provider_takes() {
        let offered = offered();
        let payload = tools_payload(&offered);
        assert_eq!(payload.len(), offered.len());
        for tool in &payload {
            assert_eq!(tool["type"], json!("function"));
            assert_eq!(
                tool["strict"],
                json!(false),
                "said, never left to a default"
            );
            assert!(tool["parameters"].is_object());
            assert!(!tool["description"].as_str().unwrap_or("").is_empty());
        }
    }

    fn response_with(items: Value) -> Value {
        json!({ "output": items })
    }

    #[test]
    fn the_calls_are_read_out_of_the_answer() {
        let response = response_with(json!([
            { "type": "reasoning", "id": "rs_1" },
            { "type": "function_call", "call_id": "c1", "name": "query_overview",
              "arguments": "{}" },
            { "type": "function_call", "call_id": "c2", "name": "timeline_move_clip",
              "arguments": "{\"clip\":\"V1#1\",\"to\":48}" },
        ]));
        let calls = calls_in(&response);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].call_id, "c1");
        assert_eq!(calls[1].arguments["to"], json!(48));
    }

    #[test]
    fn an_answer_with_no_calls_in_it_has_none() {
        assert!(calls_in(&response_with(json!([{ "type": "message" }]))).is_empty());
        assert!(calls_in(&json!({})).is_empty());
    }

    #[test]
    fn arguments_that_will_not_parse_still_leave_a_call_to_answer() {
        // Dropping it would hang the conversation: the provider will not take another
        // turn until every call it made has a result.
        let response = response_with(json!([
            { "type": "function_call", "call_id": "c1", "name": "query_overview",
              "arguments": "{not json" },
        ]));
        let calls = calls_in(&response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments, Value::Null);
    }

    #[test]
    fn a_call_with_no_identity_is_not_a_call() {
        let response = response_with(json!([
            { "type": "function_call", "name": "query_overview", "arguments": "{}" },
        ]));
        assert!(calls_in(&response).is_empty(), "nothing to answer it with");
    }

    #[test]
    fn the_tools_that_can_lose_work_are_the_ones_worth_asking_about() {
        // The approval prompt hangs off this hint, so it is worth pinning that the hint
        // actually marks what it should.
        let destructive: Vec<String> = offered()
            .into_iter()
            .filter(|t| t.annotations.destructive)
            .map(|t| t.name)
            .collect();
        let marked = |name: &str| destructive.iter().any(|n| n == name);
        // Deleting takes clips away, and placing one clears the room it needs, which
        // takes away whatever was there.
        for name in [
            "timeline.delete_clips",
            "timeline.insert_media",
            "timeline.move_clip",
        ] {
            assert!(marked(name), "{name} would run unasked: {destructive:?}");
        }
        // These change a clip to a value that can be read back first, so nothing is
        // lost. If every setter asked, the question would stop meaning anything.
        for name in ["query.overview", "timeline.trim_clip", "clip.set_transform"] {
            assert!(!marked(name), "{name} would ask needlessly");
        }
    }
}
