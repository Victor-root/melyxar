//! The one port the server answers on, in the clear and encrypted alike.
//!
//! The first byte a client sends says which it is: a TLS handshake always
//! opens with 22, and a request in the clear with a letter. So a proxy that
//! speaks in the clear keeps working whatever the administrator chooses, and
//! a browser that asks for `https://` is answered when the server has a
//! certificate. What encrypts is asked of the app at every connection, so a
//! new choice applies to the next one without a restart.
//!
//! The handshake happens away from the loop that accepts, so a client that
//! opens a connection and says nothing holds up nobody but itself. The loop
//! never waits on anything but the next connection.
//!
//! Each connection is then served here too, with a timer axum's own way of
//! serving does not set: a request has to arrive whole within a time, and a
//! connection idle between two requests is let go after the same time. While
//! an answer is being sent, a film or a live line, no timer runs, however
//! long it lasts. And one party far from this server holds only so many
//! connections at once, so that somebody opening them by the hundred and
//! saying nothing cannot take every place from everybody else.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::ConnectInfo;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use melyxar_app::AppState;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;

/// The first byte of every TLS handshake.
const HANDSHAKE: u8 = 22;

/// How long a client may take to say its first byte, and then to finish its
/// handshake, before the connection is let go.
const PATIENCE: Duration = Duration::from_secs(10);

/// How long a client may take to send the whole head of a request, counted
/// from the moment the connection is ready for one, so also how long a
/// connection may sit idle between two requests.
///
/// The library's own default, which nothing set: a browser sends a head in
/// milliseconds, and one that reopens a connection it kept idle longer than
/// this does so on its own, without anybody seeing it. Somebody sending a
/// head a byte a minute to keep a connection is let go.
const TO_ASK: Duration = Duration::from_secs(30);

/// How many connections, done with their handshake, may wait to be served.
const WAITING: usize = 128;

/// How many connections one party far from this server may hold at once.
///
/// Far more than a household needs, a browser opening six to a server at
/// most. Past it, the next one is let go at once. A party nearby is never
/// counted: a proxy in front of the server is nearby, and every visitor of
/// the server reaches it through the proxy's own address.
const MOST_FROM_ONE_PARTY: usize = 256;

/// Who is at the other end of a connection, and whether it is encrypted.
#[derive(Debug, Clone, Copy)]
pub struct Peer {
    pub address: SocketAddr,
    pub encrypted: bool,
}

/// A connection, in the clear or encrypted, holding its place for as long as
/// it lives.
struct Io {
    stream: Stream,
    _place: Place,
}

enum Stream {
    Plain(TcpStream),
    Encrypted(Box<TlsStream<TcpStream>>),
}

/// How many connections each party far from this server holds.
#[derive(Clone, Default)]
struct Held(Arc<Mutex<HashMap<IpAddr, usize>>>);

/// One connection counted against its party, given back when it ends.
struct Place {
    counted: Option<(Held, IpAddr)>,
}

impl Held {
    /// A place for one more connection from this address, or nothing when
    /// its party already holds as many as it may.
    fn place_for(&self, address: IpAddr) -> Option<Place> {
        if crate::address::nearby(address) {
            return Some(Place { counted: None });
        }
        let party = melyxar_core::network::party_of(address);
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = held.entry(party).or_insert(0);
        if *count >= MOST_FROM_ONE_PARTY {
            return None;
        }
        *count += 1;
        drop(held);
        Some(Place {
            counted: Some((self.clone(), party)),
        })
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let Some((held, party)) = self.counted.take() else {
            return;
        };
        let mut held = held.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = held.get_mut(&party) {
            *count -= 1;
            if *count == 0 {
                held.remove(&party);
            }
        }
    }
}

