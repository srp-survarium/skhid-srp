'use strict';
// Development audit only. Gameplay continues to read the built resources.db.
const fs = require('node:fs');
const path = require('node:path');
const E = require('../damage-calc/engine.js');
const ROOT = path.resolve(__dirname, '../content-edit');
const REF = require('./reference.json');
const DISTANCES = [0, 10, 15, 20, 25, 30, 40, 50, 75, 100];
const WEAPONS = ['ak_12', 'ots14', 'scar_h', 'vhs_2', 'fn_minimi', 'mp5a1',
  'svu_a', 'l85a2', 'fort_401', 'jackhammer', 'cz_75', 'mp443', 'beretta93r',
  'glock_17', 'colt_m1911', 'akmsu', 'famas', 'g3sg1', 'mauser_m712', 'ks_23m',
  'ppsh_drum', 'pkp_pecheneg'].map(n => 'wpn_' + n);
function table(file) {
  const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/).filter(l => l && !l.startsWith('#'));
  const header = lines.shift().split('\t');
  return Object.fromEntries(lines.map(line => {
    const cells = line.split('\t');
    if (cells.length !== header.length) throw new Error(`Invalid row in ${file}: ${cells[0]}`);
    return [cells[0], Object.fromEntries(header.slice(1).map((k, i) => {
      const cell = cells[i + 1];
      return [k, cell === '' ? null : Number.isFinite(Number(cell)) ? Number(cell) : cell];
    }))];
  }));
}
function weapon(r) {
  return {bullet_damage:r.damage, bullet_pierce:r.pierce, min_damage_amount:r.far_damage,
    max_damage_distance:r.near_m, min_damage_distance:r.far_m};
}
function ammo(r, key) {
  const a = REF.ammo[key];
  if (!a || !r) throw new Error(`Missing ammunition: ${key}`);
  return {k_damage:r.damage, k_arp:r.pierce, k_min_damage_amount:r.far_damage_k,
    k_max_damage_distance:r.near_k, k_min_damage_distance:r.far_k,
    muzzle_speed:r.speed, buck_shot:r.pellets, air_resistance:a.air_resistance, distance:a.distance};
}
function target() {
  const armour = {};
  const armourStats = table(path.join(ROOT, 'armor.tsv'));
  const modification = table(path.join(ROOT, 'modifications.tsv'))['1004'];
  if (modification.op !== 1) throw new Error('Expected an additive armour modifier');
  for (const [key, item] of Object.entries(REF.armour)) {
    const row = Object.entries(armourStats).find(([name]) => name.endsWith('/' + key))?.[1];
    if (!row) throw new Error(`Missing Zubr piece: ${key}`);
    for (const [part, types] of Object.entries(item.hit_params)) {
      const effect = REF.mods['1004'].effects.find(e => e.path === `hit_params/${part}/hit_types/ranged/armor`);
      if (!effect || effect.op !== 'add') throw new Error(`Missing +8 armour modifier: ${part}`);
      const base = part === 'back' && row.back !== null ? row.back
        : part.endsWith('_forearm') && row.arms !== null ? row.arms : row.armor;
      armour[part] = {ranged: base + modification.value};
    }
  }
  return E.makeTarget({body:REF.body, armour});
}
// Pump/bolt action cannot fire before chambering finishes. Burst/semi figures
// assume immediate re-trigger; actual input and animation scheduling add latency.
function interval(r) { return Math.max(60 / r.rpm, r.chamber ?? 0); }
function firingTime(r, shots) {
  // Avoid inventing timings for interruptible shell reloads or burst re-triggers.
  // A target requiring a reload has no single-magazine TTK in this audit.
  if (!Number.isFinite(shots) || shots > r.magazine) return Infinity;
  return (shots - 1) * interval(r);
}
function evaluate(r, a, distance, part, pellets = null, victim = target()) {
  const b = E.bulletAt(weapon(r), a, distance);
  // bulletAt clamps drag at lifetime; a bullet beyond that reach never arrives.
  const reachable = distance <= Math.min(b.reach, b.maxDistance);
  const result = reachable ? E.simulate(victim, b, part, 'ranged', 400, pellets) : {shots:Infinity};
  const shots = result.shots;
  return {shots, ttk:firingTime(r, shots), kills:Math.floor(r.magazine / shots),
    hit_damage:b.D * E.armourFactor(victim.armour[part].ranged, b.P),
    penetration:b.P, pellets:pellets ?? b.pellets};
}
function run(statsFile, ammoFile = path.join(ROOT, 'ammo.tsv')) {
  const stats = table(statsFile), ammos = table(ammoFile), victim = target();
  const out = [];
  for (const key of WEAPONS) {
    const r = stats[key];
    if (!r || !REF.compat[key]?.length) throw new Error(`Missing weapon or ammo compatibility: ${key}`);
    for (const distance of DISTANCES) for (const part of ['front', 'brain']) {
      const choices = REF.compat[key].map(ak => {
        const a = ammo(ammos[ak], ak);
        return {ammo:ak, ...evaluate(r, a, distance, part, null, victim)};
      }).sort((a,b) => a.ttk-b.ttk || b.hit_damage*b.pellets-a.hit_damage*a.pellets || a.ammo.localeCompare(b.ammo));
      const best = choices[0];
      out.push({weapon:key, distance, part, ...best, magazine:r.magazine, interval:interval(r),
        kills_half_accuracy:Math.floor(r.magazine / (best.shots * 2)),
        half_pellets_shots:best.pellets > 1 ? evaluate(r, ammo(ammos[best.ammo], best.ammo),
          distance, part, Math.floor(best.pellets / 2), victim).shots : null});
    }
  }
  return out;
}
function tsv(rows) {
  const fields = ['weapon','distance','part','ammo','shots','ttk','kills','magazine','interval','penetration','hit_damage','pellets','kills_half_accuracy','half_pellets_shots'];
  const cell = v => typeof v === 'number' ? (Number.isFinite(v) ? Number(v.toFixed(6)) : 'n/a') : v;
  return fields.join('\t')+'\n'+rows.map(r => fields.map(k=>cell(r[k])).join('\t')).join('\n')+'\n';
}
if (require.main === module) process.stdout.write(tsv(run(process.argv[2] || path.join(ROOT,'weapon-stats.tsv'), process.argv[3])));
module.exports = {run, table, weapon, ammo, target, interval, firingTime, evaluate, WEAPONS, DISTANCES, tsv};
