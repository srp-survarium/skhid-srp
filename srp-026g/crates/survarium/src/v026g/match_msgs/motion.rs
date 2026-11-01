//! Where a player is and how he moves, from his state in a world
//! (`docs/notes/player_bots.md` §4).
//!
//! `base_player::serialize` 0x2DAF60 ends the state with:
//!
//! ```text
//! ... u8 jump_type, f32 m_linear_horizontal_speed          0x2DB27D, 0x2DB2A5
//!     player_damage_protector + damage_model, 17 bytes       0x2E53B0
//!     body_part_parameters, one per part (20 seen)          0x32AE00
//!     usable_object_user_data: bool [+ 32 bytes]            0x32FCC0
//! u32 m_inserted_time_in_ms                                 0x2DB2F7
//! if m_is_alive: bullet_character_controller, 67 bytes      0x7F7F0 (skip at 0x2DB303)
//! u32 m_spotted_time_left_in_ms, u32 m_time_left_to_try_spot_in_ms, f32 stagger
//! u32 n, n x {u32 time, f32 stopping speed}: m_recent_hits  0x2DB385..0x2DB42F
//! bool m_respawn_disabled                                   0x2DB445
//! ```
//!
//! PROVEN in the exe; read back on the worlds of real matches. The
//! inventory before it is not walked, so the tail is read from the end
//! (the recent hits' count is tried until one fits) and the damage model
//! is found by an anchored search: its last tick time is the state's own.
//!
//! The controller's position is `m_last_pos`, in the physics' space with
//! z mirrored: the map's frame is `(x, y, -z)` (PROVEN: 0.90 ± 0.03 m above
//! the walker floor of level_05 standing, scattered without the flip). It
//! is the capsule's centre, set at the start of a physics step.

use super::players::PlayerState;

/// One player's movement state, as a world has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    pub alive: bool,
    /// The state's time (the head's), the world's own.
    pub time_ms: u32,
    pub rotation_y: f32,
    pub look_pitch: f32,
    pub actions_mask: u32,
    /// When this body was spawned (`m_inserted_time_in_ms`).
    pub inserted_time_ms: u32,
    /// `m_recent_hits`: (time, stopping speed).
    pub recent_hits: Vec<(u32, f32)>,
    /// The character controller: alive only.
    pub body: Option<Body>,
    /// `None` when the search found no single place for it.
    pub damage: Option<Damage>,
}

/// `bullet_character_controller::serialize` 0x7F7F0.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// `m_last_pos` as written: physics space, z mirrored, capsule centre.
    pub physics_position: [f32; 3],
    /// The same in the map's frame: `(x, y, -z)`. Capsule centre.
    pub position: [f32; 3],
    pub jumping: bool,
    pub input_crouch: bool,
    pub logic_crouch: bool,
    pub capsule_crouch: bool,
    pub can_stand: bool,
    pub colliding_with_player: bool,
    /// `m_is_sprinting`.
    pub sprinting: bool,
    /// Already in the map's frame (`from_bullet`).
    pub fall_and_slide_velocity: [f32; 3],
    pub supporting_surface_normal: [f32; 3],
}

impl Body {
    /// The capsule centre's height over the feet standing (PROVEN, measured).
    pub const STANDING_CENTRE: f32 = 0.9;
    /// The same crouched [GUESS: half the 1.1 m crouch capsule].
    pub const CROUCHED_CENTRE: f32 = 0.55;

