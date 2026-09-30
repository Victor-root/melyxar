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
//! opens a connection and says nothing holds up nobody but itself.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::connect_info::Connected;
use axum::serve::{IncomingStream, Listener};
use melyxar_app::AppState;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Semaphore};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;

/// The first byte of every TLS handshake.
const HANDSHAKE: u8 = 22;

/// How long a client may take to say its first byte, and then to finish its
/// handshake, before the connection is let go.
const PATIENCE: Duration = Duration::from_secs(10);

/// How many connections, done with their handshake, may wait to be served.
const WAITING: usize = 128;

/// How many connections may be opening at once. Past it, the next one waits
/// in the system's queue, so clients that connect and say nothing cannot
/// pile up without end.
const OPENING: usize = 512;

/// Who is at the other end of a connection, and whether it is encrypted.
#[derive(Debug, Clone, Copy)]
pub struct Peer {
    pub address: SocketAddr,
    pub encrypted: bool,
}

impl Connected<IncomingStream<'_, Door>> for Peer {
    fn connect_info(stream: IncomingStream<'_, Door>) -> Self {
        *stream.remote_addr()
    }
}

/// A connection, in the clear or encrypted.
pub enum Io {
    Plain(TcpStream),
    Encrypted(Box<TlsStream<TcpStream>>),
}

/// A request that came in the clear is sent to the encrypted address, once
/// the server encrypts and is not behind a proxy. Kept for the same host and
/// port, since it is the same door.
pub async fn sent_to_encrypted(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::extract::FromRequestParts;
    use axum::response::IntoResponse;

    let (mut parts, body) = request.into_parts();
    let Ok(caller) = crate::address::Caller::from_request_parts(&mut parts, &state).await;
    if !caller.encrypted && melyxar_app::access::sends_plain_to_encrypted(&state) {
        let host = parts.headers.get(axum::http::header::HOST).and_then(|host| host.to_str().ok());
        let path = parts.uri.path_and_query().map_or("/", |path| path.as_str());
        if let Some(to) = host.and_then(|host| encrypted_address(host, path)) {
            return axum::response::Redirect::permanent(&to).into_response();
        }
    }
    next.run(axum::extract::Request::from_parts(parts, body)).await
}

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
    local: SocketAddr,
    accepting: tokio::task::JoinHandle<()>,
}

impl Door {
    pub fn new(listener: TcpListener, state: AppState) -> io::Result<Self> {
        let local = listener.local_addr()?;
        let (send, arrived) = mpsc::channel(WAITING);
        let opening = Arc::new(Semaphore::new(OPENING));
        let accepting = tokio::spawn(async move {
            loop {
                let Ok(permit) = opening.clone().acquire_owned().await else {
                    return;
                };
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
                let (send, state) = (send.clone(), state.clone());
                tokio::spawn(async move {
                    let opened = opened(stream, address, &state).await;
                    drop(permit);
                    if let Some(opened) = opened {
                        let _ = send.send(opened).await;
                    }
                });
            }
        });
        Ok(Self { arrived, local, accepting })
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

/// A connection made ready to be served, or nothing when it is let go.
async fn opened(stream: TcpStream, address: SocketAddr, state: &AppState) -> Option<(Io, Peer)> {
    let _ = stream.set_nodelay(true);
    let mut first = [0u8; 1];
    let peeked = tokio::time::timeout(PATIENCE, stream.peek(&mut first)).await;
    if !matches!(peeked, Ok(Ok(1))) {
        return None;
    }
    if !is_a_handshake(first[0]) {
        return Some((Io::Plain(stream), Peer { address, encrypted: false }));
    }
    // A handshake the server has no certificate for is let go: there is
    // nothing it could say that the client would understand.
    let encryption = melyxar_app::access::encryption(state)?;
    match tokio::time::timeout(PATIENCE, TlsAcceptor::from(encryption).accept(stream)).await {
        Ok(Ok(encrypted)) => Some((Io::Encrypted(Box::new(encrypted)), Peer { address, encrypted: true })),
        Ok(Err(error)) => {
            // Every browser meeting a certificate it does not trust ends here
            // once before its visitor clicks through, so this is no trouble.
            tracing::debug!(%error, %address, "a handshake did not finish");
            None
        }
        Err(_) => None,
    }
}

impl Listener for Door {
    type Io = Io;
    type Addr = Peer;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.arrived.recv().await {
            Some(arrived) => arrived,
            // The loop that accepts never ends while the door stands.
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(Peer { address: self.local, encrypted: false })
    }
}

impl AsyncRead for Io {
    fn poll_read(self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Encrypted(stream) => Pin::new(stream.as_mut()).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for Io {
    fn poll_write(self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(context, buffer),
            Self::Encrypted(stream) => Pin::new(stream.as_mut()).poll_write(context, buffer),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write_vectored(context, buffers),
            Self::Encrypted(stream) => Pin::new(stream.as_mut()).poll_write_vectored(context, buffers),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Plain(stream) => stream.is_write_vectored(),
            Self::Encrypted(stream) => stream.is_write_vectored(),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(context),
            Self::Encrypted(stream) => Pin::new(stream.as_mut()).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Encrypted(stream) => Pin::new(stream.as_mut()).poll_shutdown(context),
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
}
