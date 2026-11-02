// Fake answers of the match history's JSON (/api/history/..., in the shapes of
// docs/notes/match_history.md) for check_history.mjs: twelve players, two
// clans, 23 public matches (Yasen in the newest 15 only, under 1000 shots),
// and Nova, RAD's commander, signed up with none (so not found, and not
// listed in RAD). Each line used one kit, every fourth two; one line a match
// has no trusted snapshot (no shots); one weapon is gone from the game data.
const NAMES = ['Sheep', 'Volk', 'Kvitka', 'Lys', 'Bars', 'Grim', 'Sova', 'Taras', 'Zhuk', 'Orel', 'Dub', 'Yasen', 'Nova'];
const CLANS = [{id: 7, name: 'Skhid', tag: 'SKH', level: 5, leader: 1042}, {id: 9, name: 'Radianty', tag: 'RAD', level: 3, leader: 1054}];
const CLAN_OF = {Sheep: 7, Volk: 7, Bars: 7, Lys: 9, Grim: 9, Nova: 9};
// Weapons (slot 7 main, 10 secondary) and armour (slots 0..6); 175 is gone from the game data.
const MAIN = [[155, 'AK-74M'], [170, 'SVD'], [180, 'Saiga-12K'], [175, null], [160, 'PKM'], [165, 'Vintorez']];
const SECOND = [[60, 'TT-33'], [62, 'MP-443 Grach']];
const ARMOUR = [[301, 'SSh-68 helmet'], [310, 'Bulat vest'], [320, 'Gorka jacket'], [330, 'Gorka trousers'], [340, 'Tactical boots']];
const MAPS = [[1, 'Vostok', 'Batteries'], [3, 'Rudnya', 'Research'], [5, 'Pobeda Factory', 'Team deathmatch'], [7, 'Forest Fort', 'Batteries']];
let seed = 26;
const random = n => { seed = (seed * 1103515245 + 12345) % 2147483648; return Math.floor(seed / 65536) % n; };
const round = (value, places) => Math.round(value * 10 ** places) / 10 ** places;
const players = NAMES.map((name, i) => ({id: 1042 + i, name, level: name === 'Nova' ? 1 : 34 - 2 * i, experience: name === 'Nova' ? 0 : 1234567 - 90000 * i, rating: 1180 - 20 * i}));
const ref = p => ({id: p.id, name: p.name, clan: CLAN_OF[p.name] ? {id: CLAN_OF[p.name], tag: CLANS.find(c => c.id === CLAN_OF[p.name]).tag} : null});
const COUNTERS = ['matches', 'wins', 'losses', 'kills', 'deaths', 'assists', 'score', 'headshots', 'grenade_kills', 'melee_kills', 'batteries', 'captures', 'artefacts', 'shots', 'hits'];
const accuracy = (shots, hits) => shots ? round(hits / shots, 3) : 0;
function totals(lines) {
  const t = Object.fromEntries(COUNTERS.map(k => [k, 0]));
  for (const l of lines) {
    t.matches++; t.wins += l.won ? 1 : 0; t.losses += l.won ? 0 : 1;
    for (const k of COUNTERS.slice(3)) t[k] += l[k];
  }
  t.captures = round(t.captures, 2);
  return {...t, accuracy: accuracy(t.shots, t.hits), kd: round(t.kills / Math.max(t.deaths, 1), 2), win_rate: t.matches ? round(t.wins / t.matches, 3) : 0, score_avg: t.matches ? round(t.score / t.matches, 1) : 0};
}
// A line's shots, hits and the kits it used (place 0, and 2 every fourth
// line), each with its share of the counters. No trusted snapshot: no shots,
// one kit with counters 0.
function kitsOf(line, i, k) {
  const item = ([id, name], slot) => ({slot, id, name});
  const kit = (place, kills, headshots, shots, hits) => ({kit: place, kills, headshots, shots, hits, accuracy: accuracy(shots, hits),
    weapons: [item(MAIN[(k + (i >> 2) + 2 * place) % MAIN.length], 7), ...((k + i) % 3 ? [item(SECOND[k % 2], 10)] : [])],
    armour: ARMOUR.slice(0, 2 + (k + place) % 4).map((a, slot) => item(a, slot))});
  if (k === 10 && i % 11 === 0) return {shots: 0, hits: 0, accuracy: 0, kits: [kit(0, 0, 0, 0, 0)]};
  const shots = line.player.name === 'Yasen' ? 3 * line.kills + 5 : 22 * line.kills + 15 * ((3 * i + k) % 9) + 20;
  const hits = Math.round(shots * (0.18 + 0.03 * ((i + 2 * k) % 7)));
  if ((i + k) % 4) return {shots, hits, accuracy: accuracy(shots, hits), kits: [kit(0, line.kills, line.headshots, shots, hits)]};
  const [kills, headshots, part, partHits] = [line.kills >> 1, line.headshots >> 1, shots >> 2, hits >> 2];
  return {shots, hits, accuracy: accuracy(shots, hits),
    kits: [kit(0, line.kills - kills, line.headshots - headshots, shots - part, hits - partHits), kit(2, kills, headshots, part, partHits)]};
}
const END = 1793500000;
const matches = [];
for (let i = 0; i < 23; i++) {
  const [map_id, map, mode] = MAPS[i % MAPS.length];
  const id = 5509 + i, winner = i === 4 ? 'draw' : random(2);
  const team_points = winner === 'draw' ? [3, 3] : winner === 0 ? [5, 2 + random(3)] : [1 + random(4), 5];
  const all = players.slice(0, 12).map((p, k) => {
    const team = k % 2, kills = random(18), deaths = 1 + random(12);
    return {player: ref(p), level: p.level, team, place: 0, won: winner !== 'draw' && winner === team, left: k === 11 && i % 5 === 0,
      rating: p.rating, score: 60 + 25 * kills + random(90), kills, deaths, kd: round(kills / deaths, 2), assists: random(6),
      headshots: random(Math.max(kills, 1)), grenade_kills: random(2), melee_kills: random(10) === 0 ? 1 : 0,
      batteries: mode === 'Batteries' ? random(4) : 0, captures: mode === 'Research' ? round(random(300) / 100, 2) : 0,
      // Not drawn from `random`, so the other numbers stay as they were.
      artefacts: (7 * i + 3 * k) % 4};
  }).map((l, k) => ({...l, ...kitsOf(l, i, k)}));
  const lines = all.filter(l => l.player.name !== 'Yasen' || i >= 8);
  for (const team of [0, 1]) lines.filter(l => l.team === team).sort((a, b) => b.score - a.score).forEach((l, k) => { l.place = k + 1; });
  lines.sort((a, b) => a.team - b.team || a.place - b.place);
  matches.push({match: {id, ended_at: END - (22 - i) * 5400, duration_s: 600 + random(600), map_id, map, mode, level: 12, winner, team_points, players: lines.length}, lines});
}
const linesOf = p => matches.map(m => m.lines.find(l => l.player.id === p.id)).filter(Boolean);
const card = p => ({player: ref(p), level: p.level, rating: p.rating, totals: totals(linesOf(p))});
const played = p => linesOf(p).length > 0;
const clanMembers = clan => players.filter(p => CLAN_OF[p.name] === clan.id);
const clanCard = clan => ({clan: {id: clan.id, name: clan.name, tag: clan.tag, level: clan.level, members: clanMembers(clan).length,
  leader: [players.find(p => p.id === clan.leader)].filter(played).map(ref)[0] ?? null},
  totals: totals(clanMembers(clan).flatMap(linesOf)),
  members: clanMembers(clan).filter(played).map(p => ({...card(p), commander: p.id === clan.leader}))
    .sort((a, b) => b.commander - a.commander || b.totals.score - a.totals.score)});
