//! How fast this server reaches a viewer, measured by the viewer.
//!
//! A player left to choose its own quality has to know what the connection
//! carries before it asks for a film: asking for the film as it is over a thin
//! connection is how it stands still every few seconds. So it downloads a
//! block of nothing in particular and times it. Made of bytes no packing can
//! shrink, and sent with packing refused outright: a block that shrinks on the
//! way measures the packing rather than the connection.

use std::sync::OnceLock;

use axum::body::{Body, Bytes};
use axum::extract::Query;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::AppState;
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/system/bandwidth", axum::routing::get(block_to_time))
}

/// What is sent when nothing is asked: enough to time on a thin connection.
const USUAL: usize = 512 * 1024;

/// The most sent in one answer: under a fifth of a second on a gigabit
/// network, and nothing a viewer asking for more could turn into a load.
const LARGEST: usize = 16 * 1024 * 1024;

/// The block every answer repeats, made once.
const BLOCK: usize = 1024 * 1024;

#[derive(Deserialize)]
struct Asked {
    bytes: Option<usize>,
}

async fn block_to_time(_: crate::account::Viewer, Query(asked): Query<Asked>) -> Response {
    sent(bytes_sent(asked.bytes))
}

/// How much is sent for what was asked, kept between one byte and the most.
fn bytes_sent(asked: Option<usize>) -> usize {
    asked.unwrap_or(USUAL).clamp(1, LARGEST)
}

/// An answer of exactly that many bytes, which nothing on the way may pack
/// nor keep.
fn sent(bytes: usize) -> Response {
    let block = block();
    let pieces = (0..bytes.div_ceil(BLOCK)).map(move |index| {
        let from = index * BLOCK;
        Ok::<Bytes, std::io::Error>(block.slice(..(bytes - from).min(BLOCK)))
    });
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            // Said already packed as it is, which is what keeps this server's
            // own packing off it.
            (header::CONTENT_ENCODING, "identity"),
            (header::CACHE_CONTROL, "no-store, no-transform"),
            (header::CONTENT_LENGTH, &bytes.to_string()),
        ],
        Body::from_stream(futures_util::stream::iter(pieces)),
    )
        .into_response()
}

/// A megabyte of bytes with no pattern to pack, drawn once.
fn block() -> Bytes {
    static BLOCK_MADE: OnceLock<Bytes> = OnceLock::new();
    BLOCK_MADE
        .get_or_init(|| {
            // Any spread of bytes will do, so long as it has no repeat a
            // packer could find: a fixed xorshift is all it takes.
            let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
            let mut made = Vec::with_capacity(BLOCK);
            while made.len() < BLOCK {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                made.extend_from_slice(&state.to_le_bytes());
            }
            Bytes::from(made)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_sent_stays_between_one_byte_and_the_most() {
        assert_eq!(bytes_sent(None), USUAL);
        assert_eq!(bytes_sent(Some(0)), 1);
        assert_eq!(bytes_sent(Some(3_000)), 3_000);
        assert_eq!(bytes_sent(Some(usize::MAX)), LARGEST);
    }

    #[tokio::test]
    async fn the_answer_holds_exactly_what_was_asked_and_refuses_packing() {
        for bytes in [1, BLOCK - 1, BLOCK, BLOCK + 5, 3 * BLOCK] {
            let answer = sent(bytes);
            assert_eq!(answer.headers()[header::CONTENT_ENCODING], "identity");
            assert_eq!(answer.headers()[header::CONTENT_LENGTH], bytes.to_string().as_str());
            let body = axum::body::to_bytes(answer.into_body(), usize::MAX).await.unwrap();
            assert_eq!(body.len(), bytes);
        }
    }

    #[test]
    fn the_block_has_no_run_a_packer_could_fold() {
        let block = block();
        assert_eq!(block.len(), BLOCK);
        let mut seen = [0usize; 256];
        for byte in block.iter() {
            seen[usize::from(*byte)] += 1;
        }
        // Every byte value turns up about as often as every other.
        let expected = BLOCK / 256;
        assert!(seen.iter().all(|count| count.abs_diff(expected) < expected / 10));
    }
}
