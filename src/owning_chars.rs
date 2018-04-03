// An iterator over the characters of a String that also owns that String.
// Credit to https://stackoverflow.com/a/43958470 for this idea.

use std::iter::Peekable;
use std::mem;
use std::str::Chars;

pub struct OwningChars {
    // `string` is never used, but it must be kept around so that the Chars are
    // valid.
    #[allow(dead_code)]
    string: String,
    chars: Peekable<Chars<'static>>
}

impl OwningChars {
    pub fn new(string: String) -> Self {
        // This is safe because the interface gives no way to drop the
        // string separately from the related "chars": the "chars" always
        // live at most as long as the string.
        let chars: Chars<'static> = unsafe {
            mem::transmute(string.chars())
        };
        OwningChars {
            string,
            chars: chars.peekable()
        }
    }

    pub fn peek(&mut self) -> Option<char> {
        return self.chars.peek().map(|e| *e);
    }
}

impl Iterator for OwningChars {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }
}