function page(list, params) {
  const before = Number(params.get('before')) || Infinity, limit = Math.min(Number(params.get('limit')) || 15, 50);
  const rows = list.filter(r => r.id < before).sort((a, b) => b.id - a.id);
  // `next` after every full page: a full last one is followed by an empty one.
  const shown = rows.slice(0, limit);
  return {matches: shown, next: shown.length === limit ? shown[limit - 1].id : null};
}

// The answer to GET /api/history/<path>?<params>: [status, body].
export function answer(path, params) {
  const found = body => body ? [200, body] : [404, {error: 'not found'}];
  const player = () => (params.has('name')
    ? players.filter(p => p.name.toLowerCase() === params.get('name').toLowerCase())
    : players.filter(p => p.id === Number(params.get('id')))).find(played);
  const clan = () => CLANS.find(c => params.has('tag') ? c.tag.toLowerCase() === params.get('tag').toLowerCase() : c.id === Number(params.get('id')));
  switch (path) {
  case 'player': { const p = player(); return found(p && {...card(p), experience: p.experience}); }
  case 'player/matches': {
    const p = player();
    if (!p) return [200, {matches: [], next: null}];
    // The match and the player's line (its level is the match's).
    const rows = matches.flatMap(m => m.lines.filter(l => l.player.id === p.id)
      .map(({player: _, level: __, ...line}) => ({...m.match, ...line})));
    return [200, page(rows, params)];
  }
  case 'player/equipment': {
    const p = player(), limit = Math.min(Number(params.get('limit')) || 10, 50);
    const items = {weapons: new Map(), armour: new Map()};
    for (const l of p ? linesOf(p) : []) {
      const seen = new Set();
      for (const kit of l.kits) {
        for (const [kind, list] of [['weapons', kit.weapons], ['armour', kit.armour]]) {
          for (const {id, name} of list) {
            const t = items[kind].get(id) || {id, name, matches: 0, kills: 0, headshots: 0, shots: 0, hits: 0};
            if (!seen.has(id)) t.matches++;
            seen.add(id);
            for (const k of ['kills', 'headshots', 'shots', 'hits']) t[k] += kit[k];
            items[kind].set(id, t);
          }
        }
      }
    }
    const list = map => [...map.values()].map(t => ({...t, accuracy: accuracy(t.shots, t.hits)}))
      .sort((a, b) => b.matches - a.matches || b.id - a.id).slice(0, limit);
    return [200, {weapons: list(items.weapons), armour: list(items.armour)}];
  }
  case 'player/days': {
    const p = player();
    if (!p) return found(null);
    const days = new Map();
    for (const m of matches) {
      const l = m.lines.find(x => x.player.id === p.id);
      if (!l) continue;
      const day = new Date(m.match.ended_at * 1000).toISOString().slice(0, 10);
      days.set(day, [...(days.get(day) || []), l]);
    }
    return [200, {days: [...days].map(([day, lines]) => ({day, ...totals(lines)}))}];
  }
  case 'matches': {
    const id = Number(params.get('clan'));
    const list = matches.filter(m => !id || m.lines.some(l => l.player.clan && l.player.clan.id === id)).map(m => m.match);
    return [200, page(list, params)];
  }
  case 'match': { const m = matches.find(x => x.match.id === Number(params.get('id'))); return found(m && {match: m.match, players: m.lines}); }
  case 'top': {
    const by = params.get('by') || 'rating';
    const key = c => by === 'rating' ? c.rating : c.totals[by];
    // By accuracy only players with 1000 shots or more; ties by account.
    return [200, {by, players: players.map(card).filter(c => c.totals.matches && (by !== 'accuracy' || c.totals.shots >= 1000))
      .sort((a, b) => key(b) - key(a) || a.player.id - b.player.id).slice(0, Number(params.get('limit')) || 50)}];
  }
  case 'best': {
    const period = params.get('period') || 'day';
    if (!['hour', 'day'].includes(period)) return [400, {error: 'bad request'}];
    const rows = matches.flatMap(m => m.lines.map(l => ({player: l.player, match: m.match.id, ended_at: m.match.ended_at, level: m.match.level,
      map: m.match.map, mode: m.match.mode, score: l.score, kills: l.kills, deaths: l.deaths})));
    return [200, {period, rows: rows.sort((a, b) => b.score - a.score).slice(0, 20)}];
  }
  case 'clans': {
    const by = params.get('by') || 'score';
    return [200, {by, clans: CLANS.map(c => { const {clan: {leader: _, ...line}, totals: t} = clanCard(c); return {...line, totals: t}; })
      .sort((a, b) => b.totals[by] - a.totals[by])}];
  }
  case 'clan': { const c = clan(); return found(c && clanCard(c)); }
  default: return [404, {error: 'not found'}];
  }
}