    /// The feet, in the map's frame.
    pub fn feet(&self) -> [f32; 3] {
        let up = if self.capsule_crouch {
            Self::CROUCHED_CENTRE
        } else {
            Self::STANDING_CENTRE
        };
        [self.position[0], self.position[1] - up, self.position[2]]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Damage {
    pub jump_type: u8,
    /// `m_linear_horizontal_speed` [LIKELY m/s: 3.1–3.6 walking, 5.2–5.3
    /// sprinting].
    pub linear_horizontal_speed: f32,
    /// `m_last_hit_time_ms`: the world's time of the last hit that took
    /// health, of any type (a bullet, a zone, a fall, a blast):
    /// `damage_model::hit_body_part` 0x2E5080, the one damage step of every
    /// hit (`base_player::hit_impl` 0x2D9B90), writes the hit's time at
    /// 0x2E5134 when the damage the armour left is above 0.0 (PROVEN)
    /// [LIKELY 0 before the first].
    pub last_hit_time_ms: u32,
    /// `m_health` of each body part (20 in every world seen); [LIKELY]
    /// part 0 is the body's, 1.0 unhurt.
    pub part_health: Vec<f32>,
}

/// The controller block's length.
const CONTROLLER_LEN: usize = 67;
/// More recent hits than this are not tried (the game's cap is unknown).
const MAX_RECENT_HITS: usize = 64;
/// Profiles with more items than this are not tried by [`Motion::find`].
const MAX_ITEMS: usize = 64;

fn u32_at(s: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(s.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(s: &[u8], at: usize) -> Option<f32> {
    u32_at(s, at).map(f32::from_bits)
}

fn f3_at(s: &[u8], at: usize) -> Option<[f32; 3]> {
    Some([f32_at(s, at)?, f32_at(s, at + 4)?, f32_at(s, at + 8)?])
}

/// The bool writer 0x80FA0 writes 0x00 or 0xFF.
fn flag(s: &[u8], at: usize) -> Option<bool> {
    match *s.get(at)? {
        0 => Some(false),
        0xFF => Some(true),
        _ => None,
    }
}

fn unit_or_zero(v: [f32; 3]) -> bool {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    n < 1e-3 || (n - 1.0).abs() < 1e-2
}

/// The controller block at `at`; `None` if it does not look like one.
fn body_at(s: &[u8], at: usize) -> Option<Body> {
    let physics_position = f3_at(s, at)?;
    if !physics_position
        .iter()
        .all(|c| c.is_finite() && c.abs() < 1e5)
    {
        return None;
    }
    let fall = f3_at(s, at + 18)?;
    let ground = f3_at(s, at + 30)?;
    let ceiling = f3_at(s, at + 42)?;
    let slide = f3_at(s, at + 54)?;
    if ![fall, ground, ceiling, slide]
        .iter()
        .flatten()
        .all(|c| c.is_finite())
        || !unit_or_zero(ground)
        || !unit_or_zero(ceiling)
    {
        return None;
    }
    let [x, y, z] = physics_position;
    Some(Body {
        physics_position,
        position: [x, y, -z],
        jumping: flag(s, at + 12)?,
        input_crouch: flag(s, at + 13)?,
        logic_crouch: flag(s, at + 14)?,
        capsule_crouch: flag(s, at + 15)?,
        can_stand: flag(s, at + 16)?,
        colliding_with_player: flag(s, at + 17)?,
        sprinting: flag(s, at + 66)?,
        fall_and_slide_velocity: fall,
        supporting_surface_normal: ground,
    })
}

impl Motion {
    /// Reads the state of a player whose profile has `items` items.
    /// `None`: not a state of that profile, or two readings fit.
    pub fn read(state: &[u8], items: usize) -> Option<Self> {
        let head_state = PlayerState::read(state, items)?;
        let head = PlayerState::head_len(items);
        let time_ms = u32_at(state, 2 + 4 * items + 19)?;
        let len = state.len();
        flag(state, len.checked_sub(1)?)?;
        let mut found = None;
        for n in 0..=MAX_RECENT_HITS {
            // The count; the 12 bytes of spot times and stagger before it.
            let Some(count_at) = len.checked_sub(5 + 8 * n) else {
                break;
            };
            if count_at < head + 12 {
                break;
            }
            if u32_at(state, count_at)? as usize != n {
                continue;
            }
            let hits: Vec<(u32, f32)> = (0..n)
                .map(|i| {
                    let at = count_at + 4 + 8 * i;
                    (
                        u32_at(state, at).unwrap_or(0),
                        f32_at(state, at + 4).unwrap_or(f32::NAN),
                    )
                })
                .collect();
            if hits.iter().any(|(t, v)| *t > time_ms || !v.is_finite())
                || hits.windows(2).any(|w| w[0].0 > w[1].0)
            {
                continue;
            }
            let (body, inserted_at) = if head_state.alive {
                let Some(at) = (count_at - 12).checked_sub(CONTROLLER_LEN) else {
                    continue;
                };
                if at < head + 4 {
                    continue;
                }
                match body_at(state, at) {
                    Some(body) => (Some(body), at - 4),
                    None => continue,
                }
            } else {
                (None, count_at - 16)
            };
            if inserted_at < head {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(Self {
                alive: head_state.alive,
                time_ms,
                rotation_y: head_state.rotation_y,
                look_pitch: head_state.look_pitch,
                actions_mask: head_state.actions_mask,
                inserted_time_ms: u32_at(state, inserted_at)?,
                recent_hits: hits,
                body,
                damage: damage_model(state, head, inserted_at, time_ms),
            });
        }
        found
    }

    /// Reads a state whose profile is not known: the item counts whose head
    /// carries the world's `time_ms` are tried, and the reading is taken
    /// when exactly one count gives one. For tools over saved worlds; the
    /// server knows each profile.
    pub fn find(state: &[u8], time_ms: u32) -> Option<(usize, Self)> {
        let mut found = None;
        for items in 0..=MAX_ITEMS {
            if u32_at(state, 2 + 4 * items + 19) != Some(time_ms) {
                continue;
            }
            if let Some(motion) = Self::read(state, items) {
                if found.is_some() {
                    return None;
                }
                found = Some((items, motion));
            }
        }
        found
    }
}

/// From `jump_type` to the usable-object flag before
/// `m_inserted_time_in_ms`:
///
/// ```text
/// u8 jump_type, f32 m_linear_horizontal_speed
/// bool m_have_second_chance, u32 m_last_hit_time_ms              0x2E53B0
/// u32 m_last_tick_time_in_ms, u32 m_invulnerability_time_in_ms,
/// u32 m_invulnerability_attenuation
/// per part (0x32AE00): f32 m_health, u32 m_last_hit_time,
///     u8 a, a x {u8, u32}, u8 h, h x {u32 hit id, f32 damage}, f32 movement modifier
/// usable_object_user_data 0x32FCC0: bool, if set u32 + f3 + f3 + f32
/// ```
///
/// The start is the one place whose last tick time is the state's and
/// whose parts end exactly at the usable-object flag.
fn damage_model(s: &[u8], head: usize, inserted_at: usize, now: u32) -> Option<Damage> {
    let mut found = None;
    for start in head..inserted_at.saturating_sub(23) {
        if u32_at(s, start + 10)? != now || flag(s, start + 5).is_none() {
            continue;
        }
        let mut at = start + 22;
        let mut parts = Vec::new();
        let ends = loop {
            // The usable-object flag: clear right before the time, or set
            // with its 32 bytes.
            if (at + 1 == inserted_at && s[at] == 0) || (at + 33 == inserted_at && s[at] == 0xFF) {
                break true;
            }
            if at + 9 > inserted_at {
                break false;
            }
            let health = f32_at(s, at)?;
            if !(health.is_finite() && (0.0..=1000.0).contains(&health)) {
                break false;
            }
            at += 9 + 5 * usize::from(s[at + 8]);
            if at >= inserted_at {
                break false;
            }
            at += 1 + 8 * usize::from(s[at]) + 4;
            parts.push(health);
        };
        if !ends || parts.is_empty() {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(Damage {
            jump_type: s[start],
            linear_horizontal_speed: f32_at(s, start + 1)?,
            last_hit_time_ms: u32_at(s, start + 6)?,
            part_health: parts,
        });
    }
    found
}

/// The end of a state as [`Motion::read`] reads it, for tests and tools
/// ([`state_bytes`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Tail {
    /// The state's time (written into the head too).
    pub time_ms: u32,
    pub speed: f32,
    pub health: f32,
    pub inserted_time_ms: u32,
    /// The controller's `m_last_pos` (physics frame); `None`: dead.
    pub physics_position: Option<[f32; 3]>,
    pub crouching: bool,
    pub sprinting: bool,
    pub recent_hits: Vec<(u32, f32)>,
}

/// A whole player state for tests and tools: the head (its time set to the
/// tail's), an opaque inventory, and the tail as the game writes it
/// (20 body parts, no usable object).
pub fn state_bytes(head: &PlayerState, inventory: &[u8], tail: &Tail) -> Vec<u8> {
    let items = head.items.len();
    let mut out = super::players::state_bytes(head, inventory);
    let now = tail.time_ms;
    out[2 + 4 * items + 19..2 + 4 * items + 23].copy_from_slice(&now.to_le_bytes());
    // Damage model.
    out.push(2); // jump_type
    out.extend(tail.speed.to_le_bytes());
    out.push(0); // second chance
    let last_hit = tail.recent_hits.iter().map(|&(time, _)| time).max();
    out.extend(last_hit.unwrap_or(0).to_le_bytes()); // last hit
    out.extend(now.to_le_bytes()); // last tick
    out.extend([0; 8]); // invulnerability
    for part in 0..20 {
        out.extend(if part == 0 { tail.health } else { 1.0f32 }.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.push(1); // one {u8, u32}
        out.extend([3, 0, 0, 0, 0]);
        out.push(0); // no hits
        out.extend(1.0f32.to_le_bytes());
    }
    out.push(0); // no usable object
    out.extend(tail.inserted_time_ms.to_le_bytes());
    if let Some(position) = tail.physics_position {
        for v in position {
            out.extend(v.to_le_bytes());
        }
        let crouch = if tail.crouching { 0xFF } else { 0 };
        out.extend([0, crouch, crouch, crouch, 0xFF, 0]);
        for v in [
            0.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ] {
            out.extend(v.to_le_bytes());
        }
        out.push(if tail.sprinting { 0xFF } else { 0 });
    }
    out.extend([0; 12]); // spot times, stagger
    out.extend((tail.recent_hits.len() as u32).to_le_bytes());
    for (time, speed) in &tail.recent_hits {
        out.extend(time.to_le_bytes());
        out.extend(speed.to_le_bytes());
    }
    out.push(0); // respawn not disabled
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u32 = 104_550;

    fn head(alive: bool, items: usize) -> PlayerState {
        PlayerState {
            current_index: 0,
            target_index: 0,
            items: (0..items as u32).map(|item| 1000 + item).collect(),
            alive,
            has_to_die: false,
            look_pitch: -0.25,
            rotation_y: 1.5,
            actions_mask: 0x21,
            effects: 0,
        }
    }

    /// A state with an opaque inventory, then the tail as the game writes
    /// it: 20 body parts, part 0 at `health`, the controller if alive,
    /// `hits`.
    fn state(alive: bool, items: usize, health: f32, hits: &[(u32, f32)]) -> Vec<u8> {
        state_bytes(
            &head(alive, items),
            &[7; 40],
            &Tail {
                time_ms: NOW,
                speed: 3.4,
                health,
                inserted_time_ms: 90_000,
                physics_position: alive.then_some([10.0, 1.9, -20.0]),
                crouching: true,
                sprinting: false,
                recent_hits: hits.to_vec(),
            },
        )
    }

    #[test]
    fn a_living_players_position_is_read_from_the_end() {
        let bytes = state(true, 5, 0.5, &[(100_000, 0.3), (104_000, 0.2)]);
        let motion = Motion::read(&bytes, 5).unwrap();
        assert!(motion.alive);
        assert_eq!(motion.time_ms, NOW);
        assert_eq!(motion.rotation_y, 1.5);
        assert_eq!(motion.inserted_time_ms, 90_000);
        assert_eq!(motion.recent_hits, vec![(100_000, 0.3), (104_000, 0.2)]);
        let body = motion.body.unwrap();
        // z mirrored into the map's frame; crouched.
        assert_eq!(body.position, [10.0, 1.9, 20.0]);
        assert!(body.capsule_crouch && !body.sprinting);
        assert!((body.feet()[1] - 1.35).abs() < 1e-5);
        let damage = motion.damage.unwrap();
        assert_eq!(damage.jump_type, 2);
        assert_eq!(damage.linear_horizontal_speed, 3.4);
        assert_eq!(damage.last_hit_time_ms, 104_000);
        assert_eq!(damage.part_health.len(), 20);
        assert_eq!(damage.part_health[0], 0.5);
    }

    #[test]
    fn a_dead_player_has_no_controller() {
        let bytes = state(false, 3, 0.0, &[]);
        let motion = Motion::read(&bytes, 3).unwrap();
        assert!(!motion.alive);
        assert_eq!(motion.body, None);
        assert_eq!(motion.inserted_time_ms, 90_000);
        assert_eq!(motion.damage.unwrap().part_health[0], 0.0);
    }

    #[test]
    fn the_profiles_item_count_is_found_by_the_worlds_time() {
        let bytes = state(true, 7, 1.0, &[]);
        let (items, motion) = Motion::find(&bytes, NOW).unwrap();
        assert_eq!(items, 7);
        assert_eq!(motion.body.unwrap().position[0], 10.0);
        assert_eq!(Motion::find(&bytes, NOW + 50), None);
    }

    #[test]
    fn bytes_that_do_not_end_a_state_are_not_read() {
        let mut bytes = state(true, 5, 1.0, &[]);
        *bytes.last_mut().unwrap() = 1; // no bool
        assert_eq!(Motion::read(&bytes, 5), None);
        assert_eq!(Motion::read(&[0; 10], 5), None);
    }
}
