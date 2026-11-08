//! Which of Melyxar's own pictures an account without a photo wears.
//!
//! Handed out by the server when the account is made and kept with it: only
//! the server sees every account, so only it can keep two of them from
//! wearing the same one.

/// One of the pictures, named as the web and the television name their files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FacePicture {
    #[default]
    Comet,
    Rings,
    Orbits,
    Crystal,
    Eclipse,
    Aurora,
    Crescent,
    Spiral,
    Binary,
    Verdant,
}

impl FacePicture {
    pub const fn every() -> [Self; 10] {
        [
            Self::Comet,
            Self::Rings,
            Self::Orbits,
            Self::Crystal,
            Self::Eclipse,
            Self::Aurora,
            Self::Crescent,
            Self::Spiral,
            Self::Binary,
            Self::Verdant,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comet => "comet",
            Self::Rings => "rings",
            Self::Orbits => "orbits",
            Self::Crystal => "crystal",
            Self::Eclipse => "eclipse",
            Self::Aurora => "aurora",
            Self::Crescent => "crescent",
            Self::Spiral => "spiral",
            Self::Binary => "binary",
            Self::Verdant => "verdant",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::every().into_iter().find(|picture| picture.as_str() == value)
    }

    /// The picture for a new account: the one the fewest others wear.
    ///
    /// Between pictures worn as often, the name decides where to start
    /// looking, so a household made in one sitting is not handed the list in
    /// its order every time.
    pub fn least_worn(name: &str, worn: impl IntoIterator<Item = Self>) -> Self {
        let every = Self::every();
        // Declared in the order of `every`, so a picture is its own place.
        let mut counts = [0usize; 10];
        for picture in worn {
            counts[picture as usize] += 1;
        }
        let fewest = counts.iter().copied().min().unwrap_or(0);
        let start = hash_of(name) as usize % every.len();
        (0..every.len())
            .map(|step| every[(start + step) % every.len()])
            .find(|picture| counts[*picture as usize] == fewest)
            .unwrap_or(every[start])
    }
}

/// FNV-1a over the name's bytes: small and well spread.
fn hash_of(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_reads_back_from_its_name() {
        for picture in FacePicture::every() {
            assert_eq!(FacePicture::parse(picture.as_str()), Some(picture));
        }
        assert_eq!(FacePicture::parse("nebula"), None);
    }

    #[test]
    fn a_household_wears_ten_different_pictures_before_one_comes_back() {
        let mut worn = Vec::new();
        for at in 0..10 {
            worn.push(FacePicture::least_worn(&format!("account {at}"), worn.clone()));
        }
        let mut distinct = worn.clone();
        distinct.sort_by_key(|picture| picture.as_str());
        distinct.dedup();
        assert_eq!(distinct.len(), 10);

        let eleventh = FacePicture::least_worn("one more", worn.clone());
        worn.push(eleventh);
        let twelfth = FacePicture::least_worn("and another", worn);
        assert_ne!(eleventh, twelfth);
    }

    #[test]
    fn the_same_name_among_the_same_accounts_gets_the_same_picture() {
        let worn = [FacePicture::Comet, FacePicture::Rings];
        assert_eq!(
            FacePicture::least_worn("Alice", worn),
            FacePicture::least_worn("Alice", worn)
        );
        assert!(!worn.contains(&FacePicture::least_worn("Alice", worn)));
    }
}
