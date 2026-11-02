// The page: reads the controls, runs engine.js, draws the result.
const $ = id => document.getElementById(id);
const el = (tag, attrs = {}, ...kids) => {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") n.className = v; else if (k === "text") n.textContent = v;
    else if (k === "html") n.innerHTML = v; else if (k.startsWith("on")) n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v);
  }
  for (const kid of kids) if (kid != null) n.append(kid);
  return n;
};
// The page's language: ?lang=, else the document's (the site sets it).
const LANG = /^(uk|ua)/.test(new URLSearchParams(location.search).get("lang") || document.documentElement.lang || "") ? "uk" : "en";
const T = (en, uk) => LANG === "uk" ? uk : en;
// A game item's name in the page's language (the game's own localization).
const nm = o => (LANG === "uk" && o.name_uk) || o.name;
const M = T("m", "м");
const hp = x => (x * 100).toFixed(x * 100 >= 10 ? 1 : 2).replace(/\.0+$/, "");
const pct = x => Math.round(x * 100) + "%";
const fx = (x, d = 2) => Number(x).toFixed(d);

const CLASSES = [
  [T("Assault rifles", "Штурмові гвинтівки"), ["wpn_aslt"]], [T("Submachine guns", "Пістолети-кулемети"), ["wpn_smg"]],
  [T("Shotguns", "Дробовики"), ["wpn_asgn", "wpn_stgn", "wpn_dbrl"]], [T("Bolt-action & sniper rifles", "Гвинтівки та снайперські гвинтівки"), ["wpn_bolt"]],
  [T("Pistols & revolvers", "Пістолети й револьвери"), ["wpn_pstl", "wpn_apst", "wpn_rvlr"]],
];
const SLOTS = [["helmet", T("Helmet", "Шолом")], ["mask", T("Mask", "Маска")], ["torso", T("Jacket", "Куртка")], ["gloves", T("Gloves", "Рукавиці")],
  ["legs", T("Trousers", "Штани")], ["boots", T("Boots", "Черевики")], ["back", T("Backpack", "Рюкзак")]];
// Names by what each location's collision boxes sit on
// (base_character_full.skinned_model/hit_targets: location -> bones).
const PARTS = {
  front: T("Chest & neck", "Груди й шия"), back: T("Back", "Спина"), brain: T("Head", "Голова"), face: T("Face", "Обличчя"),
  right_forearm: T("Right arm", "Права рука"), left_forearm: T("Left arm", "Ліва рука"), right_arm: T("Right hand", "Права кисть"), left_arm: T("Left hand", "Ліва кисть"),
  right_hip: T("Right leg", "Права нога"), left_hip: T("Left leg", "Ліва нога"), right_foot: T("Right foot", "Права стопа"), left_foot: T("Left foot", "Ліва стопа"),
};
const POOLS = { body: T("Body", "Тіло"), head: T("Head", "Голова"), pain: T("Pain", "Біль"), right_leg: T("Right leg", "Права нога"), left_leg: T("Left leg", "Ліва нога"),
  right_hand: T("Right hand", "Права кисть"), left_hand: T("Left hand", "Ліва кисть"), infection: T("Infection", "Інфекція") };
const AFFECTS = { 0: T("death", "смерть"), 1: T("bleeding", "кровотеча"), 2: T("concussion", "контузія"), 3: T("hand injury", "травма руки"),
  4: T("leg injury", "травма ноги"), 6: T("infection", "інфекція"), 8: T("blindness", "сліпота") };
const poolColor = p => p === "pain" ? "var(--pool-pain)" : p === "body" ? "var(--pool-body)" : p === "head" ? "var(--pool-head)" : "var(--pool-limb)";

const state = {
  weapon: "wpn_ak_74n", ammo: null, dist: 20, part: "front",
  armour: { helmet: "renesanse_helmet_3", mask: "", torso: "renesanse_torso_3", gloves: "", legs: "renesanse_legs_3", boots: "renesanse_boots_3", back: "" },
  cmpAmmo: "best", sort: { k: "shots", dir: 1 }, mapView: "minimap",
};
try { Object.assign(state, JSON.parse(localStorage.getItem("srp-dmg") || "{}")); } catch (e) {}
const save = () => { try { localStorage.setItem("srp-dmg", JSON.stringify(state)); } catch (e) {} };

const ammoFor = w => (DATA.compat && DATA.compat[w] && DATA.compat[w].length) ? DATA.compat[w] : Object.keys(DATA.ammo);
const defaultAmmo = w => (DATA.default_ammo && DATA.default_ammo[w]) || ammoFor(w)[0];

// ---------- shooter ----------
// Each class's weapons by level, then name.
function weaponGroups() {
  return CLASSES.map(([label, types]) => [label, Object.entries(DATA.weapons).filter(([, w]) => types.includes(w.type))
    .sort((a, b) => (a[1].item_level || 0) - (b[1].item_level || 0) || nm(a[1]).localeCompare(nm(b[1])))]);
}
const premium = w => w.premium ? T(" (premium)", " (преміум)") : "";
function fillWeapons() {
  const s = $("weapon"); s.innerHTML = "";
  for (const [label, items] of weaponGroups()) {
    const g = el("optgroup", { label });
    items.forEach(([k, w]) => g.append(el("option", { value: k, text: nm(w) + premium(w) })));
    s.append(g);
  }
  s.value = state.weapon;
  buildWeaponPicker();
}
// A list with the lobby icons in place of a <select>, which cannot show them.
// The hidden select keeps the value and gets a "change" when one is picked,
// as if it had been used. groups: [[label or null, [[key, text, note]]]].
function makePicker(name, select, groups, searchable) {
  const host = $(name); host.innerHTML = "";
  // No icon (an empty slot's "none"): an empty space, not a broken image.
  const icon = k => { const src = DATA.icons && DATA.icons[k]; return src ? el("img", { alt: "", src }) : el("span", { class: "noimg" }); };
  const btn = el("button", { type: "button", class: "wpick-btn", id: name + "-btn", "aria-haspopup": "listbox", "aria-expanded": "false" });
  const list = el("div", { class: "wpick-list", role: "listbox", id: name + "-list", tabindex: "-1" });
  const search = searchable ? el("input", { type: "search", class: "wpick-search", role: "combobox", "aria-controls": name + "-list", "aria-expanded": "true",
    autocomplete: "off", placeholder: T("Search…", "Пошук…"), "aria-label": T("Search", "Пошук") }) : null;
  const pop = el("div", { class: "wpick-pop", hidden: "" }, search, list);
  host.append(btn, pop);
  // Letters and digits only: "hk416" finds H&K 416A5, "ak74" the AK-74s.
  const bare = t => t.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
  const opts = [];
  let active = null;
  const keys = search || list;
  const activate = (x, scroll = true) => {
    if (active) active.o.classList.remove("active");
    active = x || null;
    if (!active) { keys.removeAttribute("aria-activedescendant"); return; }
    active.o.classList.add("active");
    keys.setAttribute("aria-activedescendant", active.o.id);
    if (scroll) active.o.scrollIntoView({ block: "nearest" });
  };
  for (const [label, items] of groups) {
    const group = label ? el("div", { class: "wpick-group", text: label }) : null;
    if (group) list.append(group);
    for (const [k, text, note, also] of items) {
      const o = el("div", { class: "wpick-opt", role: "option", id: `${name}-${k}`, "aria-selected": "false" },
        icon(k), el("span", { class: "n", text }), note ? el("small", { text: note }) : null);
      const x = { o, group, k, text: [text, also || ""].map(bare).join(" ") };
      // The row under the pointer is the one Enter takes, as the arrows' is.
      o.addEventListener("mousemove", () => { if (active !== x) activate(x, false); });
      o.addEventListener("mousedown", e => e.preventDefault());
      o.addEventListener("click", () => choose(k));
      list.append(o);
      opts.push(x);
    }
  }
  const visible = () => opts.filter(x => !x.o.hidden);
  const filter = () => {
    const q = search ? bare(search.value) : "";
    for (const x of opts) x.o.hidden = !!q && !x.text.includes(q);
    list.querySelectorAll(".wpick-group").forEach(g => { g.hidden = !opts.some(x => x.group === g && !x.o.hidden); });
    activate(visible().find(x => x.k === select.value) || visible()[0]);
  };
  const open = () => {
    pop.hidden = false; btn.setAttribute("aria-expanded", "true");
    // A list wider than its button, under one near the right edge, opens leftwards.
    pop.style.left = "";
    const over = pop.getBoundingClientRect().right - (document.documentElement.clientWidth - 16);
    if (over > 0) pop.style.left = -over + "px";
    if (search) search.value = "";
    filter(); keys.focus();
  };
  const close = refocus => {
    pop.hidden = true; btn.setAttribute("aria-expanded", "false");
    if (refocus) btn.focus();
  };
  function choose(k) {
    close(true);
    if (k === select.value) return;
    select.value = k;
    select.dispatchEvent(new Event("change"));
  }
  btn.addEventListener("click", () => pop.hidden ? open() : close(true));
  btn.addEventListener("keydown", e => { if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); open(); } });
  if (search) search.addEventListener("input", filter);
  keys.addEventListener("keydown", e => {
    const v = visible(), i = v.indexOf(active);
    if (e.key === "ArrowDown") { e.preventDefault(); activate(v[Math.min(v.length - 1, i + 1)]); }
    else if (e.key === "ArrowUp") { e.preventDefault(); activate(v[Math.max(0, i - 1)]); }
    else if (e.key === "Enter" || (!search && e.key === " ")) { e.preventDefault(); if (active) choose(active.k); }
    else if (e.key === "Escape") { e.preventDefault(); close(true); }
    else if (e.key === "Tab") close(false);
  });
  document.addEventListener("mousedown", e => { if (!pop.hidden && !host.contains(e.target)) close(false); });
  // The button shows the one in use.
  return {
    paint(k, text) {
      btn.innerHTML = "";
      const src = DATA.icons && DATA.icons[k];
      if (src) btn.append(el("img", { src, alt: "" }));
      btn.append(el("span", { class: "n", text }));
      for (const x of opts) x.o.setAttribute("aria-selected", String(x.k === k));
    },
  };
}
let weaponPicker = null, ammoPicker = null, ammoPickerFor = null;
function buildWeaponPicker() {
  weaponPicker = makePicker("wpick", $("weapon"), weaponGroups().map(([label, items]) =>
    [label, items.map(([k, w]) => [k, nm(w) + premium(w), T(`lvl ${w.item_level || 0}`, `рів. ${w.item_level || 0}`), w.name])]), true);
}
function paintWeaponPicker() {
  const w = DATA.weapons[state.weapon];
  if (weaponPicker && w) weaponPicker.paint(state.weapon, nm(w) + premium(w));
}
function fillAmmo() {
  const s = $("ammo");
  if (ammoPickerFor !== state.weapon) {
    // The weapon's ammo, in its own list.
    s.innerHTML = "";
    for (const k of ammoFor(state.weapon)) s.append(el("option", { value: k, text: nm(DATA.ammo[k]) }));
    ammoPicker = makePicker("apick", s, [[null, ammoFor(state.weapon).map(k => [k, nm(DATA.ammo[k]), "", DATA.ammo[k].name])]], false);
    ammoPickerFor = state.weapon;
  }
  if (!ammoFor(state.weapon).includes(state.ammo)) state.ammo = defaultAmmo(state.weapon);
  s.value = state.ammo;
  ammoPicker.paint(state.ammo, nm(DATA.ammo[state.ammo]));
}

