// The simulators page: one or two weapons (A, B: WikiWeaponSlots), each with
// a round, fired in one simulation of recoil (recoil.js), spread (spread.js)
// and the shot clips' own motions of the camera and the gun (motions.js) in
// the state its controls set. A side can be a custom gun: its weapon with
// any of those values the visitor's own.
(() => {
  const sim = document.querySelector('#recoil-sim');
  if (!sim) return;
  const weapons = [...document.querySelectorAll('#weapon-options > div')];
  const collator = new Intl.Collator(document.documentElement.lang, {numeric: true, sensitivity: 'base'});
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
  // `over`: a custom gun's own recoil values, by the config's keys.
  function model(id, roundId, kick = 0, over = {}) {
    const round = roundOf(id, roundId), key = `${id} ${round.id} ${kick} ${JSON.stringify(over)}`;
    if (!models.has(key)) {
      const raw = {...raws.get(id), ...over}, dispersion = raw.dispersion, scale = Math.max(0, 1 + kick);
      const base = typeof dispersion.base_dispersion === 'number' ? dispersion.base_dispersion * round.spread : undefined;
      const ranges = Object.fromEntries(RANGES.filter(k => typeof raw[k] === 'number').map(k => [k, raw[k] * scale]));
      models.set(key, {recoil: WeaponRecoil.parse({...raw, ...ranges}),
        spread: WeaponSpread.parse({...dispersion, base_dispersion: base}, player, raw.double_handed)});
    }
    return models.get(key);
  }
  // Each weapon's shot clips' motions (none in a build without the clips).
  const motionsOf = new Map(weapons.map(element => [element.id, WeaponMotions.parse(raws.get(element.id).shot)]));
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
  // How far (0 to 1) `time` is into the pass of the clips the series' last
  // shot started; 1 before its first shot and once the pass is over.
  function passAt(series, time) {
    let last = -1;
    for (const event of series.events) {
      if (event.time > time + 1e-9) break;
      last = event.time;
    }
    return last < 0 ? 1 : (time - last) / series.interval;
  }
  // The view at `time`, where a bullet would leave: with `layers.recoil` the
  // kick between its 50 ms frames and the shake at its own moment (its clip
  // runs ~240 frames a second, far finer than the frames), with
  // `layers.camera` the hands' clip's turn of the camera.
  function viewAt(series, time, layers) {
    const at = Math.max(0, time / .05), i = Math.min(series.frames.length - 1, Math.floor(at + 1e-6));
    const a = series.frames[i], b = series.frames[Math.min(series.frames.length - 1, i + 1)], f = Math.min(1, at - i);
    const lerp = key => a[key] + (b[key] - a[key]) * f;
    let side = 0, up = 0;
    if (series.shake) {
      const [clipSide, clipUp] = WeaponRecoil.shakeAt(series.shake.rate * (Math.min(Math.max(time, series.shake.start), series.shake.stop) - series.shake.start));
      side = clipSide * lerp('z'); up = clipUp * lerp('z');
    }
    const kick = layers.recoil ? {side: viewSide(lerp('horizontal')) + side, up: viewUp(lerp('vertical')) + up} : {side: 0, up: 0};
    const camera = layers.camera ? WeaponMotions.cameraAt(series.motions, series.aim, passAt(series, time)) : {side: 0, up: 0};
    return {side: kick.side + camera.side, up: kick.up + camera.up};
  }
  // Where the gun's sight points at `time`, and how far the gun is back (mm):
  // the view and, with `layers.gun`, the gun's own turn in front of it. From
  // the hip the sights are not looked through: the view alone.
  function sightAt(series, time, layers) {
    const view = viewAt(series, time, layers);
    const gun = layers.gun && series.aim !== 'hip' ? WeaponMotions.gunAt(series.motions, passAt(series, time)) : {back: 0, side: 0, up: 0};
    return {side: view.side + gun.side, up: view.up + gun.up, back: gun.back, shown: layers.gun && series.aim !== 'hip' && series.motions.gun !== null};
  }
  // How long a bullet is drawn on its way from the eye to the wall (s).
  const FLIGHT = .04;
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
    // The motions shown: recoil (the kick and the hip's shake), the camera's clip, the gun's.
    const layerInputs = {recoil: find('.sim-layer-recoil'), camera: find('.sim-layer-camera'), gun: find('.sim-layer-gun')};
    const layers = () => Object.fromEntries(Object.entries(layerInputs).map(([key, input]) => [key, input.checked]));
    function lastShot() {
      return Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
    }
    function draw() {
      if (simulation.kind !== 'ready') return;
      const time = simulation.time;
      find('.sim-last').disabled = time >= lastShot() - .001;
      timeInput.value = time;
      find('.sim-time-label').textContent = `${time.toFixed(2)} ${Tc('unit', 's')}`;
      const legend = find('.sim-legend'); legend.replaceChildren();
      for (const series of simulation.series) {
        const point = sample(series, time), label = sides.length > 1 ? `${series.side ? 'B' : 'A'} · ` : '';
        legend.append(node('p', `${label}${series.title} · ${T('shots')} ${series.events.filter(event => event.time <= time + 1e-9).length}/${series.shots} · ${T('up')} ${point.vertical.toFixed(3)} · ${T('side')} ${point.horizontal.toFixed(3)} · ${T('spread')} ${point.spread.toFixed(2)}°`, `series-${series.side}`));
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
        const label = node('th', sides.length > 1 ? `${series.side ? 'B' : 'A'} · ${series.title}` : series.title); label.scope = 'row';
        row.append(label, ...(spread ? [spread.base, spread.k * spread.stance, spread.k * spread.perShot, spread.base + spread.k * (spread.stance + spread.max)]
          .map(value => node('td', degrees(value))) : [node('td', '—')]));
        return row;
      }));
    }
    // The view as the shot moves it, in degrees of view at true scale both
    // ways: its path, each shot's mark and bullet's hit, the spread around it,
    // and the gun's sight where its own kick puts it. Under it the same from
    // the side: the bullet from the eye along the view. Each weapon has its
    // own pair, all on one scale so they compare.
    function drawSight(time) {
      const on = layers();
      // In degrees of view: the kick as the camera turns. The pattern is the
      // weapon's: a target changes nothing of it, only its own size shows the
      // distance (cut by the view's edge when it is the larger).
      const distance = Number(find('.sim-target').value) || 0, body = distance ? bodyDegrees(distance) : 0;
      // The aim (kick and shake), each bullet's hit and the spread around the
      // aim, in degrees of view: the view holds them all.
      const hit = (series, event) => {
        const point = viewAt(series, event.time, on);
        return {side: point.side + event.bullet.side, up: point.up + event.bullet.up};
      };
      // The path drawn every 1/240 s, fine enough for the shake and a clip's pass.
      const steps = series => Array.from({length: Math.floor(series.duration * 240) + 1}, (_, i) => i / 240);
      const points = [...simulation.series.flatMap(series => steps(series).flatMap(t => [{...viewAt(series, t, on), r: sample(series, t).spread / 2}, {...sightAt(series, t, on), r: 0}])),
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
        const svg = svgNode('svg', {viewBox:'0 0 300 300', role:'img', 'aria-label':`${series.title}: ${T('where it aims as the shot moves it')}`});
        const from = svgNode('svg', {viewBox:'0 0 300 110', role:'img', class:'sim-side', 'aria-label':`${series.title}: ${T('from the side, the bullet from the eye along the view')}`});
        const view = document.createElement('div'); view.className = 'sim-view';
        if (simulation.series.length > 1) view.append(node('p', `${series.side ? 'B' : 'A'} · ${series.title}`, `sim-view-name series-${series.side}`));
        view.append(svg, from);
        return {series, svg, from, view};
      });
      find('.sim-sight-views').replaceChildren(...views.map(({view}) => view));
      // From the side: the eye at the left, a wall at the right, the view's
      // angle up drawn to the same span as the view above.
      const sideY = degrees => 12 + (up - degrees) * 86 / Math.max(.1, up + down);
      for (const {series, svg, from} of views) {
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
        const past = [...steps(series).filter(t => t < held), held].map(t => viewAt(series, t, on));
        svg.append(svgNode('path', {d:past.map((point, i) => `${i ? 'L' : 'M'}${x(point.side).toFixed(2)},${y(point.up).toFixed(2)}`).join(' '),
          class:`sim-trail series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= held)) {
          const point = viewAt(series, event.time, on), [hx, hy] = hitAt(series, event);
          svg.append(svgNode('circle', {cx:x(point.side), cy:y(point.up), r:2, class:`sim-shot series-${series.side}`}));
          svg.append(svgNode('circle', {cx:hx, cy:hy, r:2.6, class:`sim-hit series-${series.side}`}));
        }
        const frame = sample(series, held), point = viewAt(series, held, on), cx = x(point.side), cy = y(point.up);
        // The spread: the cone's half angle around the aim.
        const radius = frame.spread / 2;
        if (radius > 0) svg.append(svgNode('circle', {cx, cy, r:radius * scale, class:`sim-spread series-${series.side}`}));
        const cross = svgNode('g', {class:`sim-cross series-${series.side}`});
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          cross.append(svgNode('line', {x1:cx + dx * 5, y1:cy + dy * 5, x2:cx + dx * 15, y2:cy + dy * 15}));
        }
        cross.append(svgNode('circle', {cx, cy, r:1.6}));
        svg.append(cross);
        // The gun's sight: a ring where the gun's own kick points it, larger as the gun comes back.
        const sight = sightAt(series, held, on);
        if (sight.shown) {
          const ring = svgNode('g', {class:`sim-gunsight series-${series.side}`});
          ring.append(svgNode('circle', {cx:x(sight.side), cy:y(sight.up), r:9 * (1 + sight.back / 250)}));
          ring.append(svgNode('circle', {cx:x(sight.side), cy:y(sight.up), r:1.8, class:'sim-gunsight-dot'}));
          svg.append(ring);
        }
        // From the side: the view's line from the eye (dashed: a bullet's way),
        // the sight's line, and each bullet from the eye to the wall along the
        // view it left on.
        const eye = [20, sideY(0)], wall = 284;
        from.append(svgNode('line', {x1:wall, x2:wall, y1:4, y2:106, class:'sim-wall'}));
        from.append(svgNode('line', {x1:eye[0], y1:eye[1], x2:wall, y2:sideY(point.up), class:`sim-side-view series-${series.side}`}));
        if (sight.shown) from.append(svgNode('line', {x1:eye[0], y1:eye[1], x2:wall, y2:sideY(sight.up), class:`sim-side-sight series-${series.side}`}));
        for (const event of series.events.filter(event => event.time <= held)) {
          const flown = Math.min(1, (held - event.time) / FLIGHT), to = sideY(hit(series, event).up);
          from.append(svgNode('circle', {cx:eye[0] + (wall - eye[0]) * flown, cy:eye[1] + (to - eye[1]) * flown, r:flown < 1 ? 3.2 : 2.6,
            class:`${flown < 1 ? 'sim-bullet' : 'sim-landed'} series-${series.side}`}));
        }
        from.append(svgNode('path', {d:`M${eye[0] - 9},${eye[1]} Q${eye[0]},${eye[1] - 8} ${eye[0] + 9},${eye[1]} Q${eye[0]},${eye[1] + 8} ${eye[0] - 9},${eye[1]} Z`, class:'sim-eye'}));
        from.append(svgNode('circle', {cx:eye[0], cy:eye[1], r:2.6, class:'sim-eye-pupil'}));
      }
    }
    // Play: the burst in real time, from where the timeline is (or the start),
    // to the last shot; the settling after it stays on the plots.
    function stop() {
      if (playing) cancelAnimationFrame(playing.frame);
      playing = null; playButton.textContent = T('Play');
    }
    function play() {
      if (simulation.kind !== 'ready') return;
      const last = Math.max(...simulation.series.flatMap(series => series.events).map(event => event.time));
      const from = simulation.time >= last - .001 ? 0 : simulation.time, start = performance.now();
      const speed = Number(find('.sim-speed').value) || 1;
      playButton.textContent = T('Pause');
      const tick = now => {
        if (simulation.kind !== 'ready') { stop(); return; }
        // A frame's timestamp can precede the press: never before `from`.
        simulation.time = Math.min(last, from + Math.max(0, now - start) / 1000 * speed); draw();
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
      const valid = sides.length > 0 && sides.every(({id, round}, i) => model(id, round, firing[i].kick, firing[i].custom.recoil).recoil);
      for (const element of root.querySelectorAll('input,select,button')) element.disabled = !valid;
      find('.sim-plots').hidden = !valid;
      find('.sim-sight').hidden = !valid;
      find('.sim-unavailable').hidden = valid;
      if (!valid) { find('.sim-legend').replaceChildren(); find('.sim-parts tbody').replaceChildren(); return; }
      seedInput.value = Math.max(1, Math.min(999999, Math.floor(Number(seedInput.value) || 1)));
      // A full magazine; with two weapons, the smaller one's from each.
      const shots = Math.min(...sides.map(({id, round}, i) => model(id, round, firing[i].kick, firing[i].custom.recoil).recoil.magazine));
      const series = sides.map(({id, round, side}, i) => {
        const state = firing[i], {recoil, spread: weapon} = model(id, round, state.kick, state.custom.recoil), spread = weapon && WeaponSpread.condition(weapon, state);
        const result = WeaponRecoil.simulate(recoil, shots, Number(seedInput.value), {aim: state.aim, spread, recoil: state.kit.recoil},
          Number(find('.sim-trigger').value));
        // An initial un-fired frame makes Reset/Next shot unambiguous in the UI.
        result.frames.forEach(frame => {frame.time += .05;});
        result.events.forEach(event => {event.time += .05;});
        result.frames.unshift({...result.frames[0], time:0, vertical:0, horizontal:0, shots:0, z:0, shake:{side:0, up:0}});
        const shake = result.shake && {...result.shake, start: result.shake.start + .05, stop: result.shake.stop + .05};
        return {...result, id, side, spread, shake, duration: result.duration + .05, title: titleOf(side, id), aim: state.aim,
          interval: recoil.interval / 1000, motions: WeaponMotions.custom(motionsOf.get(id), state.custom)};
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
    for (const input of Object.values(layerInputs)) input.addEventListener('change', draw);
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
  // Fully aimed first: how most shots that matter are fired.
  const picks = panels.map(() => ({aim: 'aimed', stance: 'idle', arms: '0'}));
  // A custom gun: a side's own values over its weapon's. `recoil` by the
  // config's keys (WeaponRecoil.parse), `camera` and `gun` each motion's
  // peaks by axis (WeaponMotions.custom). A value the visitor left is absent.
  const custom = panels.map(() => ({recoil: {}, camera: {}, gun: {}}));
  const isCustom = side => Object.values(custom[side]).some(values => Object.keys(values).length > 0);
  const titleOf = (side, id) => isCustom(side) ? `${name(id)} · ${T('custom')}` : name(id);
  // Its fields, a group a motion (named as its layer): [key, label, step].
  const up = T('Up'), sideways = T('Side');
  const FIELDS = [
    ['recoil', T('Recoil'), [
      ['rpm', T('Rate'), 1],
      ['min_top_recoil', `${up} min`, .01], ['max_top_recoil', `${up} max`, .01],
      ['min_left_recoil', `${Tc('direction', 'Left')} min`, .01], ['max_left_recoil', `${Tc('direction', 'Left')} max`, .01],
      ['min_right_recoil', `${Tc('direction', 'Right')} min`, .01], ['max_right_recoil', `${Tc('direction', 'Right')} max`, .01],
      ['recoil_power', T('Power'), .01],
      ['compensation_proportional_gain', 'P', .1], ['compensation_integral_gain', 'I', .1], ['compensation_derivational_gain', 'D', .1]]],
    ['camera', T('Camera wobble'), [['up', `${up} °`, .01], ['side', `${sideways} °`, .01]]],
    ['gun', T('Gun kick'), [['back', `${Tc('direction', 'Back')} mm`, .1], ['up', `${up} °`, .01], ['side', `${sideways} °`, .01]]],
  ];
  // A field's value on the weapon itself (a motion's: its curve's peak).
  function own(id, group, key) {
    if (group === 'recoil') return raws.get(id)[key];
    const motions = motionsOf.get(id);
    return +WeaponMotions.peak(group === 'gun' ? motions.gun : motions.camera.aimed, key).toFixed(3);
  }
  function stateOf(side) {
    const pick = picks[side], chosen = rows.filter(row => pick[row.key]);
    return {aim: pick.aim, stance: pick.stance, arms: Number(pick.arms), custom: custom[side],
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
    // The custom gun's fields, each the weapon's value until changed.
    const fields = HUD.el('details', {class: 'sim-custom'});
    fields.append(node('summary', T('Custom gun')));
    for (const [group, heading, keys] of FIELDS) {
     const grid = HUD.el('div', {class: 'sim-custom-fields'});
     fields.append(node('p', heading, 'sim-custom-group'), grid);
     for (const [key, label, step] of keys) {
      const input = HUD.el('input', {type: 'number', step: String(step)}), field = node('label', label);
      input.dataset.group = group; input.dataset.key = key;
      input.addEventListener('change', () => {
        const id = slots.sides()[side].id, value = Number(input.value);
        if (!id) return;
        if (input.value === '' || !Number.isFinite(value) || value === own(id, group, key)) {
          delete custom[side][group][key]; input.value = own(id, group, key) ?? '';
        } else custom[side][group][key] = value;
        panel.querySelector('.sim-setup-name').textContent = `${side ? 'B' : 'A'} · ${titleOf(side, id)}`;
        simulation.refresh();
      });
      field.append(input); grid.append(field);
     }
    }
    const reset = node('button', T('Reset')); reset.type = 'button';
    reset.addEventListener('click', () => { fill(side, slots.sides()[side].id); simulation.refresh(); });
    fields.append(reset); panel.append(fields);
  });
  // A side's custom gun starts as its weapon: every field the weapon's value.
  function fill(side, id) {
    custom[side] = {recoil: {}, camera: {}, gun: {}};
    for (const input of panels[side].querySelectorAll('.sim-custom input')) input.value = id ? own(id, input.dataset.group, input.dataset.key) ?? '' : '';
    if (id) panels[side].querySelector('.sim-setup-name').textContent = `${side ? 'B' : 'A'} · ${name(id)}`;
  }
  const shownIds = panels.map(() => null);
  const simulation = RecoilSim(sim, stateOf), empty = document.querySelector('#compare-empty');
  const slots = WikiWeaponSlots(weapons, roundsOf, collator, () => {
    const sides = slots.sides().map((chosen, side) => ({...chosen, side})).filter(({id}) => id);
    sim.hidden = sides.length === 0; empty.hidden = sides.length !== 0;
    panels.forEach((panel, side) => {
      const id = slots.sides()[side].id;
      panel.hidden = !id;
      // Another weapon in the slot: its custom gun starts over.
      if (id !== shownIds[side]) { shownIds[side] = id; fill(side, id); }
    });
    simulation.show(sides);
  });
  document.querySelector('#clear-comparison').addEventListener('click', () => slots.clear());
  document.querySelector('#comparison').hidden = false; slots.refresh();
})();
