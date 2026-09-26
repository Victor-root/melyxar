//! The films a saga leads to through the characters it shares with them.
//!
//! A provider cuts a universe into small sagas: the films of one hero, then
//! those where the heroes meet. What ties them together is a character played
//! by the same actor, so that is what is looked for: the actor alone would
//! bring in a whole filmography, and the character's name alone would mistake
//! two unrelated people called John for one.

use std::collections::{HashMap, HashSet};

use crate::id::{PersonId, WorkId};
use crate::text::fold_accents;

/// How far down a saga's cast a part still counts as one of its leads.
pub const LEADS: i32 = 5;

/// How many films sharing a saga's characters its row carries at most.
pub const MOST_KIN: usize = 20;

/// Names that say who someone is rather than whom they play, and so tie
/// nothing together: an actor who is himself in two films has not made them
/// one story.
const NOBODY_IN_PARTICULAR: &[&[&str]] = &[
    &["himself"],
    &["herself"],
    &["themselves"],
    &["self"],
    &["narrator"],
    &["lui", "meme"],
    &["elle", "meme"],
    &["eux", "memes"],
    &["elles", "memes"],
    &["narrateur"],
    &["narratrice"],
];

/// One part an actor plays in one work, and how high it is billed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub work_id: WorkId,
    pub person_id: PersonId,
    pub character: String,
    /// Place in the cast, nought for the first billed.
    pub billed: i32,
}

/// The names a character goes by, each as the words it is made of.
///
/// A provider writes "Tony Stark / Iron Man" for one part and notes such as
/// "(voice)" in brackets: each side of the stroke is a name, and what is in
/// brackets is none.
fn names_of(character: &str) -> Vec<HashSet<String>> {
    let mut outside = String::with_capacity(character.len());
    let mut depth = 0usize;
    for c in character.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => outside.push(c),
            _ => {}
        }
    }
    fold_accents(&outside)
        .to_lowercase()
        .split('/')
        .map(|name| {
            name.split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_string)
                .collect::<HashSet<_>>()
        })
        .filter(|words| {
            !words.is_empty()
                && !NOBODY_IN_PARTICULAR.iter().any(|nobody| {
                    nobody.len() == words.len() && nobody.iter().all(|word| words.contains(*word))
                })
        })
        .collect()
}

/// Whether two character names designate the same character: every word of
/// one of the names of the first is among the words of one of the names of
/// the second, or the other way round.
pub fn same_character(first: &str, second: &str) -> bool {
    let theirs = names_of(second);
    names_of(first).iter().any(|mine| {
        theirs
            .iter()
            .any(|other| mine.is_subset(other) || other.is_subset(mine))
    })
}

/// How much a place in a cast weighs: the first billed most.
fn weight_of(billed: i32) -> f64 {
    1.0 / (1.0 + billed.max(0) as f64)
}

/// The works where a saga's leads come back, as the same character played by
/// the same actor, the ones where they matter most first.
///
/// A lead counts for more the higher it is billed and the more films of the
/// saga it leads, and a work for more the higher those leads are billed in
/// it. `elsewhere` holds the parts played outside the saga by its leads; two
/// works that weigh the same keep the order they arrived in.
pub fn kin_of_a_saga(saga: &[Role], elsewhere: &[Role]) -> Vec<WorkId> {
    let leads: Vec<&Role> = saga.iter().filter(|role| role.billed < LEADS).collect();

    let mut weights: HashMap<WorkId, HashMap<PersonId, f64>> = HashMap::new();
    let mut arrived: Vec<WorkId> = Vec::new();
    for part in elsewhere {
        let standing: f64 = leads
            .iter()
            .filter(|lead| {
                lead.person_id == part.person_id && same_character(&lead.character, &part.character)
            })
            .map(|lead| weight_of(lead.billed))
            .sum();
        if standing == 0.0 {
            continue;
        }
        let people = weights.entry(part.work_id).or_insert_with(|| {
            arrived.push(part.work_id);
            HashMap::new()
        });
        // An actor credited twice in one film is still one character there.
        let weighed = standing * weight_of(part.billed);
        let kept = people.entry(part.person_id).or_insert(0.0);
        *kept = kept.max(weighed);
    }

    let mut kin: Vec<(WorkId, f64)> = arrived
        .into_iter()
        .map(|work| (work, weights[&work].values().sum()))
        .collect();
    kin.sort_by(|(_, first), (_, second)| second.total_cmp(first));
    kin.truncate(MOST_KIN);
    kin.into_iter().map(|(work, _)| work).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(work_id: WorkId, person_id: PersonId, character: &str, billed: i32) -> Role {
        Role {
            work_id,
            person_id,
            character: character.to_string(),
            billed,
        }
    }

    #[test]
    fn a_character_is_known_by_any_of_its_names() {
        assert!(same_character("Kael Vorn", "Kael Vorn"));
        assert!(same_character("Kael", "Kael Vorn"));
        assert!(same_character("Kael Vorn / The Storm", "The Storm"));
        assert!(same_character("Kaël Vorn (voice)", "kael vorn"));
    }

    #[test]
    fn two_characters_who_only_share_a_word_are_two_characters() {
        assert!(!same_character("John Ashgrove", "John Merrow"));
        assert!(!same_character("Kael Vorn", "Ilsa Vorne"));
    }

    #[test]
    fn being_oneself_or_telling_the_story_ties_nothing_together() {
        assert!(!same_character("Himself", "Himself"));
        assert!(!same_character("Lui-même", "Lui-même"));
        assert!(!same_character("Narrator (voice)", "Narrator"));
    }

    #[test]
    fn the_films_where_a_saga_s_leads_come_back_follow_it_the_most_present_first() {
        let [first, second, meeting, reunion, cameo, unrelated] =
            std::array::from_fn(|_| WorkId::new());
        let [hero, rival, other] = std::array::from_fn(|_| PersonId::new());
        let saga = [
            role(first, hero, "Kael Vorn", 0),
            role(first, rival, "Ilsa", 2),
            role(second, hero, "Kael Vorn", 0),
        ];
        let elsewhere = [
            // Where the heroes meet: the hero billed third.
            role(meeting, hero, "Kael", 2),
            // Where they meet again: the hero leads, and the rival is there.
            role(reunion, hero, "Kael Vorn", 0),
            role(reunion, rival, "Ilsa", 6),
            // A glimpse at the end of another story.
            role(cameo, hero, "Kael Vorn", 40),
            // The same actor, somebody else altogether.
            role(unrelated, hero, "Doran Pike", 0),
            // Somebody else in the saga's clothes.
            role(unrelated, other, "Kael Vorn", 0),
        ];

        assert_eq!(
            kin_of_a_saga(&saga, &elsewhere),
            vec![reunion, meeting, cameo]
        );
    }

    #[test]
    fn a_part_too_low_in_the_saga_s_cast_leads_nowhere() {
        let [film, other] = std::array::from_fn(|_| WorkId::new());
        let extra = PersonId::new();
        let saga = [role(film, extra, "Harbour Guard", LEADS)];
        let elsewhere = [role(other, extra, "Harbour Guard", 0)];

        assert!(kin_of_a_saga(&saga, &elsewhere).is_empty());
    }
}
