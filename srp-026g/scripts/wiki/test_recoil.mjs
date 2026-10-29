import assert from 'node:assert/strict';
import fs from 'node:fs';
import './recoil.js';
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
// Spread and shake (dispersion_calculator, the shake axis): an AK-74M's values.
globalThis.CAMERA_SHAKE ??= {period_frames:2, pitch:[.3,-.3], yaw:[.2,-.2]};
const full = WeaponRecoil.parse({...raw, min_shake_recoil:.8, max_shake_recoil:1, shake_time_scale:8,
  dispersion:{base_dispersion:.2, one_shoot_dispersion_amount:2, max_dispersion:20, speed_of_aiming:10},
  stance:{idle_dispersion:5}});
const aimed = WeaponRecoil.simulate(full,10,7), hip = WeaponRecoil.simulate(full,10,7,true);
assert.deepEqual(aimed.frames.map(frame=>[frame.vertical,frame.horizontal]), WeaponRecoil.simulate(model,10,7).frames.map(frame=>[frame.vertical,frame.horizontal]), 'spread and shake leave the kick as it was');
assert.ok(aimed.events.every(event=>event.spread===.2), 'aimed: the base spread only');
assert.ok(aimed.frames.every(frame=>frame.shake.side===0&&frame.shake.up===0), 'aimed: no shake');
assert.equal(hip.events[0].spread, 5.2, 'hip: the stance spread on the first shot');
assert.ok(Math.abs(hip.events[1].spread-(5.2+2-10*(hip.events[1].time-hip.events[0].time)))<1e-9, 'hip: a shot adds its amount, then it falls');
assert.ok(hip.events.every(event=>Math.hypot(event.bullet.side,event.bullet.up)<=event.spread/2+1e-9), 'a bullet stays inside the cone');
assert.ok(hip.frames.some(frame=>frame.shake.up!==0), 'hip: the shake plays');
assert.ok(hip.frames.at(-1).shake.up===0||Math.abs(hip.frames.at(-1).shake.up)<1e-3, 'the shake dies out');
// Clicking: the same rules at a slower pace, so the climb recovers between shots.
const held = WeaponRecoil.simulate(model,10,7), clicked = WeaponRecoil.simulate(model,10,7,false,.5);
assert.ok(Math.abs(clicked.events[1].time-clicked.events[0].time-.5)<.051, 'a click every half second');
assert.ok(Math.max(...clicked.frames.map(frame=>frame.vertical)) < Math.max(...held.frames.map(frame=>frame.vertical)), 'clicking climbs less than holding');
assert.deepEqual(WeaponRecoil.simulate(model,10,7,false,.01).frames, held.frames, 'never faster than the weapon fires');
console.log('PASS: native recoil reference, cancellation, recovery, magazine/single shot, deterministic sampling and missing data, spread and shake');
if (process.argv[2]) {
  const source=fs.readFileSync(process.argv[2], 'utf8');
  const data=JSON.parse(source.match(/^const DATA = (.+);$/m)[1]);
  for (const [id,weapon] of Object.entries(data.weapons)) {
    const parsed=WeaponRecoil.parse({rpm:weapon.rounds_per_minute,magazine:weapon.magazine_capacity,...weapon.recoil,
      dispersion:weapon.dispersion,stance:data.character_dispersion});
    assert.ok(parsed,`${id}: complete recoil data`);
    assert.ok(parsed.spread&&parsed.shake,`${id}: spread and shake data`);
    assert.ok(WeaponRecoil.simulate(parsed,10,5,true).frames.every(frame=>Number.isFinite(frame.vertical)&&Number.isFinite(frame.horizontal)&&Number.isFinite(frame.spread)),`${id}: finite motion`);
  }
  console.log(`PASS: ${Object.keys(data.weapons).length} released weapons parse and simulate, with spread and shake`);
}
