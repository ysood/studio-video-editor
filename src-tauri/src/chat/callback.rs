//! The loopback listener the browser comes back to.
//!
//! A locally installed app has no server to redirect to, so it briefly becomes one: it
//! binds a port on the loopback interface, sends the browser there, and reads the
//! authorization code out of the request. It is a server for one request and a few
//! seconds, which is why it is written out by hand here rather than by pulling in an
//! HTTP stack to handle one GET.
//!
//! The address is `127.0.0.1` and not `localhost`, which the provider requires. The two
//! are not interchangeable: `localhost` resolves through the system resolver, which can
//! be made to point elsewhere, and it may be IPv6 on one machine and IPv4 on the next.
//!
//! Three things are checked on every request, and each exists for a reason:
//!
//! - **The `Host` header**, because a page on the internet can make a browser issue
//!   requests to this port. Requiring the literal loopback address means a name that has
//!   been pointed at 127.0.0.1 does not satisfy it.
//! - **The `state`, in constant time**, because it is the only thing tying this request
//!   to the sign-in that was started here.
//! - **That a wrong `state` does not end the attempt.** Anything else on the machine can
//!   reach a loopback port. If an unrelated request could consume the pending sign-in,
//!   anything that probes local ports could cancel it.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use super::error::{ChatError, ChatErrorCode, ChatResult};
use super::oauth::csp_hash;

/// The path the provider redirects to. Part of the redirect URI, which has to match
/// byte for byte between the authorize request and the token exchange.
const PATH: &str = "/auth/callback";

/// Enough for any real request head. A request that keeps talking past this is not the
/// browser coming back, and reading it unbounded is how a loopback port becomes a way to
/// exhaust memory.
const MAX_HEAD: usize = 8 * 1024;

/// How long one connection may take to send its head.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the deadline is checked while waiting. The wait is on a person using a
/// browser, so this costs nothing and keeps the wait interruptible.
const POLL: Duration = Duration::from_millis(100);

/// Scrub the authorization code out of the browser's address bar, so it does not stay in
/// history. The code is single use and already spent by then, but there is no reason to
/// leave it lying around.
const SCRIPT: &str = "history.replaceState(null, \"\", \"/auth/complete\");";

/// What the browser brought back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pub code: String,
    /// The registration this sign-in belongs to: the one the provider just issued, or
    /// the one already held.
    pub client_id: String,
}

/// The parts of a request head this cares about.
#[derive(Debug, PartialEq, Eq)]
struct Head<'a> {
    method: &'a str,
    target: &'a str,
    host: Option<&'a str>,
}

/// What to do about one request.
#[derive(Debug)]
enum Verdict {
    /// Not the callback, or not this attempt. Reply with this and keep waiting.
    Ignore(&'static str, &'static str),
    /// This attempt. Show the page, then stop with this.
    Finish(Box<ChatResult<Callback>>),
}

fn parse_head(head: &str) -> Option<Head<'_>> {
    let mut lines = head.split("\r\n");
    let mut parts = lines.next()?.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    // Required so that a line which merely begins like a request is not read as one.
    if !parts.next()?.starts_with("HTTP/") || parts.next().is_some() {
        return None;
    }
    let host = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("host").then(|| value.trim())
    });
    Some(Head {
        method,
        target,
        host,
    })
}

/// Compare two secrets without letting the time taken say where they differ.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b) {
        difference |= x ^ y;
    }
    difference == 0
}

