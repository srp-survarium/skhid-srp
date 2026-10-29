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

  const sim = document.querySelector('#recoil-sim'), timeInput = document.querySelector('#sim-time');
  const shotInput = document.querySelector('#sim-shots'), seedInput = document.querySelector('#sim-seed');
  let simulation = {kind: 'empty'};
  const svgNS = 'http://www.w3.org/2000/svg';
  function svgNode(tag, attrs, text) {
    const element = document.createElementNS(svgNS, tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    if (text !== undefined) element.textContent = text;
    return element;
  }
  function sample(series, time) {
    return series.frames[Math.min(series.frames.length - 1, Math.floor(time / .05 + 1e-6))];
  }
  function drawSimulation() {
    if (simulation.kind !== 'ready') return;
    const time = simulation.time;
    timeInput.value = time;
    document.querySelector('#sim-time-label').textContent = `${time.toFixed(2)} ${tr('s', 'с')}`;
    const legend = document.querySelector('#sim-legend'); legend.replaceChildren();
    for (const [index, series] of simulation.series.entries()) {
      const point = sample(series, time);
      legend.append(node('p', `${index ? 'B' : 'A'} · ${name(series.id)} · ${tr('shots', 'постріли')} ${point.shots}/${series.shots} · ${tr('up', 'вгору')} ${point.vertical.toFixed(3)} · ${tr('side', 'убік')} ${point.horizontal.toFixed(3)}`, `series-${index}`));
    }
    for (const axis of ['vertical', 'horizontal']) {
      const svg = document.querySelector(`#sim-${axis}`); svg.replaceChildren();
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
      for (const [index, series] of simulation.series.entries()) {
        const path = frames => frames.map((frame, i) => `${i ? 'L' : 'M'}${x(frame.time).toFixed(2)},${y(frame[axis]).toFixed(2)}`).join(' ');
        svg.append(svgNode('path', {d:path(series.frames), class:`sim-curve series-${index} future`}));
        svg.append(svgNode('path', {d:path(series.frames.filter(frame => frame.time <= time)), class:`sim-curve series-${index}`}));
        for (const event of series.events.filter(event => event.time <= time)) {
          const point = sample(series, event.time);
          svg.append(svgNode('circle', {cx:x(event.time), cy:y(point[axis]), r:2.8, class:`sim-shot series-${index}`}));
        }
      }
      svg.append(svgNode('line', {x1:x(time), x2:x(time), y1:24, y2:218, class:'sim-cursor'}));
    }
  }
  function seek(time) {
    if (simulation.kind !== 'ready') return;
    simulation.time = time; drawSimulation();
  }
  function rebuildSimulation(time = 0) {
    const ids = [pickerA.value, pickerB.value]; sim.hidden = !ids.every(Boolean);
    simulation = {kind:'empty'};
    if (sim.hidden) return;
    const valid = ids.every(id => models.get(id));
    for (const element of sim.querySelectorAll('input,select,button')) element.disabled = !valid;
    document.querySelector('.sim-plots').hidden = !valid;
    document.querySelector('#sim-unavailable').hidden = valid;
    if (!valid) { document.querySelector('#sim-legend').replaceChildren(); return; }
    shotInput.value = Math.max(1, Math.min(60, Math.floor(Number(shotInput.value) || 1)));
    seedInput.value = Math.max(1, Math.min(999999, Math.floor(Number(seedInput.value) || 1)));
    const series = ids.map(id => {
      const result = WeaponRecoil.simulate(models.get(id), Number(shotInput.value), Number(seedInput.value));
      // An initial un-fired frame makes Reset/Next shot unambiguous in the UI.
      result.frames.forEach(frame => {frame.time += .05;});
      result.events.forEach(event => {event.time += .05;});
      result.frames.unshift({time:0, vertical:0, horizontal:0, shots:0});
      return {...result, id, duration: result.duration + .05};
    });
    const duration = Math.max(...series.map(series => series.duration));
    simulation = {kind:'ready', series, time:Math.min(time, duration), duration};
    timeInput.max = simulation.duration; drawSimulation();
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
  shotInput.addEventListener('change', () => rebuildSimulation());
  seedInput.addEventListener('change', () => rebuildSimulation(simulation.kind === 'ready' ? simulation.time : 0));
  document.querySelector('#sim-reset').addEventListener('click', () => seek(0));
  document.querySelector('#sim-step').addEventListener('click', () => {
    if (simulation.kind !== 'ready') return;
    const events = simulation.series.flatMap(series => series.events).map(event => event.time).sort((a,b)=>a-b);
    seek(events.find(time => time > simulation.time + .001) ?? simulation.duration);
  });
  timeInput.addEventListener('input', () => seek(Number(timeInput.value)));
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
