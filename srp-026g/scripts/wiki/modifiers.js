// The modifiers page's item chooser: the weapons page's chooser over every
// item that takes a modifier. Choosing one asks the server for the page with
// that item's modifiers (?item=<its dictionary name>).
(() => {
  const select = document.getElementById('item-choice');
  if (!select) return;
  const lang = document.documentElement.lang;
  const tr = (en, uk) => lang === 'uk' ? uk : en;
  const items = [...document.querySelectorAll('#item-options [data-key]')];
  const picker = WikiWeaponPicker(select, items, tr, new Intl.Collator(lang),
    {search: tr('Search items', 'Пошук предметів'), none: tr('No items match.', 'Предметів не знайдено.')});
  picker.paint();
  select.addEventListener('change', () => {
    const item = items.find(element => element.id === select.value);
    if (!item) return;
    location.assign(`?lang=${lang}&item=${encodeURIComponent(item.dataset.key)}#item`);
  });
})();
