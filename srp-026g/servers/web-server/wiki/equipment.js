// The equipment page (servers/web-server/src/equipment.rs): the slot buttons
// (one click; the choice kept in the address, ?slot=helmet), faction and level
// filters, the name search, the sorting and the grouping by set or slot.
(() => {
  const list = document.getElementById('gear-list');
  if (!list) return;
  const pieces = [...list.querySelectorAll('.gear')];
  const buttons = [...document.querySelectorAll('.slot-filter [data-slot]')];
  const search = document.getElementById('gear-search'), faction = document.getElementById('gear-faction');
  const from = document.getElementById('gear-level-from'), to = document.getElementById('gear-level-to');
  const order = document.getElementById('gear-sort'), grouping = document.getElementById('gear-group');
  const count = document.querySelector('[data-count]'), empty = document.getElementById('gear-empty');
  const lang = document.documentElement.lang;
  const collator = new Intl.Collator(lang, {numeric: true, sensitivity: 'base'});
  const normalize = text => text.normalize('NFKC').toLocaleLowerCase();
  const name = piece => piece.querySelector('.weapon-title strong').textContent;
  const setNames = new Map([...document.querySelectorAll('#gear-sets [data-set]')].map(item => [item.dataset.set, item.textContent]));
  const slots = buttons.map(button => button.dataset.slot).filter(Boolean);
  const slotNames = new Map(buttons.filter(button => button.dataset.slot)
    .map(button => [button.dataset.slot, button.querySelector('span:last-child').textContent]));
  const query = new URLSearchParams(location.search);
  let slot = query.get('slot') || '';
  if (!slotNames.has(slot)) slot = '';
  if ([...faction.options].some(option => option.value && option.value === query.get('faction'))) faction.value = query.get('faction');
  // Numbers sort ascending (level, weight) or descending (armour, isolation).
  const direction = {level: 1, weight: 1, armour: -1, isolation: -1};
  function compare(a, b) {
    const key = order.value;
    return (key === 'name' ? 0 : direction[key] * (Number(a.dataset[key]) - Number(b.dataset[key]))) ||
      Number(a.dataset.level) - Number(b.dataset.level) || collator.compare(name(a), name(b));
  }
  // The slot and the faction kept in the address (and the language link).
  function keepInAddress() {
    const keep = url => {
      for (const [key, value] of [['slot', slot], ['faction', faction.value]]) {
        if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
      }
      return url;
    };
    history.replaceState(null, '', keep(new URL(location.href)));
    const language = document.querySelector('a.lang');
    if (language) language.href = keep(new URL(language.href));
  }
  function update() {
    for (const button of buttons) button.setAttribute('aria-pressed', String(button.dataset.slot === slot));
    const query = normalize(search.value);
    const low = Math.min(Number(from.value), Number(to.value)), high = Math.max(Number(from.value), Number(to.value));
    let visible = 0;
    for (const piece of pieces) {
      const level = Number(piece.dataset.level);
      piece.hidden = !((!slot || piece.dataset.slot === slot) && (!faction.value || piece.dataset.faction === faction.value) &&
        level >= low && level <= high && normalize(name(piece)).includes(query));
      if (!piece.hidden) visible++;
    }
    // Groups: a set's pieces together (sets by their lowest level), or a slot's.
    const by = grouping.value;
    const groups = new Map();
    for (const piece of [...pieces].sort(compare)) {
      const key = by === 'set' ? piece.dataset.set : by === 'slot' ? piece.dataset.slot : '';
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(piece);
    }
    let keys = [...groups.keys()];
    if (by === 'slot') keys.sort((a, b) => slots.indexOf(a) - slots.indexOf(b));
    if (by === 'set') {
      const lowest = key => Math.min(...groups.get(key).map(piece => Number(piece.dataset.level)));
      keys.sort((a, b) => lowest(a) - lowest(b) || collator.compare(setNames.get(a) || a, setNames.get(b) || b));
    }
    const children = [];
    for (const key of keys) {
      const members = groups.get(key);
      if (by && members.some(piece => !piece.hidden)) {
        const heading = document.createElement('h2');
        heading.className = 'gear-group';
        heading.textContent = by === 'set' ? setNames.get(key) || key : slotNames.get(key) || key;
        children.push(heading);
      }
      children.push(...members);
    }
    list.replaceChildren(...children);
    count.textContent = `${visible} / ${pieces.length} ${count.dataset.label}`;
    empty.hidden = visible !== 0;
    keepInAddress();
  }
  for (const button of buttons) button.addEventListener('click', () => { slot = button.dataset.slot; update(); });
  search.addEventListener('input', update);
  for (const control of [faction, from, to, order, grouping]) control.addEventListener('change', update);
  document.querySelector('.gear-tools').hidden = false;
  update();
  const opened = pieces.find(piece => `#${piece.id}` === location.hash);
  if (opened) { opened.hidden = false; opened.open = true; opened.scrollIntoView({block: 'start'}); }
})();
