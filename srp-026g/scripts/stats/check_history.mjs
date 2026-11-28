// The stats page (/stats, servers/web-server/stats/history.js) in an isolated Firefox, over the
// site's own endpoints. Node 22+; Firefox: --headless --remote-debugging-port
// 9457. The page, its script, its policy and /api/history/... are the HTTP
// preview's (cargo test -p web-server serve_wiki_preview -- --ignored
// --nocapture), whose history is made up (history::tests::preview: twelve
// players, six clans, 23 public matches). This script serves it again on
// port 24584 with each /api/history/ answer late by 300 ms and kept, so the
// page is seen loading and its cells are compared with what was answered.
// Saves review screenshots to /tmp/wiki-stats-review.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import http from 'node:http';
const [preview = 'http://127.0.0.1:24583', port = '9457', output = '/tmp/wiki-stats-review', own = '24584'] = process.argv.slice(2);
const base = `http://127.0.0.1:${own}`;
await fs.mkdir(output, {recursive: true});
const asked = [];
// The last answer of each endpoint, by its path under /api/history/.
const answered = {};
// When set, the top list's pages are cut to this many: the preview's twelve
// players then have more than one.
let pageOf = 0;
const server = http.createServer(async (request, response) => {
  const url = new URL(request.url, base);
  if (url.pathname.startsWith('/api/history/')) {
    asked.push(url.pathname + url.search);
    const path = url.pathname.slice('/api/history/'.length);
    const asking = new URL(preview + request.url);
    if (pageOf && path === 'top') asking.searchParams.set('limit', pageOf);
    const upstream = await fetch(asking);
    const [status, body] = [upstream.status, await upstream.json()];
    answered[path] = body;
    await new Promise(resolve => setTimeout(resolve, 300));
    response.writeHead(status, {'content-type': 'application/json'}).end(JSON.stringify(body));
    return;
  }
  const upstream = await fetch(preview + request.url, {headers: {'accept-language': request.headers['accept-language'] || ''}, redirect: 'manual'});
  const headers = Object.fromEntries([...upstream.headers].filter(([name]) => !['content-length', 'connection', 'transfer-encoding'].includes(name)));
  response.writeHead(upstream.status, headers).end(Buffer.from(await upstream.arrayBuffer()));
});
await new Promise(resolve => server.listen(Number(own), '127.0.0.1', resolve));
const socket = new WebSocket(`ws://127.0.0.1:${port}/session`);
await new Promise((resolve, reject) => {socket.onopen = resolve; socket.onerror = reject;});
let id = 0;
const pending = new Map();
socket.onmessage = event => {
  const message = JSON.parse(event.data), request = pending.get(message.id);
  if (!request) return;
  pending.delete(message.id); clearTimeout(request.timer);
  message.type === 'error' ? request.reject(Error(JSON.stringify(message))) : request.resolve(message.result);
};
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const next = ++id;
    const timer = setTimeout(() => {pending.delete(next); reject(Error(`Timed out: ${method}`));}, 20000);
    pending.set(next, {resolve, reject, timer});
    socket.send(JSON.stringify({id: next, method, params}));
  });
}
try {
  await send('session.new', {capabilities: {alwaysMatch: {browserName: 'firefox'}}});
  const {context} = await send('browsingContext.create', {type: 'tab'});
  async function evaluate(expression) {
    const result = await send('script.evaluate', {expression, target: {context}, awaitPromise: true});
    if (result.type === 'exception') throw Error(JSON.stringify(result));
    return result.result.value;
  }
  // Loaded: the page not aria-busy at two looks 50 ms apart.
  const settled = () => evaluate(`new Promise(resolve => { let calm = 0;
    const look = () => { calm = document.getElementById('stats')?.getAttribute('aria-busy') === 'true' ? 0 : calm + 1;
      calm >= 2 ? resolve() : setTimeout(look, 50); }; look(); })`);
  async function navigate(path, wait = 'complete') {
    await send('browsingContext.navigate', {context, url: base + path, wait});
    if (wait === 'complete') await settled();
  }
  async function screenshot(name) {
    await evaluate('document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(resolve)))');
    const {data} = await send('browsingContext.captureScreenshot', {context, origin: 'document'});
    await fs.writeFile(`${output}/${name}.png`, Buffer.from(data, 'base64'));
  }
  const text = selector => evaluate(`document.querySelector(${JSON.stringify(selector)})?.textContent ?? null`);
  const count = selector => evaluate(`document.querySelectorAll(${JSON.stringify(selector)}).length`);
  const panel = label => evaluate(`[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === ${JSON.stringify(label)})?.nextSibling.textContent ?? null`);
  const columns = async () => (await evaluate("[...document.querySelectorAll('.stats-table thead th')].map(th => th.textContent).join('\\n')")).split('\n');
  const top = selector => evaluate(`Math.round(document.querySelectorAll('.stats-heading')[${selector}].getBoundingClientRect().top)`);
  // Where a player's lists begin: the buttons over them.
  const listsTop = () => evaluate("Math.round(document.querySelector('.stats-tabs').getBoundingClientRect().top)");
  // Accuracy as the page shows it: whole percents, none without a shot.
  const aim = (counts, lang = 'en') => counts.shots ? new Intl.NumberFormat(lang, {style: 'percent', maximumFractionDigits: 0}).format(counts.accuracy) : '—';
  const cells = async selector => JSON.parse(await evaluate(`JSON.stringify([...document.querySelectorAll(${JSON.stringify(selector)})].map(e => e.textContent))`));
  const fits = () => evaluate("[...document.querySelectorAll('.table-scroll')].every(box => box.scrollWidth <= box.clientWidth)");
  // The weapons of a line's kits as the page writes them.
  const weapons = row => row.kits.length ? row.kits.map(kit => kit.weapons.length ? kit.weapons.map(item => item.name ?? '—').join(' · ') : '—').join(' / ') : '—';
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});

  // A tab of its own beside the wiki, on the wiki's pages too.
  await navigate('/wiki');
  assert.equal(await evaluate("[...document.querySelectorAll('header nav a')].map(a => a.textContent).slice(0, 3).join()"), 'Home,Wiki,Stats');
  assert.equal(await evaluate("document.querySelector('header nav a[aria-current]').textContent"), 'Wiki');
  const policy = (await fetch(base + '/stats')).headers.get('content-security-policy');
  assert.ok(policy.includes("connect-src 'self'") && policy.includes("script-src 'self'"), policy);

  // The leaderboards: players by rating first, then each board and key.
  await navigate('/stats');
  assert.equal(await text('#stats-title'), 'Stats');
  assert.equal(await evaluate("document.querySelector('header nav a[aria-current]').textContent"), 'Stats');
  assert.equal(await count('.stats-board .stats-table tbody tr'), 12, 'every player with a match, Nova not');
  const named = ({player}) => `${player.clan ? `[${player.clan.tag}] ` : ''}${player.name}`;
  assert.equal(await text('.stats-board .stats-table tbody tr td:nth-child(2)'), named(answered.top.players[0]));
  assert.equal(named(answered.top.players[0]), '[SKH] Sheep');
  assert.equal(await text('.stats-board .stats-table thead th.sorted'), 'Rating');
  assert.equal(await evaluate("document.querySelector('.stats-keys').hidden"), true, 'the players sort by their headings');
  assert.equal(await evaluate("document.querySelector('.stats-board thead th.sorted').getAttribute('aria-sort')"), 'descending');
  assert.equal(await evaluate("document.querySelector('.breadcrumb').hidden"), true, 'no way back on the boards themselves');
  await screenshot('boards-players');
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(await text('.stats-board .stats-table thead th.sorted'), 'Kills');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-board .stats-table tbody tr')].map(r => Number(r.children[6].textContent.replace(/\\D/g, ''))).every((v, i, a) => !i || a[i - 1] >= v)"), true, 'sorted by kills');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('by')"), 'kills');
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('by=kills'), 'the language link keeps the board');
  // A second click on the sorted heading: the fewest first; a third, back.
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(answered.top.order, 'asc');
  assert.equal(await evaluate("document.querySelector('.stats-board thead th.sorted').getAttribute('aria-sort')"), 'ascending');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('order')"), 'asc');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-board .stats-table tbody tr')].map(r => Number(r.children[6].textContent.replace(/\\D/g, ''))).every((v, i, a) => !i || a[i - 1] <= v)"), true, 'the fewest kills first');
  await screenshot('boards-players-fewest');
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(answered.top.order, 'desc');
  assert.equal(await evaluate("new URLSearchParams(location.search).has('order')"), false);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-board thead th button')].map(b => b.textContent).join()"), 'Rating,Matches,Wins,Win rate,Kills,K/D,Avg score,HS %,Accuracy');
  const board = await cells('.stats-board thead th');
  assert.ok(!['Level', 'Score', 'Headshots', 'Batteries', 'Artefacts'].some(label => board.includes(label)), board.join());
  for (const [label, by, value] of [['Win rate', 'win_rate', row => row.totals.win_rate], ['K/D', 'kd', row => row.totals.kd],
    ['HS %', 'headshot_rate', row => row.totals.headshots / Math.max(row.totals.kills, 1)]]) {
    await evaluate(`[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === '${label}').click()`);
    await settled();
    assert.equal(answered.top.by, by);
    assert.equal(await text('.stats-board .stats-table thead th.sorted'), label);
    assert.equal(await text('.slot-filter .stats-note'), '5+ matches');
    const values = answered.top.players.map(value);
    assert.ok(values.length > 1 && values.every((v, i) => !i || values[i - 1] >= v - 0.006), `${by}: ${values.join()}`);
  }
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(await text('.slot-filter .stats-note'), '', 'a note only for a list that leaves players out');
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Avg score').click()");
  await settled();
  assert.equal(answered.top.by, 'score_avg');
  assert.equal(await text('.slot-filter .stats-note'), '5+ matches');
  assert.equal(await count('.stats-board .stats-table tbody tr'), answered.top.players.length);
  assert.ok(answered.top.players.length > 0);
  assert.deepEqual(await cells('.stats-board .stats-table tbody td.sorted'), answered.top.players.map(row => String(row.totals.score_avg)));
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Accuracy').click()");
  await settled();
  assert.equal(answered.top.by, 'accuracy');
  assert.equal(await text('.stats-board .stats-table thead th.sorted'), 'Accuracy');
  assert.equal(await text('.slot-filter .stats-note'), '1000+ shots');
  assert.equal(await count('.stats-board .stats-table tbody tr'), answered.top.players.length);
  assert.ok(!answered.top.players.some(row => row.player.name === 'Yasen'), 'under 1000 shots: not listed');
  assert.deepEqual(await cells('.stats-board .stats-table tbody td.sorted'), answered.top.players.map(row => aim(row.totals)));
  assert.equal(await fits(), true, 'the board fits the width');
  // The players go on past a page: here pages of five, the twelve in three.
  pageOf = 5;
  await navigate('/stats?by=kills');
  const kills = () => evaluate("[...document.querySelectorAll('.stats-board .stats-table tbody tr')].map(r => Number(r.children[6].textContent.replace(/\\D/g, '')))");
  assert.equal(await count('.stats-board .stats-table tbody tr'), 5);
  assert.equal(await text('.stats-board > button'), 'More players');
  await evaluate("document.querySelector('.stats-board > button').click()");
  await settled();
  assert.equal(answered.top.after, 5);
  assert.deepEqual(await cells('.stats-board .stats-table tbody td:first-child'), ['1', '2', '3', '4', '5', '6', '7', '8', '9', '10']);
  assert.ok((await kills()).every((v, i, a) => !i || a[i - 1] >= v), 'the order goes on');
  await screenshot('boards-players-more');
  // A sort keeps the pages open: the ten, by rating now.
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Rating').click()");
  await settled();
  assert.equal(await count('.stats-board .stats-table tbody tr'), 10, 'as many players as before the sort');
  assert.deepEqual(await cells('.stats-board .stats-table tbody td:first-child'), ['1', '2', '3', '4', '5', '6', '7', '8', '9', '10']);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-board .stats-table tbody tr')].map(r => Number(r.children[2].textContent.replace(/\\D/g, ''))).every((v, i, a) => !i || a[i - 1] >= v)"), true, 'by rating over both pages');
  assert.equal(await text('.stats-board > button'), 'More players');
  await evaluate("[...document.querySelectorAll('.stats-board thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(await count('.stats-board .stats-table tbody tr'), 10);
  await evaluate("document.querySelector('.stats-board > button').click()");
  await settled();
  assert.equal(answered.top.after, 10);
  assert.equal(await count('.stats-board .stats-table tbody tr'), 12);
  assert.ok((await kills()).every((v, i, a) => !i || a[i - 1] >= v));
  assert.equal(await count('.stats-board > button'), 0, 'nothing follows the last page');
  pageOf = 0;
  await navigate('/stats');
  assert.equal(await count('.stats-board > button'), 0, 'twelve players: one page');
  // Over the boards: the day's leaders, five lists of five, and everything added up.
  assert.equal(await count('.stats-leaders .stats-table'), 5);
  assert.equal(await count('.stats-leaders .stats-table tbody tr'), 25, 'five rows each, empty ones too');
  // Every row one line. The hours played list (everyone ties there: the first players by id) holds a
  // real long name, one as wide as the live server's widest, and the longest a player can choose with
  // the longest tag in wide capitals: the real ones whole; that one cut short below a desktop, whole in
  // its title.
  for (const width of [1440, 1024, 360]) {
    await send('browsingContext.setViewport', {context, viewport: {width, height: 1000}, devicePixelRatio: 1});
    const [heights, names] = JSON.parse(await evaluate(`JSON.stringify([
      [...new Set([...document.querySelectorAll('.stats-leaders tbody tr')].map(tr => tr.getBoundingClientRect().height))],
      Object.fromEntries([...document.querySelectorAll('.stats-leaders .table-scroll:last-child td.l')].map(td =>
        [td.textContent, [td.scrollWidth <= td.clientWidth, td.firstChild.title === td.textContent]]))])`));
    assert.equal(heights.length, 1, `${width}: every row as tall as the next`);
    for (const name of ['[ДЕДКИ] bugz_bunny_95_', '[SKH] Длинноимённый_Сталкер']) assert.deepEqual(names[name], [true, true], `${width}: ${name}`);
    assert.deepEqual(names['[ЩЮЖШЩЮ] ЩЮЖШЩЮЖШЩЮЖШЩЮЖШ'], [width >= 1280, true], `${width}: the widest name`);
  }
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  assert.deepEqual(await cells('.stats-leaders .table-scroll:first-child tbody td:nth-child(3)'),
    [...answered.today.leaders.kills.map(row => String(row.value)), ...Array(5 - answered.today.leaders.kills.length).fill('\u00a0')]);
  assert.equal(answered.today.leaders.kills.length, 5);
  // They open up: twenty or fifty each, all as long as the longest.
  assert.deepEqual(await cells('.stats-sizes button'), ['5', '20', '50']);
  assert.equal(await text('.stats-sizes button[aria-pressed=true]'), '5');
  await evaluate("[...document.querySelectorAll('.stats-sizes button')].find(b => b.textContent === '20').click()");
  await settled();
  const longest = Math.max(...Object.values(answered.today.leaders).map(rows => rows.length));
  assert.ok(longest > 5 && longest <= 20, `${longest} of twenty`);
  assert.equal(await count('.stats-leaders .stats-table tbody tr'), 5 * longest, 'every list as long as the longest');
  assert.deepEqual((await cells('.stats-leaders .table-scroll:first-child tbody td:nth-child(3)')).slice(0, answered.today.leaders.kills.length),
    answered.today.leaders.kills.map(row => String(row.value)));
  assert.equal(await text('.stats-sizes button[aria-pressed=true]'), '20');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('leaders')"), '20');
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('leaders=20'), 'the language link keeps it');
  await screenshot('boards-leaders-20');
  await evaluate("[...document.querySelectorAll('.stats-sizes button')].find(b => b.textContent === '5').click()");
  await settled();
  assert.equal(await count('.stats-leaders .stats-table tbody tr'), 25);
  assert.equal(await evaluate("new URLSearchParams(location.search).has('leaders')"), false);
  assert.deepEqual((await cells('.stats-all thead th')).slice(0, 3), ['Players', 'Matches', 'Kills']);
  assert.deepEqual((await cells('.stats-all tbody td')).slice(0, 2), [String(answered.totals.players), String(answered.totals.matches)]);
  assert.equal(await evaluate("document.querySelector('.stats-all').previousElementSibling.textContent"), 'All time', 'its heading over it, as the leaders\' is');
  await screenshot('boards-accuracy');
  await evaluate("document.querySelector('[data-board=clans]').click()");
  await settled();
  assert.equal(await count('.stats-board .stats-table tbody tr'), 6);
  const firstClan = answered.clans.clans[0];
  assert.equal(await text('.stats-board .stats-table tbody tr td:nth-child(2) a'), `[${firstClan.tag}] ${firstClan.name}`);
  assert.equal(await text('.stats-board .stats-table tbody tr td:nth-child(4)'), String(firstClan.members));
  await evaluate("document.querySelector('[data-board=best]').click()");
  await settled();
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-keys button')].map(b => b.textContent).join()"), 'Day,Hour', 'no week');
  assert.equal(await count('.stats-board .stats-table tbody tr'), 20);
  await screenshot('boards-best');
  await evaluate("document.querySelector('[data-board=latest]').click()");
  await settled();
  assert.equal(await count('.stats-board .stats-table tbody tr'), 15);
  const newest = answered.matches.matches[0].id;
  assert.equal(newest, 23);
  assert.equal(await text('.stats-board .stats-table tbody tr td:nth-child(2)'), String(newest), 'newest first');
  // Each match with its map's picture, the rooms' own.
  assert.deepEqual(JSON.parse(await evaluate("JSON.stringify([...document.querySelectorAll('.stats-board img.stats-map')].map(i => new URL(i.src).pathname + new URL(i.src).search))")),
    answered.matches.matches.map(row => row.map_icon));
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-board img.stats-map')].every(i => i.complete ? i.naturalWidth === 128 : true)"), true);
  // A click anywhere on a match's row opens it.
  await evaluate("document.querySelector('.stats-board .stats-table tbody tr td:nth-child(4)').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate('location.search'), `?match=${newest}&lang=en`);
  await navigate('/stats?board=latest&lang=en');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(await count('.stats-board .stats-table tbody tr'), 23);
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), true, 'no older page');

  // The Maps board: the counted matches (22 of the 23) by map, in the order of the rooms' create
  // window (every ported version its own map; the preview's window is every playable map, by
  // id: the stock levels in their order), each with its matches in a column; under each its
  // modes with how the sides fared, and each mode's weathers behind its button; a period of its own.
  const percent = (value, lang = 'en') => new Intl.NumberFormat(lang, {style: 'percent', maximumFractionDigits: 1}).format(value);
  const length = seconds => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  // A map's picture is never stretched (the owner, 2026-11-26): it covers a box of its own shape
  // (16:9), or is drawn in its own; one still loading (they load lazily) by its box alone. One in a
  // closed row is not drawn at all.
  const unstretched = () => evaluate(`JSON.stringify([...document.querySelectorAll('img.stats-map')].filter(i => i.getClientRects().length).filter(i => {
    const box = i.clientWidth / i.clientHeight, own = i.complete && i.naturalWidth ? i.naturalWidth / i.naturalHeight : 16 / 9;
    return !(getComputedStyle(i).objectFit === 'cover' && Math.abs(box - 16 / 9) < 0.03 || Math.abs(own - box) < 0.03);
  }).map(i => i.src))`).then(JSON.parse);
  // Every mode and weather named in words (the code's, never the game data's): no "mode 2", no "#61".
  const unnamed = () => evaluate(`JSON.stringify([...document.querySelectorAll('.stats-mode-cell, .stats-map-cell, .stats-modes td:nth-child(2)')]
    .map(cell => cell.textContent.trim()).filter(text => /(^|\\s)(mode|режим|Режим) \\d+/i.test(text) || /#\\d+/.test(text)))`).then(JSON.parse);
  const legend = sides => sides ? [`Team 1 ${percent(sides.win_rates[0])} (${sides.wins[0]})`, `Draw ${percent(sides.draw_rate)} (${sides.draws})`,
    ...(sides.unknown ? [`No result ${percent(sides.unknown / (sides.wins[0] + sides.wins[1] + sides.draws + sides.unknown))} (${sides.unknown})`] : []),
    `Team 2 ${percent(sides.win_rates[1])} (${sides.wins[1]})`].join('') : '—';
  const GAME_MODES = ['Protective device', 'Protective device: chargers', 'Team Deathmatch', 'Research', 'Protective device: blackout',
    'Free for all', 'Freeze Tag', 'Artifact Hunt', 'FFA: Racket', 'FFA: Saloon', 'FFA: Saloon (Winchester)', 'FFA: Roulette', 'FFA: Pistols'];
  await navigate('/stats');
  await evaluate("document.querySelector('[data-board=maps]').click()");
  await settled();
  assert.equal(await evaluate("location.search"), '?board=maps', 'all time: no period in the address');
  assert.deepEqual(await cells('.stats-keys button'), ['All time', 'Last 30 days', 'Last 7 days', 'Last 24 hours']);
  assert.equal(await text('.stats-keys button[aria-pressed=true]'), 'All time');
  const atlas = answered.maps;
  assert.equal(atlas.period, 'all');
  assert.equal(atlas.matches, 22, 'the counted ones: not the short one');
  assert.equal(await count('.stats-maps-table tr.stats-map-row'), atlas.maps.length);
  assert.ok(atlas.maps.length > 1);
  assert.deepEqual(await cells('.stats-map-row .stats-place-name'), atlas.maps.map(map => map.map));
  const keys = atlas.maps.map(map => map.map_key);
  assert.deepEqual(keys, [...keys].sort(), 'the create window\'s order (the preview\'s: the levels by id), not the most played first');
  assert.deepEqual(await cells('.stats-maps-table thead th'), ['Map', 'Matches', 'Avg length', 'Sides']);
  // A map's row: its team matches (the free-for-alls left out), their length and its sides.
  const modeRows = atlas.maps.flatMap(map => map.modes);
  const teamMatches = map => map.modes.filter(mode => mode.sides).reduce((sum, mode) => sum + mode.matches, 0);
  assert.deepEqual(atlas.maps.map(map => map.matches), atlas.maps.map(teamMatches), 'a map\'s matches: its team modes\'');
  assert.equal(modeRows.reduce((sum, mode) => sum + mode.matches, 0), 22, 'every match under its map');
  assert.deepEqual(await cells('.stats-map-row td:nth-child(2)'), atlas.maps.map(map => String(map.matches)), 'a map\'s matches in their column');
  assert.deepEqual(await cells('.stats-map-row td:nth-child(3)'), atlas.maps.map(map => map.matches ? length(map.duration_s) : '—'));
  assert.deepEqual(await cells('.stats-map-row .stats-sides-cell'), atlas.maps.map(map => legend(map.sides)), 'each map: team 1 against team 2');
  for (const map of atlas.maps) {
    const sum = key => map.modes.filter(mode => mode.sides).reduce((total, mode) => total + mode.sides[key], 0);
    if (map.sides) assert.deepEqual([map.sides.wins, map.sides.draws], [[0, 1].map(i => map.modes.filter(mode => mode.sides).reduce((total, mode) => total + mode.sides.wins[i], 0)), sum('draws')]);
  }
  // Its modes (in the create window's order, a free-for-all's with no bar) and their weathers: closed
  // at first, the maps close together.
  assert.ok(modeRows.every(mode => GAME_MODES.includes(mode.mode)), modeRows.map(mode => mode.mode).join());
  assert.equal(await count('.stats-mode-row'), modeRows.length);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-mode-row, .stats-version-row, .stats-weather')].every(row => row.hidden)"), true, 'closed at first');
  assert.ok(!modeRows.some(mode => mode.mode.startsWith('FFA:')), 'every free-for-all one mode');
  const rowHeights = () => evaluate("JSON.stringify([...document.querySelectorAll('.stats-map-row')].map(row => Math.round(row.getBoundingClientRect().height)))").then(JSON.parse);
  assert.ok((await rowHeights()).every(height => height < 100), `compact: ${await rowHeights()}`);
  assert.deepEqual(await cells('.stats-map-row .stats-opener'), atlas.maps.map(map => `Modes (${map.modes.length})`));
  const columnsAt = () => evaluate("JSON.stringify([...document.querySelectorAll('.stats-maps-table thead th')].map(th => Math.round(th.getBoundingClientRect().left)))");
  const before = await columnsAt();
  const several = atlas.maps.findIndex(map => map.modes.some(mode => !mode.versions && mode.weathers.length > 1) && map.modes.some(mode => mode.sides));
  assert.ok(several >= 0, 'a map with a mode played in two weathers');
  const opened = atlas.maps[several];
  await evaluate(`document.querySelectorAll('.stats-map-row .stats-opener')[${several}].click()`);
  assert.equal(await columnsAt(), before, 'opening moves no column');
  const shownModes = () => cells('.stats-mode-row:not([hidden]) .stats-mode-cell a');
  assert.deepEqual(await shownModes(), opened.modes.map(mode => mode.mode), 'its modes, in the window\'s order');
  assert.deepEqual(await cells('.stats-mode-row:not([hidden]) td:nth-child(2)'), opened.modes.map(mode => String(mode.matches)));
  assert.deepEqual(await cells('.stats-mode-row:not([hidden]) td:nth-child(3)'), opened.modes.map(mode => length(mode.duration_s)));
  assert.deepEqual(await cells('.stats-mode-row:not([hidden]) .stats-sides-cell'), opened.modes.map(mode => legend(mode.sides)));
  assert.ok(modeRows.some(mode => mode.sides === null), 'a free-for-all: no sides');
  assert.ok(atlas.maps.some(map => map.sides && map.sides.draws), 'a draw');
  // The bar, soft: team 1 from the left in its colour, team 2 at the right in its own, at 60%; a draw grey.
  const firstBar = atlas.maps.findIndex(map => map.sides && map.sides.wins[0] && map.sides.wins[1]);
  const parts = JSON.parse(await evaluate(`JSON.stringify([...[...document.querySelectorAll('.stats-map-row .stats-sides-cell')][${firstBar}]
    .querySelectorAll('.stats-bar-part')].map(p => [getComputedStyle(p).backgroundColor, getComputedStyle(p).opacity, Math.round(p.getBoundingClientRect().width / p.parentNode.getBoundingClientRect().width * 1000) / 1000]))`));
  const fared = atlas.maps[firstBar].sides;
  assert.deepEqual(parts[0].slice(0, 2), ['rgb(59, 130, 246)', '0.6']);
  assert.deepEqual(parts[parts.length - 1].slice(0, 2), ['rgb(229, 72, 77)', '0.6']);
  assert.ok(Math.abs(parts[0][2] - fared.win_rates[0]) < 0.01 && Math.abs(parts[parts.length - 1][2] - fared.win_rates[1]) < 0.01, JSON.stringify(parts));
  assert.equal(await evaluate("getComputedStyle(document.querySelector('.stats-bar')).height"), '6px', 'a thin bar');
  const draws = await evaluate("JSON.stringify([...document.querySelectorAll('.stats-bar-part.stats-draw')].map(p => getComputedStyle(p).opacity))");
  assert.ok(JSON.parse(draws).every(opacity => opacity === '0.45'), draws);
  // A mode's weathers: behind its button, each with its own numbers and the rooms' word for it.
  const mode = opened.modes.findIndex(each => !each.versions && each.weathers.length > 1);
  const button = `[...document.querySelectorAll('.stats-mode-row:not([hidden]) .stats-opener')][${mode}]`;
  assert.equal(await evaluate(`${button}.textContent`), `Weathers (${opened.modes[mode].weathers.length})`);
  await evaluate(`${button}.click()`);
  assert.equal(await evaluate(`${button}.getAttribute('aria-expanded')`), 'true');
  assert.equal(await columnsAt(), before, 'nor do the weathers');
  const shownWeathers = () => cells('.stats-weather:not([hidden]) td:nth-child(2)');
  assert.deepEqual(await shownWeathers(), opened.modes[mode].weathers.map(weather => String(weather.matches)));
  assert.deepEqual(await cells('.stats-weather:not([hidden]) .stats-sides-cell'), opened.modes[mode].weathers.map(weather => legend(weather.sides)));
  assert.ok(modeRows.flatMap(each => each.weathers).every(weather => ['day', 'evening', 'rain', 'night', 'before rain'].includes(weather.weather)), 'every weather named');
  assert.deepEqual(await cells('.stats-weather:not([hidden]) td:first-child'), opened.modes[mode].weathers.map(weather => weather.weather));
  assert.deepEqual(await unnamed(), [], 'no mode or weather by its number');
  await screenshot('board-maps-open');
  // The map's button closes its modes and what they opened.
  await evaluate(`document.querySelectorAll('.stats-map-row .stats-opener')[${several}].click()`);
  assert.deepEqual(await shownModes(), [], 'closed again');
  assert.deepEqual(await shownWeathers(), [], 'its weathers too');
  // Every free-for-all is one Free for all under its map; its versions open under it, each with its weathers.
  const free = atlas.maps.findIndex(map => map.modes.some(each => each.versions));
  assert.ok(free >= 0, 'a map with a free-for-all');
  const ffa = atlas.maps[free].modes.find(each => each.versions);
  assert.equal(ffa.mode, 'Free for all');
  assert.equal(ffa.versions.reduce((sum, version) => sum + version.matches, 0), ffa.matches);
  assert.ok(ffa.versions.some(version => version.mode === 'FFA: Racket'), 'the Racket one version of it');
  await evaluate(`document.querySelectorAll('.stats-map-row .stats-opener')[${free}].click()`);
  const ffaButton = "[...document.querySelectorAll('.stats-mode-row:not([hidden])')].find(row => row.querySelector('.stats-mode-cell a').textContent === 'Free for all').querySelector('.stats-opener')";
  assert.equal(await evaluate(`${ffaButton}.textContent`), `Variants (${ffa.versions.length})`);
  await evaluate(`${ffaButton}.click()`);
  assert.deepEqual(await cells('.stats-version-row:not([hidden]) .stats-mode-cell a'), ffa.versions.map(version => version.mode));
  assert.deepEqual(await cells('.stats-version-row:not([hidden]) td:nth-child(2)'), ffa.versions.map(version => String(version.matches)));
  await evaluate("document.querySelector('.stats-version-row:not([hidden]) .stats-opener').click()");
  assert.deepEqual(await shownWeathers(), ffa.versions[0].weathers.map(weather => String(weather.matches)), 'a version\'s weathers');
  assert.equal(await columnsAt(), before, 'nor do the versions');
  await screenshot('board-maps-free-for-all');
  await evaluate(`document.querySelectorAll('.stats-map-row .stats-opener')[${free}].click()`);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-mode-row, .stats-version-row, .stats-weather')].every(row => row.hidden)"), true, 'all closed with its map');
  // A map's name and picture lead to it in the Maps page, in its first mode.
  assert.equal(await evaluate("(a => a.pathname + a.hash)(document.querySelector('.stats-place-name a'))"), `/wiki/maps#${atlas.maps[0].map_key}/${atlas.maps[0].modes[0].rules}`);
  assert.deepEqual(await unstretched(), [], 'no map\'s picture stretched');
  assert.equal(await fits(), true, 'the maps fit the width');
  await screenshot('board-maps');
  // Another period: its matches alone, said in the address and kept by the language links.
  await evaluate("[...document.querySelectorAll('.stats-keys button')].find(b => b.textContent === 'Last 24 hours').click()");
  await settled();
  assert.equal(answered.maps.period, 'day');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('period')"), 'day');
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('period=day'));
  assert.ok(answered.maps.matches > 0 && answered.maps.matches < 22, `${answered.maps.matches} of the last day`);
  assert.equal(await count('.stats-maps-table tr.stats-map-row'), answered.maps.maps.length);
  // The Modes board: each mode's matches, share and trend, the most played first; then the map + mode pairs.
  await navigate('/stats?board=modes&period=week&lang=en');
  const tally = answered.modes;
  assert.equal(tally.period, 'week');
  assert.equal(await text('.stats-keys button[aria-pressed=true]'), 'Last 7 days');
  assert.equal(await text('.stats-board > .stats-plays > .table-scroll:first-child thead th:last-child'), 'Trend');
  const modeLines = '.stats-modes > .table-scroll:first-child tbody tr:not(.stats-version-row)';
  assert.equal(await count(modeLines), tally.modes.length);
  assert.ok(tally.modes.length >= 5, 'the modes in turn');
  // In the create window's order (Blackout right after the protective device's other kinds), # each one's popularity.
  const order = ['Protective device', 'Protective device: chargers', 'Protective device: blackout', 'Team Deathmatch', 'Research', 'Freeze Tag',
    'Free for all', 'FFA: Racket', 'FFA: Saloon', 'FFA: Saloon (Winchester)', 'Artifact Hunt'];
  assert.deepEqual(tally.modes.map(mode => mode.mode), order.filter(name => tally.modes.some(mode => mode.mode === name)), 'the window\'s order');
  assert.ok(!tally.modes.some(mode => mode.mode.startsWith('FFA:')), 'every free-for-all one mode');
  // The free-for-all's versions under it, each with its matches, share and trend; its button opens them, leading nowhere.
  const freeMode = tally.modes.find(mode => mode.versions);
  assert.equal(freeMode.mode, 'Free for all');
  assert.equal(freeMode.versions.reduce((sum, version) => sum + version.matches, 0), freeMode.matches);
  assert.equal(await count('.stats-modes .stats-version-row'), freeMode.versions.length);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-modes .stats-version-row')].every(row => row.hidden)"), true, 'closed at first');
  const here = await evaluate('location.href');
  await evaluate("document.querySelector('.stats-modes .stats-opener').click()");
  assert.equal(await evaluate('location.href'), here, 'the button opens, it leads nowhere');
  assert.equal(await text('.stats-modes .stats-opener'), `Variants (${freeMode.versions.length})`);
  assert.deepEqual(await cells('.stats-modes .stats-version-row:not([hidden]) td:nth-child(2)'), freeMode.versions.map(version => version.mode));
  assert.deepEqual(await cells('.stats-modes .stats-version-row:not([hidden]) td:nth-child(3)'), freeMode.versions.map(version => String(version.matches)));
  assert.deepEqual(await cells('.stats-modes .stats-version-row:not([hidden]) td:nth-child(4)'), freeMode.versions.map(version => percent(version.share)));
  assert.deepEqual(JSON.parse(await evaluate("JSON.stringify([...document.querySelectorAll('.stats-modes .stats-version-row:not([hidden]) .stats-trend')].map(t => t.children.length))")),
    freeMode.versions.map(() => 7));
  await screenshot('board-modes-versions');
  await evaluate("document.querySelector('.stats-modes .stats-opener').click()");
  const byMatches = [...tally.modes].sort((a, b) => b.matches - a.matches || order.indexOf(a.mode) - order.indexOf(b.mode));
  assert.deepEqual(tally.modes.map(mode => mode.rank), tally.modes.map(mode => byMatches.indexOf(mode) + 1), 'ranked by matches');
  assert.deepEqual(await cells(`${modeLines} td:nth-child(1)`), tally.modes.map(mode => String(mode.rank)));
  assert.deepEqual(await cells(`${modeLines} td:nth-child(2) a`), tally.modes.map(mode => mode.mode));
  assert.ok(tally.modes.every(mode => GAME_MODES.includes(mode.mode)), tally.modes.map(mode => mode.mode).join());
  assert.deepEqual(await unnamed(), [], 'no mode by its number');
  assert.deepEqual(await cells(`${modeLines} td:nth-child(3)`), tally.modes.map(mode => String(mode.matches)));
  assert.deepEqual(await cells(`${modeLines} td:nth-child(4)`), tally.modes.map(mode => percent(mode.share)));
  assert.equal(tally.modes.reduce((sum, mode) => sum + mode.matches, 0), tally.matches);
  assert.equal(tally.steps.length, 7);
  for (const mode of tally.modes) assert.equal(mode.trend.reduce((sum, value) => sum + value, 0), mode.matches, `${mode.mode}: the week's days hold its matches`);
  assert.deepEqual(JSON.parse(await evaluate(`JSON.stringify([...document.querySelectorAll('${modeLines} .stats-trend')].map(t => t.children.length))`)),
    tally.modes.map(() => 7), 'a column a day');
  assert.match(await text('.stats-trend-note'), /^Trend: by day, /);
  const busiest = tally.modes[0].trend.indexOf(Math.max(...tally.modes[0].trend));
  assert.equal(await evaluate(`document.querySelector('${modeLines} .stats-trend').children[${busiest}].getBoundingClientRect().height`),
    await evaluate(`document.querySelector('${modeLines} .stats-trend').getBoundingClientRect().height`), 'the busiest day full height');
  assert.equal(await text('.stats-modes h2.stats-heading'), 'Most played maps and modes');
  const pairLines = '.stats-modes > .table-scroll:last-child tbody tr';
  assert.equal(await count(pairLines), tally.pairs.length);
  assert.ok(tally.pairs.length > 1 && tally.pairs.length <= 10);
  assert.deepEqual(await cells(`${pairLines} td:nth-child(3)`), tally.pairs.map(pair => String(pair.matches)));
  assert.deepEqual(await cells(`${pairLines} .stats-map-cell .stats-under`), tally.pairs.map(pair => pair.mode));
  assert.deepEqual(await unstretched(), [], 'no map\'s picture stretched');
  assert.equal(await fits(), true, 'the modes fit the width');
  await screenshot('board-modes');
  await evaluate(`document.querySelector('${pairLines} td:nth-child(3)').click()`);
  await new Promise(resolve => setTimeout(resolve, 800));
  assert.equal(await evaluate('location.pathname + location.hash'), `/wiki/maps#${tally.pairs[0].map_key}/${tally.pairs[0].rules}`, 'a pair leads to its map');
  // All time: by week, and a day by the hour.
  await navigate('/stats?board=modes&lang=en');
  assert.equal(answered.modes.step, 'week');
  assert.equal(answered.modes.matches, 22);
  await navigate('/stats?board=modes&period=day&lang=en');
  assert.equal(answered.modes.steps.length, 24);
  assert.match(await text('.stats-trend-note'), /^Trend: by hour, /);
  // A phone, in Ukrainian: the boards fit, the pictures keep their shape.
  await send('browsingContext.setViewport', {context, viewport: {width: 390, height: 844}, devicePixelRatio: 1});
  for (const board of ['maps', 'modes']) {
    await navigate(`/stats?board=${board}&lang=uk`);
    assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, `the ${board} fit a phone`);
    // (The all-time row over the boards is wider than a phone, as before them: it scrolls.)
    assert.equal(await evaluate("[...document.querySelectorAll('.stats-plays .table-scroll')].every(box => box.scrollWidth <= box.clientWidth)"), true, `the ${board}' tables fit a phone`);
    assert.deepEqual(await unstretched(), [], `${board}: no picture stretched on a phone`);
    if (board === 'maps') {
      assert.deepEqual(await cells('.stats-keys button'), ['За весь час', 'Останні 30 днів', 'Останні 7 днів', 'Останні 24 години']);
      assert.ok((await rowHeights()).every(height => height < 140), `compact on a phone: ${await rowHeights()}`);
      const phoneColumns = await columnsAt();
      await evaluate("document.querySelector('.stats-map-row .stats-opener').click()");
      await evaluate("document.querySelector('.stats-mode-row:not([hidden]) .stats-opener').click()");
      assert.equal(await columnsAt(), phoneColumns, 'opening moves no column on a phone');
      assert.equal(await evaluate("[...document.querySelectorAll('.stats-plays .table-scroll')].every(box => box.scrollWidth <= box.clientWidth)"), true, 'open, it still fits');
      await screenshot('board-maps-uk-phone-open');
      assert.ok(await evaluate("document.querySelector('.stats-weather:not([hidden]) .stats-map-cell').getBoundingClientRect().height < 30"), 'a weather\'s name one line on a phone');
      assert.ok(await evaluate("[...document.querySelectorAll('.stats-place-name')].every(name => name.getBoundingClientRect().height < 60)"), 'a map\'s name reads across a phone');
    }
    await screenshot(`board-${board}-uk-phone`);
  }
  await navigate('/stats?board=latest&lang=uk');
  assert.deepEqual(await unstretched(), [], 'the latest matches\' pictures on a phone');
  assert.deepEqual(await unnamed(), [], 'the latest matches\' modes by their names');
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});

  // A click anywhere on a player's row opens the player, on a clan's the clan.
  await navigate('/stats');
  await evaluate("document.querySelector('.stats-board .stats-table tbody tr td:nth-child(5)').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate('location.search'), '?id=1042&lang=en', 'a link to a player is by its id');
  await navigate('/stats?board=clans&lang=en');
  await evaluate("document.querySelector('.stats-board .stats-table tbody tr td:nth-child(5)').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate("new URLSearchParams(location.search).get('clan')"), answered.clans.clans[0].tag);
  await navigate(`/stats?match=${newest}&lang=en`);
  await evaluate("document.querySelector('.stats-table tbody tr td:nth-child(5)').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate("new URLSearchParams(location.search).get('id')"), String(answered.match.players[0].player.id));

  // The search goes to a player with only what is filled in.
  await navigate('/stats');
  await evaluate("document.querySelector('.stats-find [name=player]').value = ' sheep ';document.querySelector('.stats-find button').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate('location.search'), '?lang=en&id=1042', 'found by the name, the address is the id\'s');

  // A player: nothing moves while it loads; 15 matches, then 8 older.
  await navigate('/stats?player=Sheep&lang=en', 'interactive');
  assert.equal(await count('.stats-panels dd'), 29, 'the panels are drawn before their data');
  assert.equal(await count('.stats-favourites .stats-table tbody tr'), 10, 'five empty rows of each favourite');
  const loading = [await top(0), await listsTop()];
  await settled();
  assert.deepEqual([await top(0), await listsTop()], loading, 'no layout jump');
  assert.equal(await text('#stats-title'), 'Sheep');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Clan').nextSibling.textContent"), '[SKH]');
  assert.equal(await count('.stats-paged .stats-table tbody tr'), 15);
  assert.equal(await panel('ID'), String(answered.player.player.id), 'the account\'s id');
  const left = answered.player.names.map(former => former.name);
  assert.deepEqual([...left].sort(), ['Baran', 'Yahnia'], 'the names it left (the preview leaves both in one second)');
  assert.equal(await panel('Earlier names'), left.join(', '));
  assert.ok((await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Earlier names').nextSibling.querySelector('span[title]').title")).startsWith('until '));
  assert.equal(await count('.stats-names button'), 0, 'two names: none folded away');
  assert.equal(await panel('Artefacts'), String(answered.player.totals.artefacts), 'the artefacts taken');
  assert.equal(await panel('Hours played'), (answered.player.totals.played_s / 3600).toFixed(1));
  assert.equal(await panel('Accuracy'), aim(answered.player.totals), 'accuracy in whole percents');
  assert.equal(await panel('Kills/min'), new Intl.NumberFormat('en', {minimumFractionDigits: 2, maximumFractionDigits: 2}).format(answered.player.totals.kills_per_minute), 'kills a minute in its matches');
  assert.equal(await panel('Experience/min'), new Intl.NumberFormat('en', {maximumFractionDigits: 2}).format(answered.player.totals.experience_per_minute), 'experience a minute');
  assert.ok((await columns()).includes('HS %') && !(await columns()).includes('Artefacts'), 'a match line has its headshot rate');
  assert.ok((await columns()).includes('Accuracy') && (await columns()).includes('Weapons'), 'and its accuracy and weapons');
  const lines = answered['player/matches'].matches;
  assert.deepEqual(await cells('.stats-paged .stats-kit'), lines.map(weapons), 'the used kits\' weapons');
  assert.deepEqual(await cells('.stats-paged tbody td:nth-child(12)'), lines.map(aim), 'each line\'s accuracy');
  assert.ok(lines.some(row => row.kits.length === 2) && (await cells('.stats-paged .stats-kit')).some(line => line.includes(' / ')), 'two kits');
  assert.ok((await cells('.stats-paged .stats-kit')).some(line => line.includes('—')), 'an item gone from the game data');
  const detail = await evaluate("document.querySelector('.stats-paged .stats-kit').title");
  assert.match(detail, /^Kit [123]: .+; .+\nKills \d+ · Headshots \d+ · Accuracy (\d+%|—)/, 'a kit in full on hover');
  // Favourite equipment: the matches each was used in, five rows each, empty ones too.
  const favourite = answered['player/equipment'];
  assert.ok(asked.some(path => path.includes('player/equipment') && path.includes('limit=5')));
  assert.equal(await text('.stats-heading'), 'Favourite equipment');
  assert.deepEqual(await cells('.stats-favourites .table-scroll:first-child thead th'), ['Weapon', 'Matches'], 'no kills of an item');
  assert.equal(await count('.stats-favourites .stats-table tbody tr'), 10);
  assert.equal(await count('.stats-favourites tbody .stats-item'), 10, 'an icon or its empty frame a row');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-favourites img.stats-item')].every(i => i.complete && i.naturalWidth > 0)"), true, 'the icons load');
  assert.equal(await evaluate("new Set([...document.querySelectorAll('.stats-favourites tbody tr')].map(r => r.offsetHeight)).size"), 1, 'empty rows as tall as full ones');
  assert.deepEqual(await cells('.stats-favourites .table-scroll:first-child tbody td.l'),
    [...favourite.weapons.map(item => item.name ?? '—'), ...Array(5 - favourite.weapons.length).fill('\u00a0')]);
  assert.deepEqual(await cells('.stats-favourites .table-scroll:last-child tbody td:nth-child(2)'),
    [...favourite.armour.map(item => String(item.matches)), ...Array(5 - favourite.armour.length).fill('\u00a0')]);
  assert.ok(favourite.weapons.some(item => item.name === null), 'an item gone from the game data');
  // A favourite leads to its card in the wiki: by its name's link, or a click anywhere on its row.
  const known = favourite.weapons.find(item => item.key), piece = favourite.armour[0];
  assert.deepEqual(await evaluate("[...document.querySelectorAll('.stats-favourites a')].map(a => a.pathname + a.hash).join()"),
    [...favourite.weapons.filter(item => item.key).map(item => `/wiki/weapons#wpn_${item.key}`), ...favourite.armour.map(item => `/wiki/equipment#${item.key}`)].join());
  await evaluate("document.querySelector('.stats-favourites .table-scroll:last-child tbody tr td:nth-child(2)').click()");
  await new Promise(resolve => setTimeout(resolve, 800));
  assert.equal(await evaluate('location.pathname + location.hash'), `/wiki/equipment#${piece.key}`);
  assert.equal(await evaluate(`document.getElementById(${JSON.stringify(piece.key)}).open`), true, 'the piece is opened there');
  await navigate(`/wiki/weapons?lang=en#wpn_${known.key}`);
  assert.equal(await evaluate(`document.getElementById(${JSON.stringify('wpn_' + known.key)}) !== null`), true, 'the weapon has a card');
  await navigate('/stats?player=Sheep&lang=en');
  assert.equal(await fits(), true, 'the matches fit the width');
  await screenshot('player');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(await count('.stats-paged .stats-table tbody tr'), 23);
  assert.ok(asked.some(path => path.includes('player/matches') && path.includes('before=')), 'pages by before=');
  const results = await cells('.stats-paged tbody td:nth-child(6)');
  assert.ok(results.includes('Draw') && results.includes('Win') && results.includes('Loss'), results.join());

  // Yasen played exactly a page: the older page is empty, so the button just goes.
  await navigate('/stats?player=Yasen&lang=en');
  assert.equal(await count('.stats-paged .stats-table tbody tr'), 15);
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), false, 'a full page may have more');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(answered['player/matches'].matches.length, 0, 'the page after is empty');
  assert.equal(await count('.stats-paged .stats-table tbody tr'), 15);
  assert.equal(await count('.stats-paged .stats-table'), 1, 'no second table');
  assert.equal(await count('.stats-empty'), 0, 'no notice under the rows');
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), true, 'no button left');

  // A player's weapons: its other list, what its games counted of each, most kills first.
  await navigate('/stats?player=Sheep&lang=en');
  assert.deepEqual(await cells('.stats-tabs button'), ['Matches', 'Weapons', 'Skills']);
  assert.equal(await text('.stats-tabs button[aria-pressed=true]'), 'Matches');
  await evaluate("[...document.querySelectorAll('.stats-tabs button')].find(b => b.textContent === 'Weapons').click()");
  await settled();
  assert.equal(await text('.stats-tabs button[aria-pressed=true]'), 'Weapons');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('tab')"), 'weapons');
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('tab=weapons'), 'the language link keeps the list');
  assert.equal(await count('.stats-paged'), 0, 'the matches gone');
  const guns = answered['player/weapons'].weapons;
  assert.ok(guns.length >= 2 && guns.every((gun, i) => !i || guns[i - 1].kills >= gun.kills), 'most kills first');
  assert.deepEqual(await cells('.stats-weapons thead th'),
    ['Weapon', 'Kills', 'K/D', 'Kills/min', 'HS %', 'Accuracy', 'Avg distance', 'Through walls', 'Ricochets', 'Hours played']);
  assert.equal(await text('.stats-weapons thead th.sorted'), 'Kills');
  assert.deepEqual(await cells('.stats-weapons tbody td.l'), guns.map(gun => gun.name ?? '—'));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(2)'), guns.map(gun => String(gun.kills)));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(3)'), guns.map(gun => gun.kd.toFixed(2)), 'by the deaths with it in the hands');
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(4)'),
    guns.map(gun => gun.kills_per_minute === null ? '—' : gun.kills_per_minute.toFixed(2)), 'by the time in the hands');  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(6)'), guns.map(gun => aim(gun)));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(7)'), guns.map(gun => gun.kill_distance_m === null ? '—' : `${Math.round(gun.kill_distance_m)} m`));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(8)'), guns.map(gun => String(gun.wall_kills)));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(9)'), guns.map(gun => String(gun.ricochet_kills)));
  assert.deepEqual(await cells('.stats-weapons tbody td:nth-child(10)'), guns.map(gun => (gun.held_s / 3600).toFixed(1)));
  assert.ok(guns.some(gun => gun.name === null), 'a weapon gone from the game data');
  assert.deepEqual(await evaluate("[...document.querySelectorAll('.stats-weapons a')].map(a => a.pathname + a.hash).join()"),
    guns.filter(gun => gun.key).map(gun => `/wiki/weapons#wpn_${gun.key}`).join(), 'each leads to its card in the wiki');
  assert.equal(await count('.stats-weapons tbody .stats-item'), guns.length, 'an icon or its empty frame a weapon');
  assert.equal(await count('.stats-weapons tbody img.stats-item'), guns.filter(gun => gun.icon).length);
  assert.ok(guns.some(gun => gun.icon));
  assert.equal(await fits(), true, 'the weapons fit the width');
  await screenshot('player-weapons');
  // A click on a heading sorts by its column, the largest first.
  for (const [label, by, value] of [['K/D', 'kd', gun => gun.kd], ['Kills/min', 'kills_per_minute', gun => gun.kills_per_minute ?? -1], ['HS %', 'headshot_rate', gun => gun.kills ? gun.headshots / gun.kills : -1],
    ['Accuracy', 'accuracy', gun => gun.shots ? gun.hits / gun.shots : -1], ['Avg distance', 'kill_distance', gun => gun.kill_distance_m ?? -1],
    ['Through walls', 'wall_kills', gun => gun.wall_kills], ['Ricochets', 'ricochet_kills', gun => gun.ricochet_kills],
    ['Hours played', 'held', gun => gun.held_s], ['Kills', 'kills', gun => gun.kills]]) {
    await evaluate(`[...document.querySelectorAll('.stats-weapons thead th button')].find(b => b.textContent === '${label}').click()`);
    await settled();
    const sorted = answered['player/weapons'];
    assert.equal(sorted.by, by);
    assert.equal(await text('.stats-weapons thead th.sorted'), label);
    assert.equal(await evaluate("new URLSearchParams(location.search).get('by')"), by);
    const values = sorted.weapons.map(value);
    assert.ok(values.length === guns.length && values.every((v, i) => !i || values[i - 1] >= v - 0.06), `${by}: ${values.join()}`);
    assert.deepEqual(await cells('.stats-weapons tbody td.l'), sorted.weapons.map(gun => gun.name ?? '—'));
  }
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('by=kills'), 'the language link keeps the order');
  // A second click on the sorted heading: the fewest first; a third, back.
  await evaluate("[...document.querySelectorAll('.stats-weapons thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  const fewest = answered['player/weapons'];
  assert.equal(fewest.order, 'asc');
  assert.ok(fewest.weapons.every((gun, i) => !i || fewest.weapons[i - 1].kills <= gun.kills), 'the fewest kills first');
  assert.deepEqual(await cells('.stats-weapons tbody td.l'), fewest.weapons.map(gun => gun.name ?? '—'));
  assert.equal(await evaluate("document.querySelector('.stats-weapons thead th.sorted').getAttribute('aria-sort')"), 'ascending');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('order')"), 'asc');
  await evaluate("[...document.querySelectorAll('.stats-weapons thead th button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(answered['player/weapons'].order, 'desc');
  await evaluate("[...document.querySelectorAll('.stats-tabs button')].find(b => b.textContent === 'Matches').click()");
  await settled();
  assert.equal(await evaluate('location.search'), '?lang=en&id=1042', 'the matches: neither the list nor its order in the address');
  assert.ok(!(await evaluate("document.querySelector('.lang a[hreflang=uk]').href")).includes('tab='), 'nor in the language link');
  // The address opens the list; five empty rows while it loads.
  await navigate('/stats?player=Sheep&tab=weapons&lang=en', 'interactive');
  assert.equal(await count('.stats-weapons tbody tr'), 5);
  await settled();
  assert.equal(await count('.stats-weapons tbody tr'), guns.length);
  await evaluate("document.querySelector('.stats-weapons tbody tr td:nth-child(2)').click()");
  await new Promise(resolve => setTimeout(resolve, 800));
  assert.equal(await evaluate('location.pathname + location.hash'), `/wiki/weapons#wpn_${guns[0].key}`, 'a click anywhere on its row too');
  // The skills: the wiki's trees with the player's ranks (the preview's Sheep has nine skills,
  // 25 points), drawn empty while its card loads; a click changes nothing.
  await navigate('/stats?id=1042&tab=skills&lang=en', 'interactive');
  assert.equal(await count('.stats-skills .skill'), await count('.stats-skills .skill:not(.taken)'), 'the trees are drawn before the build');
  await settled();
  await new Promise(resolve => setTimeout(resolve, 500));
  assert.equal(await count('.stats-skills .skill.taken'), 9);
  assert.equal(await text('.stats-skills .skill-branch-points'), '17', 'the first branch\'s points');
  await evaluate("document.querySelector('.stats-skills .skill:not(.taken)').click()");
  assert.equal(await count('.stats-skills .skill.taken'), 9);
  assert.equal(await count('.stats-skills .skill-plan'), 0, 'no planner');
  // Yasen's games counted none.
  await navigate('/stats?player=Yasen&tab=weapons&lang=en');
  assert.equal(await text('.stats-weapons .stats-empty'), 'No weapons counted yet.');

  // A player without a public match is the same as an unknown one.
  await navigate('/stats?player=Nova&lang=en');
  assert.equal(await text('.stats-empty'), 'No player with public matches named “Nova”.');
  assert.equal(await count('.stats-panels'), 0);
  // A name a player left finds it, while nobody has that name now.
  await navigate('/stats?player=baran&lang=en');
  assert.equal(await text('#stats-title'), 'Sheep');
  await navigate('/stats?player=Nobody&lang=en');
  assert.equal(await text('.stats-empty'), 'No player with public matches named “Nobody”.');
  assert.equal(await count('.stats-find'), 1, 'the search stays');
  await screenshot('player-not-found');

  // A match, an unknown one.
  await navigate(`/stats?match=${newest}&lang=en`, 'interactive');
  assert.equal(await count('.stats-table tbody tr'), 12, 'six empty rows a team while it loads');
  assert.equal(await count('.stats-under'), 12, 'each with room for the weapons');
  const loadingMatch = await top(1);
  await settled();
  assert.equal(await top(1), loadingMatch, 'no layout jump');
  assert.equal(await count('.stats-table'), 2);
  assert.equal(await count('.stats-table tbody tr'), 12);
  assert.match(await text('.stats-heading'), /^Team 1 · \d/);
  const matchColumns = await columns();
  assert.ok(matchColumns.includes('HS %'));
  assert.ok(matchColumns.includes('Accuracy'));
  assert.deepEqual(await cells('.stats-kit.stats-under'), answered.match.players.map(weapons), 'weapons under each name');
  assert.deepEqual(await cells('.stats-table tbody td:nth-child(7)'), answered.match.players.map(row => aim(row)));
  assert.ok(!['Level', 'Captures'].some(label => matchColumns.includes(label)), matchColumns.join());
  // A team sorts by a click on a heading, alone: the most kills first, then the fewest; the other team stays by place.
  const team = nth => evaluate(`JSON.stringify([...document.querySelectorAll('.stats-table')[${nth}].querySelectorAll('tbody tr')].map(r => [1, 4].map(i => Number(r.children[i - 1].textContent))))`).then(JSON.parse);
  assert.deepEqual((await team(0)).map(([place]) => place), [1, 2, 3, 4, 5, 6]);
  assert.deepEqual(await cells('.stats-table thead th.sorted'), ['Place', 'Place']);
  const byKills = "[...document.querySelectorAll('.stats-table')[0].querySelectorAll('thead th button')].find(b => b.textContent === 'Kills').click()";
  await evaluate(byKills);
  assert.ok((await team(0)).every(([, kills], i, rows) => !i || rows[i - 1][1] >= kills), 'the most kills first');
  assert.deepEqual(await cells('.stats-table thead th.sorted'), ['Kills', 'Place']);
  assert.deepEqual((await team(1)).map(([place]) => place), [1, 2, 3, 4, 5, 6], 'the other team as it was');
  await evaluate(byKills);
  assert.ok((await team(0)).every(([, kills], i, rows) => !i || rows[i - 1][1] <= kills), 'then the fewest');
  assert.equal(await count('.stats-table tbody tr'), 12);
  await navigate(`/stats?match=${newest}&lang=en`);
  assert.equal(await fits(), true, 'the teams fit the width');
  await screenshot('match');
  // The teams in the Maps page's colours; the map leads to it there, in the match's mode.
  assert.deepEqual(JSON.parse(await evaluate("JSON.stringify([...document.querySelectorAll('.stats-team')].map(h => getComputedStyle(h).borderLeftColor))")),
    ['rgb(59, 130, 246)', 'rgb(229, 72, 77)']);
  assert.deepEqual(JSON.parse(await evaluate("JSON.stringify([...document.querySelectorAll('.stats-panels .stats-side')].map(s => getComputedStyle(s).color))")),
    [answered.match.match.winner === 0 ? 'rgb(59, 130, 246)' : 'rgb(229, 72, 77)', 'rgb(59, 130, 246)', 'rgb(229, 72, 77)'], 'the winner and the points in the teams\' colours');
  const played = answered.match.match;
  assert.equal(await evaluate("(i => new URL(i.src).pathname + new URL(i.src).search)(document.querySelector('.stats-panels img.stats-map'))"), played.map_icon, 'the match with its map\'s picture');
  assert.deepEqual(await unstretched(), [], 'the match\'s picture not stretched');
  assert.equal(await evaluate("(a => a.pathname + a.hash)([...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Map').nextSibling.querySelector('a'))"),
    `/wiki/maps#${played.map_key}/${played.rules}`);
  assert.equal(await evaluate("(a => a.pathname + a.hash)(document.querySelector('.stats-panels a.stats-map-link'))"), `/wiki/maps#${played.map_key}/${played.rules}`, 'the picture leads there too');
  await navigate(`/wiki/maps?lang=en#${played.map_key}/${played.rules}`);
  assert.equal(await evaluate("document.querySelector('#maps [aria-pressed=true], .chip[aria-pressed=true]') !== null"), true, 'the Maps page opens');
  await navigate(`/stats?match=${newest}&lang=en`);
  // The newest match has a replay: one Download, which asks for it and takes its file. The one before it stays being made.
  assert.equal(answered.match.replay, true);
  assert.equal(await text('.stats-replay button'), 'Download');
  await evaluate("document.querySelector('.stats-replay button').click()");
  await new Promise(resolve => setTimeout(resolve, 800));
  await settled();
  assert.equal(answered.replay.status, 'ready');
  assert.equal(await evaluate("document.querySelector('.stats-replay a').pathname"), answered.replay.url);
  assert.equal(await text('.stats-replay a'), 'Download');
  assert.equal(await evaluate('location.search'), `?match=${newest}&lang=en`, 'the page stays');
  await navigate(`/stats?match=${newest - 1}&lang=en`);
  await evaluate("document.querySelector('.stats-replay button').click()");
  await new Promise(resolve => setTimeout(resolve, 800));
  assert.equal(await text('.stats-replay'), 'Preparing…');
  await navigate(`/stats?match=${newest - 2}&lang=en`);
  assert.equal(await count('.stats-replay'), 0, 'no button for a match without a recording');
  await navigate('/stats?match=99&lang=en');
  assert.equal(await text('.stats-empty'), 'No public match 99.');
  // A free-for-all: one list by place, its winners named.
  await navigate('/stats?match=3&lang=en');
  assert.equal(await count('.stats-table'), 1);
  assert.equal(await count('.stats-table tbody tr'), answered.match.players.length);
  assert.deepEqual(await cells('.stats-table tbody td:first-child'), answered.match.players.map((_, i) => String(i + 1)), 'by place');
  assert.equal(await panel('Result'), answered.match.players.filter(row => row.outcome === 'won').map(row => row.player.name).join(', '));
  assert.equal(await panel('Points'), '—');
  await screenshot('match-free-for-all');
  // One who left a match its team won lost it.
  await navigate('/stats?player=Dub&lang=en');
  assert.ok((await cells('.stats-paged tbody td:nth-child(6)')).includes('Loss · Left'));

  // A clan: its commander first, its matches paged.
  await navigate('/stats?clan=skh&lang=en');
  assert.equal(await text('#stats-title'), '[SKH] Skhid');
  assert.equal(await count('.stats-table:first-of-type tbody tr'), 3 + 15);
  assert.match(await text('.stats-table tbody tr td'), /Sheep · Commander/);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Members').nextSibling.textContent"), String(answered.clan.clan.members));
  assert.equal(await panel('Artefacts'), String(answered.clan.totals.artefacts));
  assert.ok((await columns()).includes('HS %'), 'the members have theirs');
  await screenshot('clan');
  // RAD's commander has no public match: the clan counts him, lists only who played.
  await navigate('/stats?clan=RAD&lang=en');
  assert.equal(await panel('Members'), String(answered.clan.clan.members));
  assert.equal(await count('#stats > .table-scroll tbody tr'), answered.clan.members.length);
  assert.equal(await panel('Commander'), answered.clan.clan.leader ? answered.clan.clan.leader.name : '—');
  assert.deepEqual([answered.clan.clan.members, answered.clan.members.length, answered.clan.clan.leader], [3, 2, null]);
  assert.equal(await panel('Accuracy'), aim(answered.clan.totals));
  await navigate('/stats?clan=NONE&lang=en');
  assert.equal(await text('.stats-empty'), 'No clan tagged [NONE].');

  // Ukrainian: its longer words fit too.
  for (const path of ['player=Sheep', `match=${newest}`, 'board=players&by=accuracy', 'clan=SKH']) {
    await navigate(`/stats?${path}&lang=uk`);
    assert.equal(await fits(), true, `${path} fits the width in Ukrainian`);
  }

  // Another of the game's languages: the game's names and the page's own words in it; the header
  // names it and offers the eleven others, each keeping what is shown.
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  await navigate('/stats?board=latest&lang=ru');
  assert.equal(await evaluate('document.documentElement.lang'), 'ru');
  assert.equal(await text('.lang summary'), 'Русский');
  assert.equal(await count('.lang a'), 11);
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=pl]').href")).includes('board=latest'));
  assert.equal(await text('.stats-heading'), 'Лидеры дня');
  assert.ok(asked.some(path => path.startsWith('/api/history/matches') && path.includes('lang=ru')), 'the names are asked for in it');
  assert.equal(await evaluate("document.querySelector('.stats-board .stats-map-cell a:not(.stats-map-link)').textContent"), answered.matches.matches[0].map);
  await evaluate("document.querySelector('.lang summary').click()");
  await screenshot('language-picker');

  // Ukrainian, and a phone's width.
  await send('browsingContext.setViewport', {context, viewport: {width: 390, height: 844}, devicePixelRatio: 1});
  await navigate('/stats?player=Sheep&lang=uk');
  assert.equal(await evaluate('document.documentElement.lang'), 'uk');
  assert.equal(await text('.stats-heading'), 'Улюблене спорядження');
  assert.deepEqual(await cells('.stats-tabs button'), ['Матчі', 'Зброя', 'Навички']);
  assert.notEqual(await panel('Артефакти'), null);
  assert.equal(await panel('Точність'), aim(answered.player.totals, 'uk'));
  assert.equal(await evaluate("getComputedStyle(document.querySelector('.stats-favourites')).gridTemplateColumns.split(' ').length"), 1, 'the favourites stacked');
  assert.ok((await evaluate("document.querySelector('.lang a[hreflang=en]').href")).includes('id=1042'));
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'mobile page fits');
  await navigate('/stats?player=Sheep&tab=weapons&lang=uk');
  assert.deepEqual(await cells('.stats-weapons thead th'),
    ['Зброя', 'Вбивства', 'В/С', 'Вбивств/хв', 'У голову %', 'Точність', 'Сер. відстань', 'Крізь стіни', 'Рикошетом', 'Годин у грі']);
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'the weapons fit a phone');
  await screenshot('player-weapons-uk-phone');
  await screenshot('player-mobile-uk');
  await navigate(`/stats?match=${newest}&lang=uk`);
  assert.equal(await count('.stats-kit.stats-under'), 12);
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'mobile match fits');
  await screenshot('match-mobile-uk');
  console.log(`ok: ${asked.length} answers; screenshots in ${output}`);
} finally {
  await send('session.end').catch(() => {});
  socket.close();
  server.close();
}
