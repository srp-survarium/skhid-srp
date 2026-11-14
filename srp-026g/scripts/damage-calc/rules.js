// What bonuses, skills, artefacts and drugs do, as the client applies them.
// Every hook is a no-op until something is chosen.
const RULES = (() => {
  const ART = DATA.artefacts || {};

  // Onyx, 0x312bf0: one callback per carried Onyx, before armour, on the
  // locations it protects. Passive (state 3) and active (state 4) never both.
  // Exe patch 54: one press fires every ready Onyx, each active 1 s longer
  // per other one, and only the first active one reduces a hit.
  const ONYX_EXTRA_S = 1;
  function onyxCallbacks(count, active) {
    const o = ART.onyx; if (!o || !count) return [];
    const mode = active ? o.active : o.passive;
    const out = [];
    for (let i = 0; i < (active ? 1 : count); i++) {
      out.push({
        name: `${nm(o)} ${active ? T("active", "активний") : T("passive", "пасивний")}${count > 1 && !active ? ` #${i + 1}` : ""}`,
        parts: o.protected_locations, types: [o.hit_type],
        fn(D, P) {
          let nd;
          if (active) {
            const want = (D + mode.damage_add) * mode.damage_mul;
            nd = D - Math.min(mode.absorb_total, D - want);
          } else nd = (D + mode.damage_add) * mode.damage_mul;
          const np = (P + mode.armor_piercing_add) * mode.armor_piercing_mul;
          return { D: nd, P: np, note: T(`Damage ×${mode.damage_mul}, penetration ×${mode.armor_piercing_mul}, before armour: lower penetration makes the armour stop more.`,
            `Пошкодження ×${mode.damage_mul}, бронебійність ×${mode.armor_piercing_mul}, до броні: з меншою бронебійністю броня зупиняє більше.`) };
        },
      });
    }
    return out;
  }

  // Drugs, 0x33db30: on their pool and hit type, D = 0 below the threshold,
  // else (D - threshold) * hit_coeff (modifier 58 is 0 in retail).
  function drugCallbacks(active) {
    const out = [];
    for (const [key, drug] of Object.entries(DATA.drugs || {})) {
      if (!active[key]) continue;
      for (const p of drug.damage_protection) {
        out.push({
          name: nm(drug), parts: [p.pool], types: [p.hit_type],
          fn(D, P) {
            const nd = D < p.threshold ? 0 : (D - p.threshold) * p.hit_coeff;
            const pool = POOLS[p.pool] || p.pool, total = drug.activation_delay_s + drug.activity_s;
            return { D: nd, P, note: T(`${nm(drug)}: ${pool} takes ×${p.hit_coeff} for ${total} s after use (${drug.activation_delay_s} s to kick in).`,
              `${nm(drug)}: ${pool.toLowerCase()} отримує ×${p.hit_coeff} протягом ${total} с після вживання (діє через ${drug.activation_delay_s} с).`) };
          },
        });
      }
    }
    return out;
  }

  // Forest Protection (523), modifier 41: a player's hit on someone inside an
  // anomaly's damage zone is x max(0, 1 - 0.05 a rank), before armour
  // (0x332810). On the hit's location only, not again on the pools.
  function forestCallbacks(rank) {
    const s = (DATA.skills || {}).forest_protection; if (!s || !rank) return [];
    const k = +Math.max(0, 1 - clamp(rank, 0, s.ranks) * s.step).toFixed(2);
    return [{
      name: nm(s), parts: Object.keys(PARTS),
      fn(D, P) { return { D: D * k, P, note: T(`Inside an anomaly: damage ×${k}, before armour.`, `В аномалії: пошкодження ×${k}, до броні.`) }; },
    }];
  }

  // Modifications (item_config_cook 0x2f1750): values on one path are summed,
  // then applied once: "mul" as v*max(0,1+sum), "add" as v+sum.
  const MODS = DATA.mods || {};
  // Defects are left out: the page only adds bonuses. An item with presets
  // (premium weapons) has those and nothing else.
  const idsOf = (item, pick = {}) => item.mod_types.default.length ? [...item.mod_types.default]
    : Object.values(pick.mod || {}).filter(Boolean);
  function sums(ids) {
    const out = {};
    for (const id of ids) for (const e of MODS[id].effects) {
      out[e.path] = out[e.path] || { op: e.op, sum: 0 };
      out[e.path].sum += e.value;
    }
    return out;
  }
  const apply = (v, s) => s.op === "mul" ? v * Math.max(0, 1 + s.sum) : v + s.sum;

  // The lobby's modification rows (ModifierRow, InventoryItemHint): icon,
  // "<prop>: <value>", the value in the bonus green.
  const propText = (prop, v) => {
    const n = prop.mod_type === 1 ? `${v >= 0 ? "+" : "−"}${Math.abs(v)}` : `${v >= 0 ? "+" : "−"}${Math.round(Math.abs(v) * 100)}%`;
    return n;
  };
  const label = prop => (LANG === "uk" && prop.label_uk) || prop.label;
  const good = (prop, v) => (prop.direction === 0 && v < 0) || (prop.direction === 1 && v > 0);
  const icon = m => {
    const src = DATA.mod_icons && DATA.mod_icons[m.icon];
    return src ? el("img", { class: "gico", src, alt: "" }) : el("span", { class: "gico" });
  };
  function modControls(host, item, pick, onChange) {
    const kinds = item.mod_types;
    if (!kinds.mod.length && !kinds.default.length) return false;
    const panel = el("div", { class: "gamemods" });
    for (const id of kinds.default) {
      const m = MODS[id], prop = m.props[0] || {};
      panel.append(el("div", { class: "gmrow fixed" }, icon(m), el("span", { class: "glabel", text: `${label(prop)}:` }),
        el("span", { class: good(prop, prop.value) ? "gv good" : "gv bad", text: propText(prop, prop.value) })));
    }
    if (kinds.default.length) { host.append(panel); return true; }
    for (const t of kinds.mod) {
      const options = Object.keys(MODS).filter(id => MODS[id].kind === "mod" && MODS[id].type === t)
        .sort((a, b) => MODS[a].effects[0].value - MODS[b].effects[0].value);
      const m0 = MODS[options[0]], prop = m0.props[0] || {};
      // One button per grade; clicking the picked one clears it.
      const steps = el("span", { class: "gsteps", role: "group", "aria-label": label(prop) });
      const paint = () => { for (const b of steps.children) b.setAttribute("aria-pressed", String(b.dataset.id === ((pick.mod || {})[t] || ""))); };
      for (const id of options) {
        const b = el("button", { type: "button", class: "gstep", text: propText(prop, MODS[id].props[0].value) });
        b.dataset.id = id;
        b.addEventListener("click", () => { pick.mod = pick.mod || {}; pick.mod[t] = pick.mod[t] === id ? "" : id; paint(); onChange(); });
        steps.append(b);
      }
      paint();
      panel.append(el("div", { class: "gmrow" }, icon(m0), el("span", { class: "glabel", text: `${label(prop)}:` }), steps));
    }
    host.append(panel);
    return true;
  }

  // Covered parts in the doll's order, a left and right of the same armour
  // as one ("Arms 50").
  const PAIRS = [["right_forearm", "left_forearm", ["Arms", "Руки"]], ["right_arm", "left_arm", ["Hands", "Кисті"]],
    ["right_hip", "left_hip", ["Legs", "Ноги"]], ["right_foot", "left_foot", ["Feet", "Стопи"]]];
  const partsName = parts => parts.length > 1 ? T(...PAIRS.find(([r]) => r === parts[0])[2]) : PARTS[parts[0]];
  function protParts(cover) {
    const v = p => (cover[p] || {}).ranged || 0;
    const out = [];
    for (const p of Object.keys(PARTS)) {
      if (!(v(p) > 0) || out.some(([ps]) => ps.includes(p))) continue;
      const pair = PAIRS.find(([r, l]) => r === p || l === p);
      const twin = pair && (pair[0] === p ? pair[1] : pair[0]);
      out.push([twin && v(twin) === v(p) ? [pair[0], pair[1]] : [p], v(p)]);
    }
    return out;
  }

  // What an item does, shown over its tile on hover or focus.
  // An item's card as the lobby lists it: its name, then a line each (its
  // icon when the lines have one, "<prop>:", the value; a bonus in its colour).
  function card(name, lines) {
    const icons = lines.some(l => l.icon != null);
    const box = el("div", { class: icons ? "gamemods" : "gamemods plain" }, el("span", { class: "gname", text: name }));
    for (const l of lines)
      box.append(el("div", { class: "gmrow fixed" }, icons ? icon(l) : null, el("span", { class: "glabel", text: `${label(l)}:` }),
        el("span", { class: l.good == null ? "gv" : l.good ? "gv good" : "gv bad", text: (LANG === "uk" && l.value_uk) || l.value })));
    return box;
  }
  // An artefact's: the game's own words at its pickup (the passive) and at
  // its activation, with the active's seconds.
  function artefactCard(a, seconds = (a.active || {}).duration_s) {
    const box = el("div", { class: "gamemods" });
    for (const k of ["pickup_hint", "activation_hint"]) {
      const said = (LANG === "uk" && a[k + "_uk"]) || a[k];
      if (said) box.append(el("span", { class: "gtext", text: said.replace("%s", seconds) }));
    }
    return box;
  }
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
  // A bonus row as the lobby draws it: icon, "<label>:", a button per option
  // [value, text] of picks[key], the picked one again clears it. With a card,
  // its icon and name show it on hover.
  function stepsRow(icon, label, options, picks, key, onChange, box) {
    const group = el("span", { class: "gsteps", role: "group", "aria-label": label });
    const paint = () => { for (const b of group.children) b.setAttribute("aria-pressed", String(b.dataset.v === String(picks[key] || ""))); };
    for (const [v, text] of options) {
      const b = el("button", { type: "button", class: "gstep", text });
      b.dataset.v = v;
      b.addEventListener("click", () => { picks[key] = String(picks[key] || "") === String(v) ? "" : v; paint(); onChange(); });
      group.append(b);
    }
    paint();
    return el("div", { class: "gmrow" }, icon, el("span", { class: box ? "glabel fxhint" : "glabel" }, `${label}:`, box), group);
  }
  // A skill's row (null when the game has no such skill): its lobby icon, its
  // name with its card (the lobby tooltip's line for a rank), a button per
  // rank saying what that many give.
  function skillRow(key, sign, unit, picks, onChange) {
    const s = (DATA.skills || {})[key]; if (!s) return null;
    const src = DATA.skill_icons && DATA.skill_icons[s.id];
    const box = el("div", { class: "gamemods" }, el("span", { class: "gname", text: nm(s) }),
      el("span", { class: "gtext" }, `${(LANG === "uk" && s.description_uk) || s.description} `, el("span", { class: "gv good", text: (LANG === "uk" && s.rank_uk) || s.rank })));
    const given = n => sign + (unit ? +(n * s.step).toFixed(2) + unit : `${Math.round(n * s.step * 100)}%`);
    return stepsRow(src ? el("img", { class: "gico", src, alt: "" }) : el("span", { class: "gico" }), nm(s),
      Array.from({ length: s.ranks }, (_, i) => [i + 1, given(i + 1)]), picks, key, onChange, box);
  }

  // Recovery: the healing section's picks (DATA.recovery).
  const REC = DATA.recovery || {};
  const LARK = Object.fromEntries((((ART.lifebone || {}).passive || {}).regeneration_modifiers || []).map(m => [m.body_part, m]));

  return {
    card, artefactCard, hinted, stepsRow, skillRow,
    // What the picks add up to: the kit's regeneration bonuses (each slot's
    // grade and the boosters carried: speed in one sum, seconds on the delay,
    // a share on the limit) and each skill's ranks times its step.
    recoveryKit(pick = {}) {
      const kit = { speed: 0, delay_s: 0, limit: 0 };
      for (const id of Object.values(pick.gear || {})) kit.speed += (((REC.gear || {}).grades || []).find(g => g.id === id) || { value: 0 }).value;
      for (const [key, b] of Object.entries(REC.boosters || {}))
        if ((pick.boosters || {})[key]) for (const k of ["speed", "delay_s", "limit"]) kit[k] += b[k] || 0;
      for (const key of ["metabolism", "strong_bones", "medical_training", "take_cover", "fast_bandaging"]) {
        const s = (DATA.skills || {})[key];
        kit[key] = s ? clamp((pick.skills || {})[key] || 0, 0, s.ranks) * s.step : 0;
      }
      return kit;
    },
    // One pool's regeneration, as body_part::regenerate (0x32A4B0) runs it:
    // the delay less Metabolism's seconds (set at the spawn); the speed
    // x max(0, 1 + the kit's bonuses); each Larkspur's callback (0x312590:
    // add, then multiply); then, for a pool that regenerates, the kit's
    // seconds off the delay and its share onto the limit (exe patch 94).
    // Strong Bones raises a leg's or hand's maximum.
    regeneration(row, kit, larkspurs) {
      const m = LARK[row.name];
      let speed = row.regeneration_speed * Math.max(0, 1 + kit.speed);
      let wait = Math.max(0, row.regeneration_timeout - kit.metabolism), cap = row.regeneration_threshold;
      for (let i = 0; m && i < larkspurs; i++) {
        speed = (speed + m.regen_add) * m.regen_mul;
        wait = (wait + m.timeout_add) * m.timeout_mul;
        cap = (cap + m.threshold_add) * m.threshold_mul;
      }
      if (speed > 0) { wait = Math.max(0, wait + kit.delay_s); cap += Math.max(0, 1 + kit.limit) - 1; }
      return { max: row.health * (/_(leg|hand)$/.test(row.name) ? 1 + kit.strong_bones : 1), speed, wait, cap: Math.min(1, cap) };
    },
    // A medicine that heals: Fast Bandaging on the wait before it works,
    // Medical Training (x max(0, 1 + S57), medkit::tick 0x33D790) and Take
    // Cover (while crouched) on what it heals.
    medicine(d, kit) {
      return { delay_s: d.activation_delay_s * (1 - kit.fast_bandaging), heals: Math.max(0, 1 + kit.medical_training) * (1 + kit.take_cover) };
    },
    applyWeaponMods(w, chosen) {
      const s = sums(idsOf(w, chosen));
      for (const [path, v] of Object.entries(s)) {
        const key = path.split("/")[1];
        if (key in w) w[key] = apply(w[key], v);
      }
    },
    // One item's gunshot armour per part, after its modifications.
    itemArmour(item, pick) {
      const s = sums(idsOf(item, pick));
      const out = {};
      for (const [part, types] of Object.entries(item.hit_params)) {
        out[part] = { ...types };
        const v = s[`hit_params/${part}/hit_types/ranged/armor`];
        if (v && "ranged" in types) out[part].ranged = apply(types.ranged, v);
      }
      return out;
    },
    renderMods(state, update) {
      const wbox = document.getElementById("wmods"); wbox.innerHTML = "";
      state.wmods = state.wmods || {};
      const w = DATA.weapons[state.weapon];
      state.wmods[state.weapon] = state.wmods[state.weapon] || {};
      if (!modControls(wbox, w, state.wmods[state.weapon], update)) wbox.append(el("span", { class: "note", text: T("This weapon takes no damage bonuses.", "Ця зброя не має бонусів пошкодження.") }));
      else if (w.mod_types.default.length) wbox.append(el("span", { class: "note", text: T("Come with the weapon; cannot be changed.", "Ідуть зі зброєю; змінити не можна.") }));
      else wbox.append(el("span", { class: "note", text: T("An item holds at most 3 modifications. Bonuses on one stat add up before they apply.", "Предмет має щонайбільше 3 модифікації. Бонуси на одну характеристику додаються.") }));
      // The items on the part being hit: their bonuses are the ones that count.
      const abox = document.getElementById("amods"); abox.innerHTML = "";
      state.amods = state.amods || {};
      const part = PARTS[state.part].toLowerCase();
      let covering = 0;
      for (const [slot, label] of SLOTS) {
        const key = state.armour[slot]; if (!key) continue;
        const item = DATA.armour[key];
        if (!((item.hit_params[state.part] || {}).ranged > 0)) continue;
        covering++;
        state.amods[key] = state.amods[key] || {};
        // The item and its gunshot armour on each part it covers, after the
        // bonuses picked below: the part being hit in bold.
        const prot = el("div", { class: "aprot", id: "aprot-" + slot });
        const paintProt = () => {
          const cover = this.itemArmour(item, state.amods[key]);
          prot.replaceChildren(...protParts(cover).map(([parts, v]) =>
            el("span", { class: parts.includes(state.part) ? "here" : "" }, `${partsName(parts)} `, el("b", { text: hp(v) }))));
        };
        paintProt();
        const src = DATA.icons && DATA.icons[key];
        abox.append(el("div", { class: "aitem" }, src ? el("img", { src, alt: "" }) : null,
          el("div", {}, el("span", { class: "lbl", text: `${label}: ${nm(item)}` }), prot)));
        if (!modControls(abox, item, state.amods[key], () => { paintProt(); update(); }))
          abox.append(el("span", { class: "note", text: T("This item takes no armour bonuses.", "Цей предмет не має бонусів броні.") }));
      }
      if (!covering) abox.append(el("span", { class: "note", text: T(`Nothing worn covers the ${part}.`, `Ніщо вдягнене не захищає: ${part}.`) }));
    },
    targetEffects(state) {
      const fx = state.fx || {};
      return {
        incoming: [...onyxCallbacks(fx.onyx || 0, fx.onyxMode === "active"), ...forestCallbacks((fx.skills || {}).forest_protection),
          ...drugCallbacks(fx.drugs || {})],
        final: [], hmax: {}, secondChance: null,
      };
    },
    renderExtras(state, update) {
      state.fx = state.fx || {};
      const box = document.getElementById("tfx");
      box.innerHTML = "";
      // An item tile: its lobby icon, lit when on.
      const tile = (iconKey, label, on, click) => {
        const src = DATA.icons && DATA.icons[iconKey];
        const b = el("button", { type: "button", class: "fxtile", title: label, "aria-label": label, "aria-pressed": String(on) },
          src ? el("img", { src, alt: "" }) : el("span", { text: label }));
        b.addEventListener("click", click);
        return b;
      };
      const row = (label, ...kids) => box.append(el("div", { class: "field" }, el("span", { class: "lbl", text: label }), el("div", { class: "fxrow" }, ...kids)));
      if (ART.onyx) {
        // Four artefact slots: click the n-th to carry n, the last lit one again to drop it.
        const slots = [1, 2, 3, 4].map(i => tile("artefact_onyx", `${nm(ART.onyx)} ×${i}`, i <= (state.fx.onyx || 0),
          () => {
            state.fx.onyx = (state.fx.onyx || 0) === i ? i - 1 : i;
            slots.forEach((s, j) => s.setAttribute("aria-pressed", String(j < state.fx.onyx)));
            act.textContent = activeText();
            update();
          }));
        const mode = state.fx.onyxMode || "passive";
        const chip = (k, text) => {
          const c = el("button", { type: "button", class: "chip", text, "aria-pressed": String(mode === k) });
          c.addEventListener("click", () => {
            state.fx.onyxMode = k;
            c.parentNode.querySelectorAll(".chip").forEach(x => x.setAttribute("aria-pressed", String(x === c)));
            update();
          });
          return c;
        };
        const activeText = () => `${T("active", "активний")} ×${ART.onyx.active.damage_mul}, `
          + `${ART.onyx.active.duration_s + Math.max(0, (state.fx.onyx || 1) - 1) * ONYX_EXTRA_S} ${T("s", "с")}`;
        const act = chip("active", activeText());
        // Each tile's card: the Onyx's words, the active's seconds for that many.
        row(nm(ART.onyx), ...slots.map((s, i) => hinted(s, artefactCard(ART.onyx, ART.onyx.active.duration_s + i * ONYX_EXTRA_S))), el("span", { class: "chips" },
          chip("passive", `${T("passive", "пасивний")} ×${ART.onyx.passive.damage_mul}`), act));
      }
      const shielding = Object.entries(DATA.drugs || {}).filter(([, d]) => d.damage_protection.some(p => p.hit_type === "ranged"));
      if (shielding.length) {
        state.fx.drugs = state.fx.drugs || {};
        row(T("Drugs", "Препарати"), ...shielding.map(([key, d]) => hinted(tile(key, nm(d), !!state.fx.drugs[key],
          function () { state.fx.drugs[key] = !state.fx.drugs[key]; this.setAttribute("aria-pressed", String(state.fx.drugs[key])); update(); }), card(nm(d), d.card || []))));
      }
      state.fx.skills = state.fx.skills || {};
      const forest = skillRow("forest_protection", "−", null, state.fx.skills, update);
      if (forest) box.append(el("div", { class: "field" }, el("span", { class: "lbl", text: T("Skills", "Навички") }), el("div", { class: "gamemods" }, forest)));
    },
  };
})();