// The weapon's numbers after its bonuses. Bonuses come from DATA.weapon_mods
// once those are recovered; with none chosen this is the config as it is.
function effectiveWeapon(key, chosen = (state.wmods || {})[key] || {}) {
  const w = { ...DATA.weapons[key] };
  if (RULES.applyWeaponMods) RULES.applyWeaponMods(w, chosen);
  return w;
}

// ---------- target ----------
// Clicking a card aims at what the item covers, the next covered part on each
// click. An empty slot aims at the parts that slot would cover.
const SLOT_PARTS = {
  helmet: ["brain", "face"], mask: ["face"], torso: ["front", "back", "right_forearm", "left_forearm"],
  gloves: ["right_arm", "left_arm"], legs: ["right_hip", "left_hip"], boots: ["right_foot", "left_foot"], back: ["back"],
};
function slotParts(slot) {
  const key = state.armour[slot];
  if (!key) return SLOT_PARTS[slot];
  const cover = RULES.itemArmour(DATA.armour[key], (state.amods || {})[key] || {});
  const hit = Object.keys(cover).filter(p => p in PARTS && (cover[p].ranged || 0) > 0);
  const order = [...SLOT_PARTS[slot], ...Object.keys(PARTS)];
  return hit.length ? hit.sort((a, b) => order.indexOf(a) - order.indexOf(b)) : SLOT_PARTS[slot];
}
function aimAtSlot(slot) {
  const parts = slotParts(slot);
  const i = parts.indexOf(state.part);
  state.part = parts[(i + 1) % parts.length];
  update();
}
// The slot another worn item takes over (a closed helmet takes the mask).
function takenBy(slot) {
  for (const [s, label] of SLOTS) {
    const key = state.armour[s];
    if (s !== slot && key && (DATA.armour[key].takes_slots || []).includes(slot)) return nm(DATA.armour[key]);
  }
  return null;
}
// A slot's items as the weapons are: by level, then name, with their icons.
const slotItems = slot => Object.entries(DATA.armour).filter(([, a]) => a.slot === slot)
  .sort((a, b) => (a[1].item_level || 0) - (b[1].item_level || 0) || nm(a[1]).localeCompare(nm(b[1])));
const NONE = T("— none —", "— нічого —");
const slotPickers = {};
function paintSlotPicker(slot) {
  const key = state.armour[slot] || "";
  slotPickers[slot].paint(key, key ? nm(DATA.armour[key]) + premium(DATA.armour[key]) : NONE);
}
function fillArmour() {
  const box = $("armour"); box.innerHTML = "";
  for (const [slot, label] of SLOTS) {
    const items = slotItems(slot);
    const sel = el("select", { id: "slot-" + slot, "aria-label": label, hidden: "" });
    sel.append(el("option", { value: "", text: NONE }));
    for (const [k, a] of items) sel.append(el("option", { value: k, text: nm(a) + premium(a) }));
    sel.value = state.armour[slot] || "";
    const pick = el("div", { class: "wpick", id: "spick-" + slot });
    sel.addEventListener("change", () => { state.armour[slot] = sel.value; update(); });
    const aim = el("div", { class: "aim", role: "button", tabindex: "0", id: "slotaim-" + slot },
      el("div", { class: "head" }, el("span", { class: "lbl", text: label }), el("span", { class: "v", id: "slotv-" + slot })),
      el("div", { class: "art" }, el("img", { id: "sloti-" + slot, alt: "" }), el("span", { class: "empty", id: "slote-" + slot, text: T("nothing worn", "нічого не вдягнено") })));
    aim.addEventListener("click", () => aimAtSlot(slot));
    aim.addEventListener("keydown", e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); aimAtSlot(slot); } });
    box.append(el("div", { class: "slot", id: "slot-card-" + slot }, aim, sel, pick));
    slotPickers[slot] = makePicker("spick-" + slot, sel, [[null, [["", NONE, ""],
      ...items.map(([k, a]) => [k, nm(a) + premium(a), T(`lvl ${a.item_level || 0}`, `рів. ${a.item_level || 0}`), a.name])]]], true);
    $("spick-" + slot + "-btn").setAttribute("aria-label", label);
  }
}
function armourMap() {
  const out = {};
  for (const [slot] of SLOTS) {
    const key = state.armour[slot]; if (!key) continue;
    const item = DATA.armour[key];
    const params = RULES.itemArmour ? RULES.itemArmour(item, (state.amods || {})[key] || {}) : item.hit_params;
    for (const [part, types] of Object.entries(params)) {
      out[part] = out[part] || {};
      for (const [t, v] of Object.entries(types)) out[part][t] = (out[part][t] || 0) + v;
    }
  }
  return out;
}
function buildTarget() {
  const fxs = RULES.targetEffects ? RULES.targetEffects(state) : { incoming: [], final: [], hmax: {}, secondChance: null };
  // Pain already taken, in the page's x100 units.
  const painMax = fxs.hmax.pain ?? (DATA.body.find(r => r.name === "pain") || {}).health ?? 1;
  const start = state.pain ? { pain: painMax - state.pain / 100 } : {};
  return makeTarget({ body: DATA.body, armour: armourMap(), ...fxs, start });
}

