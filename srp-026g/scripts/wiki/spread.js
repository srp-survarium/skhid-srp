// Model of 0.26g weapon spread (dispersion_calculator, weapon +3F8h), from
// the exe (re-kit rows of these addresses):
// - a bullet's spread D = base + k x (A + B) (get_dispersed_bullet_direction
//   0x2E1D50), base = base_dispersion x the round's dispersion
//   (get_static_dispersion 0x331220), k = 0 fully aimed, else 1;
// - A, the stance's part (tick 0x331330): apply(m x S + injury, dispersion,
//   C), m = aim_multiplier from the start of aiming in (m_aimed +5E8h), else
//   from_the_hip_multiplier; S the stance's value (get_target_koef
//   0x331560), injury the broken arms' (0x3317C0), C the stance's skill
//   weight (0x331480). A runs toward it; here the stance is held, so A is it;
// - B, the shots' part (fire 0x331150): apply(one_shoot_dispersion_amount,
//   dispersion, C) a shot, at most max_dispersion, falling at speed_of_aiming
//   a second;
// - apply(v, S, w) = v x max(0, 1 + w x S) (apply_modifier 0x2D5AE0; several
//   modifiers at once, apply_modifiers 0x2D5BC0: their sums added), S the
//   kit's sum of a player modifier: skills, armour, by its name;
// - base_dispersion is the weapon's cooked config's (weapon +3D8h, loaded by
//   0x330EE0 in the weapon's constructor 0x2DF0D0): the config's value x
//   max(0, 1 + the sum of the weapon's Dispersion modifications on
//   dispersion/base_dispersion, type 2, 0x358010). Only the base: the
//   stance's and the shots' parts take none of it.
(() => {
  const STANCES = ['idle', 'walk', 'crouch', 'crouch_walk', 'run', 'jump'];
  const AIMS = ['hip', 'aiming', 'aimed'];
  // Sprinting and jumping have no aimed values or weights.
  const AIMED = new Set(['idle', 'walk', 'crouch', 'crouch_walk']);
  const scale = (value, sum, weight = 1) => value * Math.max(0, 1 + weight * sum);
  const finite = (object, keys) => keys.every(key => typeof object?.[key] === 'number' && Number.isFinite(object[key]));
  // The weapon's section as the cook leaves it: `sum` its modifications'
  // sum on dispersion/base_dispersion (-0.1 for a -10 % roll).
  function rolled(dispersion, sum = 0) {
    if (typeof dispersion?.base_dispersion !== 'number') return dispersion;
    return {...dispersion, base_dispersion: dispersion.base_dispersion * Math.max(0, 1 + sum)};
  }
  // `dispersion`: the weapon's section; `player`: {stance, skill_influence},
  // default.player's character_dispersion_params and _skill_influence.
  function parse(dispersion, player, doubleHanded) {
    const values = [...STANCES.map(s => `${s}_dispersion`), ...[...AIMED].map(s => `${s}_aim_dispersion`),
      'injury_penalty_for_double_handed', 'injury_penalty_for_one_handed'];
    const weights = [...STANCES.map(s => `${s}_skill_influence`), ...[...AIMED].map(s => `${s}_aim_skill_influence`)];
    if (!finite(dispersion, ['base_dispersion', 'one_shoot_dispersion_amount', 'max_dispersion', 'speed_of_aiming']) ||
        !finite(player?.stance, values) || !finite(player?.skill_influence, weights) || typeof doubleHanded !== 'boolean') return null;
    return Object.freeze({base: dispersion.base_dispersion, perShot: dispersion.one_shoot_dispersion_amount,
      max: dispersion.max_dispersion, recovery: dispersion.speed_of_aiming,
      // The exe's defaults for a weapon without them.
      hip: dispersion.from_the_hip_multiplier ?? 1, aim: dispersion.aim_multiplier ?? 1,
      doubleHanded, stance: player.stance, weight: player.skill_influence});
  }
  // The stance's S (get_target_koef): from the hip through dispersion_from_hip,
  // walking also dispersion_movement_influence; aimed as is, but walking.
  function stanceValue(model, stance, aimed, kit) {
    const p = model.stance, hip = kit.dispersion_from_hip, moving = kit.dispersion_movement_influence;
    if (stance === 'walk') return aimed ? scale(p.walk_aim_dispersion, moving) : scale(p.walk_dispersion, moving + hip);
    if (aimed && AIMED.has(stance)) return p[`${stance}_aim_dispersion`];
    return scale(p[`${stance}_dispersion`], hip);
  }
  function injury(model, arms, kit) {
    const p = model.stance;
    const penalty = arms === 0 ? 0 : model.doubleHanded ? p.injury_penalty_for_double_handed
      : arms === 2 ? p.injury_penalty_for_one_handed : 0;
    return scale(penalty, kit.injury_penalty);
  }
  // A kit's sums of the player modifiers (the spread's, and recoil's: 18, a
  // shot's kick, recoil.js), from its sources' {name: value}.
  function kitOf(sources) {
    const kit = {dispersion: 0, aiming_speed: 0, dispersion_movement_influence: 0, dispersion_from_hip: 0, injury_penalty: 0, recoil: 0};
    for (const source of sources) for (const [name, value] of Object.entries(source)) if (name in kit) kit[name] += value;
    return Object.freeze(kit);
  }
  // The spread while firing in `aim` (AIMS), `stance` (STANCES), with `kit`
  // (kitOf) and `arms` broken (0..2): {base, k, stance: A, perShot, max, recovery}.
  function condition(model, {aim, stance, kit, arms}) {
    if (!AIMS.includes(aim) || !STANCES.includes(stance) || ![0, 1, 2].includes(arms)) throw Error(`no such state: ${aim} ${stance} ${arms}`);
    const aimed = aim !== 'hip';
    const weight = model.weight[aimed && AIMED.has(stance) ? `${stance}_aim_skill_influence` : `${stance}_skill_influence`];
    const target = (aimed ? model.aim : model.hip) * stanceValue(model, stance, aimed, kit) + injury(model, arms, kit);
    return Object.freeze({aim, base: model.base, k: aim === 'aimed' ? 0 : 1, stance: scale(target, kit.dispersion, weight),
      perShot: scale(model.perShot, kit.dispersion, weight), max: model.max, recovery: model.recovery});
  }
  globalThis.WeaponSpread = Object.freeze({STANCES, AIMS, parse, rolled, kitOf, condition});
})();
