// An iterator over the characters of a String that also owns that String.
// Credit to https://stackoverflow.com/a/43958470 for this idea.

use std::mem;
use std::str;

pub struct OwningChars {
    // `string` is never used, but it must be kept around so that the Chars are
    // valid.
    #[allow(dead_code)]
    string: String,
    chars: str::Chars<'static>
}

impl OwningChars {
    pub fn new(string: String) -> Self {
        let chars = unsafe {
            mem::transmute(string.chars())
        };
        OwningChars {
            string,
            // This is safe because the interface gives no way to drop the
            // string separately from the related "chars": the "chars" always
            // live at most as long as the string.
            chars
        }
    }
}

impl Iterator for OwningChars {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }
}
