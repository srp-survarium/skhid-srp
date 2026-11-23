// The page's own words in the reader's language (site-texts/README.md): the web server fills the
// page's <script type="application/json" id="page-words"> with every text its script asks for
// with T, Tc (a context first) and Tf (a template and its places), from words/<code>.tsv. Each
// takes its English text as a literal (the server and the words test find them so); English is
// what a text has no row for.
const PAGE_WORDS = JSON.parse(document.getElementById("page-words").textContent);
// (None calls another: each call in a script is a text's.)
const Tc = (context, english) => PAGE_WORDS[context]?.[english] ?? english;
const T = english => PAGE_WORDS[""]?.[english] ?? english;
const Tf = (english, values) => (PAGE_WORDS[""]?.[english] ?? english)
  .replace(/\{(\w+)\}/g, (place, name) => (name in values ? String(values[name]) : place));

globalThis.CAMERA_SHAKE = {"period_frames":225,"pitch":[-0.252,-0.238,-0.189,-0.107,-0.023,0.043,0.09,0.122,0.143,0.151,0.158,0.179,0.209,0.241,0.267,0.287,0.301,0.312,0.319,0.323,0.325,0.286,0.219,0.158,0.108,0.068,0.039,0.018,0.004,-0.004,-0.007,0.018,0.052,0.088,0.118,0.141,0.158,0.17,0.178,0.182,0.185,0.142,0.064,-0.005,-0.063,-0.109,-0.143,-0.167,-0.182,-0.191,-0.195,-0.175,-0.145,-0.117,-0.094,-0.076,-0.063,-0.053,-0.047,-0.044,-0.043,-0.043,-0.044,-0.045,-0.046,-0.048,-0.049,-0.049,-0.049,-0.049,-0.05,-0.056,-0.065,-0.074,-0.081,-0.087,-0.091,-0.093,-0.095,-0.096,-0.098,-0.099,-0.1,-0.101,-0.102,-0.103,-0.104,-0.105,-0.106,-0.106,-0.106,-0.083,-0.046,-0.012,0.016,0.038,0.054,0.066,0.074,0.078,0.08,0.028,-0.063,-0.145,-0.214,-0.267,-0.307,-0.336,-0.355,-0.366,-0.37,-0.28,-0.125,0.019,0.136,0.227,0.297,0.348,0.38,0.397,0.403,0.402,0.398,0.393,0.389,0.385,0.382,0.379,0.378,0.378,0.38,0.382,0.387,0.392,0.396,0.4,0.402,0.404,0.405,0.406,0.406,0.306,0.144,-0.007,-0.127,-0.221,-0.292,-0.344,-0.378,-0.397,-0.403,-0.372,-0.319,-0.271,-0.231,-0.2,-0.177,-0.16,-0.149,-0.143,-0.14,-0.173,-0.228,-0.278,-0.32,-0.352,-0.377,-0.395,-0.406,-0.413,-0.415,-0.307,-0.124,0.047,0.185,0.29,0.371,0.43,0.468,0.489,0.497,0.408,0.253,0.109,-0.007,-0.097,-0.165,-0.215,-0.248,-0.266,-0.273,-0.206,-0.088,0.019,0.107,0.176,0.228,0.265,0.289,0.302,0.308,0.244,0.135,0.034,-0.049,-0.114,-0.163,-0.198,-0.221,-0.234,-0.24,-0.243,-0.244,-0.246,-0.247,-0.249,-0.25,-0.251,-0.251,-0.252,-0.252,-0.253,-0.253,-0.253,-0.252],"yaw":[0.081,0.069,0.027,-0.043,-0.115,-0.174,-0.216,-0.245,-0.262,-0.271,-0.275,-0.251,-0.215,-0.181,-0.155,-0.136,-0.121,-0.111,-0.105,-0.101,-0.1,-0.069,-0.014,0.035,0.075,0.107,0.13,0.147,0.158,0.164,0.167,0.187,0.22,0.251,0.277,0.297,0.313,0.323,0.33,0.334,0.338,0.343,0.349,0.357,0.364,0.368,0.37,0.372,0.373,0.374,0.375,0.377,0.379,0.38,0.379,0.378,0.375,0.373,0.372,0.372,0.373,0.374,0.376,0.377,0.376,0.375,0.373,0.37,0.369,0.369,0.368,0.355,0.335,0.317,0.301,0.289,0.279,0.273,0.269,0.267,0.263,0.251,0.224,0.197,0.173,0.153,0.137,0.125,0.117,0.113,0.11,0.108,0.104,0.097,0.088,0.08,0.074,0.07,0.066,0.064,0.062,0.061,0.058,0.054,0.05,0.046,0.043,0.041,0.041,0.041,0.037,0.022,-0.005,-0.033,-0.056,-0.073,-0.087,-0.097,-0.104,-0.106,-0.11,-0.127,-0.157,-0.188,-0.215,-0.237,-0.254,-0.267,-0.275,-0.28,-0.28,-0.274,-0.262,-0.25,-0.241,-0.235,-0.23,-0.227,-0.224,-0.223,-0.224,-0.226,-0.229,-0.232,-0.235,-0.237,-0.239,-0.24,-0.241,-0.241,-0.243,-0.258,-0.282,-0.305,-0.326,-0.342,-0.353,-0.361,-0.369,-0.372,-0.37,-0.364,-0.355,-0.346,-0.339,-0.336,-0.334,-0.332,-0.331,-0.331,-0.33,-0.316,-0.298,-0.28,-0.266,-0.258,-0.251,-0.246,-0.243,-0.243,-0.239,-0.223,-0.197,-0.172,-0.15,-0.133,-0.121,-0.112,-0.107,-0.103,-0.1,-0.096,-0.09,-0.085,-0.084,-0.085,-0.087,-0.088,-0.089,-0.089,-0.09,-0.064,-0.027,0.007,0.032,0.051,0.063,0.072,0.077,0.08,0.082,0.083,0.084,0.084,0.084,0.083,0.082,0.082,0.081,0.081,0.081,0.081,0.081,0.081,0.081]};
// Model of 0.26g recoil: shot 0x331C50, ramp 0x331F82, tick 0x331A50,
// recovery 0x3321E0. Matches bots/recoil.rs at 50 ms ticks. With it, from
// the exe (re-kit rows of those addresses):
// - the camera shake, a third recoil axis `z` run like the climb (a draw in
//   [min_shake_recoil, max_shake_recoil] per shot, the same PID back to 0)
//   weighting the stock clip camerashake_v2 (CAMERA_SHAKE, degrees), which
//   only plays while firing from the hip, at shake_time_scale x 30 frames/s;
// - each axis's step a shot (the kick and the shake) times max(0, 1 + S18),
//   the kit's recoil modifier (apply_modifier, index 12h, weight 1, at
//   0x331DC7, 0x331EA5 and 0x331F5A); the ramp follows it, the recovery not;
// - the spread (spread.js: the state's parts, and the shots' part run here
//   at the ticks); a bullet tilts 0.5 x D x frac(N(0, 0.5)) degrees at a
//   uniform angle (get_dispersed_bullet_direction 0x2E1D50).
// Random draws are this file's own seeded ones, not the game's generator;
// the kick's draws are the same with or without spread and shake.
(() => {
  const TOP = 63.735, SIDE = 21.48, TICK = 50;
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const finite = (object, keys) => keys.every(key => typeof object[key] === 'number' && Number.isFinite(object[key]));
  function parseShake(raw) {
    if (!finite(raw, ['min_shake_recoil', 'max_shake_recoil', 'shake_time_scale'])) return null;
    return Object.freeze({range: [raw.min_shake_recoil, raw.max_shake_recoil], timeScale: raw.shake_time_scale});
  }
  function parse(raw) {
    const required = ['rpm', 'magazine', 'recoil_power', 'min_top_recoil', 'max_top_recoil',
      'min_left_recoil', 'max_left_recoil', 'min_right_recoil', 'max_right_recoil',
      'compensation_proportional_gain', 'compensation_integral_gain', 'compensation_derivational_gain'];
    if (!raw || required.some(k => typeof raw[k] !== 'number' || !Number.isFinite(raw[k])) ||
        raw.rpm <= 0 || raw.magazine < 1 || raw.recoil_power < 0) return null;
    for (const axis of ['top', 'left', 'right']) if (raw[`min_${axis}_recoil`] > raw[`max_${axis}_recoil`]) return null;
    return Object.freeze({interval: 60000 / raw.rpm, magazine: Math.floor(raw.magazine), power: raw.recoil_power,
      top: [raw.min_top_recoil, raw.max_top_recoil], left: [raw.min_left_recoil, raw.max_left_recoil],
      right: [raw.min_right_recoil, raw.max_right_recoil],
      gains: [raw.compensation_proportional_gain, raw.compensation_integral_gain, raw.compensation_derivational_gain],
      shake: parseShake(raw)});
  }
  function generator(seed) {
    let state = seed >>> 0 || 1;
    return () => {
      state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
      return (state >>> 0) / 4294967296;
    };
  }
  // The shake clip's view turn at `frame` (fractional, looping): [side, up] degrees.
  function shakeAt(frame) {
    const clip = globalThis.CAMERA_SHAKE;
    if (!clip) return [0, 0];
    const n = clip.period_frames, at = ((frame % n) + n) % n, i = Math.floor(at), f = at - i, j = (i + 1) % n;
    return [clip.yaw[i] + (clip.yaw[j] - clip.yaw[i]) * f, clip.pitch[i] + (clip.pitch[j] - clip.pitch[i]) * f];
  }
  // `firing`: `aim` (WeaponSpread.AIMS; the shake plays from the hip only),
  // `spread` (WeaponSpread.condition, or null for none) and `recoil`, the
  // kit's recoil modifier sum (S18). `click`:
  // seconds between clicks for single shots (never faster than the weapon
  // fires), 0 to hold the trigger. The modes share every rule (the configs
  // have none per mode): only the shots' pace differs; a shot's kick still
  // ramps over 60/rpm.
  function simulate(model, requestedShots, seed = 1, {aim = 'aimed', spread = null, recoil = 0} = {}, click = 0) {
    const hip = aim === 'hip', step = Math.max(0, 1 + recoil);
    const spacing = Math.max(model.interval, click * 1000);
    const shots = Math.min(model.magazine, Math.max(1, Math.floor(requestedShots)));
    const random = generator(seed), extra = generator((seed ^ 0x9e3779b9) >>> 0);
    const draw = range => range[0] + (range[1] - range[0]) * random();
    // N(0, sigma) by Box-Muller, from the second stream.
    const normal = sigma => Math.sqrt(-2 * Math.log(1 - extra())) * Math.cos(2 * Math.PI * extra()) * sigma;
    let state = {kind: 'rest'};
    // Axes: side, top, shake.
    const at = [0, 0, 0], speed = [0, 0, 0], error = [0, 0, 0], integral = [0, 0, 0];
    const frames = [], events = [];
    let fired = 0, shotSpread = 0, shakeFrame = 0;
    const spreadNow = () => spread ? spread.base + spread.k * (spread.stance + shotSpread) : 0;
    const lastShot = (shots - 1) * spacing;
    const end = Math.ceil(((shots - 1) * spacing + model.interval + 3000) / TICK) * TICK;
    for (let now = 0; now <= end; now += TICK) {
      if (now > 0) {
        const dt = TICK / 1000;
        const ramp = state.kind === 'shot' ? clamp((state.time + model.interval - (now - TICK)) / 1000, 0, dt) : 0;
        const rest = dt - ramp;
        for (let axis = 0; axis < 3; axis++) {
          at[axis] += speed[axis] * ramp;
          if (rest > 0) {
            const e = at[axis]; integral[axis] += e * rest;
            const derivative = (e - error[axis]) / rest; error[axis] = e;
            speed[axis] = -(model.gains[0] * e + model.gains[1] * integral[axis] + model.gains[2] * derivative);
            at[axis] += speed[axis] * rest;
          }
          at[axis] = axis === 2 ? clamp(at[axis], 0, 1) : clamp(at[axis], -1, 1);
        }
        if (spread) shotSpread = Math.max(0, shotSpread - spread.recovery * dt);
        // The clip runs while the weapon fires from the hip, and stops after.
        if (hip && model.shake && fired > 0 && now - TICK < lastShot + model.interval) shakeFrame += model.shake.timeScale * 30 * dt;
      }
      while (fired < shots && fired * spacing <= now + .001) {
        const time = fired * spacing;
        const top = draw(model.top) / TOP, right = draw(model.right), left = draw(model.left);
        const side = (right - left) / SIDE;
        const shake = model.shake ? model.shake.range[0] + (model.shake.range[1] - model.shake.range[0]) * extra() : 0;
        const target = [clamp(at[0] + step * side * (1 - Math.abs(at[0]) ** model.power), -1, 1),
          clamp(at[1] + step * top * (1 - Math.max(at[1], 0) ** model.power), -1, 1),
          clamp(at[2] + step * shake * (1 - at[2] ** model.power), 0, 1)];
        for (let axis = 0; axis < 3; axis++) {
          speed[axis] = (target[axis] - at[axis]) / (model.interval / 1000);
          error[axis] = 0; integral[axis] = 0;
        }
        // The bullet leaves inside the spread of the moment, then the shot adds to it.
        const cone = spreadNow(), n = normal(.5), tilt = .5 * cone * (n - Math.trunc(n)), angle = 2 * Math.PI * extra();
        if (spread) shotSpread = Math.min(spread.max, shotSpread + spread.perShot);
        state = {kind: 'shot', time}; fired++;
        // At the shot's own moment (60/rpm apart), not the tick that ran it.
        events.push({time: time / 1000, shot: fired, spread: cone, bullet: {side: tilt * Math.cos(angle), up: tilt * Math.sin(angle)}});
      }
      const [shakeSide, shakeUp] = hip ? shakeAt(shakeFrame).map(value => value * at[2]) : [0, 0];
      frames.push({time: now / 1000, vertical: at[1] * TOP, horizontal: at[0] * SIDE, shots: fired, z: at[2],
        spread: spreadNow(), shake: {side: shakeSide, up: shakeUp}});
    }
    // The clip's run (seconds, frames a second), for the shake at any moment.
    const shake = hip && model.shake ? {start: 0, stop: (lastShot + model.interval) / 1000, rate: model.shake.timeScale * 30} : null;
    return {frames, events, shots, shake, duration: end / 1000};
  }
  globalThis.WeaponRecoil = Object.freeze({parse, simulate, shakeAt});
})();

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
//   kit's sum of a player modifier: skills, armour, by its name.
(() => {
  const STANCES = ['idle', 'walk', 'crouch', 'crouch_walk', 'run', 'jump'];
  const AIMS = ['hip', 'aiming', 'aimed'];
  // Sprinting and jumping have no aimed values or weights.
  const AIMED = new Set(['idle', 'walk', 'crouch', 'crouch_walk']);
  const scale = (value, sum, weight = 1) => value * Math.max(0, 1 + weight * sum);
  const finite = (object, keys) => keys.every(key => typeof object?.[key] === 'number' && Number.isFinite(object[key]));
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
  globalThis.WeaponSpread = Object.freeze({STANCES, AIMS, parse, kitOf, condition});
})();

