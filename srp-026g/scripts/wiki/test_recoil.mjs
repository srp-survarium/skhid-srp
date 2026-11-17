import assert from 'node:assert/strict';
import fs from 'node:fs';
import './recoil.js';
import './spread.js';
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
// Clicking: the same rules at a slower pace, so the climb recovers between shots.
const held = WeaponRecoil.simulate(model,10,7), clicked = WeaponRecoil.simulate(model,10,7,{},.5);
assert.ok(Math.abs(clicked.events[1].time-clicked.events[0].time-.5)<.051, 'a click every half second');
assert.ok(Math.max(...clicked.frames.map(frame=>frame.vertical)) < Math.max(...held.frames.map(frame=>frame.vertical)), 'clicking climbs less than holding');
assert.deepEqual(WeaponRecoil.simulate(model,10,7,{},.01).frames, held.frames, 'never faster than the weapon fires');
console.log('PASS: native recoil reference, cancellation, recovery, magazine/single shot, deterministic sampling and missing data, spread by state and shake');
if (process.argv[2]) {
  const source=fs.readFileSync(process.argv[2], 'utf8');
  const data=JSON.parse(source.match(/^const DATA = (.+);$/m)[1]);
  for (const [id,weapon] of Object.entries(data.weapons)) {
    const parsed=WeaponRecoil.parse({rpm:weapon.rounds_per_minute,magazine:weapon.magazine_capacity,...weapon.recoil});
    const spread=WeaponSpread.parse(weapon.dispersion,data.spread,weapon.double_handed);
    assert.ok(parsed,`${id}: complete recoil data`);
    assert.ok(spread&&parsed.shake,`${id}: spread and shake data`);
    for (const stance of WeaponSpread.STANCES) {
      const firing={aim:'hip',spread:WeaponSpread.condition(spread,{aim:'hip',stance,kit:WeaponSpread.kitOf([]),arms:2})};
      assert.ok(WeaponRecoil.simulate(parsed,10,5,firing).frames.every(frame=>Number.isFinite(frame.vertical)&&Number.isFinite(frame.horizontal)&&Number.isFinite(frame.spread)),`${id}: finite motion`);
    }
  }
  console.log(`PASS: ${Object.keys(data.weapons).length} released weapons parse and simulate, with spread and shake`);
}
