// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed record only. RND1 does NOT authorize a native slot-8 wire signature.
//! Reference tokens here are portable opaque IDs, not acquired native pointers.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub width_bits: u32,
    pub height_bits: u32,
    pub rect_bits: [u32; 4],
    pub camera_control_token: Option<u64>,
    pub camera_derived: u32,
    pub camera_view: u32,
    pub camera_mode_bits: u32,
}
pub trait Input {
    fn version(&mut self) -> u16;
    fn word(&mut self) -> u32;
    fn camera_control_token(&mut self, class: u32) -> Option<u64>;
    fn error(&self) -> bool;
}
pub trait Output {
    fn version(&mut self, value: u16);
    fn word(&mut self, bits: u32);
    fn camera_control_token(&mut self, class: u32, token: Option<u64>);
    fn error(&self) -> bool;
}
impl Record {
    /// Only version mismatch stops early. Sticky stream errors are checked at
    /// the end, retaining all sequential field writes; no invented rollback.
    pub fn read(&mut self, input: &mut dyn Input) -> bool {
        if input.version() != 2 {
            return false;
        }
        self.width_bits = input.word();
        self.height_bits = input.word();
        for bits in &mut self.rect_bits {
            *bits = input.word();
        }
        self.camera_control_token = input.camera_control_token(0xc9c628ec);
        self.camera_derived = input.word();
        self.camera_view = input.word();
        self.camera_mode_bits = input.word();
        !input.error()
    }
    pub fn write(&self, output: &mut dyn Output) -> bool {
        output.version(2);
        output.word(self.width_bits);
        output.word(self.height_bits);
        for bits in self.rect_bits {
            output.word(bits);
        }
        output.camera_control_token(0xc9c628ec, self.camera_control_token);
        output.word(self.camera_derived);
        output.word(self.camera_view);
        output.word(self.camera_mode_bits);
        !output.error()
    }
}