// A weapon's two fixed motions on a shot, from its own clips (shot_motions.py
// has the bones, the signs and the units): the camera's, in the hands' shot
// clip, and the gun's in front of the camera, in its own. Neither reads the
// recoil numbers: each is one curve, played once a shot in 60/rpm seconds.
// - the camera's turns the view, so the screen moves. It no longer turns the
//   bullets (exe patch 200): a shot leaves from the gun as it is posed at the
//   shot's own time, the clip's first frame, where the game took the eye of
//   the simulation's step before, late in the pass before;
// - the gun's moves the gun and its sights over the view, since patch 172 on
//   every gun while aimed; the bullet leaves from the camera, not the barrel.
(() => {
  const AXES = {camera: ['up', 'side'], gun: ['back', 'up', 'side']};
  // A custom motion's shape where the weapon has none of its own: to its
  // peak in the pass's first fifth (on a sample), back by its end.
  const STEPS = 32, PEAK = 6 / STEPS;
  const SHAPE = Array.from({length: STEPS + 1}, (_, i) => {
    const phase = i / STEPS;
    return phase < PEAK ? phase / PEAK : ((1 - phase) / (1 - PEAK)) ** 2;
  });
  function curve(raw, axes) {
    if (!raw || axes.some(axis => !Array.isArray(raw[axis]) || raw[axis].length < 2 || !raw[axis].every(Number.isFinite))) return null;
    return Object.freeze(Object.fromEntries(axes.map(axis => [axis, Object.freeze([...raw[axis]])])));
  }
  // A weapon's `shot` (absent: a build without the clips): each motion or null.
  function parse(shot) {
    // Aimed, the hands play the hip's clip unless the weapon has another (shot_motions.py writes it only then).
    const hip = curve(shot?.camera?.hip, AXES.camera);
    return Object.freeze({camera: Object.freeze({hip, aimed: shot?.camera?.aimed ? curve(shot.camera.aimed, AXES.camera) : hip}),
      gun: curve(shot?.gun, AXES.gun)});
  }
  // The largest value of an axis, with its sign; 0 for no curve.
  function peak(curve, axis) {
    return curve ? curve[axis].reduce((most, value) => Math.abs(value) > Math.abs(most) ? value : most, 0) : 0;
  }
  // The curve with each of `peaks`' axes at that peak instead: its own shape
  // scaled, SHAPE for an axis that does not move. Null when nothing moves.
  function shaped(curve, axes, peaks) {
    const out = {};
    for (const axis of axes) {
      const own = peak(curve, axis), wanted = axis in peaks ? peaks[axis] : own;
      out[axis] = wanted === own && curve ? curve[axis] : own ? curve[axis].map(value => value * wanted / own) : SHAPE.map(value => value * wanted);
    }
    return axes.some(axis => out[axis].some(value => value !== 0)) ? Object.freeze(out) : null;
  }
  // The motions with a custom gun's peaks: {camera: {up, side}, gun: {back, up, side}}, any left out as the weapon's.
  function custom(motions, peaks) {
    const camera = view => shaped(motions.camera[view], AXES.camera, peaks.camera || {});
    return Object.freeze({camera: Object.freeze({hip: camera('hip'), aimed: camera('aimed')}), gun: shaped(motions.gun, AXES.gun, peaks.gun || {})});
  }
  // A curve's values `phase` (0 to 1) into its pass; all 0 outside it and for no curve.
  function at(curve, axes, phase) {
    if (!curve || !(phase >= 0 && phase < 1)) return Object.fromEntries(axes.map(axis => [axis, 0]));
    return Object.fromEntries(axes.map(axis => {
      const values = curve[axis], place = phase * (values.length - 1), i = Math.floor(place);
      return [axis, values[i] + (values[i + 1] - values[i]) * (place - i)];
    }));
  }
  // How far (0 to 1) `time` is into the pass of the clips the last of
  // `shots` ([{time}], in order) at or before it started, `interval` seconds
  // a pass; 1 before the first shot and once the pass is over.
  function phaseAt(shots, interval, time) {
    let last = -1;
    for (const shot of shots) {
      if (shot.time > time + 1e-9) break;
      last = shot.time;
    }
    return last < 0 ? 1 : (time - last) / interval;
  }
  // The camera's turn {up, side} degrees while firing in `aim` (WeaponSpread.AIMS), and the gun's {back mm, up, side degrees}.
  const cameraAt = (motions, aim, phase) => at(motions.camera[aim === 'hip' ? 'hip' : 'aimed'], AXES.camera, phase);
  const gunAt = (motions, phase) => at(motions.gun, AXES.gun, phase);
  globalThis.WeaponMotions = Object.freeze({AXES, parse, peak, custom, phaseAt, cameraAt, gunAt});
})();

