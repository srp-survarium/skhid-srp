//! 0.69d's slot ids, a closed set (`slot_ids.rs`, generated).

macro_rules! slot_ids {
    ($($variant:ident = $number:literal, $name:literal;)*) => {
        /// A slot of a gun or of a module on it, by 0.69d0's own id
        /// (`m24x15_muzzle`, `picatinny_grip`, `forend`, ...). The number is
        /// the table's byte.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum SlotId {
            $($variant = $number,)*
        }

        impl SlotId {
            /// Every slot id, by its number.
            pub const ALL: &'static [SlotId] = &[$(SlotId::$variant,)*];

            /// 0.69d's id.
            pub fn name(self) -> &'static str {
                match self {
                    $(SlotId::$variant => $name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<SlotId> {
                match name {
                    $($name => Some(SlotId::$variant),)*
                    _ => None,
                }
            }

            pub fn from_byte(byte: u8) -> Option<SlotId> {
                match byte {
                    $($number => Some(SlotId::$variant),)*
                    _ => None,
                }
            }

            pub fn byte(self) -> u8 {
                self as u8
            }
        }
    };
}

include!("slot_ids.rs");
