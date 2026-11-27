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
