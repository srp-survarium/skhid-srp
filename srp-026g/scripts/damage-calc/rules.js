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

  return {
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
        incoming: [...onyxCallbacks(fx.onyx || 0, fx.onyxMode === "active"), ...drugCallbacks(fx.drugs || {})],
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
        row(nm(ART.onyx), ...slots, el("span", { class: "chips" },
          chip("passive", `${T("passive", "пасивний")} ×${ART.onyx.passive.damage_mul}`), act));
      }
      const shielding = Object.entries(DATA.drugs || {}).filter(([, d]) => d.damage_protection.some(p => p.hit_type === "ranged"));
      if (shielding.length) {
        state.fx.drugs = state.fx.drugs || {};
        row(T("Drugs", "Препарати"), ...shielding.map(([key, d]) => tile(key, nm(d), !!state.fx.drugs[key],
          function () { state.fx.drugs[key] = !state.fx.drugs[key]; this.setAttribute("aria-pressed", String(state.fx.drugs[key])); update(); })));
      }
    },
  };
})();
