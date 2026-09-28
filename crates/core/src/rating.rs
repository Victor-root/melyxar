//! The ratings a work carries from elsewhere than its provider.

/// Where a rating comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RatingSource {
    /// The viewers of IMDb, out of ten, with how many voted.
    Imdb,
    /// The critics Rotten Tomatoes gathers: the share of them who liked it,
    /// out of a hundred.
    RottenTomatoes,
}

impl RatingSource {
    pub const ALL: [Self; 2] = [Self::Imdb, Self::RottenTomatoes];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Imdb => "imdb",
            Self::RottenTomatoes => "rotten_tomatoes",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|source| source.as_str() == value)
    }
}

/// One rating a work carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rating {
    pub source: RatingSource,
    pub value: f64,
    pub votes: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_round_trips_through_its_stored_form() {
        for source in RatingSource::ALL {
            assert_eq!(RatingSource::parse(source.as_str()), Some(source));
        }
        assert_eq!(RatingSource::parse("metacritic"), None);
    }
}
