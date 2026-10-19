/// Represents a bit-accurate hardware value in the simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimValue {
    pub bits: u128,
    pub width: u32,
    pub is_signed: bool,
}

impl SimValue {
    pub fn new(bits: u128, width: u32, is_signed: bool) -> Self {
        let w = width.clamp(1, 128);
        Self {
            bits: bits & bit_mask(w),
            width: w,
            is_signed,
        }
    }

    pub fn zero(width: u32, is_signed: bool) -> Self {
        Self::new(0, width, is_signed)
    }

    pub fn from_bool(b: bool) -> Self {
        Self::new(u128::from(b), 1, false)
    }

    pub fn as_bool(self) -> bool {
        self.bits != 0
    }

    pub fn as_signed(self) -> i128 {
        if self.width >= 128 {
            self.bits as i128
        } else {
            let sign_bit = 1u128 << (self.width - 1);
            if (self.bits & sign_bit) != 0 {
                let mask = !bit_mask(self.width);
                (self.bits | mask) as i128
            } else {
                self.bits as i128
            }
        }
    }

    /// Formats the value in IEEE 1364 VCD change format (`0!`, `1!`, or `b1010 !`).
    pub fn format_vcd(self, vcd_id: &str) -> String {
        if self.width <= 1 {
            let bit_char = if self.bits & 1 != 0 { '1' } else { '0' };
            format!("{}{}", bit_char, vcd_id)
        } else {
            let w = self.width as usize;
            format!("b{:0width$b} {}", self.bits, vcd_id, width = w)
        }
    }
}

pub(crate) fn bit_mask(width: u32) -> u128 {
    if width == 0 {
        0
    } else if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

/// Kind of signal within an elaborated module instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    Input,
    Output,
    Register,
    Wire,
}

/// Metadata for a signal registered in a module instance.
#[derive(Debug, Clone)]
pub struct SignalMeta {
    pub name: String,
    pub vcd_name: String,
    pub width: u32,
    pub is_signed: bool,
    pub kind: SignalKind,
    pub vcd_id: String,
}
