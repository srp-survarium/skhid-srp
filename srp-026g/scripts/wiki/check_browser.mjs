// Node 22+, isolated Firefox: --headless --remote-debugging-port 9457.
// Real HTTP preview: cargo test -p web-server serve_wiki_preview -- --ignored --nocapture
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const [base = 'http://127.0.0.1:24583', port = '9457', output = '/tmp/wiki-weapons-review'] = process.argv.slice(2);
await fs.mkdir(output, {recursive: true});
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
  async function navigate(path) {
    await send('browsingContext.navigate', {context, url: base + path, wait: 'complete'});
    await evaluate('document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(resolve)))');
  }
  async function screenshot(name) {
    await evaluate(`Promise.all([...document.querySelectorAll('img[src]')]
      .filter(e => e.getBoundingClientRect().top < innerHeight && e.getBoundingClientRect().bottom > 0)
      .map(e => new Promise((resolve, reject) => {
        if (e.complete && e.naturalWidth) return resolve();
        e.addEventListener('load', resolve, {once:true});
        e.addEventListener('error', () => reject(Error('Weapon image failed')), {once:true});
        e.loading = 'eager';
      })))`);
    const {data} = await send('browsingContext.captureScreenshot', {context});
    await fs.writeFile(`${output}/${name}.png`, Buffer.from(data, 'base64'));
  }
  const visible = "[...document.querySelectorAll('.weapon')].filter(e => !e.hidden)";
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  await navigate('/wiki');
  assert.equal(await evaluate("document.querySelector('.wiki-link').pathname"), '/wiki/weapons');
  assert.equal(await evaluate("[...document.querySelectorAll('.wiki-link')][1].pathname"), '/wiki/simulators');
  await screenshot('wiki');
  await navigate('/wiki/weapons');
  await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
  const total = await evaluate("document.querySelectorAll('.weapon').length");
  assert.ok(total > 50);
  assert.equal(await evaluate(`${visible}.length`), total);
  assert.equal(await evaluate("document.querySelector('.catalogue-tools').hidden"), false, 'script works under the real CSP');
  assert.equal(await evaluate("document.querySelector('#wpn_benelli_m4').dataset.class"), 'Shotgun');
  await screenshot('weapons-desktop');
  await evaluate("document.querySelector('#compare-a-btn').click()");
  assert.equal(await evaluate("document.activeElement.className"), 'wpick-search');
  assert.equal(await evaluate("document.querySelectorAll('#compare-a-list [role=option]').length"), total);
  assert.equal(await evaluate("[...document.querySelectorAll('#compare-a-list [role=group]')].every(group => [...group.querySelectorAll('[role=option]')].every((option,i,items) => !i || Number(items[i-1].querySelector('small').textContent.replace(/\\D/g,'')) <= Number(option.querySelector('small').textContent.replace(/\\D/g,''))))"), true, 'chooser sorts each class by level');
  await screenshot('weapon-chooser');
  await evaluate("document.activeElement.value='AK74M';document.activeElement.dispatchEvent(new Event('input'))");
  assert.equal(await evaluate("[...document.querySelectorAll('#compare-a-list [role=option]')].filter(e=>!e.hidden).length"), 1);
  await evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}))");
  assert.equal(await evaluate("document.querySelector('#compare-a').value"), 'wpn_ak_74m');
  assert.equal(await evaluate("document.activeElement.id"), 'compare-a-btn');
  assert.equal(await evaluate("document.querySelector('#compare-a-btn img').naturalWidth > 0"), true);
  await evaluate("document.querySelector('#compare-b-btn').click();document.activeElement.value='PM';document.activeElement.dispatchEvent(new Event('input'));document.querySelector('#compare-b-wpn_pm').click()");
  assert.ok((await evaluate("document.querySelector('#compare-simulate').search")).includes('a=wpn_ak_74m&b=wpn_pm'), 'the chooser updates the stats and the simulators link');
  assert.equal(await evaluate("new URLSearchParams(location.search).get('b')"), 'wpn_pm');
  await evaluate("document.querySelector('#compare-b-btn').click();document.activeElement.value='no such weapon';document.activeElement.dispatchEvent(new Event('input'));document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}))");
  assert.equal(await evaluate("document.querySelector('#compare-b-picker .wpick-empty').hidden"), false);
  assert.equal(await evaluate("document.querySelector('#compare-b').value"), 'wpn_pm', 'no-match Enter keeps selection');
  await evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");
  assert.equal(await evaluate("document.querySelector('#compare-b-btn').getAttribute('aria-expanded')"), 'false');
  await evaluate("document.querySelector('#compare-a-btn').click();document.querySelector('#compare-b-btn').focus();document.querySelector('#compare-b-btn').click()");
  assert.equal(await evaluate("document.querySelectorAll('.wpick-pop:not([hidden])').length"), 1, 'one open chooser');
  await evaluate("document.querySelector('#compare-b-btn').dispatchEvent(new MouseEvent('mousedown',{bubbles:true}));document.querySelector('#weapon-search').dispatchEvent(new MouseEvent('mousedown',{bubbles:true}));document.querySelector('#clear-comparison').click()");
  assert.equal(await evaluate("document.querySelectorAll('.wpick-pop:not([hidden])').length"), 0, 'outside click closes');
  await evaluate("document.querySelector('#weapon-search').value='AK74M';document.querySelector('#weapon-search').dispatchEvent(new Event('input'))");
  assert.equal(await evaluate(`${visible}.length`), 1, 'search ignores punctuation');
  await evaluate("document.querySelector('#wpn_ak_74m summary').click()");
  assert.equal(await evaluate("document.querySelector('#wpn_ak_74m').open"), true);
  await screenshot('weapon-details');
  await evaluate("document.querySelector('#weapon-search').value='no weapon matches this';document.querySelector('#weapon-search').dispatchEvent(new Event('input'))");
  assert.equal(await evaluate("document.querySelector('#weapons-empty').hidden"), false);
  await evaluate("document.querySelector('#weapon-search').value='';document.querySelector('#weapon-class').value='Shotgun';document.querySelector('#weapon-class').dispatchEvent(new Event('change'))");
  assert.ok(await evaluate(`${visible}.length`) > 1);
  assert.equal(await evaluate(`${visible}.every(e=>e.dataset.class==='Shotgun')`), true);
  await evaluate("document.querySelector('#weapon-class').value='';document.querySelector('#weapon-sort').value='damage';document.querySelector('#weapon-sort').dispatchEvent(new Event('change'))");
  assert.equal(await evaluate(`${visible}.every((e,i,a)=>!i||Number(a[i-1].dataset.damage)>=Number(e.dataset.damage))`), true);
  await navigate('/wiki/weapons?lang=uk#wpn_ak_74m');
  assert.equal(await evaluate('document.documentElement.lang'), 'uk');
  assert.equal(await evaluate("document.querySelector('#wpn_ak_74m').open"), true);
  await send('browsingContext.setViewport', {context, viewport: {width: 390, height: 844}, devicePixelRatio: 1});
  await evaluate('scrollTo(0,0)');
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'mobile page fits');
  await screenshot('weapons-mobile-uk');
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  await navigate('/wiki/weapons#wpn_ak_74m');
  await screenshot('weapon-expanded');
  assert.equal(await evaluate("[...document.querySelectorAll('.weapon-image[src]')].filter(e=>e.getBoundingClientRect().top<innerHeight&&e.getBoundingClientRect().bottom>0).every(e=>e.complete&&e.naturalWidth>0)"), true, 'visible images load');
  const response = await fetch(base + '/wiki/weapons');
  assert.ok(response.headers.get('content-security-policy').includes("script-src 'self'"));
  assert.equal((await fetch(base + '/wiki/unknown')).status, 404);
  // Rounds: a card's round sets its rows, the list stays on the standard round,
  // a comparison slot's round is in the address.
  await navigate('/wiki/weapons#wpn_vityaz');
  const vityaz = stat => evaluate(`document.querySelector('#wpn_vityaz .weapon-details [data-stat=${stat}] dd').textContent`);
  assert.equal(await vityaz('bullet_damage'), '40');
  assert.equal(await evaluate("document.querySelector('#wpn_vityaz-round-btn img').naturalWidth > 0"), true, 'the round chooser shows its icon');
  await evaluate("document.querySelector('#wpn_vityaz-round-btn').click()");
  assert.equal(await evaluate("[...document.querySelectorAll('#wpn_vityaz-round-list [role=option]')].filter(e => !e.hidden).length"), 3, 'only the weapon\'s rounds');
  await evaluate("document.querySelector('#wpn_vityaz-round-ammo_9x19p_7n31').click()");
  assert.equal(await evaluate("document.querySelector('#wpn_vityaz-round-value').textContent"), '9x19 7N31');
  assert.deepEqual([await vityaz('bullet_damage'), await vityaz('bullet_pierce')], ['52', '26'], 'the round scales the card');
  assert.equal(await evaluate("document.querySelector('#wpn_vityaz .quick-stats dd').textContent"), '40');
  await screenshot('weapon-round');
  assert.match(await evaluate("document.querySelector('#wpn_benelli_m4 [data-stat=bullet_damage] dd').textContent"), / × 10 pellets$/);
  await navigate('/wiki/weapons?a=wpn_vityaz&ra=ammo_9x19p_7n31&b=wpn_vityaz');
  assert.equal(await evaluate("document.querySelector('#compare-a-round').value"), 'ammo_9x19p_7n31', 'URL picks the round');
  assert.equal(await evaluate("[...document.querySelectorAll('.weapon-comparison [data-stat=bullet_damage] td')].map(td => td.firstChild.textContent).join()"), '52,40');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=bullet_damage] td').className"), 'better');
  await screenshot('comparison-round');
  await evaluate("{const s=document.querySelector('#compare-b-round');s.value='ammo_9x19p_7n31';s.dispatchEvent(new Event('change'))}");
  assert.equal(await evaluate("new URLSearchParams(location.search).get('rb')"), 'ammo_9x19p_7n31');
  await evaluate("{const s=document.querySelector('#compare-a-round');s.selectedIndex=0;s.dispatchEvent(new Event('change'))}");
  assert.equal(await evaluate("new URLSearchParams(location.search).has('ra')"), false, 'the standard round stays out of the address');
  await navigate('/wiki/weapons?a=wpn_ak_74m&b=wpn_pm');
  assert.equal(await evaluate("document.querySelector('#compare-a-value').textContent"), 'AK-74M', 'URL paints chooser');
  assert.ok(await evaluate("document.querySelectorAll('.weapon-comparison tr[data-stat]').length") >= 30, 'expanded stat coverage');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=bullet_damage] td').className"), 'better');
  assert.notEqual(await evaluate("getComputedStyle(document.querySelector('.weapon-comparison [data-stat=bullet_damage] td')).color"),
    await evaluate("getComputedStyle(document.querySelector('.weapon-comparison [data-stat=min_damage_amount] td')).color"), 'better values visibly change color');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=min_damage_amount] td').className"), '', 'equal values neutral');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=\"breath_vibration.base_period\"] td').className"), '', 'sway period neutral');
  assert.equal(await evaluate("document.querySelectorAll('.sim-vertical,#recoil-template').length"), 0, 'the simulation is on its own page');
  await screenshot('comparison-stats');
  await evaluate("document.querySelector('#compare-colors').click()");
  assert.equal(await evaluate("document.querySelectorAll('.weapon-comparison .better,.weapon-comparison .worse').length"), 0);
  await evaluate("document.querySelector('#compare-colors').click();document.querySelector('#only-differences').click()");
  assert.equal(await evaluate("!!document.querySelector('.weapon-comparison [data-stat=min_damage_amount]')"), false);
  await evaluate("document.querySelector('#compare-b').value='wpn_ak_74m';document.querySelector('#compare-b').dispatchEvent(new Event('change'))");
  assert.equal(await evaluate("document.querySelector('#compare-same').hidden"), false, 'same weapon has no differences');
  assert.equal(await evaluate("document.querySelectorAll('.weapon-comparison .better,.weapon-comparison .worse').length"), 0);
  await navigate('/wiki/weapons?lang=uk&a=wpn_ak_74m&b=wpn_pm');
  await send('browsingContext.setViewport', {context, viewport: {width:390,height:844}, devicePixelRatio:1});
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'comparison fits mobile');
  await evaluate("document.querySelector('#comparison').scrollIntoView();document.querySelector('#compare-b-btn').click()");
  assert.equal(await evaluate("(()=>{const r=document.querySelector('#compare-b-picker .wpick-pop').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth})()"), true, 'right chooser stays on screen');
  await screenshot('weapon-chooser-mobile-uk');
  await evaluate("document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");
  await evaluate("document.querySelector('#clear-comparison').click()");
  assert.equal(await evaluate("document.querySelector('#compare-table').childElementCount"), 0);
  assert.equal(await evaluate("document.querySelectorAll('.wpick-btn img').length"), 0, 'clear repaints both choosers');
  // A card links to the simulators with its weapon and round.
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  await navigate('/wiki/weapons#wpn_vityaz');
  await evaluate("document.querySelector('#wpn_vityaz-round-btn').click();document.querySelector('#wpn_vityaz-round-ammo_9x19p_7n31').click()");
  assert.equal(await evaluate("new URL(document.querySelector('#wpn_vityaz .weapon-simulate a').href).search"), '?lang=en&a=wpn_vityaz&ra=ammo_9x19p_7n31');
  // The list's quick stats are the same columns in every row, whatever a language makes of
  // them: Russian (the longest headers) and German (the longest damage, "× 10 Schrotkugeln").
  // Each column's left edge one; nothing past its column; a header one line (cut with an
  // ellipsis, its title whole, where its column is narrower); a row's values on one line.
  for (const width of [1440, 1280, 960, 640, 390]) {
    await send('browsingContext.setViewport', {context, viewport: {width, height: 900}, devicePixelRatio: 1});
    for (const lang of ['ru', 'de']) {
      await navigate(`/wiki/weapons?lang=${lang}`);
      const columns = JSON.parse(await evaluate(`JSON.stringify((() => {
        const lefts = [], over = [], tall = [], uncut = [], tops = [];
        for (const dl of document.querySelectorAll('#weapon-list .quick-stats')) {
          const valueTops = new Set();
          [...dl.children].forEach((cell, k) => {
            const box = cell.getBoundingClientRect(), dt = cell.querySelector('dt'), dd = cell.querySelector('dd');
            (lefts[k] ??= new Set()).add(Math.round(box.left));
            if (dd.getBoundingClientRect().right > box.right + 1) over.push(dd.textContent);
            const line = parseFloat(getComputedStyle(dt).lineHeight) || dt.getBoundingClientRect().height;
            if (dt.getBoundingClientRect().height > line * 1.5) tall.push(dt.textContent);
            const style = getComputedStyle(dt);
            if (dt.scrollWidth > dt.clientWidth && (style.textOverflow !== 'ellipsis' || dt.title !== dt.textContent)) uncut.push(dt.textContent);
            valueTops.add(Math.round(dd.getBoundingClientRect().top));
          });
          tops.push(valueTops.size);
        }
        return {lefts: lefts.map(set => set.size), over, tall, uncut, tops: Math.max(...tops)};
      })())`));
      const at = `${lang} at ${width} px`;
      assert.deepEqual(columns.lefts, [1, 1, 1, 1], `${at}: one left edge a column`);
      assert.deepEqual(columns.over, [], `${at}: nothing past its column`);
      assert.deepEqual(columns.tall, [], `${at}: every header one line`);
      assert.deepEqual(columns.uncut, [], `${at}: a cut header ends in an ellipsis, its title whole`);
      assert.equal(columns.tops, 1, `${at}: a row's values on one line`);
    }
  }
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  // In Russian too (the words the server fills in): the list's search, class and sort, the
  // chooser's words, and the simulators' play.
  await navigate('/wiki/weapons?lang=ru');
  await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
  assert.equal(await evaluate("document.querySelector('h1').textContent"), 'Оружие');
  await evaluate("document.querySelector('#weapon-search').value='АК74М';document.querySelector('#weapon-search').dispatchEvent(new Event('input'))");
  assert.equal(await evaluate(`${visible}.length`), 1, 'search by the Russian name');
  const russianClass = await evaluate("document.querySelector('#weapon-class').options[1].value");
  await evaluate(`document.querySelector('#weapon-search').value='';document.querySelector('#weapon-class').value=${JSON.stringify(russianClass)};document.querySelector('#weapon-class').dispatchEvent(new Event('change'))`);
  assert.ok(await evaluate(`${visible}.length`) >= 1);
  assert.equal(await evaluate(`${visible}.every(e=>e.dataset.class===${JSON.stringify(russianClass)})`), true, 'class filter, Russian');
  assert.ok(/[А-Яа-я]/.test(russianClass), `a Russian class: ${russianClass}`);
  await evaluate("document.querySelector('#compare-a-btn').click()");
  assert.equal(await evaluate("document.querySelector('#compare-a-list').closest('.wpick-pop').querySelector('.wpick-search').placeholder"), 'Поиск…');
  await navigate('/wiki/simulators?lang=ru&a=wpn_ak_74m');
  await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
  const playLabel = await evaluate("document.querySelector('#recoil-sim .sim-play').textContent");
  await evaluate("document.querySelector('#recoil-sim .sim-play').click()");
  await evaluate('new Promise(r=>setTimeout(r,300))');
  assert.notEqual(await evaluate("document.querySelector('#recoil-sim .sim-play').textContent"), playLabel, 'play turns to pause, Russian');
  assert.ok(Number(await evaluate("document.querySelector('#recoil-sim .sim-time').value")) > 0, 'play advances, Russian');
  assert.ok(/[А-Яа-я]/.test(await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")), 'the legend in Russian');
  assert.equal(await evaluate("JSON.stringify(window.pageErrors)"), '[]', 'no script errors, Russian');
  // The simulators: one or two weapons' recoil and spread.
  const sim = selector => `document.querySelector('#recoil-sim ${selector}')`;
  const choose = (selector, value) => evaluate(`{const s=${sim(selector)};s.value='${value}';s.dispatchEvent(new Event('change'))}`);
  // A side's setup row (`key`): a button by its value, its strongest, none.
  const row = (side, key) => `.sim-setup[data-side="${side}"] .gmrow[data-key="${key}"]`;
  const setRow = (side, key, value) => evaluate(`document.querySelector('${row(side, key)} .gstep[data-v="${value}"]').click()`);
  const strongest = (side, key) => evaluate(`{const b=[...document.querySelectorAll('${row(side, key)} .gstep')].at(-1);if(b.getAttribute('aria-pressed')!=='true')b.click()}`);
  const clearRow = (side, key) => evaluate(`document.querySelector('${row(side, key)} .gstep[aria-pressed=true]')?.click()`);
  const rowKeys = async side => JSON.parse(await evaluate(`JSON.stringify([...document.querySelectorAll('.sim-setup[data-side="${side}"] .gmrow')].map(r=>r.dataset.key))`));
  await navigate('/wiki/simulators?a=wpn_ak_74m&b=wpn_pm');
  await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), false);
  assert.equal(await evaluate("document.querySelector('#compare-a-value').textContent"), 'AK-74M', 'URL paints chooser');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-vertical path').length"), 4, 'two complete and two elapsed curves');
  assert.equal(await evaluate("document.querySelectorAll('.sim-step').length"), 0, 'no step-by-step mode');
  await evaluate("{const t=document.querySelector('#recoil-sim .sim-time');t.value='0.05';t.dispatchEvent(new Event('input'))}");
  assert.equal(await evaluate("Number(document.querySelector('#recoil-sim .sim-time').value)"), .05);
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).match(/shots 1\/\d+/));
  assert.equal(await evaluate("document.querySelectorAll('#sim-single,.sim-single').length"), 0, 'no single-shot mode');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-sight .sim-cross').length"), 2, 'a crosshair per weapon');
  assert.equal(await evaluate("[...document.querySelectorAll('#recoil-sim .sim-view')].map(view => view.querySelectorAll('.sim-cross').length).join()"), '1,1', 'a view per weapon');
  // The faster weapon's view holds at its last shot while the other still fires.
  {
    const held = await evaluate("{const t=document.querySelector('#recoil-sim .sim-time');document.querySelector('#recoil-sim .sim-last').click();[...document.querySelectorAll('#recoil-sim .sim-view .sim-cross circle')].map(c=>c.getAttribute('cy')).join()}");
    await evaluate("{const t=document.querySelector('#recoil-sim .sim-time');t.value=t.max;t.dispatchEvent(new Event('input'))}");
    assert.equal(await evaluate("[...document.querySelectorAll('#recoil-sim .sim-view .sim-cross circle')].map(c=>c.getAttribute('cy')).join()"), held, 'the views hold at their last shots');
    // Back to the first shot, where the checks below go on from.
    await evaluate("{const t=document.querySelector('#recoil-sim .sim-time');t.value='0.05';t.dispatchEvent(new Event('input'))}");
  }
  await evaluate("{const t=document.querySelector('#recoil-sim .sim-time');t.value='0.3';t.dispatchEvent(new Event('input'))}");
  const position = await evaluate("document.querySelector('#recoil-sim .sim-time').value");
  await evaluate('new Promise(r=>setTimeout(r,150))');
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), position, 'manual position stays fixed');
  // Play runs the burst only when asked, and a second press pauses it.
  await evaluate("document.querySelector('#recoil-sim .sim-play').click()");
  await evaluate('new Promise(r=>setTimeout(r,300))');
  await evaluate("document.querySelector('#recoil-sim .sim-play').click()");
  const played = await evaluate("document.querySelector('#recoil-sim .sim-time').value");
  assert.ok(Number(played) > Number(position), 'play advances the timeline');
  const crossAt = await evaluate("document.querySelector('#recoil-sim .sim-sight .sim-cross circle').getAttribute('cy')");
  await evaluate('new Promise(r=>setTimeout(r,150))');
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), played, 'pause holds the position');
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-sight .sim-cross circle').getAttribute('cy')"), crossAt);
  await screenshot('simulators-crosshair');
  // Last shot and Play stop at the last shot: the settling is the plots' only.
  assert.equal(await evaluate("document.querySelectorAll('#sim-shots,.sim-shots').length"), 0, 'the burst is the smaller magazine, not a setting');
  await evaluate("document.querySelector('#recoil-sim .sim-last').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-last').disabled"), true, 'at the last shot');
  const lastShot = await evaluate("document.querySelector('#recoil-sim .sim-time').value");
  assert.ok(Number(lastShot) < Number(await evaluate("document.querySelector('#recoil-sim .sim-time').max")), 'the settling stays on the timeline');
  await evaluate("document.querySelector('#recoil-sim .sim-reset').click();document.querySelector('#recoil-sim .sim-play').click()");
  await evaluate(`new Promise(r=>setTimeout(r,(Number(${lastShot})+.4)*1000))`);
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), lastShot, 'play ends at the last shot');
  await evaluate("document.querySelector('#recoil-sim .sim-time').value='1.5';document.querySelector('#recoil-sim .sim-time').dispatchEvent(new Event('input'));document.querySelector('#recoil-sim').scrollIntoView()");
  assert.equal(await evaluate("!!document.querySelector('#sim-variation,.sim-variation')"), false, 'average kick option removed');
  const beforeSeed = await evaluate("document.querySelector('#recoil-sim .sim-vertical path').getAttribute('d')");
  await evaluate("document.querySelector('#recoil-sim .sim-seed').value='42';document.querySelector('#recoil-sim .sim-seed').dispatchEvent(new Event('change'))");
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), '1.5', 'seed change preserves timeline');
  assert.notEqual(await evaluate("document.querySelector('#recoil-sim .sim-vertical path').getAttribute('d')"), beforeSeed, 'seed change redraws recoil');
  await screenshot('simulators-recoil');
  await evaluate("document.querySelector('#recoil-sim .sim-reset').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), '0');
  // One weapon is enough; clearing hides the simulation.
  await evaluate("document.querySelector('#clear-comparison').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), true);
  assert.equal(await evaluate("document.querySelector('#compare-empty').hidden"), false);
  await evaluate("document.querySelector('#compare-b').value='wpn_ak_74m';document.querySelector('#compare-b').dispatchEvent(new Event('change'))");
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), false, 'a single weapon gets its simulation');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-sight .sim-cross').length"), 1);
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-sight .sim-cross').classList.contains('series-1')"), true, 'B keeps its colour');
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).includes('/30'), 'its own magazine');
  // Last shot shows the whole pattern; a target is drawn to scale, unmagnified.
  await navigate('/wiki/simulators?a=wpn_ak_74m');
  await evaluate("document.querySelector('#recoil-sim .sim-last').click()");
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).includes('30/30'), 'last shot');
  const trail = await evaluate("document.querySelector('#recoil-sim .sim-trail').getAttribute('d')");
  await choose('.sim-target', '10');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-target-body').length"), 1, 'a target at 10 m');
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-trail').getAttribute('d')"), trail, 'a target leaves the pattern as it is');
  const near = await evaluate("document.querySelector('#recoil-sim .sim-target-body').getAttribute('transform')");
  await choose('.sim-target', '25');
  assert.notEqual(await evaluate("document.querySelector('#recoil-sim .sim-target-body').getAttribute('transform')"), near, 'the distance sizes the target');
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-trail').getAttribute('d')"), trail);
  // The shot's own motions: each weapon's view from the side with its
  // bullets; a custom gun's camera wobble moves the view between shots and
  // its gun kick the sight while aimed, each shown by its layer.
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-view svg.sim-side').length"), 1, 'the view from the side');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-side .sim-landed,#recoil-sim .sim-side .sim-bullet').length"), 30, 'each bullet, on the wall or on its way');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-speed option').length"), 3, 'slow motion');
  const custom = (side, group, key, value) => evaluate(`{const i=document.querySelector('.sim-setup[data-side="${side}"] .sim-custom input[data-group="${group}"][data-key="${key}"]');i.value='${value}';i.dispatchEvent(new Event('change'))}`);
  const layer = (key, on) => evaluate(`{const i=${sim(`.sim-layer-${key}`)};i.checked=${on};i.dispatchEvent(new Event('change'))}`);
  const seekTo = time => evaluate(`{const t=${sim('.sim-time')};t.value='${time}';t.dispatchEvent(new Event('input'))}`);
  const crossY = () => evaluate(`${sim('.sim-view .sim-cross circle')}.getAttribute('cy')`);
  const setupName = () => evaluate("document.querySelector('.sim-setup[data-side=\"0\"] .sim-setup-name').textContent");
  assert.equal(await evaluate(`document.querySelector('${row(0, 'aim')} .gstep[aria-pressed=true]').dataset.v`), 'aimed', 'fully aimed first');
  await seekTo(.07);
  const unmoved = await crossY();
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-gunsight').length"), await evaluate("document.querySelector('#wpn_ak_74m').dataset.recoil.includes('\"gun\"') ? 1 : 0"), 'the sight only with a gun kick');
  await custom(0, 'camera', 'up', 3);
  assert.ok((await setupName()).endsWith('custom'), 'a changed value makes the gun custom');
  assert.ok((await evaluate(`${sim('.sim-legend')}.textContent`)).includes('custom'));
  assert.notEqual(await crossY(), unmoved, 'the camera wobble turns the view between shots');
  await layer('camera', false);
  assert.equal(await crossY(), unmoved, 'without its layer the view is the kick alone');
  await layer('camera', true);
  await custom(0, 'gun', 'up', 2);
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-gunsight').length"), 1, 'the gun kick moves the sight while aimed');
  assert.notEqual(await evaluate(`${sim('.sim-gunsight circle')}.getAttribute('cy')`), await crossY(), 'off the view');
  await layer('gun', false);
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-gunsight').length"), 0);
  await layer('gun', true);
  await screenshot('simulators-custom');
  await setRow(0, 'aim', 'hip');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-gunsight').length"), 0, 'from the hip the sights are not looked through');
  await evaluate("document.querySelector('.sim-setup[data-side=\"0\"] .sim-custom button').click()");
  assert.equal(await setupName(), 'A · AK-74M', 'the weapon again');
  assert.equal(await evaluate("document.querySelector('.sim-setup[data-side=\"0\"] .sim-custom input[data-key=\"rpm\"]').value"), '650', 'its fields are the values of the weapon');
  await layer('recoil', false);
  assert.equal(await crossY(), await evaluate(`${sim('.sim-origin')}.getAttribute('cy')`), 'without the recoil the view stays');
  await layer('recoil', true);
  await custom(0, 'recoil', 'rpm', 300);
  await evaluate(`${sim('.sim-last')}.click()`);
  assert.ok(Number(await evaluate(`${sim('.sim-time')}.value`)) > 5, 'a custom fire rate: 30 shots take longer');
  await evaluate("document.querySelector('.sim-setup[data-side=\"0\"] .sim-custom button').click()");
  // The spread's parts, from the hip (set above); the stance, skills, gloves
  // and arms change them; fully aimed, none of them does.
  const parts = () => evaluate("[...document.querySelectorAll('#recoil-sim .sim-parts tbody td')].map(td=>td.textContent).join(' ')");
  assert.equal(await parts(), '0.2° 5° 2° 25.2°', 'standing from the hip: base, stance, a shot, at most');
  await evaluate("document.querySelector('#recoil-sim .sim-last').click()");
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-hit').length"), 30, 'a hit per shot');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-spread').length"), 1, 'the spread circle');
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).includes('°'), 'the spread in the legend');
  await evaluate("document.querySelector('#recoil-sim .sim-sight').scrollIntoView()");
  await screenshot('simulators-hip');
  await setRow(0, 'stance', 'walk');
  assert.equal(await parts(), '0.2° 10° 2° 30.2°', 'walking');
  // Every skill and modification at its strongest: the gloves' and the body
  // armour's dispersion add up (−10% each), as the pieces' sum does.
  for (const key of await rowKeys(0)) if (/^(skill|mod)-/.test(key)) await strongest(0, key);
  await setRow(0, 'arms', '2');
  const hipAll = await parts();
  assert.equal(hipAll, `0.2° ${+((10 * .76 + 10 * .4) * .8).toFixed(2)}° 1.6° ${+(0.2 + (10 * .76 + 10 * .4) * .8 + 20).toFixed(2)}°`, 'walking from the hip with all of them');
  await setRow(0, 'aim', 'aiming');
  assert.equal(await parts(), `0.2° ${+((10 * .88 + 10 * .4) * .8).toFixed(2)}° 1.6° ${+(0.2 + (10 * .88 + 10 * .4) * .8 + 20).toFixed(2)}°`, 'aiming in: the aimed values, no Accuracy');
  await setRow(0, 'aim', 'aimed');
  for (const stance of ['idle', 'walk', 'crouch', 'crouch_walk', 'run', 'jump']) {
    await setRow(0, 'stance', stance);
    assert.equal(await parts(), '0.2° 0° 0° 0.2°', `fully aimed, ${stance}: the base only, whatever else`);
  }
  await evaluate("document.querySelector('#recoil-sim .sim-last').click()");
  await screenshot('simulators-aimed');
  // Each side its own setup, the same weapon on both: the recoil pieces and
  // the weapon's modification lower B's climb, and A's aim leaves B's spread.
  {
    await navigate('/wiki/simulators?a=wpn_ak_74m&b=wpn_ak_74m');
    const ups = async () => {
      await evaluate("document.querySelector('#recoil-sim .sim-last').click()");
      return [...(await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).matchAll(/up (-?[\d.]+)/g)].map(m => Number(m[1]));
    };
    const [a0, b0] = await ups();
    assert.equal(a0, b0, 'the same weapon and setup, the same climb');
    assert.match(await evaluate("[...document.querySelectorAll('.sim-setup[data-side=\"1\"] .gmrow[data-key=\"mod-9-weapon\"] .gstep')].map(b=>b.textContent).join()"), /^−2%,.*,−10%$/, 'the five strengths, no defect');
    await strongest(1, 'mod-9-weapon');
    const [a1, weapon] = await ups();
    assert.ok(a1 === a0 && weapon < b0, `the weapon's modification lowers B's climb only (${weapon} < ${b0})`);
    await clearRow(1, 'mod-9-weapon');
    const pieces = (await rowKeys(1)).filter(key => key.startsWith('mod-27-'));
    assert.ok(pieces.includes('mod-27-gloves') && pieces.length > 1, `recoil on more than one piece (${pieces})`);
    await strongest(1, 'mod-27-gloves');
    const [, gloves] = await ups();
    for (const key of pieces) await strongest(1, key);
    const [, both] = await ups();
    assert.ok(both < gloves && gloves < b0, `the pieces add up (${both} < ${gloves} < ${b0})`);
    assert.equal(await parts(), '0.2° 0° 0° 0.2° 0.2° 0° 0° 0.2°', 'recoil leaves the spread');
    await setRow(0, 'aim', 'hip');
    assert.equal(await parts(), '0.2° 5° 2° 25.2° 0.2° 0° 0° 0.2°', 'A\'s aim is A\'s');
    await screenshot('simulators-same-weapon');
  }
  // A row's card (what it does): on hover, keyboard focus or a tap.
  {
    const label = "document.querySelector('.sim-setup[data-side=\"0\"] .gmrow[data-key=\"skill-106\"] .glabel')";
    const shown = () => evaluate(`getComputedStyle(${label}.querySelector('.gamemods')).display !== 'none'`);
    assert.equal(await shown(), false, 'closed at first');
    await evaluate(`${label}.scrollIntoView({block:'center'})`);
    const at = JSON.parse(await evaluate(`(()=>{const r=${label}.getBoundingClientRect();return JSON.stringify([Math.round(r.x+8),Math.round(r.y+r.height/2)])})()`));
    const height = await evaluate("document.documentElement.scrollHeight");
    await send('input.performActions', {context, actions: [{type: 'pointer', id: 'mouse', actions: [{type: 'pointerMove', x: at[0], y: at[1]}]}]});
    // Headless Firefox has no hovering pointer unless its prefs give one.
    const hover = await evaluate("matchMedia('(hover:hover)').matches");
    if (hover) assert.equal(await shown(), true, 'hover opens it');
    assert.equal(await evaluate("document.documentElement.scrollHeight"), height, 'it shifts nothing');
    await send('input.performActions', {context, actions: [{type: 'pointer', id: 'mouse', actions: [{type: 'pointerMove', x: 5, y: 5}]}]});
    assert.equal(await shown(), false, 'leaving closes it');
    await evaluate("[...document.querySelectorAll('.sim-setup[data-side=\"0\"] .gmrow[data-key=\"arms\"] .gstep')].at(-1).focus()");
    await send('input.performActions', {context, actions: [{type: 'key', id: 'keyboard', actions: [{type: 'keyDown', value: ''}, {type: 'keyUp', value: ''}]}]});
    assert.equal(await evaluate(`document.activeElement === ${label}`), true, 'the label takes the keyboard');
    assert.equal(await shown(), true, 'keyboard focus opens it');
    assert.ok((await evaluate(`${label}.textContent`)).includes('−4%'), 'the per-rank value from the data');
    await evaluate("document.activeElement.blur()");
  }
  await navigate('/wiki/simulators?lang=uk&a=wpn_ak_74m&b=wpn_pm');
  await send('browsingContext.setViewport', {context, viewport: {width:360,height:800}, devicePixelRatio:1});
  assert.equal(await evaluate('document.documentElement.lang'), 'uk');
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'the simulators fit 360 px');
  assert.ok((await evaluate("document.querySelector('.sim-setup .gmrow[data-key=\"stance\"] .glabel').textContent")).startsWith('Положення'));
  await evaluate("document.querySelector('.sim-setup').scrollIntoView()");
  await screenshot('simulators-mobile-uk');
  // At 360 px a tap opens a card inside the screen; a tap elsewhere closes it.
  const cards = await evaluate("document.querySelectorAll('.sim-setup[data-side=\"0\"] .glabel.fxhint').length");
  assert.ok(cards >= 5, `a card per skill and modification (${cards})`);
  for (let i = 0; i < cards; i++) {
    const label = `document.querySelectorAll('.sim-setup[data-side="0"] .glabel.fxhint')[${i}]`;
    await evaluate(`${label}.scrollIntoView({block:'center'})`);
    const at = JSON.parse(await evaluate(`(()=>{const r=${label}.getBoundingClientRect();return JSON.stringify([Math.round(r.x+8),Math.round(r.y+r.height/2)])})()`));
    await send('input.performActions', {context, actions: [{type: 'pointer', id: 'touch', parameters: {pointerType: 'touch'}, actions: [
      {type: 'pointerMove', x: at[0], y: at[1]}, {type: 'pointerDown', button: 0}, {type: 'pointerUp', button: 0}]}]});
    assert.equal(await evaluate(`getComputedStyle(${label}.querySelector('.gamemods')).display`), 'grid', `tap ${i} opens it`);
    assert.equal(await evaluate(`(()=>{const r=${label}.querySelector('.gamemods').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth})()`), true, `card ${i} fits 360 px`);
    assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'no sideways scroll');
    if (i === 0) await screenshot('simulators-mobile-uk-help');
    await evaluate("document.querySelector('#recoil-sim .sim-parts caption').scrollIntoView({block:'center'})");
    const away = JSON.parse(await evaluate("(()=>{const r=document.querySelector('#recoil-sim .sim-parts caption').getBoundingClientRect();return JSON.stringify([Math.round(r.x+r.width/2),Math.round(r.y+r.height/2)])})()"));
    await send('input.performActions', {context, actions: [{type: 'pointer', id: 'touch', parameters: {pointerType: 'touch'}, actions: [
      {type: 'pointerMove', x: away[0], y: away[1]}, {type: 'pointerDown', button: 0}, {type: 'pointerUp', button: 0}]}]});
    assert.equal(await evaluate(`getComputedStyle(${label}.querySelector('.gamemods')).display`), 'none', `a tap elsewhere closes ${i}`);
  }
  await evaluate("document.querySelector('#recoil-sim .sim-sight').scrollIntoView()");
  await screenshot('simulators-mobile-uk-sight');
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  // A rifle with no side kick: its crosshair stays in view, unmagnified.
  await navigate('/wiki/simulators?a=wpn_mosin_nagant');
  await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
  await evaluate("const t=document.querySelector('#recoil-sim .sim-time');t.value=t.max;t.dispatchEvent(new Event('input'))");
  assert.equal(await evaluate("!!document.querySelector('#recoil-sim .sim-note')"), false, 'no side magnification');
  const cx = Number(await evaluate("document.querySelector('#recoil-sim .sim-cross circle').getAttribute('cx')"));
  assert.ok(cx >= 0 && cx <= 300, `crosshair in view (${cx})`);
  assert.equal(await evaluate("JSON.stringify(window.pageErrors ?? [])"), '[]', 'no script errors');
  assert.ok((await fetch(base + '/wiki/simulators')).headers.get('content-security-policy').includes("script-src 'self'"));
  // Play from a fresh page, several times: its first frame may come before the press.
  for (let i = 0; i < 5; i++) {
    await navigate('/wiki/simulators?lang=en&a=wpn_pkp_pecheneg');
    await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message))");
    await evaluate("new Promise(r=>setTimeout(r,50)).then(()=>document.querySelector('#recoil-sim .sim-play').click())");
    await evaluate('new Promise(r=>setTimeout(r,300))');
    assert.ok(Number(await evaluate("document.querySelector('#recoil-sim .sim-time').value")) > 0, `play ${i}`);
    assert.equal(await evaluate("JSON.stringify(pageErrors)"), '[]');
  }
  // The equipment: a link straight to the helmets, one-click slots, search,
  // grouping by set, a card's protection, an icon on every row, EN/UK, mobile.
  await send('browsingContext.setViewport', {context, viewport: {width: 1440, height: 1000}, devicePixelRatio: 1});
  await navigate('/wiki/equipment?lang=en&slot=helmet');
  assert.equal(await evaluate("document.querySelector('.gear-tools').hidden"), false, 'equipment script works under the real CSP');
  assert.equal(await evaluate("document.querySelector('.slot-filter [aria-pressed=true]').dataset.slot"), 'helmet', 'the address picks the slot');
  const helmets = (await evaluate("[...document.querySelectorAll('.gear')].filter(e=>!e.hidden).map(e=>e.dataset.slot).join()")).split(',');
  assert.ok(helmets.length > 5 && helmets.every(slotName => slotName === 'helmet'), 'only helmets');
  assert.equal(await evaluate("[...document.querySelectorAll('.gear')].filter(e=>!e.querySelector('summary img.weapon-image')).map(e=>e.id).join()"), '', 'every piece has its icon');
  assert.equal(await evaluate("document.querySelectorAll('.flies-off').length"), 0, 'flying off is a hidden detail: not on the rows');
  assert.ok(await evaluate("[...document.querySelectorAll('.gear[data-slot=helmet] dt')].filter(dt => /Fatal headshot|Смертельне/.test(dt.textContent)).length") >= 8, 'the helmets that fly off say so on their cards');
  await screenshot('equipment-helmets');
  await evaluate("document.querySelector('.slot-filter [data-slot=mask]').click()");
  assert.ok((await evaluate("location.search")).includes('slot=mask'), 'the slot kept in the address');
  assert.ok(await evaluate("[...document.querySelectorAll('.gear')].filter(e=>!e.hidden).every(e=>e.dataset.slot==='mask')"));
  await evaluate("document.querySelector('.slot-filter [data-slot=\"\"]').click();{const g=document.getElementById('gear-group');g.value='set';g.dispatchEvent(new Event('change'))}");
  assert.ok(await evaluate("document.querySelectorAll('#gear-list h2.gear-group').length") > 10, 'grouped by set');
  await evaluate("{const q=document.getElementById('gear-search');q.value=document.querySelector('.gear .weapon-title strong').textContent.slice(0,6);q.dispatchEvent(new Event('input'))}");
  assert.ok(await evaluate("[...document.querySelectorAll('.gear')].filter(e=>!e.hidden).length") >= 1, 'search finds');
  await evaluate("{const q=document.getElementById('gear-search');q.value='';q.dispatchEvent(new Event('input'));const g=document.getElementById('gear-group');g.value='';g.dispatchEvent(new Event('change'))}");
  await evaluate("{const card=[...document.querySelectorAll('.gear')].find(e=>e.dataset.slot==='body');card.open=true;card.scrollIntoView()}");
  assert.equal(await evaluate("!![...document.querySelectorAll('.gear')].find(e=>e.open).querySelector('table.protection')"), true, 'a card shows its protection');
  await screenshot('equipment-card');
  await navigate('/wiki/equipment?lang=en&faction=1');
  assert.equal(await evaluate("document.getElementById('gear-faction').value"), '1', 'the address picks the faction');
  assert.equal(await evaluate("[...document.querySelectorAll('.gear')].filter(e=>!e.hidden).every(e=>e.dataset.faction==='1')"), true, 'only that faction');
  await navigate('/wiki/equipment?lang=uk&slot=boots');
  assert.equal(await evaluate("document.documentElement.lang"), 'uk');
  await send('browsingContext.setViewport', {context, viewport: {width:390,height:844}, devicePixelRatio:1});
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true, 'equipment fits mobile');
  await screenshot('equipment-mobile-uk');
  console.log(`PASS: ${total} weapons; image choosers/search/keyboard/mobile, catalogue, full stat comparison, equality/neutral colors, same weapon, simulators: recoil step/scrub/reset, spread by aim, stance, skills, gloves and arms, recoil modifications, help lines (hover, keyboard, tap), EN/UK/RU, the list's columns in RU/DE, 360 px and CSP.`);
  await send('session.end');
} finally {
  await send('session.end').catch(() => {});
  for (const request of pending.values()) clearTimeout(request.timer);
  socket.close();
}
