// The page's own words in the reader's language (site-texts/README.md): the web server fills the
// page's <script type="application/json" id="page-words"> with every text its script asks for
// with T, Tc (a context first) and Tf (a template and its places), from words/<code>.tsv. Each
// takes its English text as a literal (the server and the words test find them so); English is
// what a text has no row for.
const PAGE_WORDS = JSON.parse(document.getElementById("page-words").textContent);
// (None calls another: each call in a script is a text's.)
const Tc = (context, english) => PAGE_WORDS[context]?.[english] ?? english;
const T = english => PAGE_WORDS[""]?.[english] ?? english;
const Tf = (english, values) => (PAGE_WORDS[""]?.[english] ?? english)
  .replace(/\{(\w+)\}/g, (place, name) => (name in values ? String(values[name]) : place));

// Calculator-style chooser: lobby icons, class groups, level ordering,
// punctuation-insensitive search and arrow/Enter/Escape keyboard controls.
// The hidden select owns the selection, including URL and catalogue updates.
// `weapons` are the elements offered (the weapons page's rows, or the
// modifiers page's item entries and the weapons page's rounds, which carry
// their name in data-title); `offer(ids)` narrows them to those ids. Its
// words are the page's (site-texts/words.js).
function WikiWeaponPicker(select, weapons, collator, words = {
  search: T('Search weapons'), none: T('No weapons match.')}) {
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
  const level = weapon => make('small', {}, Tf('lvl {n}', {n: weapon.dataset.level}));
  const button = make('button', {type: 'button', class: 'wpick-btn', id: `${select.id}-btn`,
    'aria-haspopup': 'listbox', 'aria-expanded': 'false', 'aria-controls': `${select.id}-list`,
    'aria-labelledby': `${select.id}-label ${select.id}-value`});
  const list = make('div', {class: 'wpick-list', role: 'listbox', id: `${select.id}-list`, 'aria-labelledby': `${select.id}-label`});
  const search = make('input', {type: 'search', class: 'wpick-search', role: 'combobox', autocomplete: 'off',
    'aria-controls': list.id, 'aria-expanded': 'false', 'aria-autocomplete': 'list',
    'aria-label': words.search, placeholder: T('Search…')});
  const empty = make('p', {class: 'wpick-empty', role: 'status'}, words.none);
  const popup = make('div', {class: 'wpick-pop', hidden: ''}, search, list, empty);
  host.append(button, popup);
  const options = [], groups = new Map();
  let active = null, offered = new Set(weapons.map(weapon => weapon.id));
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
        icon(weapon), make('span', {class: 'n'}, title(weapon)), level(weapon));
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
    for (const option of options) option.element.hidden = !offered.has(option.weapon.id) || !option.text.includes(query);
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
    offer(ids) { offered = new Set(ids); },
    paint() {
      const selected = weapons.find(weapon => weapon.id === select.value);
      button.replaceChildren(...(selected ? [icon(selected)] : []),
        make('span', {class: 'n', id: `${select.id}-value`}, selected ? title(selected) : select.options[0].textContent),
        ...(selected ? [level(selected)] : []));
      for (const option of options) option.element.setAttribute('aria-selected', String(option.weapon.id === select.value));
    }
  };
}

// The two weapon slots, A and B (the weapons page's comparison, the
// simulators'), each a weapon and its round (#compare-a, #compare-a-round,
// ...), kept in the address (a, b, ra, rb; a round only when not its
// weapon's default, the first of `roundsOf`) and in the language link.
// `changed()` runs after each choice; `refresh()` once the page is ready.
function WikiWeaponSlots(weapons, roundsOf, collator, changed) {
  const pickers = ['a', 'b'].map(side => document.getElementById(`compare-${side}`));
  const roundPickers = ['a', 'b'].map(side => document.getElementById(`compare-${side}-round`));
  // Every round's chooser entry (its icon, name and level).
  const roundEntries = [...document.querySelectorAll('#round-options > div')];
  const roundWords = {search: T('Search rounds'), none: T('No rounds match.')};
  const choosers = pickers.map(select => WikiWeaponPicker(select, weapons, collator));
  const roundChoosers = roundPickers.map(select => WikiWeaponPicker(select, roundEntries, collator, roundWords));
  const known = new Set(weapons.map(weapon => weapon.id));
  // A slot's rounds: its weapon's, keeping the chosen round when the new
  // weapon fires it too; a dash without a weapon.
  function fillRounds(side, id, chosen) {
    const select = roundPickers[side], rounds = id ? roundsOf.get(id) : [];
    select.replaceChildren(...(rounds.length ? rounds.map(round => new Option(round.name, round.id)) : [new Option('—', '')]));
    select.value = rounds.some(round => round.id === chosen) ? chosen : rounds[0]?.id ?? '';
    roundChoosers[side].offer(rounds.map(round => round.id));
  }
  function refresh() {
    pickers.forEach((picker, side) => fillRounds(side, picker.value, roundPickers[side].value));
    for (const chooser of [...choosers, ...roundChoosers]) chooser.paint();
    const url = new URL(location.href);
    const round = side => roundPickers[side].selectedIndex > 0 ? roundPickers[side].value : '';
    const keys = [['a', pickers[0].value], ['b', pickers[1].value], ['ra', round(0)], ['rb', round(1)]];
    for (const [key, value] of keys) if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
    history.replaceState(null, '', url);
    // Each of the header's languages keeps the choice.
    for (const language of document.querySelectorAll('.lang a')) {
      const next = new URL(language.href);
      for (const [key] of keys) {
        if (url.searchParams.has(key)) next.searchParams.set(key, url.searchParams.get(key)); else next.searchParams.delete(key);
      }
      language.href = next;
    }
    changed();
  }
  for (const select of [...pickers, ...roundPickers]) select.addEventListener('change', refresh);
  const query = new URLSearchParams(location.search);
  [['a', 'ra'], ['b', 'rb']].forEach(([key, round], side) => {
    if (known.has(query.get(key))) pickers[side].value = query.get(key);
    fillRounds(side, pickers[side].value, query.get(round));
  });
  return {
    roundEntries, roundWords, refresh,
    // [{id, round}] of A and B, '' for an empty slot.
    sides: () => pickers.map((picker, side) => ({id: picker.value, round: roundPickers[side].value})),
    choose(side, id) { pickers[side].value = id; refresh(); },
    clear() { for (const picker of pickers) picker.value = ''; refresh(); },
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