// Calculator-style chooser: lobby icons, class groups, level ordering,
// punctuation-insensitive search and arrow/Enter/Escape keyboard controls.
// The hidden select owns the selection, including URL and catalogue updates.
// `weapons` are the elements offered (the weapons page's rows, or the
// modifiers page's item entries and the weapons page's rounds, which carry
// their name in data-title); `offer(ids)` narrows them to those ids. Its
// words are the page's (site-texts/words.js).
function WikiWeaponPicker(select, weapons, collator, words = {
  search: T('Search weapons'), none: T('No weapons match.')}) {
  const host = document.getElementById(`${select.id}-picker`);
  const make = (tag, attrs = {}, ...children) => {
    const element = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    element.append(...children);
    return element;
  };
  const bare = text => text.normalize('NFKC').toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');
  const title = weapon => weapon.dataset.title || weapon.querySelector('.weapon-title strong').textContent;
  const icon = weapon => {
    const source = weapon?.querySelector('img');
    return source ? make('img', {src: source.src, alt: '', width: 54, height: 26, loading: 'lazy'}) : make('span', {class: 'noimg'});
  };
  const level = weapon => make('small', {}, Tf('lvl {n}', {n: weapon.dataset.level}));
  const button = make('button', {type: 'button', class: 'wpick-btn', id: `${select.id}-btn`,
    'aria-haspopup': 'listbox', 'aria-expanded': 'false', 'aria-controls': `${select.id}-list`,
    'aria-labelledby': `${select.id}-label ${select.id}-value`});
  const list = make('div', {class: 'wpick-list', role: 'listbox', id: `${select.id}-list`, 'aria-labelledby': `${select.id}-label`});
  const search = make('input', {type: 'search', class: 'wpick-search', role: 'combobox', autocomplete: 'off',
    'aria-controls': list.id, 'aria-expanded': 'false', 'aria-autocomplete': 'list',
    'aria-label': words.search, placeholder: T('Search…')});
  const empty = make('p', {class: 'wpick-empty', role: 'status'}, words.none);
  const popup = make('div', {class: 'wpick-pop', hidden: ''}, search, list, empty);
  host.append(button, popup);
  const options = [], groups = new Map();
  let active = null, offered = new Set(weapons.map(weapon => weapon.id));
  function activate(option, scroll = true) {
    active?.element.classList.remove('active');
    active = option || null;
    if (!active) { search.removeAttribute('aria-activedescendant'); return; }
    active.element.classList.add('active');
    search.setAttribute('aria-activedescendant', active.element.id);
    if (scroll) active.element.scrollIntoView({block: 'nearest'});
  }
  for (const weapon of weapons) {
    const label = weapon.dataset.class;
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label).push(weapon);
  }
  for (const [label, items] of groups) {
    const heading = make('div', {class: 'wpick-group', id: `${select.id}-group-${groups.size}-${options.length}`}, label);
    const group = make('div', {role: 'group', 'aria-labelledby': heading.id}, heading);
    list.append(group);
    for (const weapon of items.sort((a, b) => Number(a.dataset.level) - Number(b.dataset.level) || collator.compare(title(a), title(b)))) {
      const element = make('div', {class: 'wpick-opt', role: 'option', id: `${select.id}-${weapon.id}`, 'aria-selected': 'false'},
        icon(weapon), make('span', {class: 'n'}, title(weapon)), level(weapon));
      const option = {element, group, weapon, text: [title(weapon), weapon.dataset.searchName].map(bare).join(' ')};
      element.addEventListener('mousemove', () => { if (active !== option) activate(option, false); });
      element.addEventListener('mousedown', event => event.preventDefault());
      element.addEventListener('click', () => choose(weapon.id));
      group.append(element); options.push(option);
    }
  }
  const visible = () => options.filter(option => !option.element.hidden);
  function filter() {
    const query = bare(search.value);
    for (const option of options) option.element.hidden = !offered.has(option.weapon.id) || !option.text.includes(query);
    for (const group of list.children) group.hidden = !options.some(option => option.group === group && !option.element.hidden);
    const matches = visible();
    empty.hidden = matches.length !== 0;
    activate(matches.find(option => option.weapon.id === select.value) || matches[0]);
  }
  function close(refocus = false) {
    popup.hidden = true;
    button.setAttribute('aria-expanded', 'false'); search.setAttribute('aria-expanded', 'false');
    if (refocus) button.focus();
  }
  function open() {
    // Opening the other chooser closes this one through focusout.
    popup.hidden = false;
    button.setAttribute('aria-expanded', 'true'); search.setAttribute('aria-expanded', 'true');
    popup.style.left = '';
    const overflow = popup.getBoundingClientRect().right - (document.documentElement.clientWidth - 16);
    if (overflow > 0) popup.style.left = `${-overflow}px`;
    search.value = ''; filter(); search.focus({preventScroll: true});
  }
  function choose(id) {
    close(true);
    if (id === select.value) return;
    select.value = id; select.dispatchEvent(new Event('change'));
  }
  button.addEventListener('click', () => popup.hidden ? open() : close(true));
  button.addEventListener('keydown', event => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); open(); }
  });
  search.addEventListener('input', filter);
  search.addEventListener('keydown', event => {
    const matches = visible(), index = matches.indexOf(active);
    if (event.key === 'ArrowDown') { event.preventDefault(); activate(matches[Math.min(matches.length - 1, index + 1)]); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); activate(matches[Math.max(0, index - 1)]); }
    else if (event.key === 'Enter') { event.preventDefault(); if (active) choose(active.weapon.id); }
    else if (event.key === 'Escape') { event.preventDefault(); close(true); }
    else if (event.key === 'Tab') close();
  });
  document.addEventListener('mousedown', event => { if (!host.contains(event.target)) close(); });
  host.addEventListener('focusout', event => { if (!host.contains(event.relatedTarget)) close(); });
  return {
    offer(ids) { offered = new Set(ids); },
    paint() {
      const selected = weapons.find(weapon => weapon.id === select.value);
      button.replaceChildren(...(selected ? [icon(selected)] : []),
        make('span', {class: 'n', id: `${select.id}-value`}, selected ? title(selected) : select.options[0].textContent),
        ...(selected ? [level(selected)] : []));
      for (const option of options) option.element.setAttribute('aria-selected', String(option.weapon.id === select.value));
    }
  };
}

