//! The little pictures shown while somebody drags along the playback bar.
//!
//! They are gathered into sheets rather than kept one to a file: a film of two
//! hours holds seven hundred of them, and dragging along a bar asks for dozens
//! a second. One sheet of a hundred covers a thousand seconds of film in a
//! single request.
//!
//! The shape lives here because everyone needs it: the tool that writes the
//! sheets, the table that remembers them, the route that hands one over, and
//! the page that places a thumbnail on screen.

use crate::time::Millis;

/// What is asked of the tool for one film.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// How far apart in the film two thumbnails stand.
    pub every: Millis,
    /// Height of one thumbnail, in pixels.
    ///
    /// A height rather than a width, because that is the side both the
    /// processor and a card are asked to scale by. What the width comes out as
    /// is measured afterwards rather than assumed: a film is not always the
    /// shape its pixel count suggests.
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
}

impl Layout {
    /// How many thumbnails one sheet holds.
    pub fn per_sheet(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }
}

/// What a film's thumbnails really are, once they have been made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumbnails {
    pub every: Millis,
    /// Size of one thumbnail, measured on a sheet rather than worked out.
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    /// How many came out of the reading.
    ///
    /// Counted rather than worked out from the running time: reading only the
    /// pictures that stand on their own loses the last slot of a film, and a
    /// container's stated running time is wrong often enough that nothing may
    /// be built on it.
    pub counted: u32,
    /// Sheets written, numbered from zero.
    pub sheets: u32,
}

/// Where one thumbnail is to be found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    /// Which sheet to fetch.
    pub sheet: u32,
    /// Where on it, in thumbnails from the left and from the top.
    pub column: u32,
    pub row: u32,
}

impl Thumbnails {
    /// How many thumbnails one sheet holds.
    pub fn per_sheet(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }

    /// The shape these were made to, so a film made to another one can be
    /// told apart from a film made to this one.
    pub fn layout(&self) -> Layout {
        Layout {
            every: self.every,
            height: self.height,
            columns: self.columns,
            rows: self.rows,
        }
    }

    /// Which thumbnail covers a moment of the film, when one does.
    ///
    /// Nothing beyond the last one: a sheet is filled to the end with black
    /// whatever the film gave, and a black square under the cursor is worse
    /// than no square at all.
    pub fn index_at(&self, position: Millis) -> Option<u32> {
        if self.every.get() <= 0 || self.counted == 0 {
            return None;
        }
        let index = (position.get().max(0) / self.every.get()) as u32;
        (index < self.counted).then_some(index)
    }

    /// Where to find one thumbnail, when it is one this film has.
    pub fn spot_of(&self, index: u32) -> Option<Spot> {
        if index >= self.counted {
            return None;
        }
        let per_sheet = self.per_sheet();
        if per_sheet == 0 || self.columns == 0 {
            return None;
        }
        let on_its_sheet = index % per_sheet;
        Some(Spot {
            sheet: index / per_sheet,
            column: on_its_sheet % self.columns,
            row: on_its_sheet / self.columns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ten_seconds(counted: u32) -> Thumbnails {
        Thumbnails {
            every: Millis::new(10_000),
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            counted,
            sheets: counted.div_ceil(100),
        }
    }

    #[test]
    fn a_moment_of_the_film_names_one_thumbnail() {
        let made = ten_seconds(720);
        assert_eq!(made.index_at(Millis::ZERO), Some(0));
        assert_eq!(made.index_at(Millis::new(9_999)), Some(0));
        assert_eq!(made.index_at(Millis::new(10_000)), Some(1));
        assert_eq!(made.index_at(Millis::new(1_000_000)), Some(100));
    }

    #[test]
    fn nothing_is_offered_past_the_last_thumbnail() {
        // A sheet is filled to the end with black whatever the film gave, so
        // the only thing standing between a viewer and a black square is this.
        let made = ten_seconds(21);
        assert_eq!(made.index_at(Millis::new(200_000)), Some(20));
        assert_eq!(made.index_at(Millis::new(210_000)), None);
        assert_eq!(made.spot_of(21), None);
        assert_eq!(ten_seconds(0).index_at(Millis::ZERO), None);
    }

    #[test]
    fn a_thumbnail_is_found_by_sheet_then_by_row_and_column() {
        let made = ten_seconds(250);
        assert_eq!(
            made.spot_of(0),
            Some(Spot {
                sheet: 0,
                column: 0,
                row: 0
            })
        );
        assert_eq!(
            made.spot_of(11),
            Some(Spot {
                sheet: 0,
                column: 1,
                row: 1
            })
        );
        assert_eq!(
            made.spot_of(100),
            Some(Spot {
                sheet: 1,
                column: 0,
                row: 0
            }),
            "the first thumbnail of the second sheet"
        );
        assert_eq!(
            made.spot_of(249),
            Some(Spot {
                sheet: 2,
                column: 9,
                row: 4
            })
        );
    }

    #[test]
    fn what_a_film_was_made_to_is_what_it_is_compared_against() {
        let made = ten_seconds(720);
        assert_eq!(
            made.layout(),
            Layout {
                every: Millis::new(10_000),
                height: 180,
                columns: 10,
                rows: 10
            }
        );
        assert_eq!(made.layout().per_sheet(), 100);
    }
}