// The doll: the HUD's own figure (hud.swf sprite 119), each part a red bitmap
// the game tints by its pool (CriticalIndicator.partHP, fed by the client at
// 0x2370e0 with H/Hmax and the pool's regeneration_threshold). The traced
// zones on top only take the clicks.
const HUD_POOL = { lHand: "left_hand", rHand: "right_hand", lLeg: "left_leg", rLeg: "right_leg", head: "head", body: "body" };
function hudTint(value, breakPoint) {
  const green = Math.min(value / breakPoint * 180, 180);
  const alpha = value < breakPoint ? 0.5 : 0.5 * ((1 - value) / (1 - breakPoint));
  return { fill: `rgb(255,${Math.round(green)},0)`, opacity: Math.max(0, alpha) };
}
function dollView(view) {
  const D = DATA.hud_doll;
  const id = n => `hd-${view}-${n}`;
  const defs = Object.entries(D.art).map(([n, src]) => {
    const p = D.parts.find(q => q.art === n) || { w: 63, h: 107 };
    return `<image id="${id("bm-" + n)}" width="${n === "outline" ? 63 : p.w}" height="${n === "outline" ? 107 : p.h}" href="${src}"/>`;
  }).join("") + D.parts.map(p => `<mask id="${id("m-" + p.name)}" mask-type="alpha" style="mask-type:alpha" maskUnits="userSpaceOnUse" x="0" y="-30" width="140" height="130"><use href="#${id("bm-" + p.art)}" transform="matrix(${p.matrix.join(" ")})"/></mask>`).join("");
  const mirror = view === "back" ? ` transform="translate(189 0) scale(-1 1)"` : "";
  const parts = D.parts.map(p => `<rect id="${id(p.name)}" mask="url(#${id("m-" + p.name)})" x="0" y="-30" width="140" height="130" fill="rgb(255,180,0)" fill-opacity="0"/>`).join("");
  const zones = D.zones[view].map(z => `<path data-part="${z.part}" transform="${z.transform}" d="${z.d}"/>`).join("");
  const caption = view === "back" ? T("back", "ззаду") : T("front", "спереду");
  // The extracted zone paths use front-view coordinates. Mirror the figure,
  // damage tint and interactive selection together, preserving anatomical sides.
  return `<figure><svg viewBox="58 -28 73 114" role="group" aria-label="${caption}"><defs>${defs}</defs>
    <g${mirror}>${parts}<use href="#${id("bm-outline")}" x="63" y="-24.65"/>
    <g class="zones" transform="translate(63 -24.65)">${zones}</g></g></svg><figcaption>${caption}</figcaption></figure>`;
}
function drawDoll() {
  const host = $("dollsvg");
  host.innerHTML = DATA.hud_doll ? dollView("front") + dollView("back") : defaultDoll();
  host.querySelectorAll("[data-part]").forEach(n => {
    n.classList.add("part");
    n.setAttribute("tabindex", "0"); n.setAttribute("role", "button");
    n.setAttribute("aria-label", PARTS[n.dataset.part] || n.dataset.part);
    const pick = () => { state.part = n.dataset.part; update(); };
    n.addEventListener("click", pick);
    n.addEventListener("keydown", e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(); } });
    n.append(el("title", { text: PARTS[n.dataset.part] }));
  });
}
// Colour the doll as the HUD would after `shot` hits.
function tintDoll(target, sim) {
  if (!DATA.hud_doll) return;
  const last = sim.history.length - 1;
  state.dollShot = clamp(state.dollShot || 1, 1, Math.max(1, last));
  const H = sim.history[state.dollShot];
  for (const view of ["front", "back"]) for (const [name, pool] of Object.entries(HUD_POOL)) {
    const r = $(`hd-${view}-${name}`); if (!r) continue;
    const row = target.rows[pool];
    const t = hudTint(H[pool] / target.max[pool], row.regeneration_threshold || 0.5);
    r.setAttribute("fill", t.fill); r.setAttribute("fill-opacity", t.opacity.toFixed(3));
  }
  const dead = isFinite(sim.shots) && state.dollShot >= sim.shots;
  $("dollshot").innerHTML = T("after shot", "постріл") + ` <b class="num">${state.dollShot}</b> ${T("of", "з")} ${isFinite(sim.shots) ? sim.shots : last}${dead ? T(" · dead", " · мертвий") : ""}`;
  $("dollprev").disabled = state.dollShot <= 1;
  $("dollnext").disabled = state.dollShot >= last;
}
function setIcon(img, key) {
  const src = key && DATA.icons && DATA.icons[key];
  img.style.visibility = src ? "visible" : "hidden";
  if (src) img.src = src; else img.removeAttribute("src");
}

// ---------- result ----------
function shotFor(weaponKey, ammoKey, target, part) {
  const w = effectiveWeapon(weaponKey);
  const b = bulletAt(w, DATA.ammo[ammoKey], state.dist);
  const sim = simulate(target, b, part);
  return { w, b, sim };
}
function fatalHit(sim) {
  // The damage of one shot to the pool that ends the target.
  const s = sim.firstShot, k = sim.killer;
  if (k) return { pool: k, v: s[k] };
  let best = null;
  for (const p of ["body", "head", "pain"]) if (s[p] > (best ? best.v : 0)) best = { pool: p, v: s[p] };
  return best || { pool: "pain", v: 0 };
}
const ttk = (w, shots) => isFinite(shots) && w.rounds_per_minute ? (shots - 1) * 60 / w.rounds_per_minute : Infinity;

