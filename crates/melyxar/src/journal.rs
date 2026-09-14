//! Keeping what the server says, so a screen can sort it afterwards.
//!
//! One more listener beside the one that writes to the console. The console
//! copy obeys the level in the configuration, because that is what somebody
//! reading a terminal wants; this one keeps everything Melyxar says down to
//! debug, whatever that level is, because a line that has to be switched on is
//! never there the evening it is wanted.
//!
//! Only what Melyxar itself writes. The libraries underneath say a great deal
//! that is true and useless here: every statement made to the database, every
//! socket opened. Keeping those would bury the two lines that matter.

use std::fmt;

use melyxar_core::journal;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Hands every line Melyxar writes to the journal a screen reads.
pub struct KeepWhatWasSaid;

impl<S: Subscriber> Layer<S> for KeepWhatWasSaid {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut said = Said::default();
        event.record(&mut said);
        journal::remember(
            named(event.metadata().level()),
            event.metadata().target(),
            said.finish(),
        );
    }
}

/// Whether a line is Melyxar's own, and worth keeping.
///
/// Written as a test on the line rather than a list of crate names, so a crate
/// added later is kept without anybody remembering to add it here.
pub fn worth_keeping(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.target().starts_with("melyxar") && *metadata.level() <= Level::DEBUG
}

fn named(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

/// Puts a line back together: what was said, then the values that came with it.
///
/// The message first and the fields after, in the order they were written, so
/// a pasted line reads the way it was meant to.
#[derive(Default)]
struct Said {
    message: String,
    fields: String,
}

impl Said {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (true, _) => self.fields,
            (false, true) => self.message,
            (false, false) => format!("{} {}", self.message, self.fields),
        }
    }

    fn add(&mut self, field: &Field, value: &dyn fmt::Display) {
        if field.name() == "message" {
            self.message = value.to_string();
            return;
        }
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        self.fields.push_str(field.name());
        self.fields.push('=');
        self.fields.push_str(&value.to_string());
    }
}

impl Visit for Said {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // Shown the way it was written rather than quoted: the message arrives
        // through this same door, and `"a subtitle is ready"` with its quotes
        // is not what anybody wrote.
        self.add(field, &format_args!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.add(field, &value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.add(field, &value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.add(field, &value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.add(field, &value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.add(field, &value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.add(field, &value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_reads_as_it_was_written() {
        assert_eq!(Said::default().finish(), "");

        let mut said = Said {
            message: "a subtitle is ready".into(),
            fields: String::new(),
        };
        said.add_for_test("bytes", "1204");
        assert_eq!(said.finish(), "a subtitle is ready bytes=1204");
    }

    #[test]
    fn values_with_nothing_said_still_come_through() {
        let mut said = Said::default();
        said.add_for_test("track", "01a0");
        said.add_for_test("codec", "subrip");
        assert_eq!(said.finish(), "track=01a0 codec=subrip");
    }

    impl Said {
        fn add_for_test(&mut self, name: &str, value: &str) {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            self.fields.push_str(name);
            self.fields.push('=');
            self.fields.push_str(value);
        }
    }
}
