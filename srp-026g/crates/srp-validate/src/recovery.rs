//! A receipt for a world belongs only to its receiving connection. It never
//! accuses another player. The impossible E0 chunk prefix separates it from
//! uploads and the growing-roster resource acknowledgement.

pub const SIZE: usize = 20;
pub const MAGIC: [u8; 8] = [255, 255, 255, 255, 255, 255, 1, 0];
pub const CONNECT_CAPABILITY: u8 = 0x82;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Loaded,
    Rejected,
    OptionsRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub match_id: u32,
    pub generation: u8,
    pub outcome: Outcome,
}

impl Receipt {
    pub fn encode(self) -> [u8; SIZE] {
        let mut bytes = [0; SIZE];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..12].copy_from_slice(&self.match_id.to_le_bytes());
        bytes[12] = self.generation;
        bytes[13] = match self.outcome { Outcome::Loaded => 0, Outcome::Rejected => 1, Outcome::OptionsRejected => 2 };
        bytes[14] = 1;
        bytes
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; SIZE] = bytes.try_into().ok()?;
        if bytes[..8] != MAGIC || bytes[14..] != [1, 0, 0, 0, 0, 0] {
            return None;
        }
        Some(Self {
            match_id: u32::from_le_bytes(bytes[8..12].try_into().ok()?),
            generation: bytes[12],
            outcome: match bytes[13] { 0 => Outcome::Loaded, 1 => Outcome::Rejected, 2 => Outcome::OptionsRejected, _ => return None },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_are_exact_and_separate_from_uploads_and_resource_acks() {
        for outcome in [Outcome::Loaded, Outcome::Rejected, Outcome::OptionsRejected] {
            for generation in [0, 127, 128, 255] {
                let receipt = Receipt { match_id: 0x81234567, generation, outcome };
                let bytes = receipt.encode();
                assert_eq!(Receipt::parse(&bytes), Some(receipt));
                for len in 0..SIZE { assert_eq!(Receipt::parse(&bytes[..len]), None); }
                for at in [0, 5, 6, 7, 13, 14, 15, 16, 19] {
                    let mut bad = bytes; bad[at] ^= 0x80;
                    assert_eq!(Receipt::parse(&bad), None);
                }
                let mut ack = bytes; ack[6] = 0;
                assert_eq!(Receipt::parse(&ack), None);
            }
        }
    }
}