// The two weapon slots, A and B (the weapons page's comparison, the
// simulators'), each a weapon and its round (#compare-a, #compare-a-round,
// ...), kept in the address (a, b, ra, rb; a round only when not its
// weapon's default, the first of `roundsOf`) and in the language link.
// `changed()` runs after each choice; `refresh()` once the page is ready.
function WikiWeaponSlots(weapons, roundsOf, collator, changed) {
  const pickers = ['a', 'b'].map(side => document.getElementById(`compare-${side}`));
  const roundPickers = ['a', 'b'].map(side => document.getElementById(`compare-${side}-round`));
  // Every round's chooser entry (its icon, name and level).
  const roundEntries = [...document.querySelectorAll('#round-options > div')];
  const roundWords = {search: T('Search rounds'), none: T('No rounds match.')};
  const choosers = pickers.map(select => WikiWeaponPicker(select, weapons, collator));
  const roundChoosers = roundPickers.map(select => WikiWeaponPicker(select, roundEntries, collator, roundWords));
  const known = new Set(weapons.map(weapon => weapon.id));
  // A slot's rounds: its weapon's, keeping the chosen round when the new
  // weapon fires it too; a dash without a weapon.
  function fillRounds(side, id, chosen) {
    const select = roundPickers[side], rounds = id ? roundsOf.get(id) : [];
    select.replaceChildren(...(rounds.length ? rounds.map(round => new Option(round.name, round.id)) : [new Option('—', '')]));
    select.value = rounds.some(round => round.id === chosen) ? chosen : rounds[0]?.id ?? '';
    roundChoosers[side].offer(rounds.map(round => round.id));
  }
  function refresh() {
    pickers.forEach((picker, side) => fillRounds(side, picker.value, roundPickers[side].value));
    for (const chooser of [...choosers, ...roundChoosers]) chooser.paint();
    const url = new URL(location.href);
    const round = side => roundPickers[side].selectedIndex > 0 ? roundPickers[side].value : '';
    const keys = [['a', pickers[0].value], ['b', pickers[1].value], ['ra', round(0)], ['rb', round(1)]];
    for (const [key, value] of keys) if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
    history.replaceState(null, '', url);
    // Each of the header's languages keeps the choice.
    for (const language of document.querySelectorAll('.lang a')) {
      const next = new URL(language.href);
      for (const [key] of keys) {
        if (url.searchParams.has(key)) next.searchParams.set(key, url.searchParams.get(key)); else next.searchParams.delete(key);
      }
      language.href = next;
    }
    changed();
  }
  for (const select of [...pickers, ...roundPickers]) select.addEventListener('change', refresh);
  const query = new URLSearchParams(location.search);
  [['a', 'ra'], ['b', 'rb']].forEach(([key, round], side) => {
    if (known.has(query.get(key))) pickers[side].value = query.get(key);
    fillRounds(side, pickers[side].value, query.get(round));
  });
  return {
    roundEntries, roundWords, refresh,
    // [{id, round}] of A and B, '' for an empty slot.
    sides: () => pickers.map((picker, side) => ({id: picker.value, round: roundPickers[side].value})),
    choose(side, id) { pickers[side].value = id; refresh(); },
    clear() { for (const picker of pickers) picker.value = ''; refresh(); },
  };
}

