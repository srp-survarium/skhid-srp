// The 0.26g client's damage path, as tools/re-kit/v026g/damage/formula.txt
// recovers it. Health is in the game's units (body 1.0); the page shows x100.
// Pure functions only, so tools can run them under node as well.

const clamp = (x, lo, hi) => Math.min(hi, Math.max(lo, x));

// A modifier sum S applied to a base B: B*max(0,1+S), or B+S for the indices
// the client adds (formula.txt section 4).
function applyModifier(base, sum, additive) {
  return additive ? base + sum : base * Math.max(0, 1 + sum);
}

// Speed of a bullet `r` metres out, drag only (0x32c520 with zero gravity).
function speedAt(v0, drag, r) {
  if (!(drag > 0)) return v0;
  const tc = 1 / (1.1 * drag);
  const reach = v0 * (tc - drag * tc * tc / 2);
  const d = Math.min(Math.max(r, 0), reach);
  const t = Math.min(tc, (1 - Math.sqrt(Math.max(0, 1 - 2 * d * drag / v0))) / drag);
  return v0 * Math.max(0, 1 - drag * t);
}

// What leaves the barrel and what is left of it `distance` metres out.
// `w` holds the weapon's effective numbers (after its bonuses), `a` the ammo.
// `wall`: the armour of a wall just in front of the target, or null.
function bulletAt(w, a, distance, wall = null) {
  const W = w.bullet_damage, Q = w.bullet_pierce, m = w.min_damage_amount;
  const Dmax = W * a.k_damage;
  const Dmin = W * m * a.k_min_damage_amount * a.k_damage;
  const Pmax = Q * a.k_arp;
  const Pmin = Q * m * a.k_min_damage_amount * a.k_arp;
  const v0 = a.muzzle_speed, drag = a.air_resistance;
  const tc = 1 / (1.1 * drag);
  const reach = v0 * (tc - drag * tc * tc / 2);
  const r0 = Math.min(w.max_damage_distance * a.k_max_damage_distance, reach);
  const r1 = Math.min(w.min_damage_distance * a.k_min_damage_distance, reach);
  const vFull = speedAt(v0, drag, r0), vLow = speedAt(v0, drag, r1);
  const v = speedAt(v0, drag, distance);
  const u = vFull === vLow ? (distance <= r0 ? 1 : 0) : clamp((v - vLow) / (vFull - vLow), 0, 1);
  let D = Dmin + (Dmax - Dmin) * u, P = Pmin + (Pmax - Pmin) * u;
  // A wall (try_pierce 0x32D960): k = 1 − (1 − t)², t = clamp((P − A) / A, 0, 1),
  // 1 when A = 0; k = 0 stops the bullet. Damage, penetration and speed × k;
  // at the target the slower bullet falls off again, never back up
  // (update_bullet_position 0x32EE90: D, P = min(D, P; their value at its speed)).
  let through = null;
  if (wall != null) {
    const t = wall > 0 ? clamp((P - wall) / wall, 0, 1) : 1;
    const k = 1 - (1 - t) ** 2, vk = k * v;
    const uk = vFull === vLow ? (k < 1 ? 0 : u) : clamp((vk - vLow) / (vFull - vLow), 0, 1);
    through = { A: wall, k, v: vk, u: uk, D, P };
    D = k > 0 ? Math.min(k * D, Dmin + (Dmax - Dmin) * uk) : 0;
    P = k > 0 ? Math.min(k * P, Pmin + (Pmax - Pmin) * uk) : 0;
  }
  return {
    Dmax, Dmin, Pmax, Pmin, r0, r1, reach, v0, v, u, D, P, wall: through,
    pellets: Math.max(1, Math.round(a.buck_shot || 1)),
    maxDistance: a.distance || Infinity,
  };
}

// Armour reduces damage by what it has beyond the penetration, in [0, 1].
function armourFactor(A, P) {
  return 1 - clamp(A - P, 0, 1);
}

// The target: its body rows, what armour each location gets, and the
// callbacks its items, artefacts and drugs register.
//   body:     DATA.body rows
//   armour:   { part: { type: armour } } summed over equipped items
//   incoming: [{ name, parts, types, fn(D, P, state, ctx) -> {D, P, note} }]
//   final:    same shape, after armour
//   hmax:     { pool: max health } overrides (pain_health_modifier...)
//   start:    { pool: health } the target already has, if not full
function makeTarget({ body, armour = {}, incoming = [], final = [], hmax = {}, secondChance = null, start = {} }) {
  const rows = Object.fromEntries(body.map(row => [row.name, row]));
  const max = {};
  for (const row of body) if (row.health > 0) max[row.name] = hmax[row.name] ?? row.health;
  const H0 = {};
  for (const p of Object.keys(max)) H0[p] = clamp(start[p] ?? max[p], 0, max[p]);
  return { rows, armour, incoming, final, max, start: H0, secondChance };
}

function freshState(target) {
  const H = { ...target.start };
  return { H, seen: {}, fired: {}, counters: {}, secondChanceArmed: !!target.secondChance };
}

