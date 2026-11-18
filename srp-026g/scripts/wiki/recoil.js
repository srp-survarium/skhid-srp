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
