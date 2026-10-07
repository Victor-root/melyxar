//! The brake on wrong passwords, counted per address they come from.
//!
//! Guessing a password should cost time rather than a handful of tries, and
//! checking one is made expensive on purpose, so a thousand guesses a second
//! would be asking this server to grind itself to a halt. Held back, none of
//! them is checked.
//!
//! Counted by where the tries come from rather than by the account they are
//! aimed at, whatever name is typed and whether anybody has it: counted per
//! account, whoever typed nonsense at a name held its owner back with them,
//! and the names are offered at the door. Kept in memory, so a server that
//! has just restarted has forgotten every hold, which is the right way round
//! for somebody who locked themselves out and rebooted it.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Mutex, PoisonError};

use melyxar_core::time::Timestamp;

/// How long an address is left alone the first time it is held back. Every
/// further round doubles it, up to [`LONGEST_HOLD`].
const FIRST_HOLD: time::Duration = time::Duration::minutes(1);

/// Past an hour, a longer wait slows nobody's guessing any further and only
/// keeps out a household that has mistyped a lot.
const LONGEST_HOLD: time::Duration = time::Duration::hours(1);

/// A day without a single try, and an address starts afresh.
const FORGOTTEN_AFTER: time::Duration = time::Duration::days(1);

/// How often what has been forgotten is cleared away, so that addresses which
/// tried once and went are not kept for ever.
const SWEPT_EVERY: time::Duration = time::Duration::hours(1);

/// Where tries come from, as they are counted.
///
/// An IPv4 address, or the network an IPv6 one belongs to: a single customer
/// is handed billions of IPv6 addresses, and counting each apart would let
/// one machine start afresh with every try. Nothing when the address could
/// not be told, in which case every such try is counted together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Origin(Option<IpAddr>);

impl Origin {
    pub(crate) fn of(address: Option<IpAddr>) -> Self {
        Self(address.map(|address| match address {
            IpAddr::V4(_) => address,
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => IpAddr::V4(v4),
                None => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
            },
        }))
    }
}

/// What a try is allowed, before its password is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Turn {
    /// Checked. When it is the last one allowed in a row, its address is held
    /// back from now on, and set free again if it turns out to be right.
    Checked { holds_back: bool },
    /// Not checked, for this much longer.
    HeldBack(time::Duration),
}

#[derive(Default)]
pub(crate) struct WrongAnswers {
    kept: Mutex<Kept>,
}

#[derive(Default)]
struct Kept {
    by_origin: HashMap<Origin, InARow>,
    next_sweep: Option<Timestamp>,
}

struct InARow {
    /// Tries since the last hold, or since the first one.
    tries: u32,
    /// How many times this address has been held back since it started
    /// afresh.
    rounds: u32,
    until: Option<Timestamp>,
    last: Timestamp,
}

impl InARow {
    fn held_back(&self, at: Timestamp) -> Option<time::Duration> {
        self.until.filter(|until| *until > at).map(|until| until - at)
    }

    fn forgotten(&self, at: Timestamp) -> bool {
        self.held_back(at).is_none() && self.last + FORGOTTEN_AFTER <= at
    }
}

/// How long the given round of holding back lasts.
fn hold_for(round: u32) -> time::Duration {
    // Two to the sixth minutes is already past the longest hold.
    (FIRST_HOLD * (1_i32 << round.saturating_sub(1).min(6))).min(LONGEST_HOLD)
}

impl WrongAnswers {
    fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        // Nothing in here can leave the counts half written, so a panic
        // elsewhere while it was held is no reason to stop counting.
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How much longer tries from here are being left alone, when they are.
    pub(crate) fn held_back(&self, from: Origin, at: Timestamp) -> Option<time::Duration> {
        self.kept().by_origin.get(&from)?.held_back(at)
    }

    /// Counts a try before its password is checked, at `allowed` in a row.
    ///
    /// Counted before rather than after: the check takes a while, and tries
    /// sent together would otherwise all find the count where it was and all
    /// be checked.
    pub(crate) fn take_a_turn(&self, from: Origin, allowed: u32, at: Timestamp) -> Turn {
        let mut kept = self.kept();
        if kept.next_sweep.is_none_or(|next| next <= at) {
            kept.by_origin.retain(|_, row| !row.forgotten(at));
            kept.next_sweep = Some(at + SWEPT_EVERY);
        }

        let row = kept.by_origin.entry(from).or_insert(InARow {
            tries: 0,
            rounds: 0,
            until: None,
            last: at,
        });
        if let Some(left) = row.held_back(at) {
            return Turn::HeldBack(left);
        }
        if row.forgotten(at) {
            *row = InARow {
                tries: 0,
                rounds: 0,
                until: None,
                last: at,
            };
        }

        row.last = at;
        row.tries += 1;
        let holds_back = row.tries >= allowed;
        if holds_back {
            row.tries = 0;
            row.rounds += 1;
            row.until = Some(at + hold_for(row.rounds));
        }
        Turn::Checked { holds_back }
    }