// body_part_parameters::hit_by_type for one part, then the pools it routes to
// (0x329ff0, 0x33fa80). `log` gets one entry per step for the page.
function hitPart(target, state, part, D, P, type, bulletId, log, depth = 0) {
  const row = target.rows[part];
  if (!row) return;
  const ctx = { part, type, depth, bulletId };
  for (const cb of target.incoming) {
    if (cb.types && !cb.types.includes(type)) continue;
    if (cb.parts && !cb.parts.includes(part)) continue;
    const before = D;
    const out = cb.fn(D, P, state, ctx);
    D = out.D; P = out.P;
    log && log.push({ step: "callback", part, depth, name: cb.name, before, D, P, note: out.note });
  }
  const A = (target.armour[part] && target.armour[part][type]) || 0;
  const k = armourFactor(A, P);
  const beforeArmour = D;
  D = D * k;
  log && log.push({ step: "armour", part, depth, A, P, k, before: beforeArmour, D });
  for (const cb of target.final) {
    if (cb.types && !cb.types.includes(type)) continue;
    if (cb.parts && !cb.parts.includes(part)) continue;
    const out = cb.fn(D, P, state, ctx);
    log && log.push({ step: "callback", part, depth, name: cb.name, before: D, D: out.D, P: out.P, note: out.note });
    D = out.D; P = out.P;
  }
  const hasPool = part in state.H;
  if (hasPool && target.secondChance && state.secondChanceArmed) {
    const sc = target.secondChance; // { trigger, remain } as fractions of Hmax
    const Hmax = target.max[part];
    if (D > sc.trigger * Hmax && D > state.H[part]) {
      const nd = state.H[part] - sc.remain * Hmax;
      log && log.push({ step: "second_chance", part, depth, before: D, D: nd });
      D = nd; state.secondChanceArmed = false;
    }
  }
  // One bullet, one pool: only its largest contribution counts (section 7).
  if (bulletId >= 0) {
    const key = part + "#" + bulletId;
    const prev = state.seen[key];
    if (prev !== undefined) {
      if (D <= prev) { log && log.push({ step: "dedupe", part, depth, D: 0 }); return; }
      state.seen[key] = D; D = D - prev;
    } else state.seen[key] = D;
  }
  const invulnerable = row.invulnerable && row.invulnerable.includes(type);
  if (hasPool && !invulnerable && D > 0) {
    const before = state.H[part];
    state.H[part] = clamp(before - D, 0, target.max[part]);
    log && log.push({ step: "pool", part, depth, D, before, after: state.H[part] });
  }
  const routes = (row.routes && row.routes[type]) || {};
  for (const [dest, c] of Object.entries(routes)) {
    if (c > 0) hitPart(target, state, dest, c * D, 0, type, bulletId, log, depth + 1);
  }
}

// Which pools are fatal, and the movement/injury thresholds of the others.
function thresholdsOf(row) {
  return (row.thresholds || []).map(t => ({
    value: t.value,
    affects: Array.isArray(t.affects) ? t.affects : [],
    movement: t.movement_speed_modifier || 0,
  }));
}
function isFatal(row) {
  return thresholdsOf(row).some(t => t.value === 0 && t.affects.includes(0));
}
function crossed(H, Hmax, threshold) {
  return threshold === 0 ? H <= 0 : H < threshold * Hmax;
}

// Fire `shots` at one location until the target dies (or `limit` shots).
// Each shot is `pellets` bullets with their own ids.
function simulate(target, bullet, loc, type = "ranged", limit = 400, pelletsHit = null) {
  const state = freshState(target);
  const pellets = pelletsHit ?? bullet.pellets;
  const firstLog = [];
  const events = [];
  const fatal = Object.keys(state.H).filter(p => isFatal(target.rows[p]));
  // Thresholds the target is already past do not fire again.
  const seenThreshold = new Set();
  for (const p of Object.keys(state.H)) thresholdsOf(target.rows[p]).forEach((t, i) => {
    if (crossed(state.H[p], target.max[p], t.value)) seenThreshold.add(p + ":" + i);
  });
  let id = 0, shots = 0, killer = null, firstShot = null;
  const history = [{ ...state.H }];
  while (shots < limit && !killer) {
    shots++;
    const before = { ...state.H };
    for (let i = 0; i < pellets; i++) {
      hitPart(target, state, loc, bullet.D, bullet.P, type, id++, shots === 1 ? firstLog : null);
    }
    const dealt = {};
    for (const p of Object.keys(state.H)) dealt[p] = before[p] - state.H[p];
    if (shots === 1) firstShot = dealt;
    history.push({ ...state.H });
    for (const p of Object.keys(state.H)) {
      thresholdsOf(target.rows[p]).forEach((t, i) => {
        const key = p + ":" + i;
        if (!seenThreshold.has(key) && crossed(state.H[p], target.max[p], t.value)) {
          seenThreshold.add(key);
          events.push({ shot: shots, pool: p, threshold: t.value, affects: t.affects, movement: t.movement });
        }
      });
    }
    for (const p of fatal) if (crossed(state.H[p], target.max[p], 0)) { killer = p; break; }
    if (Object.values(dealt).every(v => v <= 1e-9) && shots > 2) break;
  }
  return { firstLog, firstShot, events, history, shots: killer ? shots : Infinity, killer, final: state.H };
}

if (typeof module !== "undefined") {
  module.exports = { clamp, applyModifier, speedAt, bulletAt, armourFactor, makeTarget, hitPart, simulate, isFatal, thresholdsOf };
}
