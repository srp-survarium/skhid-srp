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
  // The classes in the reader's order (the page is one for every language).
  category.append(...[...category.options].slice(1).sort((a, b) => collator.compare(a.text, b.text)));
  const normalize = text => text.normalize('NFKC').toLocaleLowerCase().replace(/[\s\-–—]+/g, '');
  const byId = new Map(weapons.map(element => [element.id, element]));
  const differenceOnly = document.querySelector('#only-differences'), colors = document.querySelector('#compare-colors');
  const tableHost = document.querySelector('#compare-table');
  const comparison = document.querySelector('#comparison');
  // Each weapon's rounds, its default first: [{id, name, rows}], `rows`
  // the stats that differ from the default round's (build.py).
  const roundsOf = new Map(weapons.map(element => [element.id, JSON.parse(element.dataset.rounds)]));
  function roundOf(id, roundId) {
    const rounds = roundsOf.get(id);
    return rounds.find(round => round.id === roundId) || rounds[0];
  }
  // The default round's stats, as the page was built.
  const stats = new Map(weapons.map(element => [element.id,
    [...element.querySelectorAll('[data-section]')].map(section => ({
      title: section.querySelector('h2').textContent, note: section.querySelector('.stat-note')?.textContent,
      rows: [...section.querySelectorAll('[data-stat]')].map(row => ({
        id: row.dataset.stat, label: row.querySelector('dt').textContent, text: row.querySelector('dd').textContent,
        direction: row.dataset.direction, values: JSON.parse(row.dataset.values)
      }))
    }))
  ]));
  function statsFor(id, roundId) {
    const rows = roundOf(id, roundId).rows;
    return stats.get(id).map(section => ({...section, rows: section.rows.map(row => rows[row.id] ? {...row, ...rows[row.id]} : row)}));
  }
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
    const [{id: a, round: roundA}, {id: b, round: roundB}] = slots.sides();
    document.querySelector('#compare-empty').hidden = !!(a && b);
    document.querySelector('#compare-same').hidden = true;
    if (!a || !b) return;
    const table = node('table', undefined, 'weapon-comparison');
    const head = node('thead'), header = node('tr');
    const corner = node('th', T('Stat')); corner.scope = 'col'; header.append(corner);
    for (const [id, round, side] of [[a, roundA, 'A'], [b, roundB, 'B']]) {
      const cell = node('th'); cell.scope = 'col';
      const image = byId.get(id).querySelector('img');
      if (image) { const copy = image.cloneNode(); copy.loading = 'eager'; cell.append(copy); }
      cell.append(node('span', `${side} · ${name(id)}`), node('small', roundOf(id, round).name)); header.append(cell);
    }
    head.append(header); table.append(head);
    const body = node('tbody');
    const peers = new Map(statsFor(b, roundB).flatMap(section => section.rows).map(row => [row.id, row]));
    let rowCount = 0;
    for (const section of statsFor(a, roundA)) {
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
            marker.setAttribute('aria-label', verdict > 0 ? T('Better') : T('Worse'));
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

  // The slots' recoil and spread: the simulators page with the same choice.
  const simulate = document.querySelector('#compare-simulate');
  const slots = WikiWeaponSlots(weapons, roundsOf, collator, () => {
    renderComparison();
    simulate.search = location.search;
  });
  // A card's round picker (made when the card first opens) sets its stats and
  // its link to the simulators.
  for (const weapon of weapons) {
    const select = weapon.querySelector('.round-select'), link = weapon.querySelector('.weapon-simulate a');
    let chooser = null;
    weapon.addEventListener('toggle', () => {
      if (!weapon.open || chooser) return;
      chooser = WikiWeaponPicker(select, slots.roundEntries, collator, slots.roundWords);
      chooser.offer(roundsOf.get(weapon.id).map(round => round.id)); chooser.paint();
    });
    select.addEventListener('change', () => {
      const rows = new Map(statsFor(weapon.id, select.value).flatMap(section => section.rows).map(row => [row.id, row]));
      for (const row of weapon.querySelectorAll('.weapon-details [data-stat]')) {
        const stat = rows.get(row.dataset.stat);
        row.querySelector('dd').textContent = stat.text; row.dataset.values = JSON.stringify(stat.values);
      }
      const address = new URL(link.href);
      if (select.selectedIndex > 0) address.searchParams.set('ra', select.value); else address.searchParams.delete('ra');
      link.href = address;
      chooser?.paint();
    });
  }
  differenceOnly.addEventListener('change', renderComparison); colors.addEventListener('change', renderComparison);
  document.querySelector('#clear-comparison').addEventListener('click', () => slots.clear());
  for (const button of document.querySelectorAll('[data-compare-side]')) {
    button.parentElement.hidden = false;
    button.addEventListener('click', () => {
      slots.choose(button.dataset.compareSide === 'a' ? 0 : 1, button.closest('.weapon').id);
      comparison.scrollIntoView({block:'start'});
    });
  }
  comparison.hidden = false; slots.refresh();
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
