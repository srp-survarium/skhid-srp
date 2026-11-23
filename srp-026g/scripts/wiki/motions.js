// A weapon's two fixed motions on a shot, from its own clips (shot_motions.py
// has the bones, the signs and the units): the camera's, in the hands' shot
// clip, and the gun's in front of the camera, in its own. Neither reads the
// recoil numbers: each is one curve, played once a shot in 60/rpm seconds.
// - the camera's turns the view, so the screen moves, and the bullets with it:
//   a bullet leaves along the eye (player+44Ch, instant_fire 0x2E243D) as the
//   simulation's step before the shot left it (the shot is an animation event
//   in base_player::tick, the eye is posed after it, assign_physics_transform
//   0x2D93A0; steps end every 50 ms, atomic_tick 0x2D1E20). A burst's second
//   shot and on so take the clip's turn late in the pass before; the first
//   leaves from rest;
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
  // The end of the simulation's step before the one a shot at `time` falls
  // in (a shot at a step's very end falls in that step): when the eye it
  // leaves along was posed.
  const STEP = .05;
  const eyeTime = time => (Math.ceil(time / STEP - 1e-9) - 1) * STEP;
  // The camera's turn {up, side} degrees while firing in `aim` (WeaponSpread.AIMS), and the gun's {back mm, up, side degrees}.
  const cameraAt = (motions, aim, phase) => at(motions.camera[aim === 'hip' ? 'hip' : 'aimed'], AXES.camera, phase);
  const gunAt = (motions, phase) => at(motions.gun, AXES.gun, phase);
  // The camera's turn a bullet fired at `time` leaves with.
  const cameraAtShot = (motions, aim, shots, interval, time) => cameraAt(motions, aim, phaseAt(shots, interval, eyeTime(time)));
  globalThis.WeaponMotions = Object.freeze({AXES, parse, peak, custom, phaseAt, eyeTime, cameraAt, cameraAtShot, gunAt});
})();
