// Model of 0.26g recoil: shot 0x331C50, ramp 0x331F82, tick 0x331A50,
// recovery 0x3321E0. Matches bots/recoil.rs at 50 ms ticks. This is not
// a replay of the native RNG, camera animation or bullet dispersion.
(() => {
  const TOP = 63.735, SIDE = 21.48, TICK = 50;
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
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
      gains: [raw.compensation_proportional_gain, raw.compensation_integral_gain, raw.compensation_derivational_gain]});
  }
  function simulate(model, requestedShots, seed = 1) {
    const shots = Math.min(model.magazine, clamp(Math.floor(requestedShots), 1, 60));
    let randomState = seed >>> 0 || 1;
    function random() {
      randomState ^= randomState << 13; randomState ^= randomState >>> 17; randomState ^= randomState << 5;
      return (randomState >>> 0) / 4294967296;
    }
    const draw = range => range[0] + (range[1] - range[0]) * random();
    let state = {kind: 'rest'};
    const at = [0, 0], speed = [0, 0], error = [0, 0], integral = [0, 0];
    const frames = [], events = [];
    let fired = 0;
    const end = Math.ceil((shots * model.interval + 3000) / TICK) * TICK;
    for (let now = 0; now <= end; now += TICK) {
      if (now > 0) {
        const dt = TICK / 1000;
        const ramp = state.kind === 'shot' ? clamp((state.time + model.interval - (now - TICK)) / 1000, 0, dt) : 0;
        const rest = dt - ramp;
        for (let axis = 0; axis < 2; axis++) {
          at[axis] += speed[axis] * ramp;
          if (rest > 0) {
            const e = at[axis]; integral[axis] += e * rest;
            const derivative = (e - error[axis]) / rest; error[axis] = e;
            speed[axis] = -(model.gains[0] * e + model.gains[1] * integral[axis] + model.gains[2] * derivative);
            at[axis] += speed[axis] * rest;
          }
          at[axis] = clamp(at[axis], -1, 1);
        }
      }
      while (fired < shots && fired * model.interval <= now + .001) {
        const time = fired * model.interval;
        const top = draw(model.top) / TOP, right = draw(model.right), left = draw(model.left);
        const side = (right - left) / SIDE;
        const target = [clamp(at[0] + side * (1 - Math.abs(at[0]) ** model.power), -1, 1),
          clamp(at[1] + top * (1 - Math.max(at[1], 0) ** model.power), -1, 1)];
        for (let axis = 0; axis < 2; axis++) {
          speed[axis] = (target[axis] - at[axis]) / (model.interval / 1000);
          error[axis] = 0; integral[axis] = 0;
        }
        state = {kind: 'shot', time}; fired++;
        events.push({time: now / 1000, shot: fired});
      }
      frames.push({time: now / 1000, vertical: at[1] * TOP, horizontal: at[0] * SIDE, shots: fired});
    }
    return {frames, events, shots, duration: end / 1000};
  }
  globalThis.WeaponRecoil = Object.freeze({parse, simulate});
})();