/// A request that came in the clear is sent to the encrypted address, once
/// the server encrypts, is not behind a proxy and the administrator wants it,
/// unless it comes from this machine or the local network, where nobody is
/// listening in. Kept for the same host and port, since it is the same door.
/// And what is answered encrypted tells the browser to stay so, when the
/// certificate is one every browser trusts.
pub async fn sent_to_encrypted(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::extract::FromRequestParts;
    use axum::response::IntoResponse;

    let (mut parts, body) = request.into_parts();
    let Ok(caller) = crate::address::Caller::from_request_parts(&mut parts, &state).await;
    if !caller.encrypted && !caller.local && melyxar_app::access::sends_plain_to_encrypted(&state) {
        let host = parts.headers.get(axum::http::header::HOST).and_then(|host| host.to_str().ok());
        let path = parts.uri.path_and_query().map_or("/", |path| path.as_str());
        if let Some(to) = host.and_then(|host| encrypted_address(host, path)) {
            return axum::response::Redirect::permanent(&to).into_response();
        }
    }
    let mut response = next.run(axum::extract::Request::from_parts(parts, body)).await;
    if caller.encrypted && melyxar_app::access::keeps_browsers_encrypted(&state) {
        response.headers_mut().insert(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static(STAY_ENCRYPTED),
        );
    }
    response
}

/// For a year, and for this name alone: whatever else answers under the
/// same domain is left to decide for itself.
const STAY_ENCRYPTED: &str = "max-age=31536000";

/// The same address, encrypted, when the host is one a browser could have
/// sent.
fn encrypted_address(host: &str, path: &str) -> Option<String> {
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-.:[]".contains(character));
    valid.then(|| format!("https://{host}{path}"))
}

/// Whether a connection that opens with this byte is a TLS handshake.
fn is_a_handshake(first: u8) -> bool {
    first == HANDSHAKE
}

pub struct Door {
    arrived: mpsc::Receiver<(Io, Peer)>,
    accepting: tokio::task::JoinHandle<()>,
}

