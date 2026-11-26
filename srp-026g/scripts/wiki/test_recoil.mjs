import assert from 'node:assert/strict';
import fs from 'node:fs';
import './recoil.js';
import './spread.js';
import './motions.js';
const raw = {rpm:650, magazine:30, recoil_power:.8, min_top_recoil:1.8, max_top_recoil:2,
  min_left_recoil:1.4,max_left_recoil:1.8,min_right_recoil:1.4,max_right_recoil:1.8,
  compensation_proportional_gain:5,compensation_integral_gain:10,compensation_derivational_gain:0};
const model = WeaponRecoil.parse(raw);
const reference = WeaponRecoil.parse({...raw, min_top_recoil:1.9, max_top_recoil:1.9,
  min_left_recoil:1.6, max_left_recoil:1.6, min_right_recoil:1.6, max_right_recoil:1.6});
const run = WeaponRecoil.simulate(reference,12);
// Independent reference table from the native-code research, also checked by
// match-server bots/recoil.rs::an_ak_74m_burst_rises_and_comes_back_as_the_clients_code_does.
for (const [time, expected] of [[.1,1.8],[.2,3.2],[.3,4.1],[.5,4.4],[.7,5.7],[.9,7.5],[1.1,7.3],[1.3,1.5],[1.5,-.9],[1.8,-1.4]]) {
  const frame=run.frames.find(frame=>Math.abs(frame.time-time)<1e-6);
  assert.ok(Math.abs(frame.vertical-expected)<.05, `${time}: ${frame.vertical} vs ${expected}`);
}
const fixed=WeaponRecoil.parse({...raw,min_left_recoil:3,max_left_recoil:3,min_right_recoil:3,max_right_recoil:3});
assert.ok(WeaponRecoil.simulate(fixed,12).frames.every(frame=>frame.horizontal===0), 'fixed symmetric side inputs cancel');
assert.ok(run.frames.some(frame=>frame.vertical<0), 'recovery can undershoot');
assert.equal(WeaponRecoil.simulate(model,60).shots,30,'magazine cap');
assert.equal(WeaponRecoil.simulate(model,1).events.length,1,'single shot');
assert.deepEqual(WeaponRecoil.simulate(model,10,42),WeaponRecoil.simulate(model,10,42),'repeatable seed');
assert.notDeepEqual(WeaponRecoil.simulate(model,10,42).frames,WeaponRecoil.simulate(model,10,43).frames,'different samples');
assert.equal(WeaponRecoil.parse({...raw,rpm:0}),null,'invalid cadence cannot enter model');
assert.equal(WeaponRecoil.parse({rpm:650}),null,'missing fields never become invented zeroes');
// Spread and shake (dispersion_calculator, the shake axis): an AK-74M's values,
// default.player's stances (idle 5, walk 10, crouch 4, crouch-walk 8, run and
// jump 20; injuries 10; no skill weight sprinting or jumping).
globalThis.CAMERA_SHAKE ??= {period_frames:2, pitch:[.3,-.3], yaw:[.2,-.2]};
const full = WeaponRecoil.parse({...raw, min_shake_recoil:.8, max_shake_recoil:1, shake_time_scale:8});
const values = {idle:5, walk:10, crouch:4, crouch_walk:8, run:20, jump:20};
const player = {stance:{...Object.fromEntries(Object.entries(values).flatMap(([k, v]) => [[`${k}_dispersion`, v], [`${k}_aim_dispersion`, v]])),
  injury_penalty_for_double_handed:10, injury_penalty_for_one_handed:10},
  skill_influence:Object.fromEntries(Object.keys(values).flatMap(k => [[`${k}_skill_influence`, +!['run','jump'].includes(k)], [`${k}_aim_skill_influence`, 1]]))};
