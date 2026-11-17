// The game-styled rows the damage calculator and the wiki's simulators share
// (hud.css): a panel of the lobby's bonus rows (ModifierRow, InventoryItemHint),
// each its icon, "<label>:" and a button per option, and the cards that show
// what a thing does over its tile or label.
const HUD = (() => {
  const el = (tag, attrs = {}, ...kids) => {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (k === "class") n.className = v; else if (k === "text") n.textContent = v; else n.setAttribute(k, v);
    }
    for (const kid of kids) if (kid != null) n.append(kid);
    return n;
  };
  // A card: its name, then a line of text each.
  const card = (name, ...texts) => el("div", { class: "gamemods" }, el("span", { class: "gname", text: name }),
    ...texts.map(text => el("span", { class: "gtext", text })));
  // A tile with its card; a card with nothing to say leaves the tile as it is.
  // A click puts the card away until the pointer comes back, as a tooltip goes.
  function hinted(tile, box) {
    if (!box.children.length) return tile;
    tile.removeAttribute("title");
    const wrap = el("span", { class: "fxhint" }, tile, box);
    tile.addEventListener("click", () => wrap.classList.add("quiet"));
    wrap.addEventListener("mouseleave", () => wrap.classList.remove("quiet"));
    return wrap;
  }
  // A label's card opens on hover, on keyboard focus, and on a tap or click
  // (a touch screen has no hover); a press elsewhere or Escape closes it.
  const closeLabels = keep => { for (const open of document.querySelectorAll(".glabel.fxhint.open")) if (open !== keep) open.classList.remove("open"); };
  document.addEventListener("pointerdown", event => { if (!event.target.closest?.(".glabel.fxhint")) closeLabels(); });
  document.addEventListener("keydown", event => { if (event.key === "Escape") closeLabels(); });
  // A bonus row as the lobby draws it: icon, "<label>:", a button per option
  // [value, text] of picks[key], the picked one again clears it (unless
  // `required`: then a row always has one). With a card, its label shows it.
  function stepsRow(icon, label, options, picks, key, onChange, box, required = false) {
    const group = el("span", { class: "gsteps", role: "group", "aria-label": label });
    const paint = () => { for (const b of group.children) b.setAttribute("aria-pressed", String(b.dataset.v === String(picks[key] || ""))); };
    for (const [v, text] of options) {
      const b = el("button", { type: "button", class: "gstep", text });
      b.dataset.v = v;
      b.addEventListener("click", () => { picks[key] = !required && String(picks[key] || "") === String(v) ? "" : v; paint(); onChange(); });
      group.append(b);
    }
    paint();
    const name = el("span", box ? { class: "glabel fxhint", tabindex: "0" } : { class: "glabel" }, `${label}:`, box);
    if (box) name.addEventListener("click", () => { closeLabels(name); name.classList.toggle("open"); });
    return el("div", { class: "gmrow" }, icon, name, group);
  }
  return { el, card, hinted, stepsRow };
})();