/// Whether a client id looks like one, and is not the placeholder used before
/// registration. Storing the placeholder would mean every later sign-in registered
/// another copy of the app.
fn plausible_client_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id != "dynamic_agent_client"
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn decide(head: &Head<'_>, port: u16, state: &str, saved: Option<&str>) -> Verdict {
    let not_found = Verdict::Ignore("404 Not Found", "Not found");
    let (path, query) = head.target.split_once('?').unwrap_or((head.target, ""));
    if head.method != "GET" || path != PATH || head.host != Some(&format!("127.0.0.1:{port}")) {
        return not_found;
    }

    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let all = |name: &str| -> Vec<&str> {
        pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect()
    };

    // Before anything else: is this ours? A single parameter, matching exactly.
    let returned = all("state");
    if returned.len() != 1 || !same_secret(returned[0], state) {
        return Verdict::Ignore(
            "400 Bad Request",
            "This is not the sign-in that was started here.",
        );
    }

    let failed = |code: ChatErrorCode, message: &str| {
        Verdict::Finish(Box::new(Err(ChatError::new(code, message))))
    };

    if let [reported, ..] = all("error")[..] {
        return match reported {
            "access_denied" => Verdict::Finish(Box::new(Err(ChatError::cancelled()))),
            other => failed(
                ChatErrorCode::Provider,
                &format!("ChatGPT refused the sign-in ({other})."),
            ),
        };
    }

    let codes = all("code");
    let issued = all("client_id");
    // The id comes back on registration. On a later sign-in it may be left out, which
    // means the one already held still stands.
    let client_id = issued.first().copied().or(saved);
    let Some(client_id) = client_id else {
        return failed(
            ChatErrorCode::Registration,
            "ChatGPT did not finish registering Studio. Please try signing in again.",
        );
    };
    if codes.len() != 1
        || codes[0].is_empty()
        || issued.len() > 1
        || !plausible_client_id(client_id)
    {
        return failed(
            ChatErrorCode::Registration,
            "ChatGPT did not finish registering Studio. Please try signing in again.",
        );
    }
    // A different id than the one held means this is not the app we think we are.
    if let (Some(saved), Some(returned)) = (saved, issued.first()) {
        if saved != *returned {
            return failed(
                ChatErrorCode::Registration,
                "ChatGPT returned a different app registration. Sign out and sign in again.",
            );
        }
    }

    Verdict::Finish(Box::new(Ok(Callback {
        code: codes[0].to_string(),
        client_id: client_id.to_string(),
    })))
}

/// The port, bound and waiting, before the browser is opened.
///
/// Bound first on purpose: the authorize request has to carry the redirect URI, and the
/// URI is not known until the port is. Opening the browser first would also leave a
/// window where the provider could redirect to a port nothing is listening on.
pub struct Listener {
    socket: TcpListener,
    port: u16,
}

impl Listener {
    pub fn bind() -> ChatResult<Self> {
        // Port 0: the operating system picks a free one. A fixed port would collide with
        // a second copy of Studio, and the provider allows the port to vary.
        let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let socket = TcpListener::bind(address).map_err(|e| {
            ChatError::new(
                ChatErrorCode::Browser,
                format!("Studio could not open a local port to finish sign-in: {e}"),
            )
        })?;
        let port = socket.local_addr().map_err(ChatError::from)?.port();
        socket.set_nonblocking(true).map_err(ChatError::from)?;
        Ok(Self { socket, port })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{PATH}", self.port)
    }

