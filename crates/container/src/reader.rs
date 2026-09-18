//! Reading a few thousand bytes of a very large file, and nothing more.
//!
//! Every read goes against a budget. An index is small by nature, so a file
//! that keeps asking for more of itself is not an index being read but a
//! malformed one leading this round in circles, and the point of the whole
//! module is to not read the film through.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// How much of one file may be read before this stops calling it an index.
///
/// The tables of a three hour film at sixty pictures a second come to some ten
/// megabytes in the worst shape they take, one entry per picture in each of
/// them. Anything past this is not an index.
const AT_MOST: u64 = 64 * 1024 * 1024;

/// A file being read for its index.
///
/// Every method answers `None` rather than an error: a file that cannot be
/// read here is read through by the caller, which reports what went wrong
/// with the name of the film in hand.
pub(crate) struct Reader {
    file: File,
    length: u64,
    at: u64,
    read_so_far: u64,
}

impl Reader {
    pub(crate) fn open(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let length = file.metadata().ok()?.len();
        Some(Self {
            file,
            length,
            at: 0,
            read_so_far: 0,
        })
    }

    /// How long the file is.
    pub(crate) fn length(&self) -> u64 {
        self.length
    }

    /// Where the next read will start.
    pub(crate) fn at(&self) -> u64 {
        self.at
    }

    pub(crate) fn seek_to(&mut self, at: u64) -> Option<()> {
        if at > self.length {
            return None;
        }
        self.file.seek(SeekFrom::Start(at)).ok()?;
        self.at = at;
        Some(())
    }

    /// Fills the whole buffer, or answers nothing at all.
    pub(crate) fn exactly(&mut self, into: &mut [u8]) -> Option<()> {
        let wanted = into.len() as u64;
        self.read_so_far = self.read_so_far.checked_add(wanted)?;
        if self.read_so_far > AT_MOST {
            return None;
        }
        self.file.read_exact(into).ok()?;
        self.at = self.at.checked_add(wanted)?;
        Some(())
    }

    pub(crate) fn bytes(&mut self, how_many: usize) -> Option<Vec<u8>> {
        // Asked for before it is read, so a length invented by a malformed
        // file cannot reserve the memory of a film before failing.
        if how_many as u64 > AT_MOST.saturating_sub(self.read_so_far) {
            return None;
        }
        let mut into = vec![0u8; how_many];
        self.exactly(&mut into)?;
        Some(into)
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        let mut byte = [0u8; 1];
        self.exactly(&mut byte)?;
        Some(byte[0])
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        let mut four = [0u8; 4];
        self.exactly(&mut four)?;
        Some(u32::from_be_bytes(four))
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        let mut eight = [0u8; 8];
        self.exactly(&mut eight)?;
        Some(u64::from_be_bytes(eight))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn file_of(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("file");
        std::fs::File::create(&path)
            .expect("the file is created")
            .write_all(bytes)
            .expect("the file is written");
        (directory, path)
    }

    #[test]
    fn a_read_past_the_end_of_a_file_answers_nothing_rather_than_a_guess() {
        let (_directory, path) = file_of(&[1, 2, 3]);
        let mut reader = Reader::open(&path).expect("the file opens");
        assert_eq!(reader.length(), 3);
        assert!(reader.u32().is_none(), "four bytes out of three");
        assert!(reader.seek_to(9).is_none(), "past the end of the file");
    }

    #[test]
    fn a_length_a_malformed_file_invented_is_refused_before_it_is_reserved() {
        // The length comes from the file itself, so a broken one asks for a
        // buffer of four thousand megabytes. Asking is refused rather than
        // tried and failed, because trying is what would run the server out
        // of memory.
        let (_directory, path) = file_of(&[1, 2, 3]);
        let mut reader = Reader::open(&path).expect("the file opens");
        assert!(reader.bytes(usize::MAX / 2).is_none());
    }

    #[test]
    fn a_file_that_keeps_asking_for_more_of_itself_is_given_up_on() {
        let (_directory, path) = file_of(&vec![0u8; 1024]);
        let mut reader = Reader::open(&path).expect("the file opens");
        let mut read = 0u64;
        loop {
            reader.seek_to(0).expect("back to the beginning");
            if reader.bytes(1024).is_none() {
                break;
            }
            read += 1024;
            assert!(read <= AT_MOST, "the budget must stop this");
        }
    }
}
