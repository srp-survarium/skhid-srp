// The stats page (/wiki/stats, history.js) in an isolated Firefox, against
// fixture JSON in the contract's shapes (history_fixture.mjs). Node 22+;
// Firefox: --headless --remote-debugging-port 9457. The page, its script and
// its policy come from the real HTTP preview
// (cargo test -p web-server serve_wiki_preview -- --ignored --nocapture);
// this script serves them on port 24584 with /api/history/... answered from
// the fixture, each answer late by 300 ms, so the page is seen loading too.
// With a fifth argument `real`, /api/history/... is the preview's own: a
// web server over an srp.db holding the fixture's players, clans and 23
// public matches (Yasen in the newest 15, one a draw), recorded as srp
// records them (what is checked is the same; the ratings and the match
// numbers are the answers').
// Saves review screenshots to /tmp/wiki-stats-review.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import http from 'node:http';
import {answer} from './history_fixture.mjs';
const [preview = 'http://127.0.0.1:24583', port = '9457', output = '/tmp/wiki-stats-review', own = '24584', source = 'fixture'] = process.argv.slice(2);
const real = source === 'real';
const base = `http://127.0.0.1:${own}`;
await fs.mkdir(output, {recursive: true});
const asked = [];
// The last answer of each endpoint, by its path under /api/history/.
const answered = {};
const server = http.createServer(async (request, response) => {
  const url = new URL(request.url, base);
  if (url.pathname.startsWith('/api/history/')) {
    asked.push(url.pathname + url.search);
    const path = url.pathname.slice('/api/history/'.length);
    let status, body;
    if (real) {
      const upstream = await fetch(preview + request.url);
      [status, body] = [upstream.status, await upstream.json()];
    } else [status, body] = answer(path, url.searchParams);
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
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});

  // The wiki's index leads to it.
  await navigate('/wiki');
  assert.ok(await evaluate("[...document.querySelectorAll('.wiki-link')].some(a => a.pathname === '/wiki/stats')"));
  const policy = (await fetch(base + '/wiki/stats')).headers.get('content-security-policy');
  assert.ok(policy.includes("connect-src 'self'") && policy.includes("script-src 'self'"), policy);

  // The leaderboards: players by rating first, then each board and key.
  await navigate('/wiki/stats');
  assert.equal(await text('#stats-title'), 'Stats');
  assert.equal(await count('.stats-table tbody tr'), 12, 'every player with a match, Nova not');
  const named = ({player}) => `${player.clan ? `[${player.clan.tag}] ` : ''}${player.name}`;
  assert.equal(await text('.stats-table tbody tr td:nth-child(2)'), named(answered.top.players[0]));
  if (!real) assert.equal(named(answered.top.players[0]), '[SKH] Sheep');
  assert.equal(await text('.stats-table thead th.sorted'), 'Rating');
  await screenshot('boards-players');
  await evaluate("[...document.querySelectorAll('.stats-keys button')].find(b => b.textContent === 'Kills').click()");
  await settled();
  assert.equal(await text('.stats-table thead th.sorted'), 'Kills');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-table tbody tr')].map(r => Number(r.children[7].textContent.replace(/\\D/g, ''))).every((v, i, a) => !i || a[i - 1] >= v)"), true, 'sorted by kills');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('by')"), 'kills');
  assert.ok((await evaluate("document.querySelector('a.lang').href")).includes('by=kills'), 'the language link keeps the board');
  await evaluate("[...document.querySelectorAll('.stats-keys button')].find(b => b.textContent === 'Artefacts').click()");
  await settled();
  assert.equal(answered.top.by, 'artefacts');
  assert.equal(await text('.stats-table thead th.sorted'), 'Artefacts');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-table tbody tr')].map(r => Number(r.children[13].textContent.replace(/\\D/g, ''))).every((v, i, a) => !i || a[i - 1] >= v)"), true, 'sorted by artefacts');
  await evaluate("document.querySelector('[data-board=clans]').click()");
  await settled();
  assert.equal(await count('.stats-table tbody tr'), 2);
  const firstClan = answered.clans.clans[0];
  assert.equal(await text('.stats-table tbody tr td:nth-child(2) a'), `[${firstClan.tag}] ${firstClan.name}`);
  assert.equal(await text('.stats-table tbody tr td:nth-child(4)'), String(firstClan.members));
  await evaluate("document.querySelector('[data-board=best]').click()");
  await settled();
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-keys button')].map(b => b.textContent).join()"), 'Day,Hour', 'no week');
  assert.equal(await count('.stats-table tbody tr'), 20);
  await screenshot('boards-best');
  await evaluate("document.querySelector('[data-board=latest]').click()");
  await settled();
  assert.equal(await count('.stats-table tbody tr'), 15);
  const newest = answered.matches.matches[0].id;
  if (!real) assert.equal(newest, 5531);
  assert.equal(await text('.stats-table tbody tr td:nth-child(2)'), String(newest), 'newest first');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(await count('.stats-table tbody tr'), 23);
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), true, 'no older page');

  // The search goes to a player with only what is filled in.
  await navigate('/wiki/stats');
  await evaluate("document.querySelector('.stats-find [name=player]').value = ' sheep ';document.querySelector('.stats-find button').click()");
  await new Promise(resolve => setTimeout(resolve, 500));
  await settled();
  assert.equal(await evaluate('location.search'), '?player=sheep&lang=en');

  // A player: nothing moves while it loads; 15 matches, then 8 older.
  await navigate('/wiki/stats?player=Sheep&lang=en', 'interactive');
  assert.equal(await count('.stats-panels dd'), 23, 'the panels are drawn before their data');
  const loading = await top(0);
  await settled();
  assert.equal(await top(0), loading, 'no layout jump');
  assert.equal(await text('#stats-title'), 'Sheep');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Clan').nextSibling.textContent"), '[SKH]');
  assert.equal(await count('.stats-table tbody tr'), 15);
  assert.equal(await panel('Artefacts'), String(answered.player.totals.artefacts), 'the artefacts taken');
  assert.ok((await columns()).includes('Artefacts'), 'a match line has its artefacts');
  assert.equal(await evaluate("[...document.querySelectorAll('.table-scroll')].every(box => box.scrollWidth <= box.clientWidth)"), true, 'the matches fit the width');
  await screenshot('player');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(await count('.stats-table tbody tr'), 23);
  assert.ok(asked.some(path => path.includes('player/matches') && path.includes('before=')), 'pages by before=');
  assert.ok(await evaluate("[...document.querySelectorAll('.stats-table tbody td:nth-child(6)')].some(td => td.textContent === 'Draw')"));

  // Yasen played exactly a page: the older page is empty, so the button just goes.
  await navigate('/wiki/stats?player=Yasen&lang=en');
  assert.equal(await count('.stats-table tbody tr'), 15);
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), false, 'a full page may have more');
  await evaluate("document.querySelector('.stats-paged button').click()");
  await settled();
  assert.equal(answered['player/matches'].matches.length, 0, 'the page after is empty');
  assert.equal(await count('.stats-table tbody tr'), 15);
  assert.equal(await count('.stats-paged .stats-table'), 1, 'no second table');
  assert.equal(await count('.stats-empty'), 0, 'no notice under the rows');
  assert.equal(await evaluate("document.querySelector('.stats-paged button').hidden"), true, 'no button left');

  // A new player, an unknown one.
  await navigate('/wiki/stats?player=Nova&lang=en');
  assert.equal(await text('.stats-empty'), 'No public matches yet.');
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Win rate').nextSibling.textContent"), '—');
  await screenshot('player-new');
  await navigate('/wiki/stats?player=Nobody&lang=en');
  assert.equal(await text('.stats-empty'), 'No player named “Nobody”.');
  assert.equal(await count('.stats-find'), 1, 'the search stays');
  await screenshot('player-not-found');

  // A match, an unknown one.
  await navigate(`/wiki/stats?match=${newest}&lang=en`, 'interactive');
  assert.equal(await count('.stats-table tbody tr'), 12, 'six empty rows a team while it loads');
  const loadingMatch = await top(1);
  await settled();
  assert.equal(await top(1), loadingMatch, 'no layout jump');
  assert.equal(await count('.stats-table'), 2);
  assert.equal(await count('.stats-table tbody tr'), 12);
  assert.match(await text('.stats-heading'), /^Team 1 · \d/);
  assert.ok((await columns()).includes('Artefacts'));
  await screenshot('match');
  await navigate('/wiki/stats?match=99&lang=en');
  assert.equal(await text('.stats-empty'), 'No public match 99.');

  // A clan: its commander first, its matches paged.
  await navigate('/wiki/stats?clan=skh&lang=en');
  assert.equal(await text('#stats-title'), '[SKH] Skhid');
  assert.equal(await count('.stats-table:first-of-type tbody tr'), 3 + 15);
  assert.match(await text('.stats-table tbody tr td'), /Sheep · Commander/);
  assert.equal(await evaluate("[...document.querySelectorAll('.stats-panels dt')].find(e => e.textContent === 'Members').nextSibling.textContent"), String(answered.clan.clan.members));
  assert.equal(await panel('Artefacts'), String(answered.clan.totals.artefacts));
  assert.ok((await columns()).includes('Artefacts'), 'the members have theirs');
  await screenshot('clan');
  await navigate('/wiki/stats?clan=NONE&lang=en');
  assert.equal(await text('.stats-empty'), 'No clan tagged [NONE].');

  // Ukrainian, and a phone's width.
  await send('browsingContext.setViewport', {context, viewport: {width: 390, height: 844}, devicePixelRatio: 1});
  await navigate('/wiki/stats?player=Sheep&lang=uk');
  assert.equal(await evaluate('document.documentElement.lang'), 'uk');
  assert.equal(await text('.stats-heading'), 'Матчі');
  assert.notEqual(await panel('Артефакти'), null);
  assert.ok((await evaluate("document.querySelector('a.lang').href")).includes('player=Sheep'));
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'mobile page fits');
  await screenshot('player-mobile-uk');
  console.log(`ok: ${asked.length} answers from ${real ? 'the real endpoints' : 'the fixture'}; screenshots in ${output}`);
} finally {
  await send('session.end').catch(() => {});
  socket.close();
  server.close();
}
