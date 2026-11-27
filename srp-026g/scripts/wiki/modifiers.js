// The modifiers page's item chooser: the weapons page's chooser over every
// item that takes a modifier. Choosing one asks the server for the page with
// that item's modifiers (?item=<its dictionary name>).
(() => {
  const select = document.getElementById('item-choice');
  if (!select) return;
  const lang = document.documentElement.lang;
  const items = [...document.querySelectorAll('#item-options [data-key]')];
  const picker = WikiWeaponPicker(select, items, new Intl.Collator(lang),
    {search: T('Search items'), none: T('No items match.')});
  picker.paint();
  select.addEventListener('change', () => {
    const item = items.find(element => element.id === select.value);
    if (!item) return;
    location.assign(`?lang=${lang}&item=${encodeURIComponent(item.dataset.key)}#item`);
  });
})();
