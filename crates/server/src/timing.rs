//! How long each answer took, said where it can be read.
//!
//! The budgets this server is held to are written in milliseconds per request,
//! so they have to be readable per request or they are opinions. Two places
//! carry the number, for two different people: the `Server-Timing` header,
//! which the network tab of a browser shows next to the request itself, and a
//! line in the journal, which is what gets pasted into a conversation.
//!
//! Measured around the whole answer and outside the packing, so what is
//! reported is what the client waited for rather than a part of it.
//!
//! What is deliberately *not* reported here is how many statements the answer
//! ran. That number is worth more than this one, since a page that asks one
//! question per card looks fine on a small collection and only falls over on a
//! large one. But the database driver runs each statement on a thread of its
//! own, so a statement cannot be attributed to the request that asked for it
//! without wrapping every call, and a count that is only sometimes right is
//! worse than none: it would be believed. The bench reads that number a
//! different way, against a server serving nothing else.

use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

/// The header a browser reads the timing from.
const SERVER_TIMING: &str = "server-timing";

/// Below this, an answer is not worth a line of the journal.
///
/// Sixty pictures arrive per page of the grid, each one a file handed over in
/// under a millisecond. A line each would bury the one line that says what the
/// page itself cost, and the journal keeps a few thousand lines in all.
const WORTH_SAYING: Duration = Duration::from_millis(5);

/// Times one answer, says so in its headers, and writes down the slow ones.
pub async fn measured(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let started = Instant::now();
    let mut response = next.run(request).await;
    let took = started.elapsed();

    if let Ok(value) = HeaderValue::from_str(&header(took)) {
        response.headers_mut().insert(SERVER_TIMING, value);
    }

    if took >= WORTH_SAYING {
        tracing::debug!(
            method = %method,
            path = %path,
            status = response.status().as_u16(),
            ms = milliseconds(took),
            "answered"
        );
    }

    response
}

/// The header, in the form a browser shows in its network tab.
fn header(took: Duration) -> String {
    format!("total;dur={}", milliseconds(took))
}

/// A duration as milliseconds, to a tenth.
///
/// A tenth is what the budgets are argued in. Finer would be noise from one
/// run to the next, and whole milliseconds would report a five millisecond
/// budget as met or missed by a fifth of itself.
fn milliseconds(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 10_000.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_reads_as_milliseconds_to_a_tenth() {
        assert_eq!(milliseconds(Duration::from_millis(12)), 12.0);
        assert_eq!(milliseconds(Duration::from_micros(12_340)), 12.3);
        assert_eq!(milliseconds(Duration::from_micros(12_350)), 12.4);
        assert_eq!(milliseconds(Duration::ZERO), 0.0);
    }

    #[test]
    fn the_header_says_how_long_the_whole_answer_took() {
        let header = header(Duration::from_micros(18_400));
        assert_eq!(header, "total;dur=18.4");
        assert!(
            HeaderValue::from_str(&header).is_ok(),
            "what is written here has to survive being put in a header"
        );
    }

    #[test]
    fn a_picture_handed_over_from_the_cache_says_nothing() {
        // Sixty of these arrive per page of the grid.
        assert!(Duration::from_millis(2) < WORTH_SAYING);
    }
}
