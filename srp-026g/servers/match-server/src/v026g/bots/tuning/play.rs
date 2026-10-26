use super::*;

/// One play's tuning: its rank band, its considerations, and its numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayTuning {
    pub rank: Rank,
    pub considerations: Vec<Consideration>,
    /// The play's own numbers by name (`plays::<play>::NUMBERS` lists the
    /// names and defaults; another name is refused).
    #[serde(default)]
    pub numbers: BTreeMap<String, f32>,
}
