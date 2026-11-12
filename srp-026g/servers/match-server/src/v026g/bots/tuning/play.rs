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

/// By play; a play missing here runs on its defaults (`plays::default_tuning`). In a file, an object of plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlaysTuning(pub BTreeMap<PlayKind, PlayTuning>);

impl Default for PlaysTuning {
    fn default() -> Self {
        Self(PlayKind::ALL.iter().map(|&kind| (kind, plays::default_tuning(kind))).collect())
    }
}

impl std::ops::Deref for PlaysTuning {
    type Target = BTreeMap<PlayKind, PlayTuning>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PlaysTuning {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Check for PlaysTuning {
    /// Each play's numbers known to it.
    fn check(&self) -> Result<(), String> {
        for (kind, play) in &self.0 {
            for name in play.numbers.keys() {
                check(
                    plays::numbers(*kind).iter().any(|(known, _)| known == name),
                    &format!("plays.{kind:?}.numbers.{name}: unknown"),
                )?;
            }
        }
        Ok(())
    }
}
