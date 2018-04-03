#[derive(Debug, Copy, Clone)]
pub struct Pos {
    pub chr: i32,
    pub line: i32
}

impl Pos {
    pub fn new() -> Self {
        Pos { chr: 0, line: 0 }
    }

    pub fn next_char(&mut self) {
        self.chr += 1;
    }

    pub fn next_line(&mut self) {
        self.line += 1;
        self.chr = 0;
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Range {
    pub start: Pos,
    pub end: Pos
}