    /// Wait for the browser to come back. Blocking, so it belongs off the main thread.
    pub fn wait(
        &self,
        state: &str,
        saved: Option<&str>,
        timeout: Duration,
    ) -> ChatResult<Callback> {
        let until = Instant::now() + timeout;
        loop {
            match self.socket.accept() {
                Ok((stream, _)) => {
                    if let Some(result) = self.serve(stream, state, saved) {
                        return result;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(ChatError::from(e)),
            }
            if Instant::now() >= until {
                return Err(ChatError::new(
                    ChatErrorCode::Cancelled,
                    "Sign-in was not completed in time.",
                ));
            }
            std::thread::sleep(POLL);
        }
    }

    /// Answer one request. `None` means it was not this sign-in, so keep waiting.
    fn serve(
        &self,
        mut stream: TcpStream,
        state: &str,
        saved: Option<&str>,
    ) -> Option<ChatResult<Callback>> {
        stream.set_nonblocking(false).ok()?;
        stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;

        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        // One byte at a time, stopping at the end of the head: a GET carries no body, and
        // reading past the head would block until the browser closed the connection.
        while !head.ends_with(b"\r\n\r\n") {
            if head.len() >= MAX_HEAD {
                return None;
            }
            match stream.read(&mut byte) {
                Ok(0) => return None,
                Ok(_) => head.push(byte[0]),
                Err(_) => return None,
            }
        }
        let text = String::from_utf8_lossy(&head).into_owned();
        let parsed = parse_head(&text)?;

        match decide(&parsed, self.port, state, saved) {
            Verdict::Ignore(status, body) => {
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                None
            }
            Verdict::Finish(result) => {
                let body = page();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
                     Content-Security-Policy: default-src 'none'; script-src 'sha256-{}'; \
                     style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len(),
                    csp_hash(SCRIPT)
                );
                Some(*result)
            }
        }
    }
}

/// What the browser tab shows once it has handed the code over.
fn page() -> String {
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
         <title>Return to Studio</title>\
         <style>body{{font:17px -apple-system,system-ui,sans-serif;max-width:32rem;\
         margin:18vh auto;padding:24px;color:#151414;background:#e6e5e9}}\
         h1{{font-size:26px;font-weight:600}}</style>\
         <h1>Return to Studio</h1>\
         <p>Studio is finishing your ChatGPT sign-in. You can close this tab.</p>\
         <script>{SCRIPT}</script></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u16 = 51234;
    const STATE: &str = "the-state";

    fn head(target: &str) -> String {
        format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nAccept: */*\r\n\r\n")
    }

    fn decide_on(target: &str, saved: Option<&str>) -> Verdict {
        let text = head(target);
        let parsed = parse_head(&text).expect("a well formed request");
        decide(&parsed, PORT, STATE, saved)
    }

    fn callback_of(verdict: Verdict) -> ChatResult<Callback> {
        match verdict {
            Verdict::Finish(result) => *result,
            Verdict::Ignore(status, _) => panic!("ignored with {status}"),
        }
    }

    #[test]
    fn a_request_head_is_read() {
        let text = head("/auth/callback?code=c");
        assert_eq!(
            parse_head(&text).unwrap(),
            Head {
                method: "GET",
                target: "/auth/callback?code=c",
                host: Some("127.0.0.1:51234"),
            }
        );
    }

    #[test]
    fn something_that_is_not_a_request_is_not_read_as_one() {
        for text in [
            "GET /auth/callback\r\n\r\n",
            "nonsense\r\n\r\n",
            "GET /auth/callback HTTP/1.1 extra\r\n\r\n",
            "",
        ] {
            assert!(parse_head(text).is_none(), "{text:?} was accepted");
        }
    }

    #[test]
    fn the_code_and_the_issued_registration_are_taken_from_the_callback() {
        let verdict = decide_on(
            &format!("/auth/callback?code=abc&state={STATE}&client_id=oaiapp_1"),
            None,
        );
        assert_eq!(
            callback_of(verdict).unwrap(),
            Callback {
                code: "abc".into(),
                client_id: "oaiapp_1".into(),
            }
        );
    }

    #[test]
    fn a_later_sign_in_may_leave_the_registration_out() {
        let verdict = decide_on(
            &format!("/auth/callback?code=abc&state={STATE}"),
            Some("oaiapp_1"),
        );
        assert_eq!(callback_of(verdict).unwrap().client_id, "oaiapp_1");
    }

    #[test]
    fn a_registration_that_is_not_the_one_held_is_refused() {
        let verdict = decide_on(
            &format!("/auth/callback?code=abc&state={STATE}&client_id=oaiapp_other"),
            Some("oaiapp_1"),
        );
        let error = callback_of(verdict).unwrap_err();
        assert_eq!(error.code, ChatErrorCode::Registration);
    }

    #[test]
    fn the_placeholder_is_never_accepted_as_a_registration() {
        // Storing it would make every later sign-in register another copy of the app.
        let verdict = decide_on(
            &format!("/auth/callback?code=abc&state={STATE}&client_id=dynamic_agent_client"),
            None,
        );
        assert!(callback_of(verdict).is_err());
    }

    #[test]
    fn a_wrong_state_does_not_end_the_attempt() {
        // Anything on the machine can reach a loopback port. If this consumed the
        // sign-in, a port scanner could cancel it.
        let verdict = decide_on("/auth/callback?code=abc&state=guessed", None);
        assert!(matches!(verdict, Verdict::Ignore("400 Bad Request", _)));
    }

    #[test]
    fn a_state_sent_twice_is_not_a_match() {
        let verdict = decide_on(
            &format!("/auth/callback?code=abc&state={STATE}&state=other&client_id=oaiapp_1"),
            None,
        );
        assert!(matches!(verdict, Verdict::Ignore("400 Bad Request", _)));
    }

    #[test]
    fn a_request_for_somewhere_else_is_not_the_callback() {
        for target in ["/", "/auth/complete", "/auth/callback/x"] {
            let verdict = decide_on(&format!("{target}?state={STATE}&code=c"), None);
            assert!(
                matches!(verdict, Verdict::Ignore("404 Not Found", _)),
                "{target}"
            );
        }
    }

    #[test]
    fn a_request_that_arrived_under_another_name_is_refused() {
        // A page on the internet can point a name at 127.0.0.1 and have the browser send
        // requests here. Requiring the literal address is what stops that.
        let text = format!(
            "GET /auth/callback?code=c&state={STATE}&client_id=oaiapp_1 HTTP/1.1\r\n\
             Host: evil.test:{PORT}\r\n\r\n"
        );
        let parsed = parse_head(&text).unwrap();
        let verdict = decide(&parsed, PORT, STATE, None);
        assert!(matches!(verdict, Verdict::Ignore("404 Not Found", _)));

        let text = format!(
            "GET /auth/callback?code=c&state={STATE}&client_id=oaiapp_1 HTTP/1.1\r\n\
             Host: localhost:{PORT}\r\n\r\n"
        );
        let parsed = parse_head(&text).unwrap();
        assert!(matches!(
            decide(&parsed, PORT, STATE, None),
            Verdict::Ignore("404 Not Found", _)
        ));
    }

    #[test]
    fn a_declined_sign_in_is_not_an_error_to_report() {
        let verdict = decide_on(
            &format!("/auth/callback?error=access_denied&state={STATE}"),
            None,
        );
        assert_eq!(
            callback_of(verdict).unwrap_err().code,
            ChatErrorCode::Cancelled
        );
    }

    #[test]
    fn a_refusal_from_the_provider_is_reported() {
        let verdict = decide_on(
            &format!("/auth/callback?error=server_error&state={STATE}"),
            None,
        );
        let error = callback_of(verdict).unwrap_err();
        assert_eq!(error.code, ChatErrorCode::Provider);
        assert!(error.message.contains("server_error"));
    }

    #[test]
    fn secrets_are_compared_without_leaking_where_they_differ() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "ab"));
        assert!(!same_secret("", "a"));
        assert!(same_secret("", ""));
    }

    #[test]
    fn the_listener_describes_where_the_browser_should_come_back_to() {
        let listener = Listener::bind().unwrap();
        let uri = listener.redirect_uri();
        assert!(uri.starts_with("http://127.0.0.1:"), "{uri}");
        assert!(uri.ends_with("/auth/callback"), "{uri}");
        assert!(!uri.contains("localhost"), "the provider refuses localhost");
        assert_ne!(listener.port, 0, "a real port must be bound before use");
    }

    #[test]
    fn a_second_sign_in_does_not_collide_with_the_first() {
        let (a, b) = (Listener::bind().unwrap(), Listener::bind().unwrap());
        assert_ne!(a.port, b.port);
    }
}