let modSig = "", mapSig = "";
let later = null, laterQueued = false;
function flushLater() { laterQueued = false; const f = later; later = null; if (f) f(); }
function update() {
  for (const [slot] of SLOTS) if (state.armour[slot] && takenBy(slot)) state.armour[slot] = "";
  const sigMap = JSON.stringify([state.map, state.mapA, state.mapB]);
  if (sigMap !== mapSig) { mapSig = sigMap; const d = mapDistance(); if (d != null) state.dist = Math.max(0, d); }
  save();
  const sig = state.weapon + "|" + state.part + "|" + SLOTS.map(([s]) => state.armour[s]).join(",");
  if (sig !== modSig && RULES.renderMods) { modSig = sig; RULES.renderMods(state, update); }
  $("weapon").value = state.weapon;
  paintWeaponPicker();
  fillAmmo();
  $("dist").value = state.dist;
  $("distout").textContent = state.dist + " " + M;
  state.pain = clamp(Math.round(state.pain || 0), 0, 99);
  $("pain").value = state.pain;
  $("painout").textContent = state.pain ? `${state.pain} · ${T(`${100 - state.pain} left`, `лишилось ${100 - state.pain}`)}` : T("none", "немає");
  const target = buildTarget();
  const { w, b, sim } = shotFor(state.weapon, state.ammo, target, state.part);
  const a = DATA.ammo[state.ammo];

  $("wstats").innerHTML = [
    [T("damage", "пошкодження"), hp(b.Dmax) + (b.pellets > 1 ? ` ×${b.pellets}` : "")], [T("penetration", "бронебійність"), hp(b.Pmax)],
    [T("full damage to", "повне до"), fx(b.r0, 0) + " " + M], [T("minimum from", "мінімум від"), fx(b.r1, 0) + " " + M],
    [T("minimum", "мінімум"), hp(b.Dmin) + " / " + hp(b.Pmin)], [T("muzzle", "швидкість кулі"), a.muzzle_speed + T(" m/s", " м/с")],
    [T("rate", "темп"), (w.rounds_per_minute || "—") + T(" rpm", " постр./хв")], [T("magazine", "магазин"), w.magazine_capacity],
  ].map(([k, v]) => `<span>${k} <b>${v}</b></span>`).join("");
  drawScale(b);

  // Doll: selected and covered parts.
  const arm = target.armour;
  document.querySelectorAll("#dollsvg [data-part]").forEach(n => {
    n.classList.toggle("sel", n.dataset.part === state.part);
    n.classList.toggle("covered", ((arm[n.dataset.part] || {}).ranged || 0) > 0);
  });
  const A = (arm[state.part] || {}).ranged || 0;
  $("partname").textContent = `${PARTS[state.part]} · ${T("armour", "броня")} ${hp(A)}`;
  setIcon($("wicon"), state.weapon); setIcon($("aicon"), state.ammo);
  for (const [slot] of SLOTS) {
    const key = state.armour[slot];
    const v = key && RULES.itemArmour(DATA.armour[key], (state.amods || {})[key] || {})[state.part];
    setIcon($("sloti-" + slot), key);
    $("sloti-" + slot).hidden = !key; $("slote-" + slot).hidden = !!key;
    $("slotv-" + slot).textContent = key ? (v ? `${hp(v.ranged || 0)} ${T("here", "тут")}` : T("not here", "не тут")) : "";
    const taken = takenBy(slot);
    $("slot-" + slot).value = key || "";
    $("slot-" + slot).disabled = !!taken;
    $("spick-" + slot + "-btn").disabled = !!taken;
    paintSlotPicker(slot);
    $("slot-card-" + slot).classList.toggle("taken", !!taken);
    $("slote-" + slot).textContent = taken ? T(`taken by ${taken}`, `зайнято: ${taken}`) : T("nothing worn", "нічого не вдягнено");
    $("slot-card-" + slot).classList.toggle("hit", !!v);
    $("slot-card-" + slot).classList.toggle("miss", !!key && !v);
    const next = slotParts(slot), nextPart = next[(next.indexOf(state.part) + 1) % next.length];
    const here = PARTS[state.part].toLowerCase(), there = PARTS[nextPart].toLowerCase();
    $("slotaim-" + slot).title = (key ? (v ? T(`Covers the ${here}. `, `Захищає: ${here}. `) : T(`Does not cover the ${here}. `, `Не захищає: ${here}. `)) : "")
      + T(`Click to aim at the ${there}.`, `Клікніть, щоб цілити: ${there}.`);
    $("slotaim-" + slot).setAttribute("aria-label", `${SLOTS.find(s => s[0] === slot)[1]}: ${T(`aim at the ${there}`, `цілити: ${there}`)}`);
  }

  tintDoll(target, sim);
  drawMapOver(b);
  drawVerdict(w, b, sim);
  drawTrace(b, sim);
  drawPools(target, sim);
  drawChart(w, a, target);
  drawExplain();
  drawHealing();
  // The two big tables simulate every part and every gun: draw them a frame
  // later so a click paints first, once for several quick clicks.
  later = () => { drawParts(target); drawCompare(target); };
  if (!laterQueued) { laterQueued = true; requestAnimationFrame(flushLater); }
}

function drawScale(b) {
  const max = Number($("dist").max);
  const sc = $("scale"); sc.innerHTML = "";
  for (let m = 0; m <= max; m += 25) sc.append(el("i", { style: `left:${m / max * 100}%` + (m === max ? ";transform:translateX(-100%);border-left:0;border-right:1px solid var(--ink-3);padding:4px 2px 0 0" : ""), text: m }));
  sc.append(el("span", { class: "band", style: `left:${b.r0 / max * 100}%;width:${Math.max(0.5, (Math.min(b.r1, max) - b.r0) / max * 100)}%` }));
  const where = state.dist <= b.r0 ? T("Full damage at this range.", "На цій дистанції пошкодження повне.") :
    state.dist >= b.r1 ? T("Fully fallen off: minimum damage.", "Повністю спало: мінімальне пошкодження.") :
    T(`Falling off: ${pct(b.u)} of the way from minimum to full.`, `Спадає: ${pct(b.u)} шляху від мінімуму до повного.`);
  $("falloffnote").textContent = T(`Orange band: damage falls from ${fx(b.r0, 0)} m to ${fx(b.r1, 0)} m.`,
    `Помаранчева смуга: пошкодження спадає від ${fx(b.r0, 0)} м до ${fx(b.r1, 0)} м.`) + " " + where;
}

function drawVerdict(w, b, sim) {
  const f = fatalHit(sim);
  const t = ttk(w, sim.shots);
  const per = b.pellets > 1 ? T("per shot (all pellets)", "за постріл (усі дробини)") : T("per hit", "за влучання");
  const from = sim.history[0][f.pool], left = from < buildTarget().max[f.pool], pool = POOLS[f.pool].toLowerCase();
  $("verdict").innerHTML = `
    <div><div class="lbl">${T(`${POOLS[f.pool]} damage ${per}`, `${POOLS[f.pool]}: пошкодження ${per}`)}</div><div class="big num">${hp(f.v)}</div>
      <div class="sub">${T(`of ${hp(from)} ${pool}${left ? " left" : ""}`, `із ${hp(from)}${left ? ", що лишились" : ""} (${pool})`)}</div></div>
    <div><div class="lbl">${T("Hits to kill", "Влучань до смерті")}</div><div class="big num">${isFinite(sim.shots) ? sim.shots : "∞"}</div>
      <div class="sub">${sim.killer ? T(`dies when ${POOLS[sim.killer].toLowerCase()} reaches 0`, `гине, коли ${POOLS[sim.killer].toLowerCase()} доходить до 0`) : T("this shot cannot kill", "цей постріл не вбиває")}</div></div>
    <div><div class="lbl">${T("Time to kill", "Час до смерті")}</div><div class="big num">${isFinite(t) ? fx(t, 2) : "—"}<small> ${T("s", "с")}</small></div>
      <div class="sub">${T("every shot lands, no reload", "усі влучають, без перезарядки")}</div></div>`;
}

function drawTrace(b, sim) {
  const list = $("trace"); list.innerHTML = "";
  const li = (what, vals, why, hot) => list.append(el("li", { class: hot ? "hot" : "" },
    el("span", { class: "what", text: what }), el("span", { class: "vals", html: vals }), why ? el("span", { class: "why", html: why }) : null));
  li(T("Leaves the barrel", "Виліт зі ствола"), `D ${hp(b.Dmax)} · P ${hp(b.Pmax)}`,
    T(`Weapon damage × ammo <code>k_damage</code>, weapon pierce × ammo <code>k_arp</code>.`, `Пошкодження зброї × <code>k_damage</code> набою, бронебійність зброї × <code>k_arp</code> набою.`)
    + (b.pellets > 1 ? T(` Each of the ${b.pellets} pellets carries this.`, ` Так у кожної з ${b.pellets} дробин.`) : ""));
  li(T(`Flies ${state.dist} m`, `Летить ${state.dist} м`), `D ${hp(b.D)} · P ${hp(b.P)}`,
    T(`Speed ${fx(b.v0, 0)} → ${fx(b.v, 0)} m/s. Damage and penetration slide from full (${hp(b.Dmax)}/${hp(b.Pmax)}) to minimum (${hp(b.Dmin)}/${hp(b.Pmin)}) by speed between ${fx(b.r0, 0)} and ${fx(b.r1, 0)} m: here ${pct(b.u)}.`,
      `Швидкість ${fx(b.v0, 0)} → ${fx(b.v, 0)} м/с. Пошкодження й бронебійність спадають від повних (${hp(b.Dmax)}/${hp(b.Pmax)}) до мінімальних (${hp(b.Dmin)}/${hp(b.Pmin)}) за швидкістю між ${fx(b.r0, 0)} і ${fx(b.r1, 0)} м: тут ${pct(b.u)}.`), b.u < 1);
  // The first pellet of the first shot.
  const firstId = sim.firstLog.length ? 0 : null;
  let seenDepth1 = new Set();
  for (const e of sim.firstLog) {
    if (e.step === "callback") li(e.name, `D ${hp(e.before)} → ${hp(e.D)} · P ${hp(e.P)}`, e.note || "", true);
    else if (e.step === "armour" && e.depth === 0) {
      const gap = e.A - e.P;
      const why = e.A === 0 ? T("No armour here, the full amount goes in.", "Броні тут немає: проходить усе.") :
        gap <= 0 ? T(`Armour ${hp(e.A)} is not above penetration ${hp(e.P)}: nothing is stopped.`, `Броня ${hp(e.A)} не вища за бронебійність ${hp(e.P)}: нічого не зупинено.`) :
        gap >= 1 ? T(`Armour exceeds penetration by a full 100: the bullet does nothing.`, `Броня вища за бронебійність на повні 100: куля нічого не робить.`) :
        T(`Armour ${hp(e.A)} − penetration ${hp(e.P)} = ${hp(gap)}% of the damage is stopped.`, `Броня ${hp(e.A)} − бронебійність ${hp(e.P)} = зупинено ${hp(gap)}% пошкодження.`);
      li(T(`Hits ${PARTS[e.part].toLowerCase()}`, `Влучає: ${PARTS[e.part].toLowerCase()}`), `×${fx(e.k, 3)} → ${hp(e.D)}`, why, e.A > e.P);
    } else if (e.step === "pool" && e.depth >= 1) {
      if (seenDepth1.has(e.part)) continue; seenDepth1.add(e.part);
      li(`→ ${POOLS[e.part]}`, `−${hp(e.D)} (${hp(e.before)} → ${hp(e.after)})`,
        e.part === "pain" ? T("Every hit anywhere also drains the shared pain pool. Pain at 0 kills.", "Кожне влучання також знімає спільний біль. Біль на 0 вбиває.") :
        e.part === "body" ? T("Body at 0 kills.", "Тіло на 0 вбиває.") :
        e.part === "head" ? T("Head holds only 40: at 0 it kills, below 20 it concusses.", "Голова має лише 40: на 0 вбиває, нижче 20 — контузія.") :
        T("Limbs do not kill by themselves: an empty limb injures, the pain it carries can kill.", "Кінцівки самі не вбивають: порожня кінцівка травмується, а біль від неї може вбити."));
    } else if (e.step === "second_chance") li(T("Second chance", "Другий шанс"), `${hp(e.before)} → ${hp(e.D)}`, T("The skill leaves the target alive once.", "Навичка один раз лишає ціль живою."), true);
    if (b.pellets > 1 && e.step === "pool" && e.depth >= 1 && seenDepth1.size >= 2) break;
  }
  if (b.pellets > 1) li(T(`× ${b.pellets} pellets`, `× ${b.pellets} дробин`), "", T("Each pellet is its own bullet: when several land, their damage adds up.", "Кожна дробина — окрема куля: коли влучає кілька, пошкодження додається."));
}

