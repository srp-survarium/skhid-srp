// The simulators page: one or two weapons (A, B: WikiWeaponSlots), each with
// a round, fired in one simulation of recoil (recoil.js) and spread
// (spread.js) in the state its controls set.
(() => {
  const sim = document.querySelector('#recoil-sim');
  if (!sim) return;
  const weapons = [...document.querySelectorAll('#weapon-options > div')];
  const collator = new Intl.Collator(document.documentElement.lang, {numeric: true, sensitivity: 'base'});
  const tr = (en, uk) => document.documentElement.lang === 'uk' ? uk : en;
  const byId = new Map(weapons.map(element => [element.id, element]));
  // Each weapon's rounds, its default first: [{id, name, spread}].
  const roundsOf = new Map(weapons.map(element => [element.id, JSON.parse(element.dataset.rounds)]));
  function roundOf(id, roundId) {
    const rounds = roundsOf.get(id);
    return rounds.find(round => round.id === roundId) || rounds[0];
  }
  // The player's stance spread and its skill weights (default.player).
  const player = JSON.parse(sim.dataset.player);
  // A weapon's models firing a round with its modifications' `kick` (their
  // sum): its recoil, whose kick ranges are times max(0, 1 + kick) (the
  // cook's type 2, 0x358010), and its spread, whose base is times the round's
  // `dispersion` (get_static_dispersion 0x331220).
  const raws = new Map(weapons.map(element => [element.id, JSON.parse(element.dataset.recoil)]));
  const RANGES = ['top', 'left', 'right'].flatMap(axis => [`min_${axis}_recoil`, `max_${axis}_recoil`]);
  const models = new Map();
  function model(id, roundId, kick = 0) {
    const round = roundOf(id, roundId), key = `${id} ${round.id} ${kick}`;
    if (!models.has(key)) {
      const raw = raws.get(id), dispersion = raw.dispersion, scale = Math.max(0, 1 + kick);
      const base = typeof dispersion.base_dispersion === 'number' ? dispersion.base_dispersion * round.spread : undefined;
      const ranges = Object.fromEntries(RANGES.filter(k => typeof raw[k] === 'number').map(k => [k, raw[k] * scale]));
      models.set(key, {recoil: WeaponRecoil.parse({...raw, ...ranges}),
        spread: WeaponSpread.parse({...dispersion, base_dispersion: base}, player, raw.double_handed)});
    }
    return models.get(key);
  }
  function node(tag, text, cls) {
    const element = document.createElement(tag);
    if (text !== undefined) element.textContent = text;
    if (cls) element.className = cls;
    return element;
  }
  function name(id) { return byId.get(id).dataset.title; }
  const svgNS = 'http://www.w3.org/2000/svg';
  function svgNode(tag, attrs, text) {
    const element = document.createElementNS(svgNS, tag);
    for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
    if (text !== undefined) element.textContent = text;
    return element;
  }
  // Degrees of view in one plotted unit of kick, from the exe
  // (servers/match-server/src/v026g/bots/recoil.rs, "the pitch's unit"): a
  // pitch unit turns the view 60° up and 67.5° down, and the configs' kick is
  // 63.735 to a pitch unit (PROVEN): 63.735 is the vertical clip's half range,
  // so the configs' kick is in degrees. The side's 21.48 taken the same way:
  // a degree a unit. LIKELY, against the clip decode's ±2.3° (which made it
  // 0.108°, so small the H&K 416A5's random side kick and the KRISS Vector's
  // none looked alike; the owner's play: the 416 jerky, the Vector smooth).
  const UP_DEG = 60 / 63.735, DOWN_DEG = 67.5 / 63.735, SIDE_DEG = 1;
  const viewUp = v => v * (v >= 0 ? UP_DEG : DOWN_DEG), viewSide = h => h * SIDE_DEG;
  // A target's figure: the game's HUD doll, 1.8 m tall, aimed at the chest
  // (a quarter of its height below the top of the head).
  const BODY_HEIGHT = 1.8, CHEST = .25;
  let dollBox = null;
  function bodyDegrees(distance) {
    return 2 * Math.atan(BODY_HEIGHT / 2 / distance) * 180 / Math.PI;
  }
  // The aim at `time`: the kick between its 50 ms frames, and the shake at its
  // own moment (its clip runs ~240 frames a second, far finer than the frames).
  function viewAt(series, time) {
    const at = Math.max(0, time / .05), i = Math.min(series.frames.length - 1, Math.floor(at + 1e-6));
    const a = series.frames[i], b = series.frames[Math.min(series.frames.length - 1, i + 1)], f = Math.min(1, at - i);
    const lerp = key => a[key] + (b[key] - a[key]) * f;
    let side = 0, up = 0;
    if (series.shake) {
      const [clipSide, clipUp] = WeaponRecoil.shakeAt(series.shake.rate * (Math.min(Math.max(time, series.shake.start), series.shake.stop) - series.shake.start));
      side = clipSide * lerp('z'); up = clipUp * lerp('z');
    }
    return {side: viewSide(lerp('horizontal')) + side, up: viewUp(lerp('vertical')) + up};
  }
  function sample(series, time) {
    return series.frames[Math.max(0, Math.min(series.frames.length - 1, Math.floor(time / .05 + 1e-6)))];
  }
  // The simulation in `root`: its controls, plots and crosshair view, every
  // lookup inside `root`. `show(sides)` runs it for [{id, round, side}]
  // (side 0 is A's colour, 1 B's), each side firing in `stateOf(side)`'s
  // state; `refresh()` after a state changes.
  function RecoilSim(root, stateOf) {
    const find = selector => root.querySelector(selector);
    const timeInput = find('.sim-time'), seedInput = find('.sim-seed'), playButton = find('.sim-play');
    let simulation = {kind: 'empty'}, sides = [], playing = null;
    function lastShot() {
      return Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
    }
    function draw() {
      if (simulation.kind !== 'ready') return;
      const time = simulation.time;
      find('.sim-last').disabled = time >= lastShot() - .001;
      timeInput.value = time;
      find('.sim-time-label').textContent = `${time.toFixed(2)} ${tr('s', 'с')}`;
      const legend = find('.sim-legend'); legend.replaceChildren();
      for (const series of simulation.series) {
        const point = sample(series, time), label = sides.length > 1 ? `${series.side ? 'B' : 'A'} · ` : '';
        legend.append(node('p', `${label}${name(series.id)} · ${tr('shots', 'постріли')} ${series.events.filter(event => event.time <= time + 1e-9).length}/${series.shots} · ${tr('up', 'вгору')} ${point.vertical.toFixed(3)} · ${tr('side', 'убік')} ${point.horizontal.toFixed(3)} · ${tr('spread', 'розкид')} ${point.spread.toFixed(2)}°`, `series-${series.side}`));
      }
      for (const axis of ['vertical', 'horizontal']) {
        const svg = find(`.sim-${axis}`); svg.replaceChildren();
        const values = simulation.series.flatMap(series => series.frames.map(frame => frame[axis]));
        const lo = Math.min(0, ...values), hi = Math.max(.1, ...values), margin = (hi - lo) * .1;
        const min = lo - margin, max = hi + margin;
        const x = t => 54 + t / simulation.duration * 530, y = v => 218 - (v - min) / (max - min) * 194;
        for (let i = 0; i <= 4; i++) {
          const value = min + (max - min) * i / 4;
          svg.append(svgNode('line', {x1:54, x2:584, y1:y(value), y2:y(value), class:'sim-grid'}));
          svg.append(svgNode('text', {x:47, y:y(value)+4, 'text-anchor':'end'}, value.toFixed(2)));
          const t = simulation.duration * i / 4;
          svg.append(svgNode('text', {x:x(t), y:241, 'text-anchor':'middle'}, `${t.toFixed(1)}s`));
        }
        svg.append(svgNode('line', {x1:54, x2:584, y1:y(0), y2:y(0), class:'sim-zero'}));
        for (const series of simulation.series) {
          const path = frames => frames.map((frame, i) => `${i ? 'L' : 'M'}${x(frame.time).toFixed(2)},${y(frame[axis]).toFixed(2)}`).join(' ');
          svg.append(svgNode('path', {d:path(series.frames), class:`sim-curve series-${series.side} future`}));
          svg.append(svgNode('path', {d:path(series.frames.filter(frame => frame.time <= time)), class:`sim-curve series-${series.side}`}));
          for (const event of series.events.filter(event => event.time <= time)) {
            const point = sample(series, event.time);
            svg.append(svgNode('circle', {cx:x(event.time), cy:y(point[axis]), r:2.8, class:`sim-shot series-${series.side}`}));
          }
        }
        svg.append(svgNode('line', {x1:x(time), x2:x(time), y1:24, y2:218, class:'sim-cursor'}));
      }
      drawSight(time);
    }
    // Each weapon's spread in its state, in degrees: the base, the stance's
    // part, a shot's and the most it reaches.
    function drawParts() {
      const degrees = value => `${+value.toFixed(2)}°`;
      find('.sim-parts tbody').replaceChildren(...simulation.series.map(series => {
        const spread = series.spread, row = node('tr', undefined, `series-${series.side}`);
        const label = node('th', sides.length > 1 ? `${series.side ? 'B' : 'A'} · ${name(series.id)}` : name(series.id)); label.scope = 'row';
        row.append(label, ...(spread ? [spread.base, spread.k * spread.stance, spread.k * spread.perShot, spread.base + spread.k * (spread.stance + spread.max)]
          .map(value => node('td', degrees(value))) : [node('td', '—')]));
        return row;
      }));
    }
    // The crosshair as the kick moves it, in degrees of view at true scale
    // both ways: the aim's path, each shot's mark and bullet's hit, and the
    // spread around the aim. Each weapon has its own view, all on one scale
    // so they compare.
    function drawSight(time) {
      const frames = simulation.series.flatMap(series => series.frames);
      // In degrees of view: the kick as the camera turns. The pattern is the
      // weapon's: a target changes nothing of it, only its own size shows the
      // distance (cut by the view's edge when it is the larger).
      const distance = Number(find('.sim-target').value) || 0, body = distance ? bodyDegrees(distance) : 0;
      // The aim (kick and shake), each bullet's hit and the spread around the
      // aim, in degrees of view: the view holds them all.
      const hit = (series, event) => {
        const point = viewAt(series, event.time);
        return {side: point.side + event.bullet.side, up: point.up + event.bullet.up};
      };
      // The path drawn every 1/120 s, fine enough for the shake.
      const steps = series => Array.from({length: Math.floor(series.duration * 120) + 1}, (_, i) => i / 120);
      const points = [...simulation.series.flatMap(series => steps(series).map(t => ({...viewAt(series, t), r: sample(series, t).spread / 2}))),
        ...simulation.series.flatMap(series => series.events.map(event => ({...hit(series, event), r: 0})))];
      const up = Math.max(0, ...points.map(point => point.up + point.r));
      const down = Math.max(0, ...points.map(point => point.r - point.up));
      const side = Math.max(0, ...points.map(point => Math.abs(point.side) + point.r));
      const scale = 250 / Math.max(.1, up + down, 2 * side * 250 / 280);
      const oy = 25 + (250 - (up + down) * scale) / 2 + up * scale;
      const x = degrees => 150 + degrees * scale, y = degrees => oy - degrees * scale;
      const hitAt = (series, event) => {
        const point = hit(series, event);
        return [x(point.side), y(point.up)];
      };
      const views = simulation.series.map(series => {
        const svg = svgNode('svg', {viewBox:'0 0 300 300', role:'img', 'aria-label':`${name(series.id)}: ${tr('where it aims as the recoil moves it', 'куди цілиться під дією відбою')}`});
        const view = document.createElement('div'); view.className = 'sim-view';
        if (simulation.series.length > 1) view.append(node('p', `${series.side ? 'B' : 'A'} · ${name(series.id)}`, `sim-view-name series-${series.side}`));
        view.append(svg);
        return {series, svg, view};
      });
      find('.sim-sight-views').replaceChildren(...views.map(({view}) => view));
      for (const {series, svg} of views) {
        svg.append(svgNode('line', {x1:0, x2:300, y1:oy, y2:oy, class:'sim-grid'}));
        svg.append(svgNode('line', {x1:150, x2:150, y1:0, y2:300, class:'sim-grid'}));
        if (distance) {
          dollBox ??= document.querySelector('#doll-shape').getBBox();
          const size = body * scale / dollBox.height;
          svg.append(svgNode('use', {href:'#doll-shape', class:'sim-target-body',
            transform:`translate(${(150 - (dollBox.x + dollBox.width / 2) * size).toFixed(2)} ${(oy - body * CHEST * scale - dollBox.y * size).toFixed(2)}) scale(${size.toFixed(5)})`}));
        }
        svg.append(svgNode('circle', {cx:150, cy:oy, r:3, class:'sim-origin'}));
        // Each weapon's view holds at its own last shot: with two, the faster
        // finishes first, and its settling is the plots' only (as for one).
        const last = Math.max(...series.events.map(event => event.time)), held = Math.min(time, last);
        const past = [...steps(series).filter(t => t < held), held].map(t => viewAt(series, t));
        svg.append(svgNode('path', {d:past.map((point, i) => `${i ? 'L' : 'M'}${x(point.side).toFixed(2)},${y(point.up).toFixed(2)}`).join(' '),
          class:`sim-trail series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= held)) {
          const point = viewAt(series, event.time), [hx, hy] = hitAt(series, event);
          svg.append(svgNode('circle', {cx:x(point.side), cy:y(point.up), r:2, class:`sim-shot series-${series.side}`}));
          svg.append(svgNode('circle', {cx:hx, cy:hy, r:2.6, class:`sim-hit series-${series.side}`}));
        }
        const frame = sample(series, held), point = viewAt(series, held), cx = x(point.side), cy = y(point.up);
        // The spread: the cone's half angle around the aim.
        const radius = frame.spread / 2;
        if (radius > 0) svg.append(svgNode('circle', {cx, cy, r:radius * scale, class:`sim-spread series-${series.side}`}));
        const cross = svgNode('g', {class:`sim-cross series-${series.side}`});
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          cross.append(svgNode('line', {x1:cx + dx * 5, y1:cy + dy * 5, x2:cx + dx * 15, y2:cy + dy * 15}));
        }
        cross.append(svgNode('circle', {cx, cy, r:1.6}));
        svg.append(cross);
      }
    }
    // Play: the burst in real time, from where the timeline is (or the start),
    // to the last shot; the settling after it stays on the plots.
    function stop() {
      if (playing) cancelAnimationFrame(playing.frame);
      playing = null; playButton.textContent = tr('Play', 'Відтворити');
    }
    function play() {
      if (simulation.kind !== 'ready') return;
      const last = Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
      const from = simulation.time >= last - .001 ? 0 : simulation.time, start = performance.now();
      playButton.textContent = tr('Pause', 'Пауза');
      const tick = now => {
        if (simulation.kind !== 'ready') { stop(); return; }
        // A frame's timestamp can precede the press: never before `from`.
        simulation.time = Math.min(last, from + Math.max(0, now - start) / 1000); draw();
        if (simulation.time >= last) stop(); else playing.frame = requestAnimationFrame(tick);
      };
      playing = {frame: requestAnimationFrame(tick)};
    }
    function seek(time) {
      stop();
      if (simulation.kind !== 'ready') return;
      simulation.time = time; draw();
    }
    function rebuild(time = 0) {
      stop(); simulation = {kind:'empty'};
      const firing = sides.map(({side}) => stateOf(side));
      const valid = sides.length > 0 && sides.every(({id, round}, i) => model(id, round, firing[i].kick).recoil);
      for (const element of root.querySelectorAll('input,select,button')) element.disabled = !valid;
      find('.sim-plots').hidden = !valid;
      find('.sim-sight').hidden = !valid;
      find('.sim-unavailable').hidden = valid;
      if (!valid) { find('.sim-legend').replaceChildren(); find('.sim-parts tbody').replaceChildren(); return; }
      seedInput.value = Math.max(1, Math.min(999999, Math.floor(Number(seedInput.value) || 1)));
      // A full magazine; with two weapons, the smaller one's from each.
      const shots = Math.min(...sides.map(({id, round}, i) => model(id, round, firing[i].kick).recoil.magazine));
      const series = sides.map(({id, round, side}, i) => {
        const state = firing[i], {recoil, spread: weapon} = model(id, round, state.kick), spread = weapon && WeaponSpread.condition(weapon, state);
        const result = WeaponRecoil.simulate(recoil, shots, Number(seedInput.value), {aim: state.aim, spread, recoil: state.kit.recoil},
          Number(find('.sim-trigger').value));
        // An initial un-fired frame makes Reset/Next shot unambiguous in the UI.
        result.frames.forEach(frame => {frame.time += .05;});
        result.events.forEach(event => {event.time += .05;});
        result.frames.unshift({...result.frames[0], time:0, vertical:0, horizontal:0, shots:0, z:0, shake:{side:0, up:0}});
        const shake = result.shake && {...result.shake, start: result.shake.start + .05, stop: result.shake.stop + .05};
        return {...result, id, side, spread, shake, duration: result.duration + .05};
      });
      const duration = Math.max(...series.map(series => series.duration));
      simulation = {kind:'ready', series, time:Math.min(time, duration), duration};
      timeInput.max = simulation.duration; drawParts(); draw();
    }
    playButton.addEventListener('click', () => playing ? stop() : play());
    seedInput.addEventListener('change', () => rebuild(simulation.kind === 'ready' ? simulation.time : 0));
    find('.sim-trigger').addEventListener('change', () => rebuild());
    find('.sim-reset').addEventListener('click', () => seek(0));
    // Last shot: the whole pattern at once.
    find('.sim-last').addEventListener('click', () => {
      if (simulation.kind !== 'ready') return;
      seek(lastShot());
    });
    find('.sim-target').addEventListener('change', draw);
    timeInput.addEventListener('input', () => seek(Number(timeInput.value)));
    return {show(chosen) { sides = chosen; rebuild(); }, refresh() { rebuild(simulation.kind === 'ready' ? simulation.time : 0); }};
  }
  // Each side's setup, its own (the same weapon can be set up twice): a panel
  // of hud.js's rows, as the calculator's, from build.py's `setup` rows. Its
  // state: the aim, the stance, the broken arms, the kit's sums of the
  // skills' and armour pieces' modifiers, and the weapon modifications' sum
  // on its kick ranges.
  const rows = JSON.parse(sim.dataset.setup);
  const panels = [...document.querySelectorAll('.sim-setup')];
  const picks = panels.map(() => ({aim: 'hip', stance: 'idle', arms: '0'}));
  function stateOf(side) {
    const pick = picks[side], chosen = rows.filter(row => pick[row.key]);
    return {aim: pick.aim, stance: pick.stance, arms: Number(pick.arms),
      kit: WeaponSpread.kitOf(chosen.filter(row => row.effects).map(row => row.effects[pick[row.key]])),
      kick: chosen.filter(row => row.kick).reduce((sum, row) => sum + row.kick[pick[row.key]], 0)};
  }
  panels.forEach((panel, side) => {
    const box = HUD.el('div', {class: 'gamemods rows'});
    for (const row of rows) {
      const icon = row.icon ? HUD.el('img', {class: 'gico', src: row.icon, alt: ''}) : HUD.el('span', {class: 'gico'});
      const line = HUD.stepsRow(icon, row.label, row.options, picks[side], row.key, () => simulation.refresh(),
        row.card && HUD.card(...row.card), row.required);
      line.dataset.key = row.key; box.append(line);
    }
    panel.append(box);
  });
  const simulation = RecoilSim(sim, stateOf), empty = document.querySelector('#compare-empty');
  const slots = WikiWeaponSlots(weapons, roundsOf, tr, collator, () => {
    const sides = slots.sides().map((chosen, side) => ({...chosen, side})).filter(({id}) => id);
    sim.hidden = sides.length === 0; empty.hidden = sides.length !== 0;
    panels.forEach((panel, side) => {
      const id = slots.sides()[side].id;
      panel.hidden = !id;
      if (id) panel.querySelector('.sim-setup-name').textContent = `${side ? 'B' : 'A'} · ${name(id)}`;
    });
    simulation.show(sides);
  });
  document.querySelector('#clear-comparison').addEventListener('click', () => slots.clear());
  document.querySelector('#comparison').hidden = false; slots.refresh();
})();
