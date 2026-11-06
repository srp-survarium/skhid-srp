'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const A = require('./audit.js');
const root = path.resolve(__dirname, '../content-edit');
const stats = A.table(path.join(root, 'weapon-stats.tsv'));
const ammos = A.table(path.join(root, 'ammo.tsv'));
const results = A.run(path.join(root, 'weapon-stats.tsv'));
const pick = (weapon, distance, part = 'front') => results.find(r => r.weapon === weapon && r.distance === distance && r.part === part);

test('Zubr +8 adds eight points to each protected part, with a separate helmet', () => {
  assert.equal(A.target().armour.front.ranged, .98);
  assert.equal(A.target().armour.brain.ranged, .83);
  assert.equal(A.target().armour.right_forearm.ranged, .98);
  assert.equal(A.target().max.body, 1);
  assert.equal(A.target().max.head, .4);
});
test('AK-12 anchor includes armour and distance-dependent penetration', () => {
  assert.equal(pick('wpn_ak_12', 0).shots, 5);
  assert.equal(pick('wpn_ak_12', 30).shots, 7);
  assert.equal(pick('wpn_ak_12', 50).shots, 13);
  assert.equal(pick('wpn_ak_12', 0).ttk, 4 * 60 / 650);
  assert.equal(pick('wpn_ak_12', 50).kills, 2);
  assert.equal(pick('wpn_ak_12', 0, 'brain').shots, 2);
});
test('best ammo comes from compatibility, including pistol 7N31 and Groza SP-5', () => {
  assert.equal(pick('wpn_mp5a1', 10).ammo, 'ammo_9x19p_7n31');
  assert.equal(pick('wpn_ots14', 30).ammo, 'ammo_sp5');
  assert.equal(pick('wpn_ak_12', 10).ammo, 'ammo_6.5x39');
});
test('Groza penetration does not buy an automatic one-shot kill through the Zubr helmet', () => {
  for (const key of ['ammo_sp5', 'ammo_sp6']) {
    const r = A.evaluate(stats.wpn_ots14, A.ammo(ammos[key], key), 0, 'brain');
    assert.equal(r.shots, 2);
  }
});
test('pump interval honours chambering; full and half pellet hits differ', () => {
  const r = stats.wpn_ks_23m;
  assert.equal(A.interval(r), .75);
  assert.equal(pick('wpn_ks_23m', 0).shots, 2);
  assert.equal(pick('wpn_ks_23m', 0).half_pellets_shots, 4);
  assert.equal(pick('wpn_ks_23m', 0).ttk, .75);
});
test('cannot report a single-magazine kill or TTK for a weak Mauser against Zubr', () => {
  const r = pick('wpn_mauser_m712', 0);
  assert.equal(r.shots, 11);
  assert.equal(r.kills, 0);
  assert.equal(r.ttk, Infinity);
});
test('a one-shot kill starts at zero; a round beyond drag lifetime never arrives', () => {
  assert.equal(A.firingTime(stats.wpn_svu_a, 1), 0);
  const r = A.evaluate(stats.wpn_mp5a1, A.ammo(ammos.ammo_9x19p_7n31, 'ammo_9x19p_7n31'), 1000, 'front');
  assert.equal(r.shots, Infinity);
  assert.equal(r.kills, 0);
});
test('each weapon has every distance and hit location, with a conservative miss budget', () => {
  assert.equal(results.length, A.WEAPONS.length * A.DISTANCES.length * 2);
  for (const r of results) {
    assert.ok(r.kills_half_accuracy <= r.kills);
    assert.equal(r.kills, Math.floor(r.magazine / r.shots));
  }
});