// Under the doll: the pool the hit part feeds and pain, after the shot the
// stepper shows, with what that shot took and when the pool kills or breaks.
function drawPools(target, sim) {
  const box = $("pools"); box.innerHTML = "";
  const routes = (target.rows[state.part].routes || {}).ranged || {};
  const own = Object.keys(routes).find(p => p !== "pain" && p in target.max);
  const n = clamp(state.dollShot || 1, 1, sim.history.length - 1);
  const before = sim.history[n - 1], after = sim.history[n];
  for (const p of [own, "pain"].filter(Boolean)) {
    const max = target.max[p], left = after[p], lost = Math.max(0, before[p] - left);
    const zero = sim.events.find(e => e.pool === p && e.threshold === 0);
    const kills = zero && zero.affects.includes(0);
    const tag = zero && n >= zero.shot ? el("span", { class: "tag " + (kills ? "kill" : "cripple"),
      text: `${kills ? T("kills", "вбиває") : T("breaks", "ламає")} @${zero.shot}` }) : null;
    box.append(el("div", { class: "dpool", style: `color:${poolColor(p)}` },
      el("span", { class: "name" }, POOLS[p], tag),
      el("span", { class: "t", html: `${hp(left)}<small> / ${hp(max)}</small>${lost > 0 ? ` <small>−${hp(lost)}</small>` : ""}` }),
      el("div", { class: "bar", html: `<span style="width:${left / max * 100}%"></span><em style="left:${left / max * 100}%;width:${lost / max * 100}%"></em>` })));
  }
  const ev = $("events"); ev.innerHTML = "";
  for (const e of sim.events) {
    const below = T("below ", "нижче "), empty = T("empty", "на нулі"), move = T("movement", "рух");
    const what = e.affects.includes(0) ? `${POOLS[e.pool]} ${empty}: ${AFFECTS[0]}` :
      e.affects.length ? `${POOLS[e.pool]} ${e.threshold ? below + pct(e.threshold) : empty}: ${e.affects.map(x => AFFECTS[x] || x).join(", ")}${e.movement ? `, ${move} ${pct(e.movement)}` : ""}` :
      e.movement ? `${POOLS[e.pool]} ${below}${pct(e.threshold)}: ${move} ${pct(e.movement)}` :
      `${POOLS[e.pool]} ${below}${pct(e.threshold)}`;
    if (e.pool === "pain" && !e.affects.length) continue;
    ev.append(el("li", { html: `<b class="num">${T("shot", "постріл")} ${e.shot}</b> · ${what}` }));
  }
}