impl Door {
    pub fn new(listener: TcpListener, state: AppState) -> io::Result<Self> {
        let (send, arrived) = mpsc::channel(WAITING);
        let held = Held::default();
        let accepting = tokio::spawn(async move {
            loop {
                let (stream, address) = match listener.accept().await {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        // Out of file descriptors, most often: waiting a moment
                        // is all there is to do.
                        tracing::warn!(%error, "a connection could not be accepted");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                // Let go before anything is read from it: its party already
                // holds as many as it may.
                let Some(place) = held.place_for(address.ip()) else {
                    continue;
                };
                let (send, state) = (send.clone(), state.clone());
                tokio::spawn(async move {
                    if let Some((stream, peer)) = opened(stream, address, &state).await {
                        let _ = send.send((Io { stream, _place: place }, peer)).await;
                    }
                });
            }
        });
        Ok(Self { arrived, accepting })
    }

    /// Serves every connection that comes through until `until` is over,
    /// then lets each finish what it was answering.
    pub async fn serve(self, app: axum::Router, until: impl Future<Output = ()> + Send + 'static) {
        self.serve_with(app, until, TO_ASK).await;
    }

    async fn serve_with(
        mut self,
        app: axum::Router,
        until: impl Future<Output = ()> + Send + 'static,
        to_ask: Duration,
    ) {
        let (stop, stopping) = watch::channel(false);
        tokio::spawn(async move {
            until.await;
            let _ = stop.send(true);
        });
        // Every connection holds a copy for as long as it is served, so the
        // last one gone says the server has finished answering.
        let (finished, serving) = watch::channel(());

        loop {
            let (io, peer) = tokio::select! {
                arrived = self.arrived.recv() => match arrived {
                    Some(arrived) => arrived,
                    None => break,
                },
                () = stopped(stopping.clone()) => break,
            };
            let service = app
                .clone()
                .map_request(move |mut request: axum::http::Request<Incoming>| {
                    request.extensions_mut().insert(ConnectInfo(peer));
                    request.map(axum::body::Body::new)
                });
            let (stopping, serving) = (stopping.clone(), serving.clone());
            tokio::spawn(async move {
                let mut connection = std::pin::pin!(hyper::server::conn::http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(to_ask)
                    .serve_connection(TokioIo::new(io), TowerToHyperService::new(service)));
                tokio::select! {
                    _ = connection.as_mut() => {}
                    () = stopped(stopping) => {
                        connection.as_mut().graceful_shutdown();
                        let _ = connection.await;
                    }
                }
                drop(serving);
            });
        }

        drop(serving);
        self.accepting.abort();
        finished.closed().await;
    }
}

/// Once the server is told to stop.
async fn stopped(mut stopping: watch::Receiver<bool>) {
    let _ = stopping.wait_for(|stop| *stop).await;
}

impl Drop for Door {
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

/// A connection made ready to be served, or nothing when it is let go.
async fn opened(stream: TcpStream, address: SocketAddr, state: &AppState) -> Option<(Stream, Peer)> {
    let _ = stream.set_nodelay(true);
    let mut first = [0u8; 1];
    let peeked = tokio::time::timeout(PATIENCE, stream.peek(&mut first)).await;
    if !matches!(peeked, Ok(Ok(1))) {
        return None;
    }
    if !is_a_handshake(first[0]) {
        return Some((Stream::Plain(stream), Peer { address, encrypted: false }));
    }
    // A handshake the server has no certificate for is let go: there is
    // nothing it could say that the client would understand.
    let encryption = melyxar_app::access::encryption(state)?;
    match tokio::time::timeout(PATIENCE, TlsAcceptor::from(encryption).accept(stream)).await {
        Ok(Ok(encrypted)) => Some((Stream::Encrypted(Box::new(encrypted)), Peer { address, encrypted: true })),
        Ok(Err(error)) => {
            // Every browser meeting a certificate it does not trust ends here
            // once before its visitor clicks through, so this is no trouble.
            tracing::debug!(%error, %address, "a handshake did not finish");
            None
        }
        Err(_) => None,
    }
}

impl AsyncRead for Io {
    fn poll_read(self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().stream {
            Stream::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Stream::Encrypted(stream) => Pin::new(stream.as_mut()).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for Io {
    fn poll_write(self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &[u8]) -> Poll<io::Result<usize>> {
        match &mut self.get_mut().stream {
            Stream::Plain(stream) => Pin::new(stream).poll_write(context, buffer),
            Stream::Encrypted(stream) => Pin::new(stream.as_mut()).poll_write(context, buffer),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match &mut self.get_mut().stream {
            Stream::Plain(stream) => Pin::new(stream).poll_write_vectored(context, buffers),
            Stream::Encrypted(stream) => Pin::new(stream.as_mut()).poll_write_vectored(context, buffers),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match &self.stream {
            Stream::Plain(stream) => stream.is_write_vectored(),
            Stream::Encrypted(stream) => stream.is_write_vectored(),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().stream {
            Stream::Plain(stream) => Pin::new(stream).poll_flush(context),
            Stream::Encrypted(stream) => Pin::new(stream.as_mut()).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().stream {
            Stream::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Stream::Encrypted(stream) => Pin::new(stream.as_mut()).poll_shutdown(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_in_the_clear_is_sent_to_the_same_door_encrypted() {
        assert_eq!(
            encrypted_address("192.168.1.10:2100", "/library?tab=albums").as_deref(),
            Some("https://192.168.1.10:2100/library?tab=albums")
        );
        assert_eq!(encrypted_address("evil.example/@x", "/"), None);
        assert_eq!(encrypted_address("", "/"), None);
    }

    #[test]
    fn a_handshake_is_told_from_a_request_in_the_clear() {
        assert!(is_a_handshake(22));
        assert!(!is_a_handshake(b'G'));
        assert!(!is_a_handshake(b'P'));
    }

    fn address(text: &str) -> IpAddr {
        text.parse().expect("an address")
    }

    fn holding(held: &Held, text: &str) -> usize {
        let party = melyxar_core::network::party_of(address(text));
        held.0.lock().expect("held").get(&party).copied().unwrap_or(0)
    }

    #[test]
    fn a_party_far_away_holds_no_more_than_its_share_and_gets_each_back() {
        let held = Held::default();
        let places: Vec<Place> = (0..MOST_FROM_ONE_PARTY)
            .map(|_| held.place_for(address("203.0.113.9")).expect("a place"))
            .collect();
        assert!(held.place_for(address("203.0.113.9")).is_none(), "one too many");
        assert!(
            held.place_for(address("198.51.100.4")).is_some(),
            "somebody else is not held to another's share"
        );

        drop(places);
        assert_eq!(holding(&held, "203.0.113.9"), 0, "every place given back");
        assert!(held.0.lock().expect("held").is_empty(), "and nothing kept for a party gone");
    }

    #[test]
    fn a_proxy_nearby_is_never_counted() {
        // Every visitor of a server behind a proxy reaches it from the
        // proxy's own address.
        let held = Held::default();
        let places: Vec<Place> = (0..MOST_FROM_ONE_PARTY + 10)
            .map(|_| held.place_for(address("192.168.1.20")).expect("never refused"))
            .collect();
        assert_eq!(places.len(), MOST_FROM_ONE_PARTY + 10);
        assert!(held.0.lock().expect("held").is_empty());
    }

    #[test]
    fn one_ipv6_network_shares_one_share() {
        let held = Held::default();
        let _places: Vec<Place> = (0..MOST_FROM_ONE_PARTY)
            .map(|index| {
                held.place_for(address(&format!("2001:db8:1:2::{index:x}")))
                    .expect("a place")
            })
            .collect();
        assert!(held.place_for(address("2001:db8:1:2:ffff::1")).is_none());
    }

    /// How long a request may take to arrive, in these tests.
    const SHORT: Duration = Duration::from_millis(300);

    /// A server answering `/` at once and `/long` over more than four times
    /// [`SHORT`], as a film or a live line does, behind a door fed from a
    /// listener on this machine.
    async fn a_server() -> SocketAddr {
        use axum::routing::get;
        use futures_util::stream;

        let app = axum::Router::new().route("/", get(|| async { "ok" })).route(
            "/long",
            get(|| async {
                let pieces = stream::unfold(0, |sent| async move {
                    if sent == 12 {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    let piece = if sent == 11 { "end" } else { "piece " };
                    Some((Ok::<_, std::convert::Infallible>(piece), sent + 1))
                });
                axum::body::Body::from_stream(pieces)
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let here = listener.local_addr().expect("an address");
        let (send, arrived) = mpsc::channel(WAITING);
        let accepting = tokio::spawn(async move {
            while let Ok((stream, address)) = listener.accept().await {
                let io = Io {
                    stream: Stream::Plain(stream),
                    _place: Place { counted: None },
                };
                let _ = send.send((io, Peer { address, encrypted: false })).await;
            }
        });
        let door = Door { arrived, accepting };
        tokio::spawn(door.serve_with(app, std::future::pending(), SHORT));
        here
    }

    /// Everything the server sends until it lets the connection go, or
    /// nothing more than a few seconds.
    async fn read_to_the_end(client: &mut TcpStream) -> (String, bool) {
        use tokio::io::AsyncReadExt;
        let mut said = Vec::new();
        let ended = tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut said))
            .await
            .is_ok();
        (String::from_utf8_lossy(&said).into_owned(), ended)
    }

    #[tokio::test]
    async fn a_request_that_never_finishes_arriving_is_let_go() {
        use tokio::io::AsyncWriteExt;
        let mut client = TcpStream::connect(a_server().await).await.expect("connected");
        client.write_all(b"GET / HTTP/1.1\r\nHo").await.expect("half a head");

        let (_, ended) = read_to_the_end(&mut client).await;
        assert!(ended, "a head that never ends holds a connection for ever");
    }

    #[tokio::test]
    async fn a_long_answer_is_never_cut_however_long_it_takes() {
        // A film or a live line is sent for far longer than a request may
        // take to arrive: the timer is about asking, never about answering.
        use tokio::io::AsyncWriteExt;
        let mut client = TcpStream::connect(a_server().await).await.expect("connected");
        client
            .write_all(b"GET /long HTTP/1.1\r\nHost: here\r\nConnection: close\r\n\r\n")
            .await
            .expect("asked");

        let (said, ended) = read_to_the_end(&mut client).await;
        assert!(ended);
        assert!(said.starts_with("HTTP/1.1 200"), "{said}");
        assert!(said.contains("end"), "the whole answer arrived: {said}");
    }

    #[tokio::test]
    async fn a_connection_left_idle_after_its_answer_is_let_go() {
        use tokio::io::AsyncWriteExt;
        let mut client = TcpStream::connect(a_server().await).await.expect("connected");
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: here\r\n\r\n")
            .await
            .expect("asked");

        let (said, ended) = read_to_the_end(&mut client).await;
        assert!(said.starts_with("HTTP/1.1 200"), "{said}");
        assert!(ended, "kept for the next request, then let go once idle too long");
    }
}
