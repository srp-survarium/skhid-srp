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
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), false, 'chooser updates both stats and recoil');
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
  await navigate('/wiki/weapons?a=wpn_ak_74m&b=wpn_pm');
  assert.equal(await evaluate("document.querySelector('#compare-a-value').textContent"), 'AK-74M', 'URL paints chooser');
  assert.ok(await evaluate("document.querySelectorAll('.weapon-comparison tr[data-stat]').length") >= 30, 'expanded stat coverage');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=bullet_damage] td').className"), 'better');
  assert.notEqual(await evaluate("getComputedStyle(document.querySelector('.weapon-comparison [data-stat=bullet_damage] td')).color"),
    await evaluate("getComputedStyle(document.querySelector('.weapon-comparison [data-stat=min_damage_amount] td')).color"), 'better values visibly change color');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=min_damage_amount] td').className"), '', 'equal values neutral');
  assert.equal(await evaluate("document.querySelector('.weapon-comparison [data-stat=\"breath_vibration.base_period\"] td').className"), '', 'sway period neutral');
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), false);
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-vertical path').length"), 4, 'two complete and two elapsed curves');
  await screenshot('comparison-stats');
  await evaluate("document.querySelector('#compare-colors').click()");
  assert.equal(await evaluate("document.querySelectorAll('.weapon-comparison .better,.weapon-comparison .worse').length"), 0);
  await evaluate("document.querySelector('#compare-colors').click();document.querySelector('#only-differences').click()");
  assert.equal(await evaluate("!!document.querySelector('.weapon-comparison [data-stat=min_damage_amount]')"), false);
  await evaluate("document.querySelector('#recoil-sim .sim-step').click()");
  assert.equal(await evaluate("Number(document.querySelector('#recoil-sim .sim-time').value)"), .05);
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).match(/shots 1\/\d+/));
  assert.equal(await evaluate("document.querySelectorAll('#sim-single,.sim-single').length"), 0, 'no single-shot mode');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-sight .sim-cross').length"), 2, 'a crosshair per weapon');
  assert.equal(await evaluate("[...document.querySelectorAll('#recoil-sim .sim-view')].map(view => view.querySelectorAll('.sim-cross').length).join()"), '1,1', 'a view per weapon');
  await evaluate("document.querySelector('#recoil-sim .sim-step').click()");
  assert.ok(await evaluate("Number(document.querySelector('#recoil-sim .sim-time').value)") > .05, 'next shot advances timeline');
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
  await screenshot('comparison-crosshair');
  // Next shot and Play stop at the last shot: the settling is the plots' only.
  assert.equal(await evaluate("document.querySelectorAll('#sim-shots,.sim-shots').length"), 0, 'the burst is the smaller magazine, not a setting');
  await evaluate("for (let i = 0; i < 400 && !document.querySelector('#recoil-sim .sim-step').disabled; i++) document.querySelector('#recoil-sim .sim-step').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-step').disabled"), true, 'no step past the last shot');
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
  await screenshot('comparison-recoil');
  await evaluate("document.querySelector('#recoil-sim .sim-reset').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-time').value"), '0');
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
  await evaluate("document.querySelector('#recoil-sim').scrollIntoView()");
  await screenshot('comparison-recoil-mobile');
  await evaluate("document.querySelector('#clear-comparison').click()");
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), true);
  assert.equal(await evaluate("document.querySelector('#compare-table').childElementCount"), 0);
  assert.equal(await evaluate("document.querySelectorAll('.wpick-btn img').length"), 0, 'clear repaints both choosers');
  // One weapon is enough for the recoil simulation.
  await evaluate("document.querySelector('#compare-b').value='wpn_ak_74m';document.querySelector('#compare-b').dispatchEvent(new Event('change'))");
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), false, 'a single weapon gets its simulation');
  assert.equal(await evaluate("document.querySelectorAll('#recoil-sim .sim-sight .sim-cross').length"), 1);
  assert.equal(await evaluate("document.querySelector('#recoil-sim .sim-sight .sim-cross').classList.contains('series-1')"), true, 'B keeps its colour');
  assert.ok((await evaluate("document.querySelector('#recoil-sim .sim-legend').textContent")).includes('/30'), 'its own magazine');
  // A weapon's card has its own simulation, without A or B.
  await evaluate("document.querySelector('#clear-comparison').click();const card=document.querySelector('#wpn_ak_74m');card.open=true;card.scrollIntoView()");
  await evaluate('new Promise(r=>setTimeout(r,100))');
  assert.equal(await evaluate("document.querySelectorAll('#wpn_ak_74m .weapon-recoil .sim-sight .sim-cross').length"), 1, 'the card simulates its weapon');
  assert.ok((await evaluate("document.querySelector('#wpn_ak_74m .sim-legend').textContent")).includes('/30'));
  await evaluate("document.querySelector('#wpn_ak_74m .sim-play').click()");
  await evaluate('new Promise(r=>setTimeout(r,300))');
  assert.ok(Number(await evaluate("document.querySelector('#wpn_ak_74m .sim-time').value")) > 0, 'the card plays');
  assert.equal(await evaluate("document.querySelector('#recoil-sim').hidden"), true, 'the comparison is left alone');
  await screenshot('card-recoil');
  // Last shot shows the whole pattern; a target is drawn to scale, unmagnified.
  await evaluate("document.querySelector('#wpn_ak_74m .sim-last').click()");
  assert.ok((await evaluate("document.querySelector('#wpn_ak_74m .sim-legend').textContent")).includes('30/30'), 'last shot');
  assert.equal(await evaluate("document.querySelector('#wpn_ak_74m .sim-last').disabled"), true);
  const trail = await evaluate("document.querySelector('#wpn_ak_74m .sim-trail').getAttribute('d')");
  await evaluate("{const s=document.querySelector('#wpn_ak_74m .sim-target');s.value='10';s.dispatchEvent(new Event('change'))}");
  assert.equal(await evaluate("document.querySelectorAll('#wpn_ak_74m .sim-target-body').length"), 1, 'a target at 10 m');
  assert.equal(await evaluate("document.querySelector('#wpn_ak_74m .sim-trail').getAttribute('d')"), trail, 'a target leaves the pattern as it is');
  const near = await evaluate("document.querySelector('#wpn_ak_74m .sim-target-body').getAttribute('transform')");
  await evaluate("{const s=document.querySelector('#wpn_ak_74m .sim-target');s.value='25';s.dispatchEvent(new Event('change'))}");
  assert.notEqual(await evaluate("document.querySelector('#wpn_ak_74m .sim-target-body').getAttribute('transform')"), near, 'the distance sizes the target');
  assert.equal(await evaluate("document.querySelector('#wpn_ak_74m .sim-trail').getAttribute('d')"), trail);
  await evaluate("document.querySelector('#wpn_ak_74m .sim-sight').scrollIntoView()");
  await screenshot('card-target');
  // From the hip: the spread circle, a hit per shot fired, the shake.
  await evaluate("{const s=document.querySelector('#wpn_ak_74m .sim-stance');s.value='hip';s.dispatchEvent(new Event('change'))}");
  await evaluate("document.querySelector('#wpn_ak_74m .sim-last').click()");
  assert.equal(await evaluate("document.querySelectorAll('#wpn_ak_74m .sim-hit').length"), 30, 'a hit per shot');
  assert.equal(await evaluate("document.querySelectorAll('#wpn_ak_74m .sim-spread').length"), 1, 'the spread circle');
  assert.ok((await evaluate("document.querySelector('#wpn_ak_74m .sim-legend').textContent")).includes('°'), 'the spread in the legend');
  await screenshot('card-hip');
  // A rifle with no side kick: its crosshair stays in view, unmagnified.
  await evaluate("const m=document.querySelector('#wpn_mosin_nagant');m.open=true;m.scrollIntoView()");
  await evaluate('new Promise(r=>setTimeout(r,100))');
  await evaluate("const t=document.querySelector('#wpn_mosin_nagant .sim-time');t.value=t.max;t.dispatchEvent(new Event('input'))");
  assert.equal(await evaluate("!!document.querySelector('#wpn_mosin_nagant .sim-note')"), false, 'no side magnification');
  const cx = Number(await evaluate("document.querySelector('#wpn_mosin_nagant .sim-cross circle').getAttribute('cx')"));
  assert.ok(cx >= 0 && cx <= 300, `crosshair in view (${cx})`);
  assert.equal(await evaluate("JSON.stringify(window.pageErrors ?? [])"), '[]', 'no script errors (since the last weapons page load)');
  // Play from a fresh card, several times: its first frame may come before the press.
  for (let i = 0; i < 5; i++) {
    await navigate('/wiki/weapons?lang=en');
    await evaluate("window.pageErrors=[];addEventListener('error',event=>pageErrors.push(event.message));const c=document.querySelector('#wpn_pkp_pecheneg');c.open=true");
    await evaluate("new Promise(r=>setTimeout(r,50)).then(()=>document.querySelector('#wpn_pkp_pecheneg .sim-play').click())");
    await evaluate('new Promise(r=>setTimeout(r,300))');
    assert.ok(Number(await evaluate("document.querySelector('#wpn_pkp_pecheneg .sim-time').value")) > 0, `card play ${i}`);
    assert.equal(await evaluate("JSON.stringify(pageErrors)"), '[]');
  }
  console.log(`PASS: ${total} weapons; image choosers/search/keyboard/mobile, catalogue, full stat comparison, equality/neutral colors, same weapon, recoil step/scrub/reset, EN/UK and CSP.`);
  await send('session.end');
} finally {
  await send('session.end').catch(() => {});
  for (const request of pending.values()) clearTimeout(request.timer);
  socket.close();
}