function drawChart(w, a, target) {
  const host = $("chart");
  const max = Number($("dist").max);
  const W = 520, H = 200, L = 34, R = 10, Tp = 10, B = 24;
  const A = (target.armour[state.part] || {}).ranged || 0;
  const pts = [];
  for (let d = 0; d <= max; d += 2) {
    const b = bulletAt(w, a, d);
    pts.push({ d, raw: b.D * b.pellets, hit: b.D * armourFactor(A, b.P) * b.pellets });
  }
  const ymax = Math.max(...pts.map(p => p.raw)) * 1.1 || 1;
  const x = d => L + d / max * (W - L - R), y = v => Tp + (1 - v / ymax) * (H - Tp - B);
  const line = k => pts.map((p, i) => (i ? "L" : "M") + x(p.d).toFixed(1) + " " + y(p[k]).toFixed(1)).join("");
  const b0 = bulletAt(w, a, 0);
  let grid = "";
  const step = ymax > 1.5 ? 0.5 : ymax > 0.6 ? 0.2 : 0.1;
  for (let v = 0; v <= ymax; v += step) grid += `<line x1="${L}" x2="${W - R}" y1="${y(v)}" y2="${y(v)}"/>`;
  let ax = "";
  for (let v = 0; v <= ymax; v += step) ax += `<text x="${L - 6}" y="${y(v) + 3}" text-anchor="end">${hp(v)}</text>`;
  for (let m = 0; m <= max; m += 25) ax += `<text x="${x(m)}" y="${H - 6}" text-anchor="middle">${m} ${M}</text>`;
  const cur = bulletAt(w, a, state.dist);
  host.innerHTML = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${T("Damage over distance", "Пошкодження за дистанцією")}">
    <rect class="zone" x="${x(Math.min(b0.r0, max))}" y="${Tp}" width="${Math.max(0, x(Math.min(b0.r1, max)) - x(Math.min(b0.r0, max)))}" height="${H - Tp - B}"/>
    <g class="grid">${grid}</g><g class="axis">${ax}</g>
    <path class="lineA" d="${line("raw")}"/><path class="lineB" d="${line("hit")}"/>
    <line class="cursor" x1="${x(Math.min(state.dist, max))}" x2="${x(Math.min(state.dist, max))}" y1="${Tp}" y2="${H - B}"/>
    <circle class="dotA" cx="${x(Math.min(state.dist, max))}" cy="${y(cur.D * cur.pellets)}" r="4"/>
    <circle class="dot" cx="${x(Math.min(state.dist, max))}" cy="${y(cur.D * armourFactor(A, cur.P) * cur.pellets)}" r="4.5"/>
    <rect x="${L}" y="0" width="${W - L - R}" height="${H}" fill="transparent" id="chart-hit"/></svg>
    <div class="tip" id="tip" hidden></div>`;
  const svg = host.querySelector("svg"), tip = $("tip");
  const toD = ev => { const r = svg.getBoundingClientRect(); const px = (ev.clientX - r.left) / r.width * W; return clamp(Math.round((px - L) / (W - L - R) * max), 0, max); };
  $("chart-hit").addEventListener("pointermove", ev => {
    const d = toD(ev), b = bulletAt(w, a, d), r = svg.getBoundingClientRect();
    tip.hidden = false; tip.style.left = (x(d) / W * r.width) + "px"; tip.style.top = (y(b.D * b.pellets) / H * r.height) + "px";
    tip.textContent = `${d} ${M} · ${hp(b.D * b.pellets)} → ${hp(b.D * armourFactor(A, b.P) * b.pellets)} ${T("after armour", "після броні")}`;
  });
  $("chart-hit").addEventListener("pointerleave", () => { tip.hidden = true; });
  $("chart-hit").addEventListener("click", ev => { state.dist = toD(ev); update(); });
}

function drawParts(target) {
  const t = $("parts");
  const pools = ["body", "head", "pain"];
  let h = `<thead><tr><th>${T("Body part", "Частина тіла")}</th><th>${T("Armour", "Броня")}</th><th>${T("Stopped", "Зупинено")}</th>${pools.map(p => `<th>${POOLS[p]}</th>`).join("")}<th>${T("Limb", "Кінцівка")}</th><th>${T("Hits to kill", "Влучань до смерті")}</th><th>${T("Also", "Також")}</th></tr></thead><tbody>`;
  for (const part of Object.keys(PARTS)) {
    const { b, sim } = shotFor(state.weapon, state.ammo, target, part);
    const A = (target.armour[part] || {}).ranged || 0;
    const limb = Object.keys(sim.firstShot).find(p => /leg|hand/.test(p) && sim.firstShot[p] > 0);
    const side = sim.events.filter(e => /leg|hand/.test(e.pool) && e.threshold === 0).map(e => `<span class="tag cripple">${AFFECTS[e.affects[0]] || T("injury", "травма")} @${e.shot}</span>`).join(" ");
    h += `<tr class="click${part === state.part ? " me" : ""}" data-part="${part}"><td>${PARTS[part]}</td><td class="n">${hp(A)}</td>
      <td class="n">${pct(1 - armourFactor(A, b.P))}</td>
      ${pools.map(p => `<td class="n">${sim.firstShot[p] > 0 ? hp(sim.firstShot[p]) : "·"}</td>`).join("")}
      <td class="n">${limb ? hp(sim.firstShot[limb]) : "·"}</td>
      <td class="n"><b>${isFinite(sim.shots) ? sim.shots : "∞"}</b> <span class="note">${sim.killer ? POOLS[sim.killer].toLowerCase() : ""}</span></td><td>${side}</td></tr>`;
  }
  t.innerHTML = h + "</tbody>";
  t.querySelectorAll("tr.click").forEach(r => r.addEventListener("click", () => { state.part = r.dataset.part; update(); }));
}

function drawCompare(target) {
  const modes = [["best", T("best ammo", "найкращий набій")], ["default", T("default ammo", "стандартний набій")]];
  const cm = $("cmpmode"); cm.innerHTML = "";
  for (const [k, l] of modes) cm.append(el("button", { class: "chip", "aria-pressed": String(state.cmpAmmo === k), text: l, onclick: () => { state.cmpAmmo = k; update(); } }));
  const rows = [];
  for (const [key, w0] of Object.entries(DATA.weapons)) {
    if (w0.premium) continue;
    const cands = state.cmpAmmo === "best" ? ammoFor(key) : [defaultAmmo(key)];
    let best = null;
    for (const ak of cands) {
      const r = shotFor(key, ak, target, state.part);
      const f = fatalHit(r.sim);
      const t = ttk(r.w, r.sim.shots);
      const cand = { key, ak, w: r.w, b: r.b, sim: r.sim, f, t };
      if (!best || r.sim.shots < best.sim.shots || (r.sim.shots === best.sim.shots && f.v > best.f.v)) best = cand;
    }
    rows.push(best);
  }
  const K = state.sort.k, dir = state.sort.dir;
  const val = r => K === "name" ? nm(r.w) : K === "ammo" ? nm(DATA.ammo[r.ak]) : K === "level" ? r.w.item_level || 0 :
    K === "dmg" ? -r.f.v : K === "ttk" ? r.t : K === "rpm" ? -(r.w.rounds_per_minute || 0) : K === "pen" ? -r.b.P : r.sim.shots;
  rows.sort((a, b) => { const x = val(a), y = val(b); return (typeof x === "string" ? x.localeCompare(y) : (x - y) || (a.t - b.t)) * dir; });
  const th = (k, l) => `<th data-k="${k}">${l}${K === k ? (dir > 0 ? " ▲" : " ▼") : ""}</th>`;
  let h = `<thead><tr>${th("name", T("Weapon", "Зброя"))}${th("level", T("Level", "Рівень"))}${th("ammo", T("Ammo", "Набій"))}${th("pen", T("Pen. here", "Бронеб. тут"))}${th("dmg", T("Damage / hit", "Пошк. / влуч."))}${th("shots", T("Hits to kill", "Влучань до смерті"))}${th("rpm", T("RPM", "Темп"))}${th("ttk", T("Time to kill", "Час до смерті"))}</tr></thead><tbody>`;
  for (const r of rows) {
    h += `<tr class="click${r.key === state.weapon ? " me" : ""}" data-w="${r.key}" data-a="${r.ak}"><td class="wname"><img class="wico" alt="" data-icon="${r.key}">${nm(r.w)}</td><td class="n">${r.w.item_level ?? "—"}</td><td>${nm(DATA.ammo[r.ak])}</td>
      <td class="n">${hp(r.b.P)}</td><td class="n">${hp(r.f.v)}</td><td class="n"><b>${isFinite(r.sim.shots) ? r.sim.shots : "∞"}</b></td>
      <td class="n">${r.w.rounds_per_minute || "—"}</td><td class="n">${isFinite(r.t) ? fx(r.t, 2) + T(" s", " с") : "—"}</td></tr>`;
  }
  const t = $("cmp"); t.innerHTML = h + "</tbody>";
  // The lobby icons, by reference: not copied into the table's markup.
  t.querySelectorAll("img.wico").forEach(img => {
    const src = DATA.icons && DATA.icons[img.dataset.icon];
    if (src) img.src = src; else img.remove();
  });
  t.querySelectorAll("th[data-k]").forEach(c => c.addEventListener("click", () => {
    state.sort = { k: c.dataset.k, dir: state.sort.k === c.dataset.k ? -state.sort.dir : 1 }; update();
  }));
  t.querySelectorAll("tr.click").forEach(r => r.addEventListener("click", () => {
    state.weapon = r.dataset.w; state.ammo = r.dataset.a; window.scrollTo({ top: 0, behavior: "smooth" }); update();
  }));
}

// The rules in words, with the numbers taken from the data where they live.
function drawExplain() {
  const onyx = DATA.artefacts && DATA.artefacts.onyx, pk = DATA.drugs && DATA.drugs.painkiller;
  $("explain").textContent = (LANG === "uk" ? [
    "D, P  = зброя × набій, менші між ближньою й дальньою дистанцією",
    onyx ? `D, P ×= ${onyx.passive.damage_mul} за кожен ${nm(onyx)} (${onyx.active.damage_mul} активний, один раз на всі)` : null,
    "hit   = D × (1 − clamp(броня − P, 0, 1))",
    "пул  −= hit       груди, спина → тіло · голова, обличчя → голова · нога · кисть",
    `біль −= hit × k   k = 1 груди, голова, руки · 0.9 ноги · 0.8 кисті, стопи${pk ? ` · ×${pk.damage_protection[0].hit_coeff} ${nm(pk)}` : ""}`,
    "смерть, коли тіло, голова або біль доходить до 0",
    "",
    "одна куля крізь кілька зон (дві кисті й груди): кожна зона рахується зі своєю бронею,",
    "а кожен пул бере від цієї кулі найбільше, не суму: біль від кистей 40 і 40, від грудей 35 → −40, не −115",
  ] : [
    "D, P  = weapon × ammo, lowered between near and far range",
    onyx ? `D, P ×= ${onyx.passive.damage_mul} per ${nm(onyx)} (${onyx.active.damage_mul} active, once for all)` : null,
    "hit   = D × (1 − clamp(armour − P, 0, 1))",
    "pool −= hit       chest, back → body · head, face → head · leg · hand",
    `pain −= hit × k   k = 1 chest, head, arms · 0.9 legs · 0.8 hands, feet${pk ? ` · ×${pk.damage_protection[0].hit_coeff} ${nm(pk)}` : ""}`,
    "dead when body, head or pain reaches 0",
    "",
    "one bullet through several boxes (both hands and the chest): each box counts with its own armour,",
    "and each pool takes that bullet's largest share, not the sum: pain 40 and 40 from the hands, 35 from the chest → −40, not −115",
  ]).filter(x => x !== null).join("\n");
}

// How many Larkspurs the healing table counts (its own tiles), 1 to 4.
const larkspurs = () => clamp(state.larkspur || 1, 1, 4);
function fillHealPick() {
  const lark = DATA.artefacts && DATA.artefacts.lifebone, host = $("healpick");
  if (!lark) { host.hidden = true; return; }
  const src = DATA.icons && DATA.icons.artefact_lifebone;
  const tiles = [1, 2, 3, 4].map(i => {
    const b = el("button", { type: "button", class: "fxtile", title: `${nm(lark)} ×${i}`, "aria-label": `${nm(lark)} ×${i}`, "aria-pressed": String(i <= larkspurs()) },
      src ? el("img", { src, alt: "" }) : el("span", { text: `×${i}` }));
    b.addEventListener("click", () => {
      state.larkspur = i; save();
      tiles.forEach((t, j) => t.setAttribute("aria-pressed", String(j < i)));
      drawHealing();
    });
    return b;
  });
  host.append(el("span", { class: "lbl", text: nm(lark) }), ...tiles);
}
// Healing compared: recovering by itself, the Larkspur carried and activated,
// the medikit and bandages, every number from the data.
function drawHealing() {
  const row = n => DATA.body.find(r => r.name === n);
  const pools = [["body"], ["head"], ["right_leg", "right_hand"], ["pain"]];
  const perS = T("/s", "/с"), sec = T("s", "с"), dash = "—";
  const full = T("full", "повністю"), fixed = T("fixed", "лікує");
  const regen = (name, mod, count = 0) => {
    const r = row(name);
    let speed = r.regeneration_speed, wait = r.regeneration_timeout, cap = r.regeneration_threshold;
    // One Larkspur callback (0x312590) per Larkspur carried: add, then multiply.
    for (let i = 0; mod && i < count; i++) {
      speed = (speed + mod.regen_add) * mod.regen_mul;
      wait = (wait + mod.timeout_add) * mod.timeout_mul;
      cap = (cap + mod.threshold_add) * mod.threshold_mul;
    }
    return { cell: `${hp(speed)}${perS} → ${cap >= 1 ? full : hp(r.health * cap)}`, wait };
  };
  const rows = [];
  const own = pools.map(([p]) => regen(p));
  // A row's name with its item's lobby icon.
  const named = (text, key) => { const src = key && DATA.icons && DATA.icons[key]; return (src ? `<img class="wico" alt="" src="${src}">` : "") + `<b>${text}</b>`; };
  rows.push([named(T("By itself", "Само собою")), T(`${own[0].wait} s without hits`, `${own[0].wait} с без влучань`), ...own.map(x => x.cell),
    T(`ends after ${own[0].wait} s without hits`, `минає за ${own[0].wait} с без влучань`)]);
  const lark = DATA.artefacts && DATA.artefacts.lifebone;
  if (lark) {
    const mods = Object.fromEntries((lark.passive.regeneration_modifiers || []).map(m => [m.body_part, m]));
    const n = larkspurs();
    const carried = pools.map(([p]) => regen(p, mods[p], n));
    rows.push([named(`${nm(lark)} ×${n}, ${T("carried", "у слоті")}`, "artefact_lifebone"), T(`${carried[0].wait} s without hits`, `${carried[0].wait} с без влучань`), ...carried.map(x => x.cell), dash]);
    const cures = (lark.active.remove_affects || []).length;
    rows.push([named(`${nm(lark)}, ${T("activated", "активований")}`, "artefact_lifebone"), T(`at once, every ${lark.cooldown_s} s`, `одразу, раз на ${lark.cooldown_s} с`), dash, dash, dash, dash, cures ? fixed : dash]);
  }
  for (const key of ["medkit", "military_medkit", "blackmarket_medkit", "science_medkit", "bandages"]) {
    const d = DATA.drugs && DATA.drugs[key]; if (!d) continue;
    const heal = d.heal_per_s || {};
    const cell = ([p]) => {
      const gain = (heal[p] || 0) * d.activity_s;
      return gain <= 0 ? dash : gain >= row(p).health ? full : `+${hp(gain)}`;
    };
    rows.push([named(nm(d), key), T(`${d.activation_delay_s} s after use, for ${d.activity_s} s`, `через ${d.activation_delay_s} с, ${d.activity_s} с`),
      ...pools.map(cell), (d.remove_affects || []).length ? fixed : dash]);
  }
  // From empty to full: the wait, then the pool at its speed. Each Larkspur
  // adds speed and wait alike, so past two the wait outgrows what it saves.
  if (lark) {
    const mods = Object.fromEntries((lark.passive.regeneration_modifiers || []).map(m => [m.body_part, m]));
    const cols = [["body"], ["right_leg"], ["pain"]];
    const timeTo = (p, k) => {
      const r = row(p), m = mods[p];
      let speed = r.regeneration_speed, wait = r.regeneration_timeout, cap = r.regeneration_threshold;
      for (let i = 0; m && i < k; i++) { speed = (speed + m.regen_add) * m.regen_mul; wait = (wait + m.timeout_add) * m.timeout_mul; cap = (cap + m.threshold_add) * m.threshold_mul; }
      return cap >= 1 ? wait + r.health / speed : { stops: r.health * cap };
    };
    const table = [0, 1, 2, 3, 4].map(k => cols.map(([p]) => timeTo(p, k)));
    const best = cols.map((_, c) => Math.min(...table.map(t => typeof t[c] === "number" ? t[c] : Infinity)));
    const fcell = (v, c) => typeof v === "number" ? (Math.abs(v - best[c]) < 1e-9 ? `<b>${fx(v, 1)} ${sec}</b>` : `${fx(v, 1)} ${sec}`)
      : T(`stops at ${hp(v.stops)}`, `до ${hp(v.stops)}`);
    const n = larkspurs();
    $("fulltime").innerHTML = `<thead><tr><th>${T("From empty to full", "Від нуля до повного")}</th>${[POOLS.body, T("Leg or hand", "Нога чи кисть"), POOLS.pain].map(h => `<th>${h}</th>`).join("")}</tr></thead><tbody>` +
      table.map((t, k) => `<tr${k === n ? ' class="me"' : ""}><td>${k ? `${nm(lark)} ×${k}` : T("No Larkspur", "Без Живокосту")}</td>${t.map((v, c) => `<td class="n">${fcell(v, c)}</td>`).join("")}</tr>`).join("") + "</tbody>";
  }
  const head = ["", T("Starts", "Починає"), POOLS.body, POOLS.head, T("Legs, hands", "Ноги, кисті"), POOLS.pain, T("Broken leg, hand", "Зламана нога, кисть")];
  $("heal").innerHTML = `<thead><tr>${head.map(h => `<th>${h}</th>`).join("")}</tr></thead><tbody>` +
    rows.map(r => `<tr><td class="hname">${r[0]}</td>${r.slice(1).map(c => `<td>${c}</td>`).join("")}</tr>`).join("") + "</tbody>";
}

// ---------- map ----------
// Top-down renders on the in-game minimap's square: pixel <-> world metres is
// one affine per map (image right = +x, up = +z). Heights are a 64x64 grid of
// ground level, so the distance includes the slope (not floors in buildings).
const affine = (m, x, y) => [m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]];
function groundAt(map, x, z) {
  const e = map.elevation; if (!e) return 0;
  const fc = (x - map.bounds.x_min) / e.cell_m - 0.5, fr = (map.bounds.z_max - z) / e.cell_m - 0.5;
  const c0 = clamp(Math.floor(fc), 0, e.grid - 1), r0 = clamp(Math.floor(fr), 0, e.grid - 1);
  const c1 = Math.min(c0 + 1, e.grid - 1), r1 = Math.min(r0 + 1, e.grid - 1);
  const tx = clamp(fc - c0, 0, 1), ty = clamp(fr - r0, 0, 1), h = e.heights;
  return (h[r0][c0] * (1 - tx) + h[r0][c1] * tx) * (1 - ty) + (h[r1][c0] * (1 - tx) + h[r1][c1] * tx) * ty;
}
function fillMaps() {
  if (!DATA.maps) { $("h-map").closest("section").hidden = true; return; }
  const s = $("mapsel");
  for (const [k, m] of Object.entries(DATA.maps)) s.append(el("option", { value: k, text: nm(m) }));
  if (!DATA.maps[state.map]) state.map = Object.keys(DATA.maps)[0];
  s.value = state.map;
  s.addEventListener("change", () => { state.map = s.value; state.mapA = state.mapB = null; drawMapBase(); update(); });
  $("mapclear").addEventListener("click", () => { state.mapA = state.mapB = null; update(); });
  // The render or the game's minimap (same frame), where the map has one.
  const views = $("mapviews");
  const paintViews = () => {
    views.hidden = !DATA.maps[state.map].minimap;
    for (const c of views.children) c.setAttribute("aria-pressed", String(c.dataset.view === state.mapView));
  };
  for (const [v, text] of [["minimap", T("Minimap", "Мінімапа")], ["render", T("From above", "Згори")]]) {
    const c = el("button", { type: "button", class: "chip", text });
    c.dataset.view = v;
    c.addEventListener("click", () => { state.mapView = v; paintViews(); drawMapBase(); update(); });
    views.append(c);
  }
  s.addEventListener("change", paintViews);
  paintViews();
  drawMapBase();
}
function drawMapBase() {
  const m = DATA.maps[state.map];
  // The play area (the native minimap's part of the render), not the whole square.
  const [vx, vy, vw, vh] = m.play || [0, 0, m.size[0], m.size[1]];
  $("mapview").innerHTML = `<svg viewBox="${vx} ${vy} ${vw} ${vh}" style="width:min(100%, calc(90vh * ${vw / vh}))" role="img" aria-label="${T("Top-down view of", "Вигляд згори:")} ${nm(m)}">
    <image href="${state.mapView === "minimap" && m.minimap ? m.minimap : m.image}" width="${m.size[0]}" height="${m.size[1]}" preserveAspectRatio="none"/><g id="mapover"></g></svg>`;
  const svg = $("mapview").querySelector("svg");
  svg.addEventListener("click", ev => {
    const r = svg.getBoundingClientRect();
    const px = vx + (ev.clientX - r.left) / r.width * vw, py = vy + (ev.clientY - r.top) / r.height * vh;
    const [x, z] = affine(m.p2w, px, py);
    if (!state.mapA || state.mapB) { state.mapA = [x, z]; state.mapB = null; }
    else state.mapB = [x, z];
    update();
  });
}
function drawMapOver(b) {
  if (!DATA.maps) return;
  const m = DATA.maps[state.map], g = $("mapover"); if (!g) return;
  const ppm = 1 / m.mpp;
  let h = "";
  const P = w => affine(m.w2p, w[0], w[1]);
  if (state.mapA) {
    const [ax, ay] = P(state.mapA);
    h += `<circle class="ring min" cx="${ax}" cy="${ay}" r="${b.r1 * ppm}"/><circle class="ring full" cx="${ax}" cy="${ay}" r="${b.r0 * ppm}"/>`;
    if (state.mapB) {
      const [bx, by] = P(state.mapB);
      h += `<line class="shotline" x1="${ax}" y1="${ay}" x2="${bx}" y2="${by}"/>`;
      h += `<circle class="pt b" cx="${bx}" cy="${by}" r="12"/><text x="${bx + 18}" y="${by - 14}">${mapDistance()} ${M}</text>`;
    }
    h += `<circle class="pt a" cx="${ax}" cy="${ay}" r="12"/>`;
  }
  g.innerHTML = h;
  const read = $("mapread");
  // Under the map, two lines whatever the state: what to do or what was
  // measured, then the rings' legend. The map never moves.
  let line;
  if (state.mapA && state.mapB) {
    const [x1, z1] = state.mapA, [x2, z2] = state.mapB;
    const flat = Math.hypot(x2 - x1, z2 - z1);
    const dh = groundAt(m, x2, z2) - groundAt(m, x1, z1);
    const slant = Math.hypot(flat, dh), sign = dh >= 0 ? "+" : "−";
    line = T(`<b>${slant.toFixed(1)} m</b> (${flat.toFixed(1)} across, ${sign}${Math.abs(dh).toFixed(1)} height). Click again to start over.`,
      `<b>${slant.toFixed(1)} м</b> (${flat.toFixed(1)} по горизонталі, ${sign}${Math.abs(dh).toFixed(1)} по висоті). Клікніть ще раз, щоб почати знову.`);
  } else line = state.mapA ? T("Now click the target.", "Тепер клікніть ціль.") : T("Click the shooter, then the target.", "Клікніть стрільця, потім ціль.");
  read.innerHTML = line + "<br>" + T(`Orange ring: full damage to ${b.r0.toFixed(0)} m; dashed: minimum from ${b.r1.toFixed(0)} m.`,
    `Помаранчеве коло: повне пошкодження до ${b.r0.toFixed(0)} м; пунктирне: мінімум від ${b.r1.toFixed(0)} м.`);
}
// Move the target along the shooter's line until the slant distance is `dist`.
function slideMapTarget(dist) {
  if (!DATA.maps || !state.mapA || !state.mapB) return;
  const m = DATA.maps[state.map], [x1, z1] = state.mapA;
  const flat0 = Math.hypot(state.mapB[0] - x1, state.mapB[1] - z1);
  if (!(flat0 > 0)) return;
  const ux = (state.mapB[0] - x1) / flat0, uz = (state.mapB[1] - z1) / flat0, g0 = groundAt(m, x1, z1);
  let flat = dist;
  for (let i = 0; i < 6; i++) flat = Math.sqrt(Math.max(0, dist * dist - (groundAt(m, x1 + ux * flat, z1 + uz * flat) - g0) ** 2));
  state.mapB = [x1 + ux * flat, z1 + uz * flat];
  mapSig = JSON.stringify([state.map, state.mapA, state.mapB]);
}
// The measured distance, rounded to the metre the slider uses.
function mapDistance() {
  if (!DATA.maps || !state.mapA || !state.mapB) return null;
  const m = DATA.maps[state.map], [x1, z1] = state.mapA, [x2, z2] = state.mapB;
  return Math.round(Math.hypot(Math.hypot(x2 - x1, z2 - z1), groundAt(m, x2, z2) - groundAt(m, x1, z1)));
}

// ---------- wiring ----------
// The template's own text: data-uk holds the Ukrainian, data-uk-label an aria-label.
if (LANG === "uk") {
  document.documentElement.lang = "uk";
  document.title = "Калькулятор шкоди Survarium";
  document.querySelectorAll("[data-uk]").forEach(n => { n.textContent = n.dataset.uk; });
  document.querySelectorAll("[data-uk-label]").forEach(n => n.setAttribute("aria-label", n.dataset.ukLabel));
}
fillWeapons(); fillArmour(); drawDoll(); fillMaps(); fillHealPick();
if (DATA.stamp) $("source").textContent = T(DATA.stamp, DATA.stamp_uk || DATA.stamp);
$("dollprev").addEventListener("click", () => { state.dollShot = (state.dollShot || 1) - 1; update(); });
$("dollnext").addEventListener("click", () => { state.dollShot = (state.dollShot || 1) + 1; update(); });
if (RULES.renderExtras) RULES.renderExtras(state, update);
$("weapon").addEventListener("change", e => { state.weapon = e.target.value; state.ammo = defaultAmmo(state.weapon); update(); });
$("ammo").addEventListener("change", e => { state.ammo = e.target.value; update(); });
$("dist").addEventListener("input", e => { state.dist = Number(e.target.value); slideMapTarget(state.dist); update(); });
$("pain").addEventListener("input", e => { state.pain = Number(e.target.value); update(); });
if (!DATA.weapons[state.weapon]) state.weapon = Object.keys(DATA.weapons)[0];
update();