// The game-styled rows the damage calculator and the wiki's simulators share
// (hud.css): a panel of the lobby's bonus rows (ModifierRow, InventoryItemHint),
// each its icon, "<label>:" and a button per option, and the cards that show
// what a thing does over its tile or label.
const HUD = (() => {
  const el = (tag, attrs = {}, ...kids) => {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (k === "class") n.className = v; else if (k === "text") n.textContent = v; else n.setAttribute(k, v);
    }
    for (const kid of kids) if (kid != null) n.append(kid);
    return n;
  };
  // A card: its name, then a line of text each.
  const card = (name, ...texts) => el("div", { class: "gamemods" }, el("span", { class: "gname", text: name }),
    ...texts.map(text => el("span", { class: "gtext", text })));
  // A tile with its card; a card with nothing to say leaves the tile as it is.
  // A click puts the card away until the pointer comes back, as a tooltip goes.
  function hinted(tile, box) {
    if (!box.children.length) return tile;
    tile.removeAttribute("title");
    const wrap = el("span", { class: "fxhint" }, tile, box);
    tile.addEventListener("click", () => wrap.classList.add("quiet"));
    wrap.addEventListener("mouseleave", () => wrap.classList.remove("quiet"));
    return wrap;
  }
  // A label's card opens on hover, on keyboard focus, and on a tap or click
  // (a touch screen has no hover); a press elsewhere or Escape closes it.
  const closeLabels = keep => { for (const open of document.querySelectorAll(".glabel.fxhint.open")) if (open !== keep) open.classList.remove("open"); };
  document.addEventListener("pointerdown", event => { if (!event.target.closest?.(".glabel.fxhint")) closeLabels(); });
  document.addEventListener("keydown", event => { if (event.key === "Escape") closeLabels(); });
  // A bonus row as the lobby draws it: icon, "<label>:", a button per option
  // [value, text] of picks[key], the picked one again clears it (unless
  // `required`: then a row always has one). With a card, its label shows it.
  function stepsRow(icon, label, options, picks, key, onChange, box, required = false) {
    const group = el("span", { class: "gsteps", role: "group", "aria-label": label });
    const paint = () => { for (const b of group.children) b.setAttribute("aria-pressed", String(b.dataset.v === String(picks[key] || ""))); };
    for (const [v, text] of options) {
      const b = el("button", { type: "button", class: "gstep", text });
      b.dataset.v = v;
      b.addEventListener("click", () => { picks[key] = !required && String(picks[key] || "") === String(v) ? "" : v; paint(); onChange(); });
      group.append(b);
    }
    paint();
    const name = el("span", box ? { class: "glabel fxhint", tabindex: "0" } : { class: "glabel" }, `${label}:`, box);
    if (box) name.addEventListener("click", () => { closeLabels(name); name.classList.toggle("open"); });
    return el("div", { class: "gmrow" }, icon, name, group);
  }
  return { el, card, hinted, stepsRow };
})();

