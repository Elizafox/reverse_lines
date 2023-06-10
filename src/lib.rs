//! ### ReverseLines
//!
//! This library provides a small Rust Iterator for reading files or anything that implements
//! `std::io::Seek` and `std::io::Read` in reverse.
//!
//! It is a rework of [rev_lines](https://docs.rs/rev_lines/latest/rev_lines/) with improved error
//! handling and allowance for more types.
//!
//! #### Example
//!
//! ```
//!  extern crate reverse_lines;
//!
//!  use reverse_lines::ReverseLines;
//!  use std::io::BufReader;
//!  use std::fs::File;
//!
//!  fn main() {
//!      let file = File::open("tests/multi_line_file").unwrap();
//!      let reverse_lines = ReverseLines::new(BufReader::new(file)).unwrap();
//!
//!      for line in reverse_lines {
//!          println!("{}", line.unwrap());
//!      }
//!  }
//! ```
//!
//! If a line with invalid UTF-8 is encountered, or if there is an I/O error, the iterator will
//! yield an `std::io::Error`.
//!
//! This method uses logic borrowed from [uutils/coreutils
//! tail](https://github.com/uutils/coreutils/blob/f2166fed0ad055d363aedff6223701001af090d3/src/tail/tail.rs#L399-L402)
//! and code borrowed from [rev_lines](https://docs.rs/rev_lines/latest/rev_lines/).

use std::cmp::min;
use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom};
use std::iter::FusedIterator;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

const DEFAULT_SIZE: usize = 4096;

static LF_BYTE: u8 = b'\n';
static CR_BYTE: u8 = b'\r';

/// `ReverseLines` struct
pub struct ReverseLines<R: Seek + Read> {
    reader: R,
    reader_pos: u64,
    buf_size: u64,
    is_error: bool,
}

impl<R: Seek + Read> ReverseLines<R> {
    /// Create a new `ReverseLines` struct from a `<R>`. Internal
    /// buffering for iteration will default to 4096 bytes at a time.
    pub fn new(reader: R) -> Result<ReverseLines<R>> {
        ReverseLines::with_capacity(DEFAULT_SIZE, reader)
    }

    /// Create a new `ReverseLines` struct from a `<R>`. Interal
    /// buffering for iteration will use `cap` bytes at a time.
    pub fn with_capacity(cap: usize, mut reader: R) -> Result<ReverseLines<R>> {
        // Seek to end of reader now
        let reader_size = reader.seek(SeekFrom::End(0))?;

        let mut reverse_lines = ReverseLines {
            reader,
            reader_pos: reader_size,
            buf_size: cap as u64,
            is_error: false,
        };

        // Handle any trailing new line characters for the reader
        // so the first next call does not return Some("")

        // Read at most 2 bytes
        let end_size = min(reader_size, 2);
        let end_buf = reverse_lines.read_to_buffer(end_size)?;

        if end_size == 1 {
            if end_buf[0] != LF_BYTE {
                reverse_lines.move_reader_position(1)?;
            }
        } else if end_size == 2 {
            if end_buf[0] != CR_BYTE {
                reverse_lines.move_reader_position(1)?;
            }

            if end_buf[1] != LF_BYTE {
                reverse_lines.move_reader_position(1)?;
            }
        }

        Ok(reverse_lines)
    }

    fn read_to_buffer(&mut self, size: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0; size as usize];
        let offset = -(size as i64);

        self.reader.seek(SeekFrom::Current(offset))?;
        self.reader.read_exact(&mut buf[0..(size as usize)])?;
        self.reader.seek(SeekFrom::Current(offset))?;

        self.reader_pos -= size;

        Ok(buf)
    }

    fn move_reader_position(&mut self, offset: u64) -> Result<()> {
        self.reader.seek(SeekFrom::Current(offset as i64))?;
        self.reader_pos += offset;

        Ok(())
    }
}

impl<R: Read + Seek> Iterator for ReverseLines<R> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_error {
            return None;
        }

        let mut result: Vec<u8> = Vec::new();

        'outer: loop {
            if self.reader_pos < 1 {
                if !result.is_empty() {
                    break;
                }

                return None;
            }

            // Read the of minimum between the desired
            // buffer size or remaining length of the reader
            let size = min(self.buf_size, self.reader_pos);

            match self.read_to_buffer(size) {
                Ok(buf) => {
                    for (idx, ch) in buf.iter().enumerate().rev() {
                        // Found a new line character to break on
                        if *ch == LF_BYTE {
                            let mut offset = idx as u64;

                            // Add an extra byte cause of CR character
                            if idx > 1 && buf[idx - 1] == CR_BYTE {
                                offset -= 1;
                            }

                            match self.reader.seek(SeekFrom::Current(offset as i64)) {
                                Ok(_) => {
                                    self.reader_pos += offset;
                                    break 'outer;
                                }

                                Err(e) => {
                                    self.is_error = true;
                                    return Some(Err(e));
                                }
                            }
                        } else {
                            result.push(*ch);
                        }
                    }
                }

                Err(e) => {
                    self.is_error = true;
                    return Some(Err(e));
                }
            }
        }

        // Reverse the results since they were written backwards
        result.reverse();

        // Convert to a String
        Some(String::from_utf8(result).map_err(|e| Error::new(ErrorKind::InvalidData, e)))
    }
}

impl<R: Read + Seek> FusedIterator for ReverseLines<R> {}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn it_handles_empty_files() {
        let file = File::open("tests/empty_file").unwrap();
        let mut rev_lines = ReverseLines::new(file).unwrap();

        assert_matches!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_one_line() {
        let file = File::open("tests/one_line_file").unwrap();
        let mut rev_lines = ReverseLines::new(file).unwrap();

        assert_eq!(rev_lines.next().unwrap().unwrap(), "ABCD".to_string());
        assert_matches!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_multi_lines() {
        let file = File::open("tests/multi_line_file").unwrap();
        let mut rev_lines = ReverseLines::new(file).unwrap();

        assert_eq!(rev_lines.next().unwrap().unwrap(), "UVWXYZ".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "LMNOPQRST".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "GHIJK".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "ABCDEF".to_string());
        assert_matches!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_blank_lines() {
        let file = File::open("tests/blank_line_file").unwrap();
        let mut rev_lines = ReverseLines::new(file).unwrap();

        assert_eq!(rev_lines.next().unwrap().unwrap(), "".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "XYZ".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "ABCD".to_string());
        assert_matches!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_multi_lines_and_with_capacity() {
        let file = File::open("tests/multi_line_file").unwrap();
        let mut rev_lines = ReverseLines::with_capacity(5, file).unwrap();

        assert_eq!(rev_lines.next().unwrap().unwrap(), "UVWXYZ".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "LMNOPQRST".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "GHIJK".to_string());
        assert_eq!(rev_lines.next().unwrap().unwrap(), "ABCDEF".to_string());
        assert_matches!(rev_lines.next(), None);
    }

    #[test]
    fn it_errors_on_invalid_utf8() {
        let file = File::open("tests/invalid_utf8").unwrap();
        let mut rev_lines = ReverseLines::with_capacity(5, file).unwrap();

        assert_eq!(rev_lines.next().unwrap().unwrap(), "Valid UTF8".to_string());
        assert_matches!(rev_lines.next().unwrap(), Err(_));
        assert_matches!(rev_lines.next(), None);
    }
}
