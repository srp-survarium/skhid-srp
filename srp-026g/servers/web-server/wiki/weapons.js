globalThis.CAMERA_SHAKE = {"period_frames":225,"pitch":[-0.252,-0.238,-0.189,-0.107,-0.023,0.043,0.09,0.122,0.143,0.151,0.158,0.179,0.209,0.241,0.267,0.287,0.301,0.312,0.319,0.323,0.325,0.286,0.219,0.158,0.108,0.068,0.039,0.018,0.004,-0.004,-0.007,0.018,0.052,0.088,0.118,0.141,0.158,0.17,0.178,0.182,0.185,0.142,0.064,-0.005,-0.063,-0.109,-0.143,-0.167,-0.182,-0.191,-0.195,-0.175,-0.145,-0.117,-0.094,-0.076,-0.063,-0.053,-0.047,-0.044,-0.043,-0.043,-0.044,-0.045,-0.046,-0.048,-0.049,-0.049,-0.049,-0.049,-0.05,-0.056,-0.065,-0.074,-0.081,-0.087,-0.091,-0.093,-0.095,-0.096,-0.098,-0.099,-0.1,-0.101,-0.102,-0.103,-0.104,-0.105,-0.106,-0.106,-0.106,-0.083,-0.046,-0.012,0.016,0.038,0.054,0.066,0.074,0.078,0.08,0.028,-0.063,-0.145,-0.214,-0.267,-0.307,-0.336,-0.355,-0.366,-0.37,-0.28,-0.125,0.019,0.136,0.227,0.297,0.348,0.38,0.397,0.403,0.402,0.398,0.393,0.389,0.385,0.382,0.379,0.378,0.378,0.38,0.382,0.387,0.392,0.396,0.4,0.402,0.404,0.405,0.406,0.406,0.306,0.144,-0.007,-0.127,-0.221,-0.292,-0.344,-0.378,-0.397,-0.403,-0.372,-0.319,-0.271,-0.231,-0.2,-0.177,-0.16,-0.149,-0.143,-0.14,-0.173,-0.228,-0.278,-0.32,-0.352,-0.377,-0.395,-0.406,-0.413,-0.415,-0.307,-0.124,0.047,0.185,0.29,0.371,0.43,0.468,0.489,0.497,0.408,0.253,0.109,-0.007,-0.097,-0.165,-0.215,-0.248,-0.266,-0.273,-0.206,-0.088,0.019,0.107,0.176,0.228,0.265,0.289,0.302,0.308,0.244,0.135,0.034,-0.049,-0.114,-0.163,-0.198,-0.221,-0.234,-0.24,-0.243,-0.244,-0.246,-0.247,-0.249,-0.25,-0.251,-0.251,-0.252,-0.252,-0.253,-0.253,-0.253,-0.252],"yaw":[0.081,0.069,0.027,-0.043,-0.115,-0.174,-0.216,-0.245,-0.262,-0.271,-0.275,-0.251,-0.215,-0.181,-0.155,-0.136,-0.121,-0.111,-0.105,-0.101,-0.1,-0.069,-0.014,0.035,0.075,0.107,0.13,0.147,0.158,0.164,0.167,0.187,0.22,0.251,0.277,0.297,0.313,0.323,0.33,0.334,0.338,0.343,0.349,0.357,0.364,0.368,0.37,0.372,0.373,0.374,0.375,0.377,0.379,0.38,0.379,0.378,0.375,0.373,0.372,0.372,0.373,0.374,0.376,0.377,0.376,0.375,0.373,0.37,0.369,0.369,0.368,0.355,0.335,0.317,0.301,0.289,0.279,0.273,0.269,0.267,0.263,0.251,0.224,0.197,0.173,0.153,0.137,0.125,0.117,0.113,0.11,0.108,0.104,0.097,0.088,0.08,0.074,0.07,0.066,0.064,0.062,0.061,0.058,0.054,0.05,0.046,0.043,0.041,0.041,0.041,0.037,0.022,-0.005,-0.033,-0.056,-0.073,-0.087,-0.097,-0.104,-0.106,-0.11,-0.127,-0.157,-0.188,-0.215,-0.237,-0.254,-0.267,-0.275,-0.28,-0.28,-0.274,-0.262,-0.25,-0.241,-0.235,-0.23,-0.227,-0.224,-0.223,-0.224,-0.226,-0.229,-0.232,-0.235,-0.237,-0.239,-0.24,-0.241,-0.241,-0.243,-0.258,-0.282,-0.305,-0.326,-0.342,-0.353,-0.361,-0.369,-0.372,-0.37,-0.364,-0.355,-0.346,-0.339,-0.336,-0.334,-0.332,-0.331,-0.331,-0.33,-0.316,-0.298,-0.28,-0.266,-0.258,-0.251,-0.246,-0.243,-0.243,-0.239,-0.223,-0.197,-0.172,-0.15,-0.133,-0.121,-0.112,-0.107,-0.103,-0.1,-0.096,-0.09,-0.085,-0.084,-0.085,-0.087,-0.088,-0.089,-0.089,-0.09,-0.064,-0.027,0.007,0.032,0.051,0.063,0.072,0.077,0.08,0.082,0.083,0.084,0.084,0.084,0.083,0.082,0.082,0.081,0.081,0.081,0.081,0.081,0.081,0.081]};
// Model of 0.26g recoil: shot 0x331C50, ramp 0x331F82, tick 0x331A50,
// recovery 0x3321E0. Matches bots/recoil.rs at 50 ms ticks. With it, from
// the exe (re-kit rows of those addresses):
// - the camera shake, a third recoil axis `z` run like the climb (a draw in
//   [min_shake_recoil, max_shake_recoil] per shot, the same PID back to 0)
//   weighting the stock clip camerashake_v2 (CAMERA_SHAKE, degrees), which
//   only plays while firing from the hip, at shake_time_scale x 30 frames/s;
// - the spread (get_dispersed_bullet_direction 0x2E1D50): D = base_dispersion
//   aimed, and from the hip + the stance's (idle_dispersion x
//   from_the_hip_multiplier) + the shots' part (one_shoot_dispersion_amount a
//   shot, at most max_dispersion, falling at speed_of_aiming a second); a
//   bullet tilts 0.5 x D x frac(N(0, 0.5)) degrees at a uniform angle.
// Random draws are this file's own seeded ones, not the game's generator;
// the kick's draws are the same with or without spread and shake.
(() => {
  const TOP = 63.735, SIDE = 21.48, TICK = 50;
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const finite = (object, keys) => keys.every(key => typeof object[key] === 'number' && Number.isFinite(object[key]));
  function parseSpread(raw) {
    const d = raw.dispersion || {}, stance = raw.stance || {};
    if (!finite(d, ['base_dispersion', 'one_shoot_dispersion_amount', 'max_dispersion', 'speed_of_aiming']) ||
        !finite(stance, ['idle_dispersion'])) return null;
    return Object.freeze({base: d.base_dispersion, perShot: d.one_shoot_dispersion_amount, max: d.max_dispersion,
      recovery: d.speed_of_aiming, hip: (d.from_the_hip_multiplier ?? 1) * stance.idle_dispersion});
  }
  function parseShake(raw) {
    if (!finite(raw, ['min_shake_recoil', 'max_shake_recoil', 'shake_time_scale'])) return null;
    return Object.freeze({range: [raw.min_shake_recoil, raw.max_shake_recoil], timeScale: raw.shake_time_scale});
  }
  function parse(raw) {
    const required = ['rpm', 'magazine', 'recoil_power', 'min_top_recoil', 'max_top_recoil',
      'min_left_recoil', 'max_left_recoil', 'min_right_recoil', 'max_right_recoil',
      'compensation_proportional_gain', 'compensation_integral_gain', 'compensation_derivational_gain'];
    if (!raw || required.some(k => typeof raw[k] !== 'number' || !Number.isFinite(raw[k])) ||
        raw.rpm <= 0 || raw.magazine < 1 || raw.recoil_power < 0) return null;
    for (const axis of ['top', 'left', 'right']) if (raw[`min_${axis}_recoil`] > raw[`max_${axis}_recoil`]) return null;
    return Object.freeze({interval: 60000 / raw.rpm, magazine: Math.floor(raw.magazine), power: raw.recoil_power,
      top: [raw.min_top_recoil, raw.max_top_recoil], left: [raw.min_left_recoil, raw.max_left_recoil],
      right: [raw.min_right_recoil, raw.max_right_recoil],
      gains: [raw.compensation_proportional_gain, raw.compensation_integral_gain, raw.compensation_derivational_gain],
      spread: parseSpread(raw), shake: parseShake(raw)});
  }
  function generator(seed) {
    let state = seed >>> 0 || 1;
    return () => {
      state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
      return (state >>> 0) / 4294967296;
    };
  }
  // The shake clip's view turn at `frame` (fractional, looping): [side, up] degrees.
  function shakeAt(frame) {
    const clip = globalThis.CAMERA_SHAKE;
    if (!clip) return [0, 0];
    const n = clip.period_frames, at = ((frame % n) + n) % n, i = Math.floor(at), f = at - i, j = (i + 1) % n;
    return [clip.yaw[i] + (clip.yaw[j] - clip.yaw[i]) * f, clip.pitch[i] + (clip.pitch[j] - clip.pitch[i]) * f];
  }
  // `hip`: firing from the hip (the spread grows, the shake plays), else aimed.
  function simulate(model, requestedShots, seed = 1, hip = false) {
    const shots = Math.min(model.magazine, Math.max(1, Math.floor(requestedShots)));
    const random = generator(seed), extra = generator((seed ^ 0x9e3779b9) >>> 0);
    const draw = range => range[0] + (range[1] - range[0]) * random();
    // N(0, sigma) by Box-Muller, from the second stream.
    const normal = sigma => Math.sqrt(-2 * Math.log(1 - extra())) * Math.cos(2 * Math.PI * extra()) * sigma;
    let state = {kind: 'rest'};
    // Axes: side, top, shake.
    const at = [0, 0, 0], speed = [0, 0, 0], error = [0, 0, 0], integral = [0, 0, 0];
    const frames = [], events = [];
    let fired = 0, shotSpread = 0, shakeFrame = 0;
    const spreadNow = () => model.spread ? model.spread.base + (hip ? model.spread.hip + shotSpread : 0) : 0;
    const lastShot = (shots - 1) * model.interval;
    const end = Math.ceil((shots * model.interval + 3000) / TICK) * TICK;
    for (let now = 0; now <= end; now += TICK) {
      if (now > 0) {
        const dt = TICK / 1000;
        const ramp = state.kind === 'shot' ? clamp((state.time + model.interval - (now - TICK)) / 1000, 0, dt) : 0;
        const rest = dt - ramp;
        for (let axis = 0; axis < 3; axis++) {
          at[axis] += speed[axis] * ramp;
          if (rest > 0) {
            const e = at[axis]; integral[axis] += e * rest;
            const derivative = (e - error[axis]) / rest; error[axis] = e;
            speed[axis] = -(model.gains[0] * e + model.gains[1] * integral[axis] + model.gains[2] * derivative);
            at[axis] += speed[axis] * rest;
          }
          at[axis] = axis === 2 ? clamp(at[axis], 0, 1) : clamp(at[axis], -1, 1);
        }
        if (model.spread) shotSpread = Math.max(0, shotSpread - model.spread.recovery * dt);
        // The clip runs while the weapon fires from the hip, and stops after.
        if (hip && model.shake && fired > 0 && now - TICK < lastShot + model.interval) shakeFrame += model.shake.timeScale * 30 * dt;
      }
      while (fired < shots && fired * model.interval <= now + .001) {
        const time = fired * model.interval;
        const top = draw(model.top) / TOP, right = draw(model.right), left = draw(model.left);
        const side = (right - left) / SIDE;
        const shake = model.shake ? model.shake.range[0] + (model.shake.range[1] - model.shake.range[0]) * extra() : 0;
        const target = [clamp(at[0] + side * (1 - Math.abs(at[0]) ** model.power), -1, 1),
          clamp(at[1] + top * (1 - Math.max(at[1], 0) ** model.power), -1, 1),
          clamp(at[2] + shake * (1 - at[2] ** model.power), 0, 1)];
        for (let axis = 0; axis < 3; axis++) {
          speed[axis] = (target[axis] - at[axis]) / (model.interval / 1000);
          error[axis] = 0; integral[axis] = 0;
        }
        // The bullet leaves inside the spread of the moment, then the shot adds to it.
        const spread = spreadNow(), n = normal(.5), tilt = .5 * spread * (n - Math.trunc(n)), angle = 2 * Math.PI * extra();
        if (model.spread) shotSpread = Math.min(model.spread.max, shotSpread + model.spread.perShot);
        state = {kind: 'shot', time}; fired++;
        events.push({time: now / 1000, shot: fired, spread, bullet: {side: tilt * Math.cos(angle), up: tilt * Math.sin(angle)}});
      }
      const [shakeSide, shakeUp] = hip ? shakeAt(shakeFrame).map(value => value * at[2]) : [0, 0];
      frames.push({time: now / 1000, vertical: at[1] * TOP, horizontal: at[0] * SIDE, shots: fired,
        spread: spreadNow(), shake: {side: shakeSide, up: shakeUp}});
    }
    return {frames, events, shots, duration: end / 1000};
  }
  globalThis.WeaponRecoil = Object.freeze({parse, simulate});
})();