// The simulators page: one or two weapons (A, B: WikiWeaponSlots), each with
// a round, fired in one simulation of recoil (recoil.js), spread (spread.js)
// and the shot clips' own motions of the camera and the gun (motions.js) in
// the state its controls set. A side can be a custom gun: its weapon with
// any of those values the visitor's own.
(() => {
  const sim = document.querySelector('#recoil-sim');
  if (!sim) return;
  const weapons = [...document.querySelectorAll('#weapon-options > div')];
  const collator = new Intl.Collator(document.documentElement.lang, {numeric: true, sensitivity: 'base'});
  const byId = new Map(weapons.map(element => [element.id, element]));
  // Each weapon's rounds, its default first: [{id, name, spread}].
  const roundsOf = new Map(weapons.map(element => [element.id, JSON.parse(element.dataset.rounds)]));
  function roundOf(id, roundId) {
    const rounds = roundsOf.get(id);
    return rounds.find(round => round.id === roundId) || rounds[0];
  }
  // The player's stance spread and its skill weights (default.player).
  const player = JSON.parse(sim.dataset.player);
  // A weapon's models firing a round with its modifications' `kick` (their
  // sum): its recoil, whose kick ranges are times max(0, 1 + kick) (the
  // cook's type 2, 0x358010), and its spread, whose base is times the round's
  // `dispersion` (get_static_dispersion 0x331220).
  const raws = new Map(weapons.map(element => [element.id, JSON.parse(element.dataset.recoil)]));
  const RANGES = ['top', 'left', 'right'].flatMap(axis => [`min_${axis}_recoil`, `max_${axis}_recoil`]);
  const models = new Map();
  // `over`: a custom gun's own recoil values, by the config's keys.
  function model(id, roundId, kick = 0, over = {}) {
    const round = roundOf(id, roundId), key = `${id} ${round.id} ${kick} ${JSON.stringify(over)}`;
    if (!models.has(key)) {
      const raw = {...raws.get(id), ...over}, dispersion = raw.dispersion, scale = Math.max(0, 1 + kick);
      const base = typeof dispersion.base_dispersion === 'number' ? dispersion.base_dispersion * round.spread : undefined;
      const ranges = Object.fromEntries(RANGES.filter(k => typeof raw[k] === 'number').map(k => [k, raw[k] * scale]));
      models.set(key, {recoil: WeaponRecoil.parse({...raw, ...ranges}),
        spread: WeaponSpread.parse({...dispersion, base_dispersion: base}, player, raw.double_handed)});
    }
    return models.get(key);
  }
  // Each weapon's shot clips' motions (none in a build without the clips).
  const motionsOf = new Map(weapons.map(element => [element.id, WeaponMotions.parse(raws.get(element.id).shot)]));
  function node(tag, text, cls) {
    const element = document.createElement(tag);
    if (text !== undefined) element.textContent = text;
    if (cls) element.className = cls;
    return element;
  }
  function name(id) { return byId.get(id).dataset.title; }
  const svgNS = 'http://www.w3.org/2000/svg';
  function svgNode(tag, attrs, text) {
    const element = document.createElementNS(svgNS, tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    if (text !== undefined) element.textContent = text;
    return element;
  }
  // Degrees of view in one plotted unit of kick, from the exe
  // (servers/match-server/src/v026g/bots/recoil.rs, "the pitch's unit"): a
  // pitch unit turns the view 60° up and 67.5° down, and the configs' kick is
  // 63.735 to a pitch unit (PROVEN): 63.735 is the vertical clip's half range,
  // so the configs' kick is in degrees. The side's 21.48 taken the same way:
  // a degree a unit. LIKELY, against the clip decode's ±2.3° (which made it
  // 0.108°, so small the H&K 416A5's random side kick and the KRISS Vector's
  // none looked alike; the owner's play: the 416 jerky, the Vector smooth).
  const UP_DEG = 60 / 63.735, DOWN_DEG = 67.5 / 63.735, SIDE_DEG = 1;
  const viewUp = v => v * (v >= 0 ? UP_DEG : DOWN_DEG), viewSide = h => h * SIDE_DEG;
  // A target's figure: the game's HUD doll, 1.8 m tall, aimed at the chest
  // (a quarter of its height below the top of the head).
  const BODY_HEIGHT = 1.8, CHEST = .25;
  let dollBox = null;
  function bodyDegrees(distance) {
    return 2 * Math.atan(BODY_HEIGHT / 2 / distance) * 180 / Math.PI;
  }
  // How far (0 to 1) `time` is into the pass of the clips the series' last
  // shot started; 1 before its first shot and once the pass is over.
  function passAt(series, time) {
    return WeaponMotions.phaseAt(series.events, series.interval, time);
  }
  // The view at `time`, where a bullet would leave: with `layers.recoil` the
  // kick between its 50 ms frames and the shake at its own moment (its clip
  // runs ~240 frames a second, far finer than the frames), with
  // `layers.camera` the hands' clip's turn of the camera.
  function viewAt(series, time, layers) {
    const at = Math.max(0, time / .05), i = Math.min(series.frames.length - 1, Math.floor(at + 1e-6));
    const a = series.frames[i], b = series.frames[Math.min(series.frames.length - 1, i + 1)], f = Math.min(1, at - i);
    const lerp = key => a[key] + (b[key] - a[key]) * f;
    let side = 0, up = 0;
    if (series.shake) {
      const [clipSide, clipUp] = WeaponRecoil.shakeAt(series.shake.rate * (Math.min(Math.max(time, series.shake.start), series.shake.stop) - series.shake.start));
      side = clipSide * lerp('z'); up = clipUp * lerp('z');
    }
    const kick = layers.recoil ? {side: viewSide(lerp('horizontal')) + side, up: viewUp(lerp('vertical')) + up} : {side: 0, up: 0};
    const camera = layers.camera ? WeaponMotions.cameraAt(series.motions, series.aim, passAt(series, time)) : {side: 0, up: 0};
    return {side: kick.side + camera.side, up: kick.up + camera.up};
  }
  // Where the gun's sight points at `time`, and how far the gun is back (mm):
  // the view and, with `layers.gun`, the gun's own turn in front of it. From
  // the hip the sights are not looked through: the view alone.
  function sightAt(series, time, layers) {
    const view = viewAt(series, time, layers);
    const gun = layers.gun && series.aim !== 'hip' ? WeaponMotions.gunAt(series.motions, passAt(series, time)) : {back: 0, side: 0, up: 0};
    return {side: view.side + gun.side, up: view.up + gun.up, back: gun.back, shown: layers.gun && series.aim !== 'hip' && series.motions.gun !== null};
  }
  // How long a bullet is drawn on its way from the eye to the wall (s).
  const FLIGHT = .04;
  function sample(series, time) {
    return series.frames[Math.max(0, Math.min(series.frames.length - 1, Math.floor(time / .05 + 1e-6)))];
  }
  // The simulation in `root`: its controls, plots and crosshair view, every
  // lookup inside `root`. `show(sides)` runs it for [{id, round, side}]
  // (side 0 is A's colour, 1 B's), each side firing in `stateOf(side)`'s
  // state; `refresh()` after a state changes.
  function RecoilSim(root, stateOf) {
    const find = selector => root.querySelector(selector);
    const timeInput = find('.sim-time'), seedInput = find('.sim-seed'), playButton = find('.sim-play');
    let simulation = {kind: 'empty'}, sides = [], playing = null;
    // The motions shown: recoil (the kick and the hip's shake), the camera's clip, the gun's.
    const layerInputs = {recoil: find('.sim-layer-recoil'), camera: find('.sim-layer-camera'), gun: find('.sim-layer-gun')};
    const layers = () => Object.fromEntries(Object.entries(layerInputs).map(([key, input]) => [key, input.checked]));
    function lastShot() {
      return Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
    }
    function draw() {
      if (simulation.kind !== 'ready') return;
      const time = simulation.time;
      find('.sim-last').disabled = time >= lastShot() - .001;
      timeInput.value = time;
      find('.sim-time-label').textContent = `${time.toFixed(2)} ${Tc('unit', 's')}`;
      const legend = find('.sim-legend'); legend.replaceChildren();
      for (const series of simulation.series) {
        const point = sample(series, time), label = sides.length > 1 ? `${series.side ? 'B' : 'A'} · ` : '';
        legend.append(node('p', `${label}${series.title} · ${T('shots')} ${series.events.filter(event => event.time <= time + 1e-9).length}/${series.shots} · ${T('up')} ${point.vertical.toFixed(3)} · ${T('side')} ${point.horizontal.toFixed(3)} · ${T('spread')} ${point.spread.toFixed(2)}°`, `series-${series.side}`));
      }
      for (const axis of ['vertical', 'horizontal']) {
        const svg = find(`.sim-${axis}`); svg.replaceChildren();
        const values = simulation.series.flatMap(series => series.frames.map(frame => frame[axis]));
        const lo = Math.min(0, ...values), hi = Math.max(.1, ...values), margin = (hi - lo) * .1;
        const min = lo - margin, max = hi + margin;
        const x = t => 54 + t / simulation.duration * 530, y = v => 218 - (v - min) / (max - min) * 194;
        for (let i = 0; i <= 4; i++) {
          const value = min + (max - min) * i / 4;
          svg.append(svgNode('line', {x1:54, x2:584, y1:y(value), y2:y(value), class:'sim-grid'}));
          svg.append(svgNode('text', {x:47, y:y(value)+4, 'text-anchor':'end'}, value.toFixed(2)));
          const t = simulation.duration * i / 4;
          svg.append(svgNode('text', {x:x(t), y:241, 'text-anchor':'middle'}, `${t.toFixed(1)}s`));
        }
        svg.append(svgNode('line', {x1:54, x2:584, y1:y(0), y2:y(0), class:'sim-zero'}));
        for (const series of simulation.series) {
          const path = frames => frames.map((frame, i) => `${i ? 'L' : 'M'}${x(frame.time).toFixed(2)},${y(frame[axis]).toFixed(2)}`).join(' ');
          svg.append(svgNode('path', {d:path(series.frames), class:`sim-curve series-${series.side} future`}));
          svg.append(svgNode('path', {d:path(series.frames.filter(frame => frame.time <= time)), class:`sim-curve series-${series.side}`}));
          for (const event of series.events.filter(event => event.time <= time)) {
            const point = sample(series, event.time);
            svg.append(svgNode('circle', {cx:x(event.time), cy:y(point[axis]), r:2.8, class:`sim-shot series-${series.side}`}));
          }
        }
        svg.append(svgNode('line', {x1:x(time), x2:x(time), y1:24, y2:218, class:'sim-cursor'}));
      }
      drawSight(time);
    }
    // Each weapon's spread in its state, in degrees: the base, the stance's
    // part, a shot's and the most it reaches.
    function drawParts() {
      const degrees = value => `${+value.toFixed(2)}°`;
      find('.sim-parts tbody').replaceChildren(...simulation.series.map(series => {
        const spread = series.spread, row = node('tr', undefined, `series-${series.side}`);
        const label = node('th', sides.length > 1 ? `${series.side ? 'B' : 'A'} · ${series.title}` : series.title); label.scope = 'row';
        row.append(label, ...(spread ? [spread.base, spread.k * spread.stance, spread.k * spread.perShot, spread.base + spread.k * (spread.stance + spread.max)]
          .map(value => node('td', degrees(value))) : [node('td', '—')]));
        return row;
      }));
    }
    // The view as the shot moves it, in degrees of view at true scale both
    // ways: its path, each shot's mark and bullet's hit, the spread around it,
    // and the gun's sight where its own kick puts it. Under it the same from
    // the side: the bullet from the eye along the view. Each weapon has its
    // own pair, all on one scale so they compare.
    function drawSight(time) {
      const on = layers();
      // In degrees of view: the kick as the camera turns. The pattern is the
      // weapon's: a target changes nothing of it, only its own size shows the
      // distance (cut by the view's edge when it is the larger).
      const distance = Number(find('.sim-target').value) || 0, body = distance ? bodyDegrees(distance) : 0;
      // The aim (kick and shake), each bullet's hit and the spread around the
      // aim, in degrees of view: the view holds them all.
      // A bullet's hit: the view at its shot (the camera's clip is at its
      // first frame then, motions.js) and its own tilt in the spread.
      const hit = (series, event) => {
        const point = viewAt(series, event.time, on);
        return {side: point.side + event.bullet.side, up: point.up + event.bullet.up};
      };
      // The path drawn every 1/240 s, fine enough for the shake and a clip's pass.
      const steps = series => Array.from({length: Math.floor(series.duration * 240) + 1}, (_, i) => i / 240);
      const points = [...simulation.series.flatMap(series => steps(series).flatMap(t => [{...viewAt(series, t, on), r: sample(series, t).spread / 2}, {...sightAt(series, t, on), r: 0}])),
        ...simulation.series.flatMap(series => series.events.map(event => ({...hit(series, event), r: 0})))];
      const up = Math.max(0, ...points.map(point => point.up + point.r));
      const down = Math.max(0, ...points.map(point => point.r - point.up));
      const side = Math.max(0, ...points.map(point => Math.abs(point.side) + point.r));
      const scale = 250 / Math.max(.1, up + down, 2 * side * 250 / 280);
      const oy = 25 + (250 - (up + down) * scale) / 2 + up * scale;
      const x = degrees => 150 + degrees * scale, y = degrees => oy - degrees * scale;
      const hitAt = (series, event) => {
        const point = hit(series, event);
        return [x(point.side), y(point.up)];
      };
      const views = simulation.series.map(series => {
        const svg = svgNode('svg', {viewBox:'0 0 300 300', role:'img', 'aria-label':`${series.title}: ${T('where it aims as the shot moves it')}`});
        const from = svgNode('svg', {viewBox:'0 0 300 110', role:'img', class:'sim-side', 'aria-label':`${series.title}: ${T('from the side, the bullet from the eye along the view')}`});
        const view = document.createElement('div'); view.className = 'sim-view';
        if (simulation.series.length > 1) view.append(node('p', `${series.side ? 'B' : 'A'} · ${series.title}`, `sim-view-name series-${series.side}`));
        view.append(svg, from);
        return {series, svg, from, view};
      });
      find('.sim-sight-views').replaceChildren(...views.map(({view}) => view));
      // From the side: the eye at the left, a wall at the right, the view's
      // angle up drawn to the same span as the view above.
      const sideY = degrees => 12 + (up - degrees) * 86 / Math.max(.1, up + down);
      for (const {series, svg, from} of views) {
        svg.append(svgNode('line', {x1:0, x2:300, y1:oy, y2:oy, class:'sim-grid'}));
        svg.append(svgNode('line', {x1:150, x2:150, y1:0, y2:300, class:'sim-grid'}));
        if (distance) {
          dollBox ??= document.querySelector('#doll-shape').getBBox();
          const size = body * scale / dollBox.height;
          svg.append(svgNode('use', {href:'#doll-shape', class:'sim-target-body',
            transform:`translate(${(150 - (dollBox.x + dollBox.width / 2) * size).toFixed(2)} ${(oy - body * CHEST * scale - dollBox.y * size).toFixed(2)}) scale(${size.toFixed(5)})`}));
        }
        svg.append(svgNode('circle', {cx:150, cy:oy, r:3, class:'sim-origin'}));
        // Each weapon's view holds at its own last shot: with two, the faster
        // finishes first, and its settling is the plots' only (as for one).
        const last = Math.max(...series.events.map(event => event.time)), held = Math.min(time, last);
        const past = [...steps(series).filter(t => t < held), held].map(t => viewAt(series, t, on));
        svg.append(svgNode('path', {d:past.map((point, i) => `${i ? 'L' : 'M'}${x(point.side).toFixed(2)},${y(point.up).toFixed(2)}`).join(' '),
          class:`sim-trail series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= held)) {
          const point = viewAt(series, event.time, on), [hx, hy] = hitAt(series, event);
          svg.append(svgNode('circle', {cx:x(point.side), cy:y(point.up), r:2, class:`sim-shot series-${series.side}`}));
          svg.append(svgNode('circle', {cx:hx, cy:hy, r:2.6, class:`sim-hit series-${series.side}`}));
        }
        const frame = sample(series, held), point = viewAt(series, held, on), cx = x(point.side), cy = y(point.up);
        // The spread: the cone's half angle around the aim.
        const radius = frame.spread / 2;
        if (radius > 0) svg.append(svgNode('circle', {cx, cy, r:radius * scale, class:`sim-spread series-${series.side}`}));
        const cross = svgNode('g', {class:`sim-cross series-${series.side}`});
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          cross.append(svgNode('line', {x1:cx + dx * 5, y1:cy + dy * 5, x2:cx + dx * 15, y2:cy + dy * 15}));
        }
        cross.append(svgNode('circle', {cx, cy, r:1.6}));
        svg.append(cross);
        // The gun's sight: a ring where the gun's own kick points it, larger as the gun comes back.
        const sight = sightAt(series, held, on);
        if (sight.shown) {
          const ring = svgNode('g', {class:`sim-gunsight series-${series.side}`});
          ring.append(svgNode('circle', {cx:x(sight.side), cy:y(sight.up), r:9 * (1 + sight.back / 250)}));
          ring.append(svgNode('circle', {cx:x(sight.side), cy:y(sight.up), r:1.8, class:'sim-gunsight-dot'}));
          svg.append(ring);
        }
        // From the side: the view's line from the eye (dashed: a bullet's way),
        // the sight's line, and each bullet from the eye to the wall along the
        // view it left on.
        const eye = [20, sideY(0)], wall = 284;
        from.append(svgNode('line', {x1:wall, x2:wall, y1:4, y2:106, class:'sim-wall'}));
        from.append(svgNode('line', {x1:eye[0], y1:eye[1], x2:wall, y2:sideY(point.up), class:`sim-side-view series-${series.side}`}));
        if (sight.shown) from.append(svgNode('line', {x1:eye[0], y1:eye[1], x2:wall, y2:sideY(sight.up), class:`sim-side-sight series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= held)) {
          const flown = Math.min(1, (held - event.time) / FLIGHT), to = sideY(hit(series, event).up);
          from.append(svgNode('circle', {cx:eye[0] + (wall - eye[0]) * flown, cy:eye[1] + (to - eye[1]) * flown, r:flown < 1 ? 3.2 : 2.6,
            class:`${flown < 1 ? 'sim-bullet' : 'sim-landed'} series-${series.side}`}));
        }
        from.append(svgNode('path', {d:`M${eye[0] - 9},${eye[1]} Q${eye[0]},${eye[1] - 8} ${eye[0] + 9},${eye[1]} Q${eye[0]},${eye[1] + 8} ${eye[0] - 9},${eye[1]} Z`, class:'sim-eye'}));
        from.append(svgNode('circle', {cx:eye[0], cy:eye[1], r:2.6, class:'sim-eye-pupil'}));
      }
    }
    // Play: the burst in real time, from where the timeline is (or the start),
    // to the last shot; the settling after it stays on the plots.
    function stop() {
      if (playing) cancelAnimationFrame(playing.frame);
      playing = null; playButton.textContent = T('Play');
    }
    function play() {
      if (simulation.kind !== 'ready') return;
      const last = Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
      const from = simulation.time >= last - .001 ? 0 : simulation.time, start = performance.now();
      const speed = Number(find('.sim-speed').value) || 1;
      playButton.textContent = T('Pause');
      const tick = now => {
        if (simulation.kind !== 'ready') { stop(); return; }
        // A frame's timestamp can precede the press: never before `from`.
        simulation.time = Math.min(last, from + Math.max(0, now - start) / 1000 * speed); draw();
        if (simulation.time >= last) stop(); else playing.frame = requestAnimationFrame(tick);
      };
      playing = {frame: requestAnimationFrame(tick)};
    }
    function seek(time) {
      stop();
      if (simulation.kind !== 'ready') return;
      simulation.time = time; draw();
    }
    function rebuild(time = 0) {
      stop(); simulation = {kind:'empty'};
      const firing = sides.map(({side}) => stateOf(side));
      const valid = sides.length > 0 && sides.every(({id, round}, i) => model(id, round, firing[i].kick, firing[i].custom.recoil).recoil);
      for (const element of root.querySelectorAll('input,select,button')) element.disabled = !valid;
      find('.sim-plots').hidden = !valid;
      find('.sim-sight').hidden = !valid;
      find('.sim-unavailable').hidden = valid;
      if (!valid) { find('.sim-legend').replaceChildren(); find('.sim-parts tbody').replaceChildren(); return; }
      seedInput.value = Math.max(1, Math.min(999999, Math.floor(Number(seedInput.value) || 1)));
      // A full magazine; with two weapons, the smaller one's from each.
      const shots = Math.min(...sides.map(({id, round}, i) => model(id, round, firing[i].kick, firing[i].custom.recoil).recoil.magazine));
      const series = sides.map(({id, round, side}, i) => {
        const state = firing[i], {recoil, spread: weapon} = model(id, round, state.kick, state.custom.recoil), spread = weapon && WeaponSpread.condition(weapon, state);
        const result = WeaponRecoil.simulate(recoil, shots, Number(seedInput.value), {aim: state.aim, spread, recoil: state.kit.recoil},
          Number(find('.sim-trigger').value));
        // An initial un-fired frame makes Reset/Next shot unambiguous in the UI.
        result.frames.forEach(frame => {frame.time += .05;});
        result.events.forEach(event => {event.time += .05;});
        result.frames.unshift({...result.frames[0], time:0, vertical:0, horizontal:0, shots:0, z:0, shake:{side:0, up:0}});
        const shake = result.shake && {...result.shake, start: result.shake.start + .05, stop: result.shake.stop + .05};
        return {...result, id, side, spread, shake, duration: result.duration + .05, title: titleOf(side, id), aim: state.aim,
          interval: recoil.interval / 1000, motions: WeaponMotions.custom(motionsOf.get(id), state.custom)};
      });
      const duration = Math.max(...series.map(series => series.duration));
      simulation = {kind:'ready', series, time:Math.min(time, duration), duration};
      timeInput.max = simulation.duration; drawParts(); draw();
    }
    playButton.addEventListener('click', () => playing ? stop() : play());
    seedInput.addEventListener('change', () => rebuild(simulation.kind === 'ready' ? simulation.time : 0));
    find('.sim-trigger').addEventListener('change', () => rebuild());
    find('.sim-reset').addEventListener('click', () => seek(0));
    // Last shot: the whole pattern at once.
    find('.sim-last').addEventListener('click', () => {
      if (simulation.kind !== 'ready') return;
      seek(lastShot());
    });
    find('.sim-target').addEventListener('change', draw);
    for (const input of Object.values(layerInputs)) input.addEventListener('change', draw);
    timeInput.addEventListener('input', () => seek(Number(timeInput.value)));
    return {show(chosen) { sides = chosen; rebuild(); }, refresh() { rebuild(simulation.kind === 'ready' ? simulation.time : 0); }};
  }
  // Each side's setup, its own (the same weapon can be set up twice): a panel
  // of hud.js's rows, as the calculator's, from build.py's `setup` rows. Its
  // state: the aim, the stance, the broken arms, the kit's sums of the
  // skills' and armour pieces' modifiers, and the weapon modifications' sum
  // on its kick ranges.
  const rows = JSON.parse(sim.dataset.setup);
  const panels = [...document.querySelectorAll('.sim-setup')];
  // Fully aimed first: how most shots that matter are fired.
  const picks = panels.map(() => ({aim: 'aimed', stance: 'idle', arms: '0'}));
  // A custom gun: a side's own values over its weapon's. `recoil` by the
  // config's keys (WeaponRecoil.parse), `camera` and `gun` each motion's
  // peaks by axis (WeaponMotions.custom). A value the visitor left is absent.
  const custom = panels.map(() => ({recoil: {}, camera: {}, gun: {}}));
  const isCustom = side => Object.values(custom[side]).some(values => Object.keys(values).length > 0);
  const titleOf = (side, id) => isCustom(side) ? `${name(id)} · ${T('custom')}` : name(id);
  // Its fields, a group a motion (named as its layer): [key, label, step].
  const up = T('Up'), sideways = T('Side');
  const FIELDS = [
    ['recoil', T('Recoil'), [
      ['rpm', T('Rate'), 1],
      ['min_top_recoil', `${up} min`, .01], ['max_top_recoil', `${up} max`, .01],
      ['min_left_recoil', `${Tc('direction', 'Left')} min`, .01], ['max_left_recoil', `${Tc('direction', 'Left')} max`, .01],
      ['min_right_recoil', `${Tc('direction', 'Right')} min`, .01], ['max_right_recoil', `${Tc('direction', 'Right')} max`, .01],
      ['recoil_power', T('Power'), .01],
      ['compensation_proportional_gain', 'P', .1], ['compensation_integral_gain', 'I', .1], ['compensation_derivational_gain', 'D', .1]]],
    ['camera', T('Camera wobble'), [['up', `${up} °`, .01], ['side', `${sideways} °`, .01]]],
    ['gun', T('Gun kick'), [['back', `${Tc('direction', 'Back')} mm`, .1], ['up', `${up} °`, .01], ['side', `${sideways} °`, .01]]],
  ];
  // A field's value on the weapon itself (a motion's: its curve's peak).
  function own(id, group, key) {
    if (group === 'recoil') return raws.get(id)[key];
    const motions = motionsOf.get(id);
    return +WeaponMotions.peak(group === 'gun' ? motions.gun : motions.camera.aimed, key).toFixed(3);
  }
  function stateOf(side) {
    const pick = picks[side], chosen = rows.filter(row => pick[row.key]);
    return {aim: pick.aim, stance: pick.stance, arms: Number(pick.arms), custom: custom[side],
      kit: WeaponSpread.kitOf(chosen.filter(row => row.effects).map(row => row.effects[pick[row.key]])),
      kick: chosen.filter(row => row.kick).reduce((sum, row) => sum + row.kick[pick[row.key]], 0)};
  }
  panels.forEach((panel, side) => {
    const box = HUD.el('div', {class: 'gamemods rows'});
    for (const row of rows) {
      const icon = row.icon ? HUD.el('img', {class: 'gico', src: row.icon, alt: ''}) : HUD.el('span', {class: 'gico'});
      const line = HUD.stepsRow(icon, row.label, row.options, picks[side], row.key, () => simulation.refresh(),
        row.card && HUD.card(...row.card), row.required);
      line.dataset.key = row.key; box.append(line);
    }
    panel.append(box);
    // The custom gun's fields, each the weapon's value until changed.
    const fields = HUD.el('details', {class: 'sim-custom'});
    fields.append(node('summary', T('Custom gun')));
    for (const [group, heading, keys] of FIELDS) {
     const grid = HUD.el('div', {class: 'sim-custom-fields'});
     fields.append(node('p', heading, 'sim-custom-group'), grid);
     for (const [key, label, step] of keys) {
      const input = HUD.el('input', {type: 'number', step: String(step)}), field = node('label', label);
      input.dataset.group = group; input.dataset.key = key;
      input.addEventListener('change', () => {
        const id = slots.sides()[side].id, value = Number(input.value);
        if (!id) return;
        if (input.value === '' || !Number.isFinite(value) || value === own(id, group, key)) {
          delete custom[side][group][key]; input.value = own(id, group, key) ?? '';
        } else custom[side][group][key] = value;
        panel.querySelector('.sim-setup-name').textContent = `${side ? 'B' : 'A'} · ${titleOf(side, id)}`;
        simulation.refresh();
      });
      field.append(input); grid.append(field);
     }
    }
    const reset = node('button', T('Reset')); reset.type = 'button';
    reset.addEventListener('click', () => { fill(side, slots.sides()[side].id); simulation.refresh(); });
    fields.append(reset); panel.append(fields);
  });
  // A side's custom gun starts as its weapon: every field the weapon's value.
  function fill(side, id) {
    custom[side] = {recoil: {}, camera: {}, gun: {}};
    for (const input of panels[side].querySelectorAll('.sim-custom input')) input.value = id ? own(id, input.dataset.group, input.dataset.key) ?? '' : '';
    if (id) panels[side].querySelector('.sim-setup-name').textContent = `${side ? 'B' : 'A'} · ${name(id)}`;
  }
  const shownIds = panels.map(() => null);
  const simulation = RecoilSim(sim, stateOf), empty = document.querySelector('#compare-empty');
  const slots = WikiWeaponSlots(weapons, roundsOf, collator, () => {
    const sides = slots.sides().map((chosen, side) => ({...chosen, side})).filter(({id}) => id);
    sim.hidden = sides.length === 0; empty.hidden = sides.length !== 0;
    panels.forEach((panel, side) => {
      const id = slots.sides()[side].id;
      panel.hidden = !id;
      // Another weapon in the slot: its custom gun starts over.
      if (id !== shownIds[side]) { shownIds[side] = id; fill(side, id); }
    });
    simulation.show(sides);
  });
  document.querySelector('#clear-comparison').addEventListener('click', () => slots.clear());
  document.querySelector('#comparison').hidden = false; slots.refresh();
})();
