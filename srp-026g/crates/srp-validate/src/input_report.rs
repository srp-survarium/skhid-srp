//! Client-report kind 3, one cumulative foreground-input summary per moment.
//! One moment byte (as for modules), then three little-endian u32 counters.
use crate::modules::{Minute, Moment};

pub(crate) const SIZE: usize = 13;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub key_down: u32,
    pub button_down: u32,
    pub mouse_move: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Minute(Minute),
    Ended,
}

impl When {
    pub fn wire(self) -> Moment {
        match self {
            Self::Minute(minute) => Moment::Minute(minute),
            Self::Ended => Moment::Ended,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub moment: When,
    pub counts: Counts,
}

impl Summary {
    pub(crate) fn write(self, body: &mut [u8]) -> Option<usize> {
        let (byte, counts) = body.get_mut(..SIZE)?.split_first_mut()?;
        *byte = self.moment.wire().byte();
        for (out, value) in
            counts
                .chunks_exact_mut(4)
                .zip([self.counts.key_down, self.counts.button_down, self.counts.mouse_move])
        {
            out.copy_from_slice(&value.to_le_bytes());
        }
        Some(SIZE)
    }

    pub(crate) fn parse(body: &[u8]) -> Option<Self> {
        let (byte, rest) = body.split_first()?;
        let moment = match Moment::of(*byte) {
            Moment::Ended => When::Ended,
            Moment::Minute(minute) => When::Minute(minute),
            Moment::Loaded => return None,
        };
        let mut words = rest.chunks_exact(4);
        let mut next = || Some(u32::from_le_bytes(words.next()?.try_into().ok()?));
        Some(Self {
            moment,
            counts: Counts {
                key_down: next()?,
                button_down: next()?,
                mouse_move: next()?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Body, Carrier, Report, carrier_len};

    #[test]
    fn one_part_with_distinct_retry_keys_and_fixed_length() {
        for when in [When::Minute(Minute::FIRST), When::Minute(Minute::LAST), When::Ended] {
            let summary = Summary {
                moment: when,
                counts: Counts {
                    key_down: 1,
                    button_down: 511,
                    mouse_move: u32::MAX,
                },
            };
            let report = Report::Input(summary);
            let (mut bytes, len) = report.part(7, 0).unwrap();
            let carrier = Carrier::parse(&bytes[..len]).unwrap();
            assert_eq!(carrier.body, Body::Input(summary));
            assert_eq!(carrier.key(), (3, when.wire().byte(), 0));
            assert_eq!(carrier_len(&bytes), Some(len));
            assert!((0..len).all(|cut| Carrier::parse(&bytes[..cut]).is_none()));
            assert!(Carrier::parse(&bytes[..len + 1]).is_none());
            assert!(report.part(7, 1).is_none());
            bytes[14] = 2;
            assert!(Carrier::parse(&bytes[..len]).is_none());
            bytes[14] = 1;
            bytes[15] = 0;
            assert!(Carrier::parse(&bytes[..len]).is_none());
        }
    }
}