// Calculator-style chooser: lobby icons, class groups, level ordering,
// punctuation-insensitive search and arrow/Enter/Escape keyboard controls.
// The hidden select owns the selection, including URL and catalogue updates.
// `weapons` are the elements offered (the weapons page's rows, or the
// modifiers page's item entries, which carry their name in data-title).
function WikiWeaponPicker(select, weapons, tr, collator, words = {
  search: tr('Search weapons', 'Пошук зброї'), none: tr('No weapons match.', 'Зброю не знайдено.')}) {
  const host = document.getElementById(`${select.id}-picker`);
  const make = (tag, attrs = {}, ...children) => {
    const element = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    element.append(...children);
    return element;
  };
  const bare = text => text.normalize('NFKC').toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');
  const title = weapon => weapon.dataset.title || weapon.querySelector('.weapon-title strong').textContent;
  const icon = weapon => {
    const source = weapon?.querySelector('img');
    return source ? make('img', {src: source.src, alt: '', width: 54, height: 26, loading: 'lazy'}) : make('span', {class: 'noimg'});
  };
  const button = make('button', {type: 'button', class: 'wpick-btn', id: `${select.id}-btn`,
    'aria-haspopup': 'listbox', 'aria-expanded': 'false', 'aria-controls': `${select.id}-list`,
    'aria-labelledby': `${select.id}-label ${select.id}-value`});
  const list = make('div', {class: 'wpick-list', role: 'listbox', id: `${select.id}-list`, 'aria-labelledby': `${select.id}-label`});
  const search = make('input', {type: 'search', class: 'wpick-search', role: 'combobox', autocomplete: 'off',
    'aria-controls': list.id, 'aria-expanded': 'false', 'aria-autocomplete': 'list',
    'aria-label': words.search, placeholder: tr('Search…', 'Пошук…')});
  const empty = make('p', {class: 'wpick-empty', role: 'status'}, words.none);
  const popup = make('div', {class: 'wpick-pop', hidden: ''}, search, list, empty);
  host.append(button, popup);
  const options = [], groups = new Map();
  let active = null;
  function activate(option, scroll = true) {
    active?.element.classList.remove('active');
    active = option || null;
    if (!active) { search.removeAttribute('aria-activedescendant'); return; }
    active.element.classList.add('active');
    search.setAttribute('aria-activedescendant', active.element.id);
    if (scroll) active.element.scrollIntoView({block: 'nearest'});
  }
  for (const weapon of weapons) {
    const label = weapon.dataset.class;
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label).push(weapon);
  }
  for (const [label, items] of groups) {
    const heading = make('div', {class: 'wpick-group', id: `${select.id}-group-${groups.size}-${options.length}`}, label);
    const group = make('div', {role: 'group', 'aria-labelledby': heading.id}, heading);
    list.append(group);
    for (const weapon of items.sort((a, b) => Number(a.dataset.level) - Number(b.dataset.level) || collator.compare(title(a), title(b)))) {
      const element = make('div', {class: 'wpick-opt', role: 'option', id: `${select.id}-${weapon.id}`, 'aria-selected': 'false'},
        icon(weapon), make('span', {class: 'n'}, title(weapon)),
        make('small', {}, tr(`lvl ${weapon.dataset.level}`, `рів. ${weapon.dataset.level}`)));
      const option = {element, group, weapon, text: [title(weapon), weapon.dataset.searchName].map(bare).join(' ')};
      element.addEventListener('mousemove', () => { if (active !== option) activate(option, false); });
      element.addEventListener('mousedown', event => event.preventDefault());
      element.addEventListener('click', () => choose(weapon.id));
      group.append(element); options.push(option);
    }
  }
  const visible = () => options.filter(option => !option.element.hidden);
  function filter() {
    const query = bare(search.value);
    for (const option of options) option.element.hidden = !option.text.includes(query);
    for (const group of list.children) group.hidden = !options.some(option => option.group === group && !option.element.hidden);
    const matches = visible();
    empty.hidden = matches.length !== 0;
    activate(matches.find(option => option.weapon.id === select.value) || matches[0]);
  }
  function close(refocus = false) {
    popup.hidden = true;
    button.setAttribute('aria-expanded', 'false'); search.setAttribute('aria-expanded', 'false');
    if (refocus) button.focus();
  }
  function open() {
    // Opening the other chooser closes this one through focusout.
    popup.hidden = false;
    button.setAttribute('aria-expanded', 'true'); search.setAttribute('aria-expanded', 'true');
    popup.style.left = '';
    const overflow = popup.getBoundingClientRect().right - (document.documentElement.clientWidth - 16);
    if (overflow > 0) popup.style.left = `${-overflow}px`;
    search.value = ''; filter(); search.focus({preventScroll: true});
  }
  function choose(id) {
    close(true);
    if (id === select.value) return;
    select.value = id; select.dispatchEvent(new Event('change'));
  }
  button.addEventListener('click', () => popup.hidden ? open() : close(true));
  button.addEventListener('keydown', event => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); open(); }
  });
  search.addEventListener('input', filter);
  search.addEventListener('keydown', event => {
    const matches = visible(), index = matches.indexOf(active);
    if (event.key === 'ArrowDown') { event.preventDefault(); activate(matches[Math.min(matches.length - 1, index + 1)]); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); activate(matches[Math.max(0, index - 1)]); }
    else if (event.key === 'Enter') { event.preventDefault(); if (active) choose(active.weapon.id); }
    else if (event.key === 'Escape') { event.preventDefault(); close(true); }
    else if (event.key === 'Tab') close();
  });
  document.addEventListener('mousedown', event => { if (!host.contains(event.target)) close(); });
  host.addEventListener('focusout', event => { if (!host.contains(event.relatedTarget)) close(); });
  return {
    paint() {
      const selected = weapons.find(weapon => weapon.id === select.value);
      button.replaceChildren(...(selected ? [icon(selected)] : []),
        make('span', {class: 'n', id: `${select.id}-value`}, selected ? title(selected) : select.options[0].textContent));
      for (const option of options) option.element.setAttribute('aria-selected', String(option.weapon.id === select.value));
    }
  };
}