    /// Somebody from here got it right, so nothing is held against it any more.
    pub(crate) fn forget(&self, from: Origin) {
        self.kept().by_origin.remove(&from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const ALLOWED: u32 = 10;
    const AT: Timestamp = datetime!(2026-01-01 12:00 UTC);

    fn from(address: &str) -> Origin {
        Origin::of(Some(address.parse().expect("an address")))
    }

    fn wrong(answers: &WrongAnswers, origin: Origin, how_many: u32, at: Timestamp) -> Vec<Turn> {
        (0..how_many).map(|_| answers.take_a_turn(origin, ALLOWED, at)).collect()
    }

    #[test]
    fn the_last_try_allowed_is_checked_and_holds_its_address_back() {
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");

        let turns = wrong(&answers, here, ALLOWED, AT);
        assert!(turns[..9].iter().all(|turn| *turn == Turn::Checked { holds_back: false }));
        assert_eq!(turns[9], Turn::Checked { holds_back: true });
        assert_eq!(answers.held_back(here, AT), Some(FIRST_HOLD));
        assert_eq!(answers.take_a_turn(here, ALLOWED, AT), Turn::HeldBack(FIRST_HOLD));
    }

    #[test]
    fn tries_sent_together_are_counted_before_any_is_checked() {
        // A hundred tries arriving in the same breath, none of them checked
        // yet: only the allowed number get a turn.
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");
        let turns = wrong(&answers, here, 100, AT);
        let checked = turns.iter().filter(|turn| matches!(turn, Turn::Checked { .. })).count();
        assert_eq!(checked, ALLOWED as usize);
    }

    #[test]
    fn holding_one_address_back_leaves_every_other_alone() {
        // Otherwise anybody could shut everyone else out of their own server
        // by typing nonsense at their names.
        let answers = WrongAnswers::default();
        wrong(&answers, from("203.0.113.9"), ALLOWED, AT);
        assert_eq!(answers.held_back(from("198.51.100.4"), AT), None);
        assert_eq!(
            answers.take_a_turn(from("198.51.100.4"), ALLOWED, AT),
            Turn::Checked { holds_back: false }
        );
    }

    #[test]
    fn each_further_round_holds_back_twice_as_long_up_to_an_hour() {
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");
        let mut at = AT;
        for expected in [1, 2, 4, 8, 16, 32, 60, 60] {
            wrong(&answers, here, ALLOWED, at);
            let held = answers.held_back(here, at).expect("held back");
            assert_eq!(held, time::Duration::minutes(expected));
            at += held;
        }
    }

    #[test]
    fn a_hold_is_a_wait_rather_than_a_lock_somebody_has_to_undo() {
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");
        wrong(&answers, here, ALLOWED, AT);
        assert_eq!(answers.held_back(here, AT + FIRST_HOLD), None);
        assert_eq!(
            answers.take_a_turn(here, ALLOWED, AT + FIRST_HOLD),
            Turn::Checked { holds_back: false }
        );
    }

    #[test]
    fn getting_it_right_clears_what_was_held_against_an_address() {
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");
        wrong(&answers, here, ALLOWED, AT);
        answers.forget(here);
        assert_eq!(answers.held_back(here, AT), None);

        // Back to nothing, rounds included: the next hold is a minute again.
        wrong(&answers, here, ALLOWED, AT);
        assert_eq!(answers.held_back(here, AT), Some(FIRST_HOLD));
    }

    #[test]
    fn a_quiet_day_starts_an_address_afresh() {
        let answers = WrongAnswers::default();
        let here = from("203.0.113.9");
        wrong(&answers, here, ALLOWED, AT);
        wrong(&answers, here, ALLOWED - 1, AT + FIRST_HOLD);

        let later = AT + FIRST_HOLD + FORGOTTEN_AFTER;
        assert_eq!(
            answers.take_a_turn(here, ALLOWED, later),
            Turn::Checked { holds_back: false },
            "the nine of yesterday are not added to today's first"
        );
        wrong(&answers, here, ALLOWED - 1, later);
        assert_eq!(answers.held_back(here, later), Some(FIRST_HOLD), "and the rounds start over");
    }

    #[test]
    fn addresses_that_went_quiet_are_cleared_away() {
        let answers = WrongAnswers::default();
        answers.take_a_turn(from("203.0.113.9"), ALLOWED, AT);
        answers.take_a_turn(from("198.51.100.4"), ALLOWED, AT + FORGOTTEN_AFTER);
        assert_eq!(answers.kept().by_origin.len(), 1);
    }

    #[test]
    fn one_ipv6_network_is_one_origin() {
        assert_eq!(from("2001:db8:1:2::1"), from("2001:db8:1:2:ffff::9"));
        assert_ne!(from("2001:db8:1:2::1"), from("2001:db8:1:3::1"));
        assert_eq!(from("::ffff:203.0.113.9"), from("203.0.113.9"));
        assert_ne!(from("203.0.113.9"), from("203.0.113.10"));
    }
}
