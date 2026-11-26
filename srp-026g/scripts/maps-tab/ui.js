// The Maps tab: one map at a time, its spawns for the chosen mode, every kind toggled on its own.
// World coordinates throughout: SVG x = world x, SVG y = -z (north up).
(() => {
  // The page's words are the server's (site-texts/words.js: T, Tf); the game's names, each
  // language's (MAPS: {en, uk, ...}).
  const LANG = document.documentElement.lang;
  const N = o => o[LANG] ?? o.en;
  const $ = id => document.getElementById(id);
  const SVG = "http://www.w3.org/2000/svg";
  const svgEl = (tag, attrs = {}, ...kids) => {
    const e = document.createElementNS(SVG, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    e.append(...kids);
    return e;
  };
  const el = (tag, attrs = {}, ...kids) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) k === "text" ? (e.textContent = v) : e.setAttribute(k, v);
    e.append(...kids);
    return e;
  };
  const store = {
    get(k) { try { return JSON.parse(localStorage.getItem("maps." + k)); } catch { return null; } },
    set(k, v) { try { localStorage.setItem("maps." + k, JSON.stringify(v)); } catch { /* private window */ } },
  };

  const LAYERS = [
    { key: "respawn", label: T("Player spawns") },
    { key: "spawners", label: T("Battery spawns") },
    { key: "stations", label: T("Chargers") },
    { key: "containers", label: T("Containers") },
    { key: "hunt", label: T("Glow of the Forest") },
    { key: "bases", label: T("Bases") },
    { key: "units", label: T("Research units") },
    { key: "resupply", label: T("Ammo stashes") },
    { key: "anomalies", label: T("Anomalies") },
    ...Object.entries(MAPS.artefacts).map(([k, a]) => ({ key: "art:" + k, label: N(a.name), icon: a.icon })),
  ];
  const state = {
    map: MAPS.maps[0].key, mode: null, view: store.get("view") || "minimap",
    on: Object.assign(Object.fromEntries(LAYERS.map(l => [l.key, true])), store.get("on") || {}),
  };
  const fromHash = () => {
    const [key, mode] = decodeURIComponent(location.hash.slice(1)).split("/");
    if (MAPS.maps.some(m => m.key === key)) { state.map = key; state.mode = mode || null; }
  };
  fromHash();
  const map = () => MAPS.maps.find(m => m.key === state.map);
  // Two maps in (nearly) one frame: a port and its original.
  const sameFrame = (a, b) => a.box.every((v, i) => Math.abs(v - b.box[i]) < 2);
  const mode = () => map().modes.find(m => m.key === state.mode) || map().modes[0];

  document.title = T("Survarium maps");
  $("title").textContent = T("Maps");
  $("stamp").textContent = Tf("Skhid revision {revision}", { revision: MAPS.revision });

  // A chip group: one pressed.
  function chips(host, items, current, pick) {
    host.replaceChildren(...items.map(([value, text]) => {
      const b = el("button", { type: "button", class: "chip", "aria-pressed": String(value === current), text });
      b.addEventListener("click", () => pick(value));
      return b;
    }));
  }
  const modeName = (m, all) => {
    const same = all.filter(o => N(o.name) === N(m.name));
    if (m.layout) return `${N(m.name)} · ${m.layout + 1}`;
    return same.length > 1 && m.stations.length ? `${N(m.name)} · ${T("chargers")}` : N(m.name);
  };

  // The HUD minimap's icons (MAPS.hud). A team's icon is painted in its
  // colour by the icon's own lightness, measured by the build, solid, with
  // the HUD's dark outline. Others keep their colours and get the outline.
  // `solo`: a spawn of a mode without teams.
  const TEAM = { t0: "#3b82f6", t1: "#e5484d", tx: "#9aa0a6", solo: "#f2c94c" };
  const defs = svgEl("svg", { width: 0, height: 0, "aria-hidden": "true", style: "position:absolute" }, svgEl("defs"));
  document.body.append(defs);
  // A crisp one-pixel dark edge round the icon (no blur), after its paint.
  const filter = (id, values) => {
    if (!document.getElementById(id))
      defs.firstChild.append(svgEl("filter", { id, "color-interpolation-filters": "sRGB", primitiveUnits: "objectBoundingBox", x: "-20%", y: "-20%", width: "140%", height: "140%" },
        svgEl("feColorMatrix", { type: "matrix", values: values || "1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0", result: "icon" }),
        svgEl("feMorphology", { in: "icon", operator: "dilate", radius: 0.05, result: "wide" }),
        svgEl("feFlood", { "flood-color": "#000", "flood-opacity": 0.85 }),
        svgEl("feComposite", { in2: "wide", operator: "in", result: "edge" }),
        svgEl("feMerge", {}, svgEl("feMergeNode", { in: "edge" }), svgEl("feMergeNode", { in: "icon" }))));
    return `url(#${id})`;
  };
  const outline = () => filter("outline");
  // Colour x (0.7 + 0.3 x lightness / the icon's own): its faces are the
  // colour itself, its shading at most 30% darker, so every team icon reads
  // as the same blue or red.
  const paint = (name, tint) => {
    const i = MAPS.hud[name], [r, g, b] = [1, 3, 5].map(k => parseInt(TEAM[tint].slice(k, k + 2), 16) / 255);
    const row = c => { const k = (0.3 * c) / i.light; return `${0.3 * k} ${0.59 * k} ${0.11 * k} 0 ${0.7 * c}`; };
    return filter(`paint-${name}-${tint}`, `${row(r)} ${row(g)} ${row(b)} 0 0 0 ${1 / i.opaque} 0`);
  };
  const tintOf = t => (t === 0 ? "t0" : t === 1 ? "t1" : "tx");
  // A mode none of whose spawns is a team's (Free for All: each player on
  // his own) has one kind of spawn: one colour, one swatch, no team named.
  const solo = mo => !mo.respawn.some(([, , , t]) => t === 0 || t === 1);
  // One icon centred on (x, y), `px` screen pixels high: painted in a team's
  // colour when `tint` is given.
  function hud(name, x, y, px, u, tint = null, rotate = 0) {
    const i = MAPS.hud[name], k = (px * u) / i.h;
    const attrs = { href: i.src, x: x - i.ax * k, y: y - i.ay * k, width: i.w * k, height: i.h * k, filter: tint ? paint(name, tint) : outline() };
    if (rotate) attrs.transform = `rotate(${rotate} ${x} ${y})`;
    return svgEl("image", attrs);
  }
  // A Research unit (mode 5), as the HUD draws it with nobody owning it
  // (its icon is drawn in code, `ResearchMinimapPoint`): its white letter
  // (cp_id 0, 1, 2: A, B, C) on a grey disc, `px` screen pixels across,
  // centred on (x, y).
  const letter = cp => String.fromCharCode(65 + cp);
  const unitMark = (cp, x, y, px, u) => {
    const r = (px * u) / 2;
    return svgEl("g", { filter: outline() },
      svgEl("circle", { cx: x, cy: y, r, fill: "#8a8a8a" }),
      svgEl("text", { x, y, "font-size": r * 1.3, "font-weight": 700, "text-anchor": "middle", "dominant-baseline": "central", fill: "#fff" },
        letter(cp)));
  };
  // Artifact Hunt's artefact (mode 13), as the HUD's minimap draws it lying
  // (ArtefactMinimapPoint, ArtefactHuntLook.drawOrb; drawn in code): a dark
  // disc, the orb's halo, core and highlight in the pale gold of its light,
  // `px` screen pixels across, centred on (x, y).
  const GLOW = "#f4e6a2";
  const orb = (x, y, px, u) => {
    const R = (px * u) / 2, r = (R * 7) / 9;
    return svgEl("g", {},
      svgEl("circle", { cx: x, cy: y, r: R, fill: "#000", "fill-opacity": 0.5 }),
      svgEl("circle", { cx: x, cy: y, r, fill: "none", stroke: GLOW, "stroke-opacity": 0.85, "stroke-width": r * 0.16 }),
      svgEl("circle", { cx: x, cy: y, r: r * 0.56, fill: GLOW }),
      svgEl("circle", { cx: x - r * 0.18, cy: y - r * 0.18, r: r * 0.2, fill: "#fff", "fill-opacity": 0.9 }));
  };
  // An anomaly is tinted by the artefacts it holds: each kind its glow's colour (artefacts.md, "The
  // stock glow": Lifebone green, Spring blue, Onyx orange); one holding two kinds, in stripes of both.
  const KIND = { lifebone: "#3fc41a", spring: "#2f7dff", onyx: "#ff8a1f" };
  const kindsOf = a => a.gives.map(([k]) => k);
  // Its paint: a colour, or a stripes pattern of `defs` (drawn anew with the map: `u` metres a pixel).
  function tint(a, defs, u) {
    const kinds = kindsOf(a).filter(k => KIND[k]);
    if (kinds.length < 2) return KIND[kinds[0]] || "#9aa0a6";
    const id = "mix-" + kinds.join("-"), w = 6 * u;
    if (!defs.querySelector("#" + id))
      defs.append(svgEl("pattern", { id, patternUnits: "userSpaceOnUse", width: w * kinds.length, height: w * kinds.length, patternTransform: "rotate(45)" },
        ...kinds.map((k, n) => svgEl("rect", { x: n * w, y: 0, width: w, height: w * kinds.length, fill: KIND[k] }))));
    return `url(#${id})`;
  }
  // Its zones seen from above: the damaging ones filled, the harmless ones (sound and sight) faint and
  // dashed. Each layer's opacity is its group's, so overlapping shapes (School's fruit has 42) read
  // as one flat area; the edges are drawn over them, thin.
  function area(a, i, u, defs) {
    const paint = tint(a, defs, u);
    const shape = (s, look) => (s.c ? svgEl("circle", { cx: s.c[0], cy: -s.c[1], r: s.c[2], ...look })
      : svgEl("polygon", { points: s.p.map(([x, z]) => `${x},${-z}`).join(" "), ...look }));
    const layer = (hurt, opacity) => svgEl("g", { opacity }, ...a.areas.filter(s => s.hurt === hurt).map(s => shape(s, { fill: paint })));
    const edge = { fill: "none", stroke: paint, "stroke-opacity": 0.85, "stroke-width": 1.2 * u };
    return svgEl("g", { class: "anom", "data-a": i, tabindex: 0, role: "button", "aria-label": T("Anomaly") },
      layer(false, 0.1), layer(true, 0.32),
      ...a.areas.map(s => shape(s, s.hurt ? edge : { ...edge, "stroke-dasharray": `${4 * u} ${3 * u}` })));
  }

  // Legend swatches, drawn as the markers are.
  // Two teams in one swatch: blue above and behind, red below and in front.
  const swatch = (key, mo) => {
    const s = svgEl("svg", { viewBox: "-11 -11 22 22", "aria-hidden": "true" });
    if (key === "respawn" && solo(mo)) s.append(hud("player", 0, 0, 16, 1, "solo"));
    else if (key === "respawn") s.append(hud("player", -3, -3, 14, 1, "t0"), hud("player", 3, 3, 14, 1, "t1"));
    if (key === "spawners") s.append(hud("battery", 0, 0, 14, 1));
    if (key === "stations") s.append(hud("charger", 0, 0, 22, 1));
    if (key === "containers") s.append(hud("base", -4, -2, 16, 1, "t0"), hud("base", 4, 2, 16, 1, "t1"));
    if (key === "resupply") s.append(hud("resupply", -4, -2, 16, 1, "t0"), hud("resupply", 4, 2, 16, 1, "t1"));
    if (key === "units") s.append(unitMark(0, 0, 0, 18, 1));
    if (key === "hunt") s.append(orb(0, 0, 18, 1));
    if (key === "bases") s.append(hud("base", -4, -2, 16, 1, "t0"), hud("base", 4, 2, 16, 1, "t1"));
    if (key === "anomalies") {
      const kinds = [...new Set(map().anomalies.flatMap(kindsOf))].filter(k => KIND[k]);
      kinds.forEach((k, n) => s.append(svgEl("circle", { cx: (n - (kinds.length - 1) / 2) * 6, cy: 0, r: 6, fill: KIND[k], "fill-opacity": 0.45, stroke: KIND[k], "stroke-width": 1.2 })));
    }
    return s;
  };

  function drawControls() {
    const m = map();
    if (!m.modes.some(o => o.key === state.mode)) state.mode = m.modes[0].key;
    chips($("maps"), MAPS.maps.map(o => [o.key, N(o.name)]), state.map, k => { state.map = k; go(); });
    chips($("modes"), m.modes.map(o => [o.key, modeName(o, m.modes)]), state.mode, k => { state.mode = k; go(); });
    chips($("views"), [["minimap", T("Minimap")], ["render", T("From above")]], state.view,
      v => { state.view = v; store.set("view", v); go(); });
    const mo = mode();
    const counts = {
      respawn: mo.respawn.length, spawners: mo.spawners.length, stations: mo.stations.length, containers: mo.containers.length,
      resupply: mo.resupply.length, units: (mo.units || []).length, hunt: mo.hunt.length, bases: mo.bases.length,
      anomalies: m.anomalies.length,
      ...Object.fromEntries(Object.keys(MAPS.artefacts).map(k => ["art:" + k, m.artefacts.filter(a => a[3].includes(k)).length])),
    };
    $("layers").replaceChildren(...LAYERS.map(l => {
      const n = counts[l.key];
      const extra = l.key === "stations" && mo.stations_active && mo.stations_active < n ? ` (${Tf("{n} active", { n: mo.stations_active })})` : "";
      const b = el("button", { type: "button", class: "layer", "aria-pressed": String(state.on[l.key]) },
        l.icon ? el("img", { src: l.icon, alt: "" }) : swatch(l.key, mo), document.createTextNode(l.label), el("b", { text: n + extra }));
      b.hidden = !n;
      b.addEventListener("click", () => { state.on[l.key] = !state.on[l.key]; store.set("on", state.on); go(); });
      return b;
    }));
    const tdm = !mo.spawners.length && !mo.stations.length;
    $("note").textContent = mo.batteries && !tdm ? Tf("{n} batteries to win.", { n: mo.batteries }) : "";
    $("anote").textContent = m.anomalies.length
      ? T("Artefacts come from the anomalies. Each anomaly puts one of its artefacts into one of its free containers, one per time window, until its maximum lies uncollected; a pickup starts the window again. It gives only those of its artefacts that no player is carrying. Point at an anomaly or an artefact (or tap it) for its numbers.")
      : "";
  }

  // The floating card of anomaly `i` of the map: what it holds and when it gives it.
  const card = $("card");
  const secs = (lo, hi) => (lo === hi ? Tf("{s} s", { s: lo }) : Tf("{a}–{b} s", { a: lo, b: hi }));
  const span = list => secs(Math.min(...list), Math.max(...list));
  function fillCard(i) {
    const a = map().anomalies[i], art = MAPS.artefacts;
    const rows = [
      [T("Holds"), a.gives.map(([k, n]) => `${art[k] ? N(art[k].name) : k} ×${n}`).join(" · ")],
      [T("First one"), secs(...a.first)],
      [T("Then every"), secs(...a.next)],
      ...(a.chance < 1 ? [[T("Chance"), `${Math.round(a.chance * 100)} %`]] : []),
      [T("Max uncollected"), String(a.max)],
      [T("Containers"), String(a.containers)],
      [Tc("artefact", "Search"), span(a.search)],
      ...a.switch.map(([lo, hi]) => [T("Zones switch every"), secs(lo, hi)]),
    ];
    const harmless = a.areas.length && !a.areas.some(s => s.hurt);
    card.replaceChildren(
      el("div", { class: "card-h" }, ...kindsOf(a).filter(k => art[k]).map(k => el("img", { src: art[k].icon, alt: "" })),
        el("span", { text: kindsOf(a).map(k => (art[k] ? N(art[k].name) : k)).join(" · ") })),
      el("dl", {}, ...rows.flatMap(([k, v]) => [el("dt", { text: k }), el("dd", { text: v })])),
      el("p", { text: Tf("After a pickup the next one comes {next} later; that container stays empty for {cooldown}.", { next: secs(...a.next), cooldown: span(a.cooldown) }) }),
      ...(harmless ? [el("p", { text: T("Its own zone does no damage.") })] : []));
  }
  // Shown at (x, y), the pointer's place in the stage, kept inside it.
  function showCard(i, x, y) {
    if (card.dataset.a !== String(i)) { fillCard(i); card.dataset.a = i; }
    card.hidden = false;
    // Beside the pointer, inside the map (the board's top left is the map's); a phone's is under the map.
    const s = $("stage").getBoundingClientRect(), w = card.offsetWidth, h = card.offsetHeight;
    const under = getComputedStyle(card).position === "static";
    card.style.left = under ? "" : `${x + 16 + w > s.width ? Math.max(4, x - 16 - w) : x + 16}px`;
    card.style.top = under ? "" : `${Math.max(4, Math.min(y + 16, s.height - h - 4))}px`;
    svg.classList.add("focus");
    for (const e of svg.querySelectorAll("[data-a]")) e.classList.toggle("on", e.dataset.a.split(" ").includes(String(i)));
  }
  function hideCard() {
    pinned = null;
    card.hidden = true;
    delete card.dataset.a;
    svg.classList.remove("focus");
  }

  // The view box: the play area, the same for maps in one frame.
  const svg = $("map");
  // The play area, grown to hold every point of the map: of every map in
  // its frame, so maps that share one start in the same place.
  const fit = () => {
    const same = MAPS.maps.filter(m => sameFrame(m, map()));
    let [x0, y0, x1, y1] = map().play;
    for (const m of same) { x0 = Math.min(x0, m.play[0]); y0 = Math.min(y0, m.play[1]); x1 = Math.max(x1, m.play[2]); y1 = Math.max(y1, m.play[3]); }
    const points = same.flatMap(m => [...m.artefacts, ...m.modes.flatMap(o => [...o.respawn, ...o.spawners, ...o.stations, ...o.containers, ...o.resupply, ...(o.units || []), ...o.hunt, ...o.bases])]);
    for (const [x, z] of points) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, -z); y1 = Math.max(y1, -z); }
    const pad = Math.max(x1 - x0, y1 - y0) * 0.03;
    return [x0 - pad, y0 - pad, x1 - x0 + 2 * pad, y1 - y0 + 2 * pad];
  };
  // Metres per screen pixel, so markers keep their size on screen (the HUD's
  // bitmaps at their own pixel size: crisp).
  const unit = () => {
    const r = svg.getBoundingClientRect(), [, , w, h] = fit();
    return Math.max(w / (r.width || 800), h / (r.height || 800));
  };

  function drawMap() {
    const m = map(), mo = mode();
    svg.setAttribute("viewBox", fit().join(" "));
    svg.setAttribute("aria-label", N(m.name));
    const u = unit();
    const kids = [];
    const img = state.view === "render" && m.render ? m.render : { src: m.minimap, box: m.box };
    const [bx0, by0, bx1, by1] = img.box;
    kids.push(svgEl("image", { href: img.src, x: bx0, y: by0, width: bx1 - bx0, height: by1 - by0, preserveAspectRatio: "none" }));
    const defs = svgEl("defs");
    kids.push(defs);
    if (state.on.anomalies) m.anomalies.forEach((a, i) => kids.push(area(a, i, u, defs)));
    const team = t => (t === 0 ? "var(--team0)" : t === 1 ? "var(--team1)" : "var(--team-x)");
    const teamName = t => (t === 0 || t === 1 ? Tf("team {n}", { n: t + 1 }) : T("any team"));
    const tip = text => svgEl("title", {}, text);
    if (state.on.resupply) for (const [x, z, , t] of mo.resupply)
      kids.push(svgEl("g", { class: "mk" }, hud("resupply", x, -z, MAPS.hud.resupply.h, u, tintOf(t)), tip(`${T("Ammo stash")}, ${teamName(t)}`)));
    if (state.on.containers) for (const [x, z, , t] of mo.containers)
      kids.push(svgEl("g", { class: "mk" }, hud("base", x, -z, MAPS.hud.base.h, u, tintOf(t)), tip(`${T("Container")}, ${teamName(t)}`)));
    if (state.on.spawners) for (const [x, z] of mo.spawners)
      kids.push(svgEl("g", { class: "mk" }, hud("battery", x, -z, 20, u), tip(T("Battery spawn"))));
    if (state.on.stations) for (const [x, z] of mo.stations)
      kids.push(svgEl("g", { class: "mk" }, hud("charger", x, -z, MAPS.hud.charger.h, u), tip(T("Charger"))));
    // Each container's artefacts; pointing at one shows its anomaly's card (`data-a`: its anomalies).
    for (const [x, z, , kinds, owners] of m.artefacts) {
      const shown = kinds.filter(k => state.on["art:" + k]);
      shown.forEach((k, i) => {
        const s = 24 * u, dx = (i - (shown.length - 1) / 2) * s * 0.9;
        kids.push(svgEl("g", { class: "mk art", "data-a": owners.join(" "), "aria-label": kinds.map(k => N(MAPS.artefacts[k].name)).join(", ") },
          svgEl("image", { href: MAPS.artefacts[k].icon, x: x + dx - s / 2, y: -z - s / 2, width: s, height: s, filter: outline() })));
      });
    }
    if (state.on.bases) for (const [x, z, , t] of mo.bases)
      kids.push(svgEl("g", { class: "mk" }, hud("base", x, -z, MAPS.hud.base.h, u, tintOf(t)), tip(`${Tc("hunt", "Base")}, ${teamName(t)}`)));
    if (state.on.hunt) for (const [x, z] of mo.hunt)
      kids.push(svgEl("g", { class: "mk" }, orb(x, -z, 18, u), tip(T("Glow of the Forest"))));
    if (state.on.units) for (const [x, z, , cp] of mo.units || [])
      kids.push(svgEl("g", { class: "mk" }, unitMark(cp, x, -z, 22, u), tip(`${T("Research unit")} ${letter(cp)}`)));
    // The HUD's player triangle, turned the way the player faces at spawn.
    if (state.on.respawn) for (const [x, z, , t, facing] of mo.respawn)
      kids.push(svgEl("g", { class: "mk" }, hud("player", x, -z, MAPS.hud.player.h, u, solo(mo) ? "solo" : tintOf(t), facing),
        tip(solo(mo) ? T("Spawn") : `${T("Spawn")}, ${teamName(t)}`)));
    svg.replaceChildren(...kids);
    hideCard();
  }

  // The card follows the pointer over an anomaly or an artefact; a tap (or a click) keeps it until
  // a tap elsewhere; the keyboard shows it by an anomaly's focus.
  let pinned = null;
  const target = e => e.target.closest?.("[data-a]");
  const at = e => { const s = $("stage").getBoundingClientRect(); return [e.clientX - s.left, e.clientY - s.top]; };
  svg.addEventListener("pointermove", e => {
    if (pinned !== null) return;
    const t = target(e);
    if (t && t.dataset.a !== "") showCard(Number(t.dataset.a.split(" ")[0]), ...at(e));
    else hideCard();
  });
  svg.addEventListener("pointerleave", () => { if (pinned === null) hideCard(); });
  svg.addEventListener("click", e => {
    const t = target(e);
    if (t && t.dataset.a !== "") { pinned = Number(t.dataset.a.split(" ")[0]); showCard(pinned, ...at(e)); }
    else hideCard();
  });
  svg.addEventListener("focusin", e => {
    const t = target(e);
    if (!t) return;
    const r = t.getBoundingClientRect(), s = $("stage").getBoundingClientRect();
    showCard(Number(t.dataset.a.split(" ")[0]), r.right - s.left, r.top - s.top);
  });
  svg.addEventListener("focusout", () => { if (pinned === null) hideCard(); });
  addEventListener("keydown", e => { if (e.key === "Escape") hideCard(); });

  function go() {
    history.replaceState(null, "", `#${state.map}/${state.mode || ""}`);
    drawControls();
    drawMap();
  }

  addEventListener("resize", () => drawMap());
  addEventListener("hashchange", () => { fromHash(); go(); });
  go();
})();