(() => {
  const list = document.querySelector('#weapon-list');
  if (!list) return;
  const weapons = [...list.querySelectorAll('.weapon')];
  const search = document.querySelector('#weapon-search');
  const category = document.querySelector('#weapon-class');
  const order = document.querySelector('#weapon-sort');
  const count = document.querySelector('[data-count]');
  const empty = document.querySelector('#weapons-empty');
  const collator = new Intl.Collator(document.documentElement.lang, {numeric: true, sensitivity: 'base'});
  const normalize = text => text.normalize('NFKC').toLocaleLowerCase().replace(/[\s\-–—]+/g, '');
  const tr = (en, uk) => document.documentElement.lang === 'uk' ? uk : en;
  const byId = new Map(weapons.map(element => [element.id, element]));
  const pickerA = document.querySelector('#compare-a'), pickerB = document.querySelector('#compare-b');
  const choosers = [pickerA, pickerB].map(select => WikiWeaponPicker(select, weapons, tr, collator));
  const differenceOnly = document.querySelector('#only-differences'), colors = document.querySelector('#compare-colors');
  const tableHost = document.querySelector('#compare-table');
  const comparison = document.querySelector('#comparison');
  const models = new Map(weapons.map(element => [element.id, WeaponRecoil.parse(JSON.parse(element.dataset.recoil))]));
  const stats = new Map(weapons.map(element => [element.id,
    [...element.querySelectorAll('[data-section]')].map(section => ({
      title: section.querySelector('h2').textContent, note: section.querySelector('.stat-note')?.textContent,
      rows: [...section.querySelectorAll('[data-stat]')].map(row => ({
        id: row.dataset.stat, label: row.querySelector('dt').textContent, text: row.querySelector('dd').textContent,
        direction: row.dataset.direction, values: JSON.parse(row.dataset.values)
      }))
    }))
  ]));
  function node(tag, text, cls) {
    const element = document.createElement(tag);
    if (text !== undefined) element.textContent = text;
    if (cls) element.className = cls;
    return element;
  }
  function name(id) { return byId.get(id).querySelector('.weapon-title strong').textContent; }
  function compareValues(a, b) {
    if (!a.values.length || a.values.length !== b.values.length || a.direction === 'neutral') return 0;
    // Ranges are better only when both endpoints are no worse. Overlapping
    // trade-offs do not receive a misleading single green/red verdict.
    const sign = a.direction === 'higher' ? 1 : -1;
    const differences = a.values.map((value, index) => sign * (value - b.values[index]));
    if (differences.every(value => value >= 0) && differences.some(value => value > 0)) return 1;
    if (differences.every(value => value <= 0) && differences.some(value => value < 0)) return -1;
    return 0;
  }
  function renderComparison() {
    tableHost.replaceChildren();
    const a = pickerA.value, b = pickerB.value;
    document.querySelector('#compare-empty').hidden = !!(a && b);
    document.querySelector('#compare-same').hidden = true;
    if (!a || !b) return;
    const table = node('table', undefined, 'weapon-comparison');
    const head = node('thead'), header = node('tr');
    const corner = node('th', tr('Stat', 'Характеристика')); corner.scope = 'col'; header.append(corner);
    for (const [id, side] of [[a, 'A'], [b, 'B']]) {
      const cell = node('th'); cell.scope = 'col';
      const image = byId.get(id).querySelector('img');
      if (image) { const copy = image.cloneNode(); copy.loading = 'eager'; cell.append(copy); }
      cell.append(node('span', `${side} · ${name(id)}`)); header.append(cell);
    }
    head.append(header); table.append(head);
    const body = node('tbody');
    const peers = new Map(stats.get(b).flatMap(section => section.rows).map(row => [row.id, row]));
    let rowCount = 0;
    for (const section of stats.get(a)) {
      const rows = section.rows.filter(row => !differenceOnly.checked || row.text !== peers.get(row.id).text);
      if (!rows.length) continue;
      const heading = node('tr', undefined, 'compare-section'), cell = node('th'); cell.colSpan = 3;
      cell.append(node('span', section.title));
      if (section.note) cell.append(node('small', section.note));
      heading.append(cell); body.append(heading);
      for (const row of rows) {
        const peer = peers.get(row.id), direction = colors.checked ? compareValues(row, peer) : 0;
        const line = node('tr'); line.dataset.stat = row.id;
        const label = node('th', row.label); label.scope = 'row'; line.append(label);
        for (const [entry, verdict] of [[row, direction], [peer, -direction]]) {
          const value = node('td', entry.text, verdict > 0 ? 'better' : verdict < 0 ? 'worse' : '');
          if (verdict) {
            const marker = node('span', verdict > 0 ? ' ↑' : ' ↓', 'comparison-marker');
            marker.setAttribute('aria-label', verdict > 0 ? tr('Better', 'Краще') : tr('Worse', 'Гірше'));
            value.append(marker);
          }
          line.append(value);
        }
        body.append(line); rowCount++;
      }
    }
    table.append(body); tableHost.append(table);
    document.querySelector('#compare-same').hidden = rowCount !== 0;
  }

  const svgNS = 'http://www.w3.org/2000/svg';
  function svgNode(tag, attrs, text) {
    const element = document.createElementNS(svgNS, tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    if (text !== undefined) element.textContent = text;
    return element;
  }
  // Degrees of view in one plotted unit of kick, from the exe
  // (servers/match-server/src/v026g/bots/recoil.rs, "the pitch's unit"): a
  // pitch unit turns the view 60° up and 67.5° down, and the configs' kick is
  // 63.735 to a pitch unit (PROVEN); the side about 2.33° of yaw per pitch
  // unit at 21.48 to one (LIKELY).
  const UP_DEG = 60 / 63.735, DOWN_DEG = 67.5 / 63.735, SIDE_DEG = 2.33 / 21.48;
  const viewUp = v => v * (v >= 0 ? UP_DEG : DOWN_DEG), viewSide = h => h * SIDE_DEG;
  // A target's figure: the game's HUD doll, 1.8 m tall, aimed at the chest
  // (a quarter of its height below the top of the head).
  const BODY_HEIGHT = 1.8, CHEST = .25;
  let dollBox = null;
  function bodyDegrees(distance) {
    return 2 * Math.atan(BODY_HEIGHT / 2 / distance) * 180 / Math.PI;
  }
  function sample(series, time) {
    return series.frames[Math.max(0, Math.min(series.frames.length - 1, Math.floor(time / .05 + 1e-6)))];
  }
  // One recoil simulation in `root` (the comparison's, or a weapon card's):
  // its own controls, plots and crosshair view, every lookup inside `root`.
  // `show(sides)` runs it for [{id, side}] (side 0 is A's colour, 1 B's).
  function RecoilSim(root) {
    const find = selector => root.querySelector(selector);
    const timeInput = find('.sim-time'), seedInput = find('.sim-seed'), playButton = find('.sim-play');
    let simulation = {kind: 'empty'}, sides = [], playing = null;
    function nextShot() {
      if (simulation.kind !== 'ready') return undefined;
      return simulation.series.flatMap(series => series.events).map(event => event.time)
        .sort((a, b) => a - b).find(time => time > simulation.time + .001);
    }
    function draw() {
      if (simulation.kind !== 'ready') return;
      const time = simulation.time;
      find('.sim-step').disabled = find('.sim-last').disabled = nextShot() === undefined;
      timeInput.value = time;
      find('.sim-time-label').textContent = `${time.toFixed(2)} ${tr('s', 'с')}`;
      const legend = find('.sim-legend'); legend.replaceChildren();
      for (const series of simulation.series) {
        const point = sample(series, time), label = sides.length > 1 ? `${series.side ? 'B' : 'A'} · ` : '';
        legend.append(node('p', `${label}${name(series.id)} · ${tr('shots', 'постріли')} ${point.shots}/${series.shots} · ${tr('up', 'вгору')} ${point.vertical.toFixed(3)} · ${tr('side', 'убік')} ${point.horizontal.toFixed(3)} · ${tr('spread', 'розкид')} ${point.spread.toFixed(2)}°`, `series-${series.side}`));
      }
      for (const axis of ['vertical', 'horizontal']) {
        const svg = find(`.sim-${axis}`); svg.replaceChildren();
        const values = simulation.series.flatMap(series => series.frames.map(frame => frame[axis]));
        const lo = Math.min(0, ...values), hi = Math.max(.1, ...values), margin = (hi - lo) * .1;
        const min = lo - margin, max = hi + margin;
        const x = t => 54 + t / simulation.duration * 530, y = v => 218 - (v - min) / (max - min) * 194;
        for (let i = 0; i <= 4; i++) {
          const value = min + (max - min) * i / 4;
          svg.append(svgNode('line', {x1:54, x2:584, y1:y(value), y2:y(value), class:'sim-grid'}));
          svg.append(svgNode('text', {x:47, y:y(value)+4, 'text-anchor':'end'}, value.toFixed(2)));
          const t = simulation.duration * i / 4;
          svg.append(svgNode('text', {x:x(t), y:241, 'text-anchor':'middle'}, `${t.toFixed(1)}s`));
        }
        svg.append(svgNode('line', {x1:54, x2:584, y1:y(0), y2:y(0), class:'sim-zero'}));
        for (const series of simulation.series) {
          const path = frames => frames.map((frame, i) => `${i ? 'L' : 'M'}${x(frame.time).toFixed(2)},${y(frame[axis]).toFixed(2)}`).join(' ');
          svg.append(svgNode('path', {d:path(series.frames), class:`sim-curve series-${series.side} future`}));
          svg.append(svgNode('path', {d:path(series.frames.filter(frame => frame.time <= time)), class:`sim-curve series-${series.side}`}));
          for (const event of series.events.filter(event => event.time <= time)) {
            const point = sample(series, event.time);
            svg.append(svgNode('circle', {cx:x(event.time), cy:y(point[axis]), r:2.8, class:`sim-shot series-${series.side}`}));
          }
        }
        svg.append(svgNode('line', {x1:x(time), x2:x(time), y1:24, y2:218, class:'sim-cursor'}));
      }
      drawSight(time);
    }
    // The crosshair as the kick moves it: the kick climbing from the origin,
    // the path so far and each shot's mark. The side kick is far smaller than
    // the climb: it fills the width, and the corner says how much the climb
    // is squeezed against it (at most ÷100). Each weapon has
    // its own view, all on one scale so they compare.
    function drawSight(time) {
      const frames = simulation.series.flatMap(series => series.frames);
      // In degrees of view: the kick as the camera turns. The pattern is the
      // weapon's: a target changes nothing of it, only its own size shows the
      // distance (cut by the view's edge when it is the larger).
      const distance = Number(find('.sim-target').value) || 0, body = distance ? bodyDegrees(distance) : 0;
      // The aim (kick and shake), each bullet's hit and the spread around the
      // aim, in degrees of view: the view holds them all.
      const aim = frame => ({side: viewSide(frame.horizontal) + frame.shake.side, up: viewUp(frame.vertical) + frame.shake.up});
      const hit = (series, event) => {
        const point = aim(sample(series, event.time));
        return {side: point.side + event.bullet.side, up: point.up + event.bullet.up};
      };
      const points = [...frames.map(frame => ({...aim(frame), r: frame.spread / 2})),
        ...simulation.series.flatMap(series => series.events.map(event => ({...hit(series, event), r: 0})))];
      const up = Math.max(0, ...points.map(point => point.up + point.r));
      const down = Math.max(0, ...points.map(point => point.r - point.up));
      // The aim's path may be stretched sideways (its side drift is small);
      // the spread and the hits are drawn at true size around the aim.
      const side = Math.max(0, ...frames.map(frame => Math.abs(aim(frame).side)));
      const scale = 250 / Math.max(.1, up + down);
      // A weapon with no side kick (bolt-action rifles) keeps one scale: its
      // side values are rounding noise, not drift to magnify.
      const room = 140 - Math.max(0, ...points.map(point => point.r)) * scale;
      const sideScale = side > 1e-4 ? Math.min(scale * 100, Math.max(scale, Math.max(20, room) / side)) : scale;
      const oy = 25 + (250 - (up + down) * scale) / 2 + up * scale;
      const x = degrees => 150 + degrees * sideScale, y = degrees => oy - degrees * scale;
      // A hit: its aim point on the stretched path, its bullet's turn at true size.
      const hitAt = (series, event) => {
        const point = aim(sample(series, event.time));
        return [x(point.side) + event.bullet.side * scale, y(point.up + event.bullet.up)];
      };
      const magnified = Math.round(sideScale / scale);
      const views = simulation.series.map(series => {
        const svg = svgNode('svg', {viewBox:'0 0 300 300', role:'img', 'aria-label':`${name(series.id)}: ${tr('where it aims as the recoil moves it', 'куди цілиться під дією відбою')}`});
        const view = document.createElement('div'); view.className = 'sim-view';
        if (simulation.series.length > 1) view.append(node('p', `${series.side ? 'B' : 'A'} · ${name(series.id)}`, `sim-view-name series-${series.side}`));
        view.append(svg);
        return {series, svg, view};
      });
      find('.sim-sight-views').replaceChildren(...views.map(({view}) => view));
      for (const {series, svg} of views) {
        if (magnified > 1) svg.append(svgNode('text', {x:292, y:292, 'text-anchor':'end', class:'sim-note'}, tr(`vertical ÷${magnified}`, `вертикаль ÷${magnified}`)));
        svg.append(svgNode('line', {x1:0, x2:300, y1:oy, y2:oy, class:'sim-grid'}));
        svg.append(svgNode('line', {x1:150, x2:150, y1:0, y2:300, class:'sim-grid'}));
        if (distance) {
          dollBox ??= document.querySelector('#doll-shape').getBBox();
          const size = body * scale / dollBox.height;
          svg.append(svgNode('use', {href:'#doll-shape', class:'sim-target-body',
            transform:`translate(${(150 - (dollBox.x + dollBox.width / 2) * size).toFixed(2)} ${(oy - body * CHEST * scale - dollBox.y * size).toFixed(2)}) scale(${size.toFixed(5)})`}));
        }
        svg.append(svgNode('circle', {cx:150, cy:oy, r:3, class:'sim-origin'}));
        const past = series.frames.filter(frame => frame.time <= time);
        svg.append(svgNode('path', {d:past.map((frame, i) => `${i ? 'L' : 'M'}${x(aim(frame).side).toFixed(2)},${y(aim(frame).up).toFixed(2)}`).join(' '),
          class:`sim-trail series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= time)) {
          const point = aim(sample(series, event.time)), [hx, hy] = hitAt(series, event);
          svg.append(svgNode('circle', {cx:x(point.side), cy:y(point.up), r:2, class:`sim-shot series-${series.side}`}));
          svg.append(svgNode('circle', {cx:hx, cy:hy, r:2.6, class:`sim-hit series-${series.side}`}));
        }
        const frame = sample(series, time), point = aim(frame), cx = x(point.side), cy = y(point.up);
        // The spread: the cone's half angle around the aim.
        const radius = frame.spread / 2;
        if (radius > 0) svg.append(svgNode('circle', {cx, cy, r:radius * scale, class:`sim-spread series-${series.side}`}));
        const cross = svgNode('g', {class:`sim-cross series-${series.side}`});
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          cross.append(svgNode('line', {x1:cx + dx * 5, y1:cy + dy * 5, x2:cx + dx * 15, y2:cy + dy * 15}));
        }
        cross.append(svgNode('circle', {cx, cy, r:1.6}));
        svg.append(cross);
      }
    }
    // Play: the burst in real time, from where the timeline is (or the start),
    // to the last shot; the settling after it stays on the plots.
    function stop() {
      if (playing) cancelAnimationFrame(playing.frame);
      playing = null; playButton.textContent = tr('Play', 'Відтворити');
    }
    function play() {
      if (simulation.kind !== 'ready') return;
      const last = Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
      const from = simulation.time >= last - .001 ? 0 : simulation.time, start = performance.now();
      playButton.textContent = tr('Pause', 'Пауза');
      const tick = now => {
        if (simulation.kind !== 'ready') { stop(); return; }
        // A frame's timestamp can precede the press: never before `from`.
        simulation.time = Math.min(last, from + Math.max(0, now - start) / 1000); draw();
        if (simulation.time >= last) stop(); else playing.frame = requestAnimationFrame(tick);
      };
      playing = {frame: requestAnimationFrame(tick)};
    }
    function seek(time) {
      stop();
      if (simulation.kind !== 'ready') return;
      simulation.time = time; draw();
    }
    function rebuild(time = 0) {
      const ids = sides.map(({id}) => id);
      stop(); simulation = {kind:'empty'};
      const valid = ids.length > 0 && ids.every(id => models.get(id));
      for (const element of root.querySelectorAll('input,select,button')) element.disabled = !valid;
      find('.sim-plots').hidden = !valid;
      find('.sim-sight').hidden = !valid;
      find('.sim-unavailable').hidden = valid;
      if (!valid) { find('.sim-legend').replaceChildren(); return; }
      seedInput.value = Math.max(1, Math.min(999999, Math.floor(Number(seedInput.value) || 1)));
      // A full magazine; with two weapons, the smaller one's from each.
      const shots = Math.min(...ids.map(id => models.get(id).magazine));
      const series = sides.map(({id, side}) => {
        const result = WeaponRecoil.simulate(models.get(id), shots, Number(seedInput.value), find('.sim-stance').value === 'hip');
        // An initial un-fired frame makes Reset/Next shot unambiguous in the UI.
        result.frames.forEach(frame => {frame.time += .05;});
        result.events.forEach(event => {event.time += .05;});
        result.frames.unshift({...result.frames[0], time:0, vertical:0, horizontal:0, shots:0, shake:{side:0, up:0}});
        return {...result, id, side, duration: result.duration + .05};
      });
      const duration = Math.max(...series.map(series => series.duration));
      simulation = {kind:'ready', series, time:Math.min(time, duration), duration};
      timeInput.max = simulation.duration; draw();
    }
    playButton.addEventListener('click', () => playing ? stop() : play());
    seedInput.addEventListener('change', () => rebuild(simulation.kind === 'ready' ? simulation.time : 0));
    find('.sim-stance').addEventListener('change', () => rebuild(simulation.kind === 'ready' ? simulation.time : 0));
    find('.sim-reset').addEventListener('click', () => seek(0));
    // Last shot: the whole pattern at once.
    find('.sim-last').addEventListener('click', () => {
      if (simulation.kind !== 'ready') return;
      seek(Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time)));
    });
    find('.sim-target').addEventListener('change', draw);
    // Next shot stops at the last one: the settling after it is the timeline's.
    find('.sim-step').addEventListener('click', () => {
      const next = nextShot();
      if (next !== undefined) seek(next);
    });
    timeInput.addEventListener('input', () => seek(Number(timeInput.value)));
    return {show(chosen) { sides = chosen; rebuild(); }};
  }
  // The comparison's simulation: one weapon chosen (A or B) or both, each in
  // its side's colour.
  const sim = document.querySelector('#recoil-sim'), comparisonSim = RecoilSim(sim);
  function rebuildSimulation() {
    const sides = [pickerA.value, pickerB.value].map((id, side) => ({id, side})).filter(({id}) => id);
    sim.hidden = sides.length === 0;
    comparisonSim.show(sides);
  }
  // A weapon's own simulation, in its card, made when the card first opens.
  const template = document.querySelector('#recoil-template');
  for (const weapon of weapons) {
    weapon.addEventListener('toggle', () => {
      const host = weapon.querySelector('.weapon-recoil');
      if (!weapon.open || !host || host.childElementCount) return;
      host.append(template.content.cloneNode(true));
      RecoilSim(host).show([{id: weapon.id, side: 0}]);
    });
  }
  function selectionChanged() {
    for (const chooser of choosers) chooser.paint();
    renderComparison(); rebuildSimulation();
    const url = new URL(location.href);
    for (const [key, value] of [['a', pickerA.value], ['b', pickerB.value]]) {
      if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
    }
    history.replaceState(null, '', url);
    const language = document.querySelector('a.lang');
    if (language) {
      const next = new URL(language.href);
      for (const key of ['a', 'b']) {
        if (url.searchParams.has(key)) next.searchParams.set(key, url.searchParams.get(key)); else next.searchParams.delete(key);
      }
      language.href = next;
    }
  }
  pickerA.addEventListener('change', selectionChanged); pickerB.addEventListener('change', selectionChanged);
  differenceOnly.addEventListener('change', renderComparison); colors.addEventListener('change', renderComparison);
  document.querySelector('#clear-comparison').addEventListener('click', () => {pickerA.value=''; pickerB.value=''; selectionChanged();});
  for (const button of document.querySelectorAll('[data-compare-side]')) {
    button.parentElement.hidden = false;
    button.addEventListener('click', () => {
      (button.dataset.compareSide === 'a' ? pickerA : pickerB).value = button.closest('.weapon').id;
      selectionChanged(); comparison.scrollIntoView({block:'start'});
    });
  }
  const query = new URLSearchParams(location.search);
  for (const [key, picker] of [['a', pickerA], ['b', pickerB]]) if (byId.has(query.get(key))) picker.value = query.get(key);
  comparison.hidden = false; selectionChanged();
  function update() {
    const query = normalize(search.value);
    let visible = 0;
    for (const weapon of weapons) {
      weapon.hidden = !(normalize(weapon.dataset.name).includes(query) &&
        (!category.value || weapon.dataset.class === category.value));
      if (!weapon.hidden) visible++;
    }
    const key = order.value;
    const direction = ['damage', 'rpm'].includes(key) ? -1 : 1;
    weapons.sort((a, b) => (key === 'name' ? 0 :
      direction * (Number(a.dataset[key]) - Number(b.dataset[key]))) ||
      collator.compare(a.dataset.name, b.dataset.name));
    list.append(...weapons);
    count.textContent = `${visible} / ${weapons.length} ${count.dataset.label}`;
    empty.hidden = visible !== 0;
  }
  search.addEventListener('input', update);
  category.addEventListener('change', update);
  order.addEventListener('change', update);
  document.querySelector('.catalogue-tools').hidden = false;
  function revealHash() {
    const weapon = weapons.find(row => `#${row.id}` === location.hash);
    if (weapon) {
      search.value = ''; category.value = ''; update();
      weapon.open = true; weapon.scrollIntoView({block: 'start'});
    }
  }
  addEventListener('hashchange', revealHash);
  update(); revealHash();
})();
