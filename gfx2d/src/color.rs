#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Color([u8; 4]);

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color([r, g, b, 255])
}

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color([r, g, b, a])
}

impl Color {
    pub fn r(&self) -> u8 {
        self.0[0]
    }

    pub fn g(&self) -> u8 {
        self.0[1]
    }

    pub fn b(&self) -> u8 {
        self.0[2]
    }

    pub fn a(&self) -> u8 {
        self.0[3]
    }

    pub fn set_r(&mut self, value: u8) {
        self.0[0] = value;
    }

    pub fn set_g(&mut self, value: u8) {
        self.0[1] = value;
    }

    pub fn set_b(&mut self, value: u8) {
        self.0[2] = value;
    }

    pub fn set_a(&mut self, value: u8) {
        self.0[3] = value;
    }
}

impl From<Color> for [u8; 4] {
    fn from(color: Color) -> [u8; 4] {
        color.0
    }
}