const ak = {base_dispersion:.2, one_shoot_dispersion_amount:2, max_dispersion:20, speed_of_aiming:10};
const rifle = WeaponSpread.parse(ak, player, true), pistol = WeaponSpread.parse({...ak, from_the_hip_multiplier:2}, player, false);
const none = WeaponSpread.kitOf([]);
const spreadOf = (aim, stance = 'idle', kit = none, arms = 0, model = rifle) => WeaponSpread.condition(model, {aim, stance, kit, arms});
const aimed = WeaponRecoil.simulate(full,10,7,{aim:'aimed', spread:spreadOf('aimed')}), hip = WeaponRecoil.simulate(full,10,7,{aim:'hip', spread:spreadOf('hip')});
assert.deepEqual(aimed.frames.map(frame=>[frame.vertical,frame.horizontal]), WeaponRecoil.simulate(model,10,7).frames.map(frame=>[frame.vertical,frame.horizontal]), 'spread and shake leave the kick as it was');
assert.ok(aimed.events.every(event=>event.spread===.2), 'aimed: the base spread only');
assert.ok(aimed.frames.every(frame=>frame.shake.side===0&&frame.shake.up===0), 'aimed: no shake');
assert.equal(hip.events[0].spread, 5.2, 'hip: the stance spread on the first shot');
// The fall runs per 50 ms tick: between one and two ticks before the second shot.
assert.ok(hip.events[1].spread >= 5.2+2-10*.1-1e-9 && hip.events[1].spread <= 5.2+2-10*.05+1e-9, 'hip: a shot adds its amount, then it falls');
assert.ok(Math.abs(hip.events[1].time-full.interval/1000)<1e-9, 'a shot at its own moment, 60/rpm after the last');
assert.ok(hip.events.every(event=>Math.hypot(event.bullet.side,event.bullet.up)<=event.spread/2+1e-9), 'a bullet stays inside the cone');
assert.ok(hip.frames.some(frame=>frame.shake.up!==0), 'hip: the shake plays');
assert.ok(hip.frames.at(-1).shake.up===0||Math.abs(hip.frames.at(-1).shake.up)<1e-3, 'the shake dies out');
assert.ok(WeaponRecoil.simulate(full,10,7,{aim:'aiming', spread:spreadOf('aiming')}).frames.every(frame=>frame.shake.up===0), 'aiming in: no shake');
// The state's parts (get_target_koef 0x331560, tick 0x331330, fire 0x331150, 0x3317C0).
const close = (a, b, what) => assert.ok(Math.abs(a - b) < 1e-9, `${what}: ${a} vs ${b}`);
const skills = WeaponSpread.kitOf([{dispersion_from_hip:-.12}, {dispersion_movement_influence:-.12}, {injury_penalty:-.6}, {dispersion:-.1}]);
for (const stance of WeaponSpread.STANCES) for (const arms of [0, 1, 2]) {
  const state = spreadOf('aimed', stance, skills, arms);
  assert.equal(state.k, 0, `${stance}: fully aimed, the base only`);
}
close(spreadOf('hip', 'walk').stance, 10, 'walking from the hip');
// The weapon's Dispersion modification (2020..2024, type 2 on
// dispersion/base_dispersion): the base only, so all of the fully aimed spread.
{
  const less = WeaponSpread.parse(WeaponSpread.rolled(ak, -.1), player, true);
  const at = (aim, stance, kit = none) => WeaponSpread.condition(less, {aim, stance, kit, arms: 0});
  close(at('aimed', 'idle').base, .2 * .9, 'fully aimed: -10 % of the base');
  close(at('hip', 'walk', skills).base, .18, 'from the hip: the same base');
  close(at('hip', 'walk', skills).stance, spreadOf('hip', 'walk', skills).stance, 'the stance\'s part as it was');
  close(at('hip', 'idle').perShot, spreadOf('hip', 'idle').perShot, 'a shot\'s part as it was');
  close(WeaponSpread.rolled(ak, -.04 - .06).base_dispersion, .18, 'two strengths add, then scale');
  close(WeaponSpread.rolled(ak, -2).base_dispersion, 0, 'a sum never below nothing');
  assert.deepEqual(WeaponSpread.rolled(ak), ak, 'no roll, the weapon\'s own');
  const shots = WeaponRecoil.simulate(full, 10, 7, {aim: 'aimed', spread: at('aimed', 'idle')});
  assert.ok(shots.events.every(event => Math.abs(event.spread - .18) < 1e-9), 'fully aimed, every shot\'s spread -10 %');
}
close(spreadOf('hip', 'walk', skills).stance, 10 * (1 - .12 - .12) * .9, 'walking from the hip: both skills added, then the gloves');
close(spreadOf('aiming', 'walk', skills).stance, 10 * .88 * .9, 'walking aiming in: Firm grip, then the gloves');
close(spreadOf('aiming', 'idle', skills).stance, 5 * .9, 'standing aiming in: the raw value, then the gloves');
close(spreadOf('hip', 'run', skills).stance, 20 * .88, 'sprinting: no skill weight, so no gloves');
close(spreadOf('hip', 'run', skills).perShot, 2, 'sprinting: a shot unchanged');
close(spreadOf('hip', 'crouch', skills).perShot, 2 * .9, 'a shot through the gloves');
close(spreadOf('hip', 'idle', none, 1).stance, 15, 'a broken arm, two-handed');
close(spreadOf('hip', 'idle', none, 1, pistol).stance, 10, 'a broken arm, one-handed: none, the hip multiplier 2');
close(spreadOf('hip', 'idle', skills, 2, pistol).stance, (2 * 5 * .88 + 10 * .4) * .9, 'two broken arms, one-handed, Equanimity');
close(spreadOf('hip', 'idle', WeaponSpread.kitOf([{dispersion_from_hip:-2}])).stance, 0, 'a sum never below nothing');
assert.throws(() => spreadOf('prone'), 'no such state');
assert.equal(WeaponSpread.parse(ak, {stance:{idle_dispersion:5}, skill_influence:{}}, true), null, 'missing stances never become zeroes');
// The kit's recoil modifier (18) scales each axis's step a shot, the shake's too (0x331DC7, 0x331EA5, 0x331F5A).
{
  const first = run => run.frames.find(frame => frame.shots === 1 && frame.time >= full.interval / 1000);
  const base = WeaponRecoil.simulate(full,1,7,{aim:'hip'}), less = WeaponRecoil.simulate(full,1,7,{aim:'hip',recoil:-.1});
  close(first(less).vertical, first(base).vertical * .9, 'the first kick, 10% less');
  close(first(less).horizontal, first(base).horizontal * .9, 'the first side kick, 10% less');
  close(first(less).z, first(base).z * .9, 'the first shake step, 10% less');
  assert.ok(WeaponRecoil.simulate(full,10,7,{aim:'hip',recoil:-2}).frames.every(frame=>frame.vertical===0&&frame.z===0), 'a sum never below nothing');
}
// Clicking: the same rules at a slower pace, so the climb recovers between shots.
const held = WeaponRecoil.simulate(model,10,7), clicked = WeaponRecoil.simulate(model,10,7,{},.5);
assert.ok(Math.abs(clicked.events[1].time-clicked.events[0].time-.5)<.051, 'a click every half second');
assert.ok(Math.max(...clicked.frames.map(frame=>frame.vertical)) < Math.max(...held.frames.map(frame=>frame.vertical)), 'clicking climbs less than holding');
assert.deepEqual(WeaponRecoil.simulate(model,10,7,{},.01).frames, held.frames, 'never faster than the weapon fires');
console.log('PASS: native recoil reference, cancellation, recovery, magazine/single shot, deterministic sampling and missing data, spread by state and shake');
if (process.argv[2]) {
  const source=fs.readFileSync(process.argv[2], 'utf8');
  const data=JSON.parse(source.match(/^const DATA = (.+);$/m)[1]);
  // The weapon's Dispersion row: five strengths on the base, every weapon's.
  const rolls=data.simulators.mods.filter(m=>m.kind==='mod'&&m.weapon?.spread!==undefined);
  assert.deepEqual(rolls.map(m=>[m.id,m.weapon]).sort(),[['2020',{spread:-.02}],['2021',{spread:-.04}],['2022',{spread:-.06}],['2023',{spread:-.08}],['2024',{spread:-.1}]],'the Dispersion modifications, on the base spread');
  for (const m of rolls) assert.deepEqual(m.weapons,Object.keys(data.weapons).sort(),`${m.id}: every weapon rolls it`);
  for (const [id,weapon] of Object.entries(data.weapons)) {
    const parsed=WeaponRecoil.parse({rpm:weapon.rounds_per_minute,magazine:weapon.magazine_capacity,...weapon.recoil});
    const spread=WeaponSpread.parse(weapon.dispersion,data.simulators,weapon.double_handed);
    assert.ok(parsed,`${id}: complete recoil data`);
    assert.ok(spread&&parsed.shake,`${id}: spread and shake data`);
    for (const stance of WeaponSpread.STANCES) {
      const firing={aim:'hip',spread:WeaponSpread.condition(spread,{aim:'hip',stance,kit:WeaponSpread.kitOf([]),arms:2})};
      assert.ok(WeaponRecoil.simulate(parsed,10,5,firing).frames.every(frame=>Number.isFinite(frame.vertical)&&Number.isFinite(frame.horizontal)&&Number.isFinite(frame.spread)),`${id}: finite motion`);
    }
  }
  console.log(`PASS: ${Object.keys(data.weapons).length} released weapons parse and simulate, with spread and shake`);
}
// The shot clips' motions (motions.js): a curve is played once a shot, starts
// and ends at rest, and a custom gun's peak scales it or takes the standard shape.
{
  const shot = {camera: {aimed: {up: [0, -1, -.5, 0], side: [0, .5, .25, 0]}}, gun: {back: [0, 30, 10, 0], up: [0, 1.2, .4, 0], side: [0, 0, 0, 0]}};
  const motions = WeaponMotions.parse(shot), none = WeaponMotions.parse(undefined);
  assert.equal(motions.camera.hip, null, 'no hip clip, no hip motion');
  assert.deepEqual(WeaponMotions.parse({camera: {hip: {up: [0, 1], side: [0, 0]}}}).camera.aimed, {up: [0, 1], side: [0, 0]}, 'aimed is the hip clip unless another is given');
  assert.deepEqual(WeaponMotions.cameraAt(motions, 'aimed', 0), {up: 0, side: 0}, 'a pass starts from rest');
  // A bullet leaves along the eye of the step before its shot (steps end every 50 ms).
  for (const [time, eye] of [[.05, 0], [.07, .05], [.1, .05], [.1206, .1], [.15, .1]]) assert.ok(Math.abs(WeaponMotions.eyeTime(time) - eye) < 1e-9, `the eye of a shot at ${time}`);
  const burst = [{time: .05}, {time: .11}, {time: .17}];
  assert.equal(WeaponMotions.phaseAt(burst, .06, .04), 1, 'no shot yet');
  assert.ok(Math.abs(WeaponMotions.phaseAt(burst, .06, .1) - 5 / 6) < 1e-9, 'late in the first pass');
  assert.deepEqual(WeaponMotions.cameraAtShot(motions, 'aimed', burst, .06, .05), {up: 0, side: 0}, 'the first bullet leaves from rest');
  // the second shot, at 0.11: the eye of 0.10, 5/6 into the first pass, halfway down the curve's last third
  const second = WeaponMotions.cameraAtShot(motions, 'aimed', burst, .06, .11);
  assert.ok(Math.abs(second.up + .25) < 1e-9 && Math.abs(second.side - .125) < 1e-9, `the second takes the turn of the pass before (${JSON.stringify(second)})`);
  assert.deepEqual(WeaponMotions.cameraAt(motions, 'aiming', 1 / 3), {up: -1, side: .5}, 'the aimed clip while aiming in');
  assert.deepEqual(WeaponMotions.cameraAt(motions, 'hip', 1 / 3), {up: 0, side: 0});
  assert.deepEqual(WeaponMotions.cameraAt(motions, 'aimed', 1), {up: 0, side: 0}, 'over with its pass');
  assert.ok(Math.abs(WeaponMotions.gunAt(motions, 1 / 6).back - 15) < 1e-9, 'between two samples');
  assert.deepEqual(WeaponMotions.gunAt(none, .3), {back: 0, up: 0, side: 0}, 'a build without the clips moves nothing');
  assert.equal(WeaponMotions.peak(motions.camera.aimed, 'up'), -1);
  assert.equal(WeaponMotions.parse({gun: {back: [0, 1], up: [0, NaN], side: [0, 0]}}).gun, null, 'a broken curve is no curve');
  const same = WeaponMotions.custom(motions, {});
  assert.deepEqual(same.gun, motions.gun, 'no custom value: the same curves');
  const more = WeaponMotions.custom(motions, {camera: {up: -2}, gun: {back: 15, side: .5}});
  assert.deepEqual(more.camera.aimed.up, [0, -2, -1, 0], 'its own shape, to the custom peak');
  assert.deepEqual(more.gun.back, [0, 15, 5, 0]);
  assert.equal(WeaponMotions.peak(more.gun, 'side'), .5, 'the standard shape where the weapon has none');
  assert.equal(more.gun.side[0], 0);
  assert.equal(more.gun.side.at(-1), 0);
  assert.equal(more.camera.hip.up.length, 33, 'a custom camera motion from the hip too');
  assert.equal(WeaponMotions.custom(none, {}).gun, null);
  assert.equal(WeaponMotions.custom(motions, {gun: {back: 0, up: 0}}).gun, null, 'all zero: no motion');
}
