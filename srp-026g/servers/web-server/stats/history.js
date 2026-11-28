// The stats page (servers/web-server/src/history.rs): public matches from the
// match history's JSON (/api/history/..., docs/notes/match_history.md), asked
// for as the page is viewed. The address says what it shows: ?id=<account>
// (a player: its matches, or with ?tab=weapons its weapons, ?tab=skills its
// skills; every link to a player is by the id, as a name can change;
// ?player=<name>, the search's, finds one and becomes its id's address),
// ?match=<number>, ?clan=<tag>, else the leaderboards (?board=, ?by=).
// Each view is drawn once empty, in its final shape, then with its data, so
// nothing moves while it loads; long lists grow by "Older matches".
(() => {
  const root = document.getElementById('stats');
  if (!root) return;
  // The page's language: its links' and the names the API gives (the game's eleven). The page's
  // own words (TEXT, by name) are the server's, in that language where the site has them, else in
  // English: `written` is the one they are in, for the numbers and dates beside them.
  const lang = document.documentElement.lang || 'en';
  const written = root.dataset.locale || 'en';
  const TEXT = JSON.parse(document.getElementById('stats-text').textContent);
  const PAGE = 15, TOP = 50;
  // An empty cell's text, so it has a line's height.
  const BLANK = '\u00a0';
  const BOARDS = {
    players: ['rating', 'matches', 'wins', 'win_rate', 'kills', 'kd', 'score_avg', 'headshot_rate', 'accuracy'],
    clans: ['matches', 'wins', 'kills'],
    best: ['day', 'hour'],
    latest: [],
    // Their periods (`&period=`, PERIODS), all time first.
    maps: ['all', 'month', 'week', 'day'],
    modes: ['all', 'month', 'week', 'day'],
  };
  // The boards whose keys are periods of the matches, said by `&period=` (all time: none).
  const BY_PERIOD = ['maps', 'modes'];
  // What a list by a key leaves out, said beside the boards.
  const KEY_NOTE = {accuracy: 'accuracyNote', score_avg: 'averageNote', win_rate: 'averageNote', kd: 'averageNote',
    headshot_rate: 'averageNote'};
  const numbers = new Intl.NumberFormat(written, {maximumFractionDigits: 2});
  const pair = new Intl.NumberFormat(written, {minimumFractionDigits: 2, maximumFractionDigits: 2});
  const percent = new Intl.NumberFormat(written, {style: 'percent', maximumFractionDigits: 1});
  const hours = new Intl.NumberFormat(written, {minimumFractionDigits: 1, maximumFractionDigits: 1});
  // Accuracy in whole percents: shots count pellets and kits, not aimed bullets.
  const whole = new Intl.NumberFormat(written, {style: 'percent', maximumFractionDigits: 0});
  const metres = new Intl.NumberFormat(written, {style: 'unit', unit: 'meter', maximumFractionDigits: 0});
  const dates = new Intl.DateTimeFormat(written, {dateStyle: 'medium', timeStyle: 'short'});
  const day = new Intl.DateTimeFormat(written, {dateStyle: 'medium'});
  const dayThisYear = new Intl.DateTimeFormat(written, {month: 'short', day: 'numeric'});
  const time = new Intl.DateTimeFormat(written, {timeStyle: 'short'});
  const query = new URLSearchParams(location.search);
  const asked = name => (query.get(name) || '').trim();

  // DOM pieces: text only, never markup from the data.
  function el(tag, props = {}, ...children) {
    const node = Object.assign(document.createElement(tag), props);
    node.append(...children.flat().filter(child => child !== null && child !== undefined && child !== false));
    return node;
  }
  const address = params => `/stats?${new URLSearchParams({...params, lang})}`;
  const link = (params, text) => el('a', {href: address(params)}, text);
  const n = value => value === null || value === undefined ? '—' : numbers.format(value);
  const kd = value => value === null || value === undefined ? '—' : pair.format(value);
  const rate = value => value === null || value === undefined ? '—' : percent.format(value);
  // Hits by shots of totals, a match line or a kit; none without a shot.
  const aim = counts => counts.shots ? whole.format(counts.accuracy) : '—';
  // How many of its kills were headshots; none without a kill.
  const headshotRate = counts => counts.kills ? whole.format(counts.headshots / counts.kills) : '—';
  // A map's, a mode's or an item's name: null for one the game data no longer has.
  const name = value => value ?? '—';
  const when = seconds => dates.format(new Date(seconds * 1000));
  // A match list's: the day (its year only when not this one) over the time.
  const ended = seconds => {
    const date = new Date(seconds * 1000);
    return [(date.getFullYear() === new Date().getFullYear() ? dayThisYear : day).format(date),
      el('small', {className: 'stats-under'}, time.format(date))];
  };
  const twoLines = () => [BLANK, el('small', {className: 'stats-under'}, BLANK)];
  const duration = seconds => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  const say = (template, value) => template.replace('{}', value);
  function playerLink(player) {
    return el('span', {}, player.clan ? [link({clan: player.clan.tag}, `[${player.clan.tag}]`), ' '] : [],
      link({id: player.id}, player.name));
  }

  // A heading's look: the page's title, the line over it, the breadcrumb's last part.
  function heading(title, eyebrow, crumb) {
    document.getElementById('stats-title').textContent = title;
    document.getElementById('stats-eyebrow').textContent = eyebrow;
    document.getElementById('stats-crumb').textContent = crumb ? ` / ${crumb}` : '';
    // The way back to the leaderboards, where there is one to go.
    document.querySelector('.breadcrumb').hidden = !crumb;
    document.title = `${title} · ${document.title.split(' · ').pop()}`;
  }

  // Panels of label/value pairs; values null while loading.
  function panels(...groups) {
    return el('div', {className: 'weapon-details stats-panels'}, groups.map(([title, rows]) =>
      el('section', {}, el('h2', {}, title), el('dl', {}, rows.map(([label, value]) =>
        el('div', {}, el('dt', {}, label), el('dd', {}, value === null ? BLANK : value)))))));
  }

  // A table: columns [label, cell(row), className, sort key, empty()]; rows
  // null while loading (`placeholder` empty rows of the same height: a
  // column's `empty` gives its empty cell the shape of a full one). With
  // `open` (where a row leads: the stats' own parameters, or an address;
  // null for nowhere), a click anywhere on a row goes there, but on a link
  // of its own or while text is selected. With `sort` ({ask, up}), the
  // heading of a column with a key is a button that asks for the list by
  // it (`ask(key)`); `up`: the sorted one is smallest first.
  function table(columns, rows, placeholder = PAGE, sorted = '', open = null, sort = null) {
    const body = el('tbody');
    const head = el('thead', {}, el('tr', {}, columns.map(([label, , kind, key]) => {
      const th = el('th', {className: `${kind || ''}${key && key === sorted ? ' sorted' : ''}`, scope: 'col'}, label);
      const up = Boolean(sort && sort.up);
      if (key && key === sorted) th.setAttribute('aria-sort', up ? 'ascending' : 'descending');
      if (key && key === sorted && up) th.classList.add('up');
      if (sort && key) {
        const button = el('button', {type: 'button'}, label);
        button.addEventListener('click', () => sort.ask(key));
        th.replaceChildren(button);
      }
      return th;
    })));
    const wrap = el('div', {className: 'table-scroll'}, el('table', {className: 'stats-table'}, head, body));
    wrap.add = list => body.append(...list.map(row => {
      const to = open && open(row);
      const line = el('tr', {className: to ? 'stats-open' : ''}, columns.map(([, cell, kind, key]) =>
        el('td', {className: `${kind || ''}${key && key === sorted ? ' sorted' : ''}`}, cell(row))));
      if (to) line.addEventListener('click', event => {
        if (!event.target.closest('a') && !String(getSelection())) location.assign(typeof to === 'string' ? to : address(to));
      });
      return line;
    }));
    wrap.blank = count => {
      for (let i = 0; i < count; i++) body.append(el('tr', {}, columns.map(([, , kind, , empty]) => el('td', {className: kind || ''}, empty ? empty() : BLANK))));
    };
    if (rows) wrap.add(rows);
    else wrap.blank(placeholder);
    return wrap;
  }

  const notice = text => el('p', {className: 'stats-empty'}, text);

  // GET /api/history/<path>; null for a 404, else throws a line to show.
  // The page is aria-busy while one is on its way.
  let asking = 0;
  async function api(path, params) {
    root.setAttribute('aria-busy', String(++asking > 0));
    try {
      const response = await fetch(`/api/history/${path}?${new URLSearchParams({...params, lang})}`);
      if (response.status === 404 || response.status === 400) return null;
      if (response.status === 503) throw TEXT.noHistory;
      if (!response.ok) throw TEXT.failed;
      return await response.json();
    } catch (error) {
      throw typeof error === 'string' ? error : TEXT.failed;
    } finally {
      root.setAttribute('aria-busy', String(--asking > 0));
    }
  }

  // A list that grows by `before=` pages: its table and the button under it.
  // `next` follows every full page, so the last one may come back empty:
  // nothing is added and the button goes.
  function paged(columns, load, empty, open) {
    const box = el('div', {className: 'stats-paged'});
    const more = el('button', {type: 'button', className: 'quiet', hidden: true}, TEXT.older);
    let next = null, shown = 0, placeholder = table(columns, null);
    box.append(placeholder, more);
    async function page() {
      more.disabled = true;
      let data;
      try {
        data = await load(next);
      } catch (error) {
        box.replaceChildren(notice(String(error)));
        return;
      }
      if (placeholder) {
        const real = table(columns, [], PAGE, '', open);
        placeholder.replaceWith(real);
        placeholder = null;
        box.table = real;
      }
      const rows = data ? data.matches : [];
      box.table.add(rows);
      shown += rows.length;
      next = data ? data.next : null;
      more.hidden = next === null;
      more.disabled = false;
      if (!shown) box.replaceChildren(notice(empty));
    }
    more.addEventListener('click', page);
    page();
    return box;
  }

  // A line's outcome: one who left a match that had a winner lost it.
  function result(row) {
    const text = {won: TEXT.win, lost: TEXT.loss}[row.outcome] || (row.winner === 'draw' ? TEXT.draw : '—');
    return row.left ? `${text} · ${TEXT.left}` : text;
  }
  // The weapons of the kits a match line used, on one line (kits by " / "),
  // each kit in full with its counters on hover.
  function kitWeapons(row, under = false) {
    const items = list => list.length ? list.map(item => name(item.name)).join(', ') : '—';
    const line = row.kits.length
      ? row.kits.map(kit => kit.weapons.length ? kit.weapons.map(item => name(item.name)).join(' · ') : '—').join(' / ')
      : '—';
    const detail = row.kits.map(kit => `${TEXT.kit} ${kit.kit + 1}: ${items(kit.weapons)}; ${items(kit.armour)}\n`
      + `${TEXT.kills} ${n(kit.kills)} · ${TEXT.headshots} ${n(kit.headshots)} · ${TEXT.accuracy} ${aim(kit)}`).join('\n');
    return el('span', {className: `stats-kit${under ? ' stats-under' : ''}`, title: detail}, line);
  }
  // A match line's player with the weapons under the name.
  const playerKit = row => [playerLink(row.player), row.left ? el('small', {}, ` · ${TEXT.left}`) : null, kitWeapons(row, true)];

  // A player's most used weapons and armour (player/equipment), FAVOURITES
  // rows each, empty ones too, so its height is the same loading or not.
  const FAVOURITES = 5;
  // An item's place in the wiki: a weapon's card, an armour piece's; none
  // for one the game data no longer has.
  const wiki = (page, anchor) => item => item.key ? `/wiki/${page}?${new URLSearchParams({lang})}#${anchor}${item.key}` : null;
  const wikiWeapon = wiki('weapons', 'wpn_');
  // An item's icon as the lobby draws it, beside its name (a link with
  // `href`); an empty frame of its size while loading, in an empty row,
  // and for an item without one.
  const itemCell = (item, href) => el('span', {className: 'stats-item-cell'},
    item && item.icon ? el('img', {className: 'stats-item', src: item.icon, alt: '', width: 54, height: 26, loading: 'lazy'})
      : el('span', {className: 'stats-item'}),
    item ? (href ? el('a', {href}, name(item.name)) : name(item.name)) : BLANK);
  function favourites(data) {
    const list = (label, items, where) => {
      const box = table([[label, item => itemCell(item, where(item)), 'l', '', () => itemCell(null)],
        [TEXT.matches, item => n(item.matches)]], items, FAVOURITES, '', where);
      if (items) box.blank(FAVOURITES - items.length);
      return box;
    };
    return [el('h2', {className: 'stats-heading'}, TEXT.favourites),
      el('div', {className: 'stats-favourites'}, list(TEXT.weapon, data && data.weapons, wikiWeapon), list(TEXT.armour, data && data.armour, wiki('equipment', '')))];
  }
  // A player's weapons (player/weapons): what its games counted of each in
  // its counted matches, sorted by the column whose heading was clicked
  // (`?by=`; most kills first at first); K/D by the deaths with it in the
  // hands. WEAPONS empty rows while loading, and without an `id` yet; a
  // list sorted anew stays until the new one is here.
  const WEAPONS = 5;
  const WEAPON_KEYS = ['kills', 'kd', 'kills_per_minute', 'headshot_rate', 'accuracy', 'kill_distance', 'wall_kills', 'ricochet_kills', 'held'];
  function weaponList(id) {
    const columns = [[TEXT.weapon, item => itemCell(item, wikiWeapon(item)), 'l', '', () => itemCell(null)],
      [TEXT.kills, item => n(item.kills), '', 'kills'], [TEXT.kd, item => kd(item.kd), '', 'kd'],
      [TEXT.killsPerMinute, item => kd(item.kills_per_minute), '', 'kills_per_minute'], [TEXT.headshotRate, headshotRate, '', 'headshot_rate'],
      [TEXT.accuracy, aim, '', 'accuracy'],
      [TEXT.avgDistance, item => item.kill_distance_m === null ? '—' : metres.format(item.kill_distance_m), '', 'kill_distance'],
      [TEXT.wallKills, item => n(item.wall_kills), '', 'wall_kills'], [TEXT.ricochetKills, item => n(item.ricochet_kills), '', 'ricochet_kills'],
      [TEXT.hours, item => hours.format(item.held_s / 3600), '', 'held']];
    let by = WEAPON_KEYS.includes(asked('by')) ? asked('by') : WEAPON_KEYS[0], up = asked('order') === 'asc', asking = 0;
    const box = el('div', {className: 'stats-weapons'});
    // A second click on the sorted heading: the smallest first, and back.
    const sort = () => ({up, ask: key => {
      up = key === by && !up;
      by = key;
      const url = new URL(location.href);
      url.searchParams.set('by', by);
      if (up) url.searchParams.set('order', 'asc'); else url.searchParams.delete('order');
      history.replaceState(null, '', url);
      keepLanguage();
      load();
    }});
    function load() {
      const mine = ++asking;
      if (!box.firstChild || id === null) box.replaceChildren(table(columns, null, WEAPONS, by, null, sort()));
      if (id !== null) api('player/weapons', {id, by, limit: TOP, ...(up ? {order: 'asc'} : {})}).then(data => {
        if (mine !== asking) return;
        const rows = data ? data.weapons : [];
        box.replaceChildren(rows.length ? table(columns, rows, 0, by, wikiWeapon, sort()) : notice(TEXT.noWeapons));
      }, error => { if (mine === asking) box.replaceChildren(notice(String(error))); });
    }
    load();
    return box;
  }
  const matchLink = row => link({match: row.id}, `${row.id}`);
  // A map's name, leading to it in the wiki's Maps page in the match's mode;
  // plain for a map the game data no longer has.
  // (A mode srp no longer knows has no rules: the map in its first mode.)
  const mapAddress = row => `/wiki/maps?${new URLSearchParams({lang})}#${row.map_key}${row.rules === null || row.rules === undefined ? '' : `/${row.rules}`}`;
  const mapLink = row => row.map_key ? el('a', {href: mapAddress(row)}, row.map) : name(row.map);
  // A mode's name, linked to its section on the wiki's Modes page.
  const modeLink = row => row.mode_url ? el('a', {href: row.mode_url}, name(row.mode)) : name(row.mode);
  // A map's picture as the rooms draw it, beside its name and mode; an
  // empty frame of its size while loading, and for a map without one.
  // The picture leads where the name does.
  // `big`: the Maps board's, a map's own heading. Either is a box of the
  // picture's 16:9 that it covers, never stretched.
  const mapPicture = (row, big = false) => {
    const kind = `stats-map${big ? ' stats-map-big' : ''}`;
    if (!row || !row.map_icon) return el('span', {className: kind});
    const picture = el('img', {className: kind, src: row.map_icon, alt: '', width: big ? 112 : 64, height: big ? 63 : 36, loading: 'lazy'});
    return row.map_key ? el('a', {className: 'stats-map-link', href: mapAddress(row), ariaLabel: row.map, tabIndex: -1}, picture) : picture;
  };
  const mapCell = row => el('span', {className: 'stats-map-cell'}, mapPicture(row),
    el('span', {}, row ? [mapLink(row), el('small', {className: 'stats-under'}, name(row.mode))] : twoLines()));
  // Where a click on a match's row goes, and on a player's.
  const openMatch = row => ({match: row.id});
  const openPlayer = row => ({id: row.player.id});
  // The two teams' points; a free-for-all has none.
  const points = row => row.winner === 'free_for_all' ? '—' : row.team_points.map(n).join(' : ');
  // Two lines a match: the time under the day, the mode under the map.
  const MATCH_COLUMNS = [
    [TEXT.ended, row => ended(row.ended_at), 'l', '', twoLines], [TEXT.match, matchLink],
    [TEXT.map, mapCell, 'l', '', () => mapCell(null)],
    [TEXT.level, row => n(row.level)],
  ];

  // The day totals of the last 30 days, added up.
  function lastDays(days) {
    const sum = key => days.reduce((total, day) => total + day[key], 0);
    const matches = sum('matches'), kills = sum('kills'), deaths = sum('deaths');
    return [[TEXT.matches, n(matches)], [TEXT.winRate, matches ? rate(sum('wins') / matches) : '—'],
      [TEXT.kd, kd(kills / Math.max(deaths, 1))],
      [TEXT.scoreAvg, matches ? n(Math.round(10 * sum('score') / matches) / 10) : '—']];
  }
  const blank = labels => labels.map(label => [label, null]);
  function combat(totals) {
    const v = key => totals ? n(totals[key]) : null;
    // A minute is the player's own time in its matches (played_s), as the
    // weapons' Kills/min is a weapon's time in the hands; experience's is
    // that of the matches that kept their experience. '—' without one.
    return [[TEXT.kills, v('kills')], [TEXT.deaths, v('deaths')], [TEXT.kd, totals ? kd(totals.kd) : null],
      [TEXT.killsPerMinute, totals ? kd(totals.kills_per_minute) : null],
      [TEXT.experiencePerMinute, totals ? n(totals.experience_per_minute) : null],
      [TEXT.accuracy, totals ? aim(totals) : null], [TEXT.assists, v('assists')],
      [TEXT.headshotRate, totals ? headshotRate(totals) : null], [TEXT.grenadeKills, v('grenade_kills')], [TEXT.meleeKills, v('melee_kills')],
      [TEXT.batteries, v('batteries')], [TEXT.captures, v('captures')], [TEXT.artefacts, v('artefacts')]];
  }
  function record(totals) {
    const v = key => totals ? n(totals[key]) : null;
    return [[TEXT.matches, v('matches')], [TEXT.wins, v('wins')], [TEXT.losses, v('losses')],
      [TEXT.winRate, totals ? (totals.matches ? rate(totals.win_rate) : '—') : null],
      [TEXT.scoreAvg, totals ? (totals.matches ? n(totals.score_avg) : '—') : null],
      [TEXT.hours, totals ? hours.format(totals.played_s / 3600) : null]];
  }

  // The names a player left, the last left first, each saying until when
  // on a hover: the last FORMER_SHOWN, the rest behind a button that says
  // how many they are.
  const FORMER_SHOWN = 5;
  function formerNames(names) {
    if (!names.length) return '—';
    const said = list => list.flatMap((former, i) => [i ? ', ' : '',
      el('span', {title: say(TEXT.until, day.format(new Date(former.until * 1000)))}, former.name)]);
    const box = el('span', {className: 'stats-names'}, said(names.slice(0, FORMER_SHOWN)));
    if (names.length > FORMER_SHOWN) {
      const rest = el('button', {type: 'button', className: 'quiet', ariaLabel: TEXT.formerNames}, `+${n(names.length - FORMER_SHOWN)}`);
      rest.addEventListener('click', () => box.replaceChildren(...said(names)));
      box.append(' ', rest);
    }
    return box;
  }
  // A player, by its account's id or by a name (`key`: {id} or {name}).
  async function playerView(key) {
    heading(key.name ?? TEXT.player, TEXT.player, TEXT.player);
    const head = (card, days) => panels(
      [TEXT.overall, [[TEXT.id, card ? String(card.player.id) : null], [TEXT.formerNames, card ? formerNames(card.names) : null], [TEXT.level, card ? n(card.level) : null], [TEXT.experience, card ? n(card.experience) : null],
        [TEXT.rating, card ? n(card.rating) : null],
        [TEXT.clan, card ? (card.player.clan ? link({clan: card.player.clan.tag}, `[${card.player.clan.tag}]`) : '—') : null], ...record(card && card.totals)]],
      [TEXT.inMatches, combat(card && card.totals)],
      [TEXT.last30, days ? lastDays(days) : blank([TEXT.matches, TEXT.winRate, TEXT.kd, TEXT.scoreAvg])]);
    const columns = [...MATCH_COLUMNS, [TEXT.weapons, row => kitWeapons(row), 'l kits'], [TEXT.result, result, 'l'], [TEXT.place, row => n(row.place)],
      [TEXT.score, row => n(row.score)], [TEXT.kills, row => n(row.kills)], [TEXT.deaths, row => n(row.deaths)],
      [TEXT.kd, row => kd(row.kd)], [TEXT.accuracy, aim], [TEXT.assists, row => n(row.assists)], [TEXT.headshotRate, headshotRate],
      [TEXT.rating, row => n(row.rating)]];
    // Its matches, its weapons or its skills, as ?tab= says; empty until
    // the player's card is here.
    const trees = document.getElementById('stats-skills');
    const TABS = [['matches', TEXT.matches], ['weapons', TEXT.weapons], ...(trees ? [['skills', TEXT.skills]] : [])];
    let tab = TABS.some(([value]) => value === asked('tab')) ? asked('tab') : 'matches', id = null, card = null;
    const tabs = el('div', {className: 'slot-filter stats-tabs', role: 'group'});
    const body = el('div');
    // The skills: the wiki's trees (the page's template, src/skills.rs)
    // under the wiki's own script, which draws the build the trees carry
    // (the card's) and takes no change to it. The script looks for the
    // trees once, as it starts: they stay in the page, hidden under
    // another tab, and it starts when the card is here.
    const skills = el('div', {className: 'stats-skills', hidden: true});
    let script = null;
    function drawSkills() {
      if (!skills.firstChild) {
        const made = trees.content.cloneNode(true);
        script = made.querySelector('script');
        script.remove();
        skills.append(made);
      }
      if (card && script) {
        skills.querySelector('.skill-trees').dataset.build = card.skills.map(([skill, points]) => `${skill}.${points}`).join(',');
        skills.append(script);
        script = null;
      }
    }
    function show() {
      for (const button of tabs.children) button.setAttribute('aria-pressed', String(button.dataset.tab === tab));
      const url = new URL(location.href);
      if (tab === 'matches') url.searchParams.delete('tab');
      else url.searchParams.set('tab', tab);
      if (tab !== 'weapons') for (const gone of ['by', 'order']) url.searchParams.delete(gone);
      history.replaceState(null, '', url);
      keepLanguage();
      body.hidden = tab === 'skills';
      skills.hidden = !body.hidden;
      if (tab === 'skills') return drawSkills();
      body.replaceChildren(tab === 'weapons' ? weaponList(id) : id === null ? table(columns, null)
        : paged(columns, before => api('player/matches', {id, limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches, openMatch));
    }
    for (const [value, label] of TABS) {
      const button = el('button', {type: 'button'}, label);
      button.dataset.tab = value;
      button.addEventListener('click', () => { tab = value; show(); });
      tabs.append(button);
    }
    root.replaceChildren(head(null, null), ...favourites(null), tabs, body, skills);
    show();
    // A player without a public match is not found, as an unknown name.
    card = await api('player', key);
    if (!card) return root.replaceChildren(notice(say(TEXT.noPlayer, key.name ?? key.id)), finder());
    heading(card.player.name, TEXT.player, TEXT.player);
    // The address is the id's from here: the name can change.
    const url = new URL(location.href);
    url.searchParams.delete('player');
    url.searchParams.set('id', card.player.id);
    history.replaceState(null, '', url);
    const [days, equipment] = await Promise.all([api('player/days', {id: card.player.id, days: 30}).catch(() => null),
      api('player/equipment', {id: card.player.id, limit: FAVOURITES}).catch(() => null)]);
    id = card.player.id;
    root.replaceChildren(head(card, days ? days.days : []), ...favourites(equipment || {weapons: [], armour: []}), tabs, body, skills);
    show();
  }

  // A match's replay (replay): one Download. Pressed, the replay is asked
  // for, asked again every two seconds while it is being made (said in
  // its place), and its file then downloads by itself (the game's updater
  // opens it); Download stays, the file's own link now. It stays out of a
  // match without a replay.
  function replayCell(id) {
    const cell = el('span', {className: 'stats-replay'});
    const button = el('button', {type: 'button', className: 'quiet'}, TEXT.replayFile);
    const say = text => cell.replaceChildren(text);
    button.addEventListener('click', async () => {
      // 90 asks: three minutes, as long as the game itself waits.
      for (let asks = 0; asks < 90; asks++) {
        let answer;
        try {
          answer = await api('replay', {id});
        } catch (error) {
          return say(TEXT.replayNo.busy);
        }
        if (!answer) return say(TEXT.replayNo.other);
        if (answer.status === 'ready') {
          const file = el('a', {href: answer.url, download: ''}, TEXT.replayFile);
          cell.replaceChildren(file);
          return file.click();
        }
        if (answer.status !== 'preparing') return say(TEXT.replayNo[answer.status] || TEXT.replayNo.other);
        say(answer.place > 1 ? `${TEXT.replayPreparing} (${n(answer.place)})` : TEXT.replayPreparing);
        await new Promise(resolve => setTimeout(resolve, 2000));
      }
      say(TEXT.replayNo.busy);
    });
    cell.append(button);
    return cell;
  }

  // The two teams' points, each in its team's colour.
  const sidePoints = m => [el('span', {className: 'stats-side stats-team0'}, n(m.team_points[0])), ' : ',
    el('span', {className: 'stats-side stats-team1'}, n(m.team_points[1]))];

  // A list that is whole in the page (`rows`; null while loading) and
  // sorts there: by the column whose heading was clicked, the largest
  // first, the smallest on a second click. A column's sixth piece is the
  // number it sorts by; `key` and `up`: how the list comes.
  function sortable(columns, rows, placeholder, open, key, up) {
    const box = el('div');
    const draw = () => {
      const by = columns.find(column => column[3] === key)[5];
      const sorted = rows && [...rows].sort((a, b) => (up ? 1 : -1) * (by(a) - by(b)));
      box.replaceChildren(table(columns, sorted, placeholder, key, open,
        {up, ask: asked => { up = asked === key && !up; key = asked; draw(); }}));
    };
    draw();
    return box;
  }
  async function matchView(id) {
    heading(`${TEXT.match} ${id}`, TEXT.match, `${TEXT.match} ${id}`);
    const of = (label, key, cell = row => n(row[key]), value = row => row[key]) => [label, cell, '', key, null, value];
    const columns = [of(TEXT.place, 'place'), [TEXT.player, playerKit, 'l', '', twoLines],
      of(TEXT.score, 'score'), of(TEXT.kills, 'kills'), of(TEXT.deaths, 'deaths'), of(TEXT.kd, 'kd', row => kd(row.kd)),
      of(TEXT.accuracy, 'accuracy', aim, row => row.shots ? row.accuracy : -1), of(TEXT.assists, 'assists'),
      of(TEXT.headshotRate, 'headshots', headshotRate, row => row.kills ? row.headshots / row.kills : -1),
      of(TEXT.grenadeKills, 'grenade_kills'), of(TEXT.meleeKills, 'melee_kills'), of(TEXT.rating, 'rating')];
    const view = data => {
      const m = data && data.match;
      // A free-for-all: everyone its own side, one list by place.
      const free = m && m.winner === 'free_for_all';
      const winner = !m ? null : m.winner === 'draw' ? TEXT.draw
        : free ? data.players.filter(row => row.outcome === 'won').map(row => row.player.name).join(', ') || '—'
        : m.winner === null ? '—' : el('span', {className: `stats-side stats-team${m.winner}`}, `${TEXT.team} ${m.winner + 1} ${TEXT.won}`);
      const teams = free
        ? [[el('h2', {className: 'stats-heading'}, TEXT.players), sortable(columns, data.players, 0, openPlayer, 'place', true)]]
        : [0, 1].map(team => {
          const rows = data ? data.players.filter(row => row.team === team) : null;
          const title = `${TEXT.team} ${team + 1}${m ? ` · ${n(m.team_points[team])}` : ''}${m && m.winner === team ? ` · ${TEXT.win}` : ''}`;
          return [el('h2', {className: `stats-heading stats-team stats-team${team}`}, title), sortable(columns, rows, 6, openPlayer, 'place', true)];
        });
      return [panels([TEXT.match, [[TEXT.ended, m ? when(m.ended_at) : null], [TEXT.duration, m ? duration(m.duration_s) : null],
        [TEXT.map, el('span', {className: 'stats-map-cell'}, m ? mapLink(m) : BLANK, mapPicture(m))],
        [TEXT.mode, m ? modeLink(m) : null]]],
      [TEXT.result, [[TEXT.result, winner], [TEXT.points, m ? (free ? '—' : sidePoints(m)) : null],
        [TEXT.level, m ? n(m.level) : null], [TEXT.players, m ? n(m.players) : null],
        // Always there, so that nothing moves when the answer comes; a match without one has none.
        [TEXT.replay, m ? (data.replay ? replayCell(m.id) : '—') : null]]]), ...teams.flat()];
    };
    root.replaceChildren(...view(null));
    const data = /^\d+$/.test(id) ? await api('match', {id}) : null;
    if (!data) return root.replaceChildren(notice(say(TEXT.noMatch, id)), finder());
    root.replaceChildren(...view(data));
  }

  async function clanView(tag) {
    heading(`[${tag}]`, TEXT.clan, TEXT.clan);
    const memberColumns = [[TEXT.player, row => [playerLink(row.player), row.commander ? el('small', {}, ` · ${TEXT.commander}`) : null], 'l'],
      [TEXT.level, row => n(row.level)], [TEXT.rating, row => n(row.rating)], [TEXT.matches, row => n(row.totals.matches)],
      [TEXT.winRate, row => row.totals.matches ? rate(row.totals.win_rate) : '—'], [TEXT.kd, row => kd(row.totals.kd)],
      [TEXT.scoreAvg, row => row.totals.matches ? n(row.totals.score_avg) : '—'], [TEXT.kills, row => n(row.totals.kills)],
      [TEXT.headshotRate, row => headshotRate(row.totals)]];
    const matchColumns = [...MATCH_COLUMNS, [TEXT.points, points], [TEXT.players, row => n(row.players)]];
    const head = data => panels(
      [TEXT.clan, [[TEXT.level, data ? n(data.clan.level) : null], [TEXT.members, data ? n(data.clan.members) : null],
        [TEXT.commander, data ? (data.clan.leader ? link({id: data.clan.leader.id}, data.clan.leader.name) : '—') : null],
        ...record(data && data.totals)]],
      [TEXT.inMatches, combat(data && data.totals)]);
    root.replaceChildren(head(null), el('h2', {className: 'stats-heading'}, TEXT.members), table(memberColumns, null, 8));
    const data = await api('clan', {tag});
    if (!data) return root.replaceChildren(notice(say(TEXT.noClan, tag)), finder());
    heading(`[${data.clan.tag}] ${data.clan.name}`, TEXT.clan, TEXT.clan);
    root.replaceChildren(head(data), el('h2', {className: 'stats-heading'}, TEXT.members), table(memberColumns, data.members, 0, '', openPlayer),
      el('h2', {className: 'stats-heading'}, TEXT.matches),
      paged(matchColumns, before => api('matches', {clan: data.clan.id, limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches, openMatch));
  }

  // The search: a plain form, so it works as the address does.
  function finder() {
    const field = (label, name, props = {}) => el('label', {}, label, el('input', {name, autocomplete: 'off', ...props}));
    const form = el('form', {className: 'catalogue-tools stats-find', action: '/stats', method: 'get'},
      field(TEXT.playerName, 'player', {type: 'search'}), field(TEXT.clanTag, 'clan', {type: 'search'}),
      field(TEXT.matchNumber, 'match', {type: 'number', min: 1}),
      el('input', {type: 'hidden', name: 'lang', value: lang}), el('button', {type: 'submit'}, TEXT.find));
    form.addEventListener('submit', event => {
      event.preventDefault();
      const filled = [...new FormData(form)].filter(([name, value]) => name !== 'lang' && value.trim());
      if (filled.length) location.assign(address(Object.fromEntries(filled.slice(0, 1).map(([name, value]) => [name, value.trim()]))));
    });
    return form;
  }

  // The landing's own (today, totals): the day's leaders, five short lists
  // (the day is UTC's), and everything counted added up. The lists open
  // up: five each, twenty or fifty (`?leaders=`), all as long as the
  // longest, five rows at least. Gone when the day cannot be read.
  const LEADERS = [['kills', 'kills'], ['assists', 'assists'], ['score', 'score'], ['wins', 'wins'], ['played_s', 'hours']];
  const LEADER_SIZES = [5, 20, 50];
  function leaders() {
    const value = (key, amount) => key === 'played_s' ? hours.format(amount / 3600) : n(amount);
    let size = LEADER_SIZES.includes(Number(asked('leaders'))) ? Number(asked('leaders')) : LEADER_SIZES[0], asking = 0;
    const sizes = el('div', {className: 'slot-filter stats-sizes', role: 'group', ariaLabel: TEXT.today});
    const lists = el('div', {className: 'stats-leaders'});
    const box = el('div', {}, el('div', {className: 'stats-heading-row'}, el('h2', {className: 'stats-heading'}, TEXT.today), sizes), lists);
    // A name too long for its list is cut short: whole in its title.
    const named = player => { const name = playerLink(player); name.title = name.textContent; return name; };
    const draw = data => {
      const of = key => data && (data.leaders[key] || []).map((row, rank) => ({...row, rank}));
      const longest = Math.max(LEADER_SIZES[0], ...LEADERS.map(([key]) => (of(key) || []).length));
      lists.replaceChildren(...LEADERS.map(([key, label]) => {
        const rows = of(key);
        const list = table([[TEXT.rank, row => n(row.rank + 1)], [TEXT.player, row => named(row.player), 'l'],
          [el('span', {className: 'stats-over'}, TEXT[label]), row => value(key, row.value)]], rows, longest, '', openPlayer);
        if (rows) list.blank(longest - rows.length);
        return list;
      }));
    };
    // A list asked anew stays until the new one is here.
    function load() {
      const mine = ++asking;
      for (const button of sizes.children) button.setAttribute('aria-pressed', String(Number(button.dataset.size) === size));
      api('today', {limit: size}).then(data => { if (mine === asking) draw(data || {leaders: {}}); }, () => { if (mine === asking) box.replaceChildren(); });
    }
    for (const each of LEADER_SIZES) {
      const button = el('button', {type: 'button'}, n(each));
      button.dataset.size = each;
      button.addEventListener('click', () => {
        size = each;
        const url = new URL(location.href);
        if (size === LEADER_SIZES[0]) url.searchParams.delete('leaders'); else url.searchParams.set('leaders', size);
        history.replaceState(null, '', url);
        keepLanguage();
        load();
      });
      sizes.append(button);
    }
    draw(null);
    load();
    return box;
  }
  // Everything added up: its heading, then one row in the tables' look.
  function allTime(data) {
    const box = table([[TEXT.players, row => n(row.players)], [TEXT.matches, row => n(row.matches)], [TEXT.kills, row => n(row.kills)],
      [TEXT.assists, row => n(row.assists)], [TEXT.shots, row => n(row.shots)], [TEXT.accuracy, aim],
      [TEXT.hours, row => hours.format(row.played_s / 3600)]], data && [data], 1);
    box.classList.add('stats-all');
    return [el('h2', {className: 'stats-heading'}, TEXT.allTime), box, ...historyNote(data)];
  }
  // When the history began, and what it has of before: the matches made again from their
  // recordings, which are not all of them.
  function historyNote(data) {
    if (!data || !data.since) return [];
    const on = at => day.format(new Date(at * 1000));
    const text = data.recorded
      ? TEXT.historySinceRecorded.replaceAll('{0}', on(data.since)).replace('{1}', numbers.format(data.recorded.matches))
        .replace('{2}', on(data.recorded.since))
      : TEXT.historySince.replace('{0}', on(data.since));
    return [el('p', {className: 'stats-note'}, text)];
  }

  // The Maps and Modes boards (maps, modes): the counted public matches of a period (`&period=`,
  // all time first), every version and weather of a map one map.
  const PERIODS = {all: 'allTime', month: 'last30', week: 'last7', day: 'last24'};
  const STEP_NOTE = {hour: 'perHour', day: 'perDay', week: 'perWeek'};
  // A bar of shares, each part ([share 0..1, className, title]) as wide as its share; the rest is
  // the track's. Its numbers are beside it, the parts' own on a hover.
  function bar(parts) {
    return el('span', {className: 'stats-bar'}, parts.filter(([share]) => share > 0).map(([share, kind, title]) => {
      const part = el('span', {className: `stats-bar-part ${kind}`, title});
      part.style.width = `${Math.min(share, 1) * 100}%`;
      return part;
    }));
  }
  // How the team matches of a map and mode ended: a bar (team 1's wins from the left, team 2's
  // from the right, the draws and those of no known end between) over each side's share and
  // count, in the teams' colours. A free-for-all has no sides.
  function sides(row) {
    const s = row.sides;
    if (!s) return '—';
    const teams = s.wins[0] + s.wins[1] + s.draws + s.unknown;
    const said = (label, share, count) => `${label} ${rate(share)} (${n(count)})`;
    const items = [[`${TEXT.team} 1`, s.win_rates[0], s.wins[0], 'stats-team0'], [TEXT.draw, s.draw_rate, s.draws, 'stats-draw'],
      ...(s.unknown ? [[TEXT.noResult, s.unknown / teams, s.unknown, 'stats-unknown']] : []),
      [`${TEXT.team} 2`, s.win_rates[1], s.wins[1], 'stats-team1']];
    return el('span', {className: 'stats-sides'},
      bar(items.map(([label, share, count, kind]) => [share, kind, said(label, share, count)])),
      el('span', {className: 'stats-legend'}, items.map(([label, share, count, kind]) =>
        el('span', {className: `stats-legend-item ${kind}`}, el('b', {}, `${label} ${rate(share)}`), ` (${n(count)})`))));
  }
  // A share of all the period's matches: its number over its bar.
  const shareCell = row => el('span', {className: 'stats-share'}, rate(row.share), bar([[row.share, 'stats-fill', rate(row.share)]]));
  const blankSides = () => el('span', {className: 'stats-sides'}, el('span', {className: 'stats-bar'}), el('span', {className: 'stats-legend'}, BLANK));
  // A step of a trend as its hover says it: the hour's day and time, or the day (a week's Monday).
  const stepName = (step, seconds) => step === 'hour' ? when(seconds) : day.format(new Date(seconds * 1000));
  // A mode's matches in each step of the period, oldest first: columns as tall as their share of
  // the mode's busiest step, each saying its own on a hover.
  function trend(row, data) {
    const most = Math.max(1, ...row.trend);
    return el('span', {className: 'stats-trend', role: 'img',
      ariaLabel: `${TEXT.trend}: ${row.trend.map(n).join(', ')}`}, row.trend.map((count, i) => {
      const column = el('span', {className: `stats-trend-bar${count ? '' : ' zero'}`, title: `${stepName(data.step, data.steps[i])}: ${n(count)}`});
      if (count) column.style.height = `${Math.max(8, 100 * count / most)}%`;
      return column;
    }));
  }
  // A map as the Maps page shows it: in its most played mode.
  const placeRow = place => place && {...place, rules: place.modes.length ? place.modes[0].rules : 0, mode: null};
  // The Maps board: one table, its columns fixed (what opens moves none). A map's row: its picture
  // and name, its team matches (the free-for-alls left out: no side wins one), their average length
  // and how its sides fared; the maps in the order of the rooms' create window (every ported
  // version its own map). Under it, closed until its "Modes (n)" opens them, each mode played on
  // it in the window's order with its own (a free-for-all's: no bar); under a mode, closed until
  // its "Weathers (n)" opens them, its weathers (each its map id).
  const MAP_COLUMNS = () => [[TEXT.map, 'l stats-mode-cell stats-col-map'], [TEXT.matches, 'stats-col-n'], [TEXT.avgLength, 'stats-col-n'],
    [TEXT.sides, 'l stats-sides-cell stats-col-sides']];
  // A button that opens and closes `rows` (each closing what it opened under it).
  function opener(label, rows) {
    const button = el('button', {type: 'button', className: 'stats-opener', ariaExpanded: 'false'}, label);
    button.addEventListener('click', event => {
      // Not the row's own click (a mode's row leads to its wiki section).
      event.stopPropagation();
      const open = button.getAttribute('aria-expanded') !== 'true';
      button.setAttribute('aria-expanded', String(open));
      for (const row of rows) {
        row.hidden = !open;
        if (!open) for (const inner of row.closes || []) inner();
      }
    });
    button.close = () => { if (button.getAttribute('aria-expanded') === 'true') button.click(); };
    return button;
  }
  function mapsTable(data) {
    const head = el('thead', {}, el('tr', {}, MAP_COLUMNS().map(([label, kind]) => el('th', {className: kind, scope: 'col'}, label))));
    const body = el('tbody');
    const line = (kind, cells) => el('tr', {className: kind}, cells.map((cell, i) => el('td', {className: MAP_COLUMNS()[i][1]}, cell)));
    if (!data) {
      for (let i = 0; i < 5; i++) body.append(line('stats-map-row', [el('span', {className: 'stats-map-cell'}, mapPicture(null, true), twoLines()), BLANK, BLANK, blankSides()]));
    } else {
      for (const place of data.maps) {
        const row = placeRow(place);
        // A mode's (or a free-for-all version's) row, closed, with its weathers under it behind its button.
        const withWeathers = (mode, kind) => {
          const weathers = mode.weathers.map(weather => line('stats-weather', [
            el('span', {className: 'stats-map-cell'}, mapPicture({...weather, map: place.map, rules: mode.rules}), weather.weather ?? '—'),
            n(weather.matches), duration(weather.duration_s), sides(weather)]));
          const button = opener(`${TEXT.weathers} (${n(mode.weathers.length)})`, weathers);
          const row = line(kind, [[modeLink(mode), el('span', {className: 'stats-under'}, button)],
            n(mode.matches), duration(mode.duration_s), sides(mode)]);
          row.closes = [() => button.close()];
          for (const each of [row, ...weathers]) each.hidden = true;
          return [row, weathers];
        };
        // Every free-for-all is one mode: its versions under it (each with its weathers), not its weathers.
        const modeRows = place.modes.map(mode => {
          if (!mode.versions) return withWeathers(mode, 'stats-mode-row');
          const versions = mode.versions.map(version => withWeathers(version, 'stats-version-row'));
          const button = opener(`${TEXT.variants} (${n(mode.versions.length)})`, versions.map(([row]) => row));
          const modeRow = line('stats-mode-row', [[modeLink(mode), el('span', {className: 'stats-under'}, button)],
            n(mode.matches), duration(mode.duration_s), sides(mode)]);
          modeRow.closes = [() => button.close()];
          modeRow.hidden = true;
          return [modeRow, versions];
        });
        const button = opener(`${TEXT.modes} (${n(place.modes.length)})`, modeRows.map(([modeRow]) => modeRow));
        body.append(line('stats-map-row', [el('span', {className: 'stats-map-cell'}, mapPicture(row, true),
          el('span', {}, el('b', {className: 'stats-place-name'}, mapLink(row)), el('span', {className: 'stats-under'}, button))),
          n(place.matches), place.matches ? duration(place.duration_s) : '—', sides(place)]), ...modeRows.flat(Infinity));
      }
    }
    return el('div', {className: 'table-scroll'}, el('table', {className: 'stats-table stats-maps-table'}, head, body));
  }
  function mapsBoard(data) {
    if (data && !data.maps.length) return [notice(TEXT.noMatches)];
    return [mapsTable(data)];
  }
  // The Modes board: each mode's matches, its share of them all and its trend, the most played
  // first (the popularity's rank), the trend's steps said under it; then the most played map +
  // mode pairs, each leading to its map in the Maps page.
  const PAIRS = 10;
  function modesBoard(data) {
    if (data && !data.modes.length) return [notice(TEXT.noMatches)];
    const ranked = list => list && list.map((row, rank) => ({...row, rank}));
    // The modes in the create window's order; # is a mode's popularity (1: the most played).
    const modeColumns = [[TEXT.rank, row => n(row.rank)], [TEXT.mode, modeLink, 'l stats-mode-cell'], [TEXT.matches, row => n(row.matches)],
      [TEXT.share, shareCell, 'stats-share-cell'], [TEXT.trend, row => trend(row, data), 'stats-trend-cell']];
    const pairColumns = [[TEXT.rank, row => n(row.rank + 1)], [TEXT.map, mapCell, 'l stats-mode-cell', '', () => mapCell(null)],
      [TEXT.matches, row => n(row.matches)], [TEXT.share, shareCell, 'stats-share-cell']];
    const steps = data && data.steps;
    const note = el('p', {className: 'stats-note stats-trend-note'}, data
      ? `${TEXT.trend}: ${TEXT[STEP_NOTE[data.step]]}, ${stepName(data.step, steps[0])} – ${stepName(data.step, steps[steps.length - 1])}` : BLANK);
    const modes = table(modeColumns, data && data.modes, 8, '', row => row.mode_url || null);
    // Every free-for-all is one mode: its versions under it (the Racket, the Saloon, the Roulette, the Pistols),
    // each with its own numbers, closed until its button opens them.
    if (data) data.modes.forEach((mode, i) => {
      if (!mode.versions) return;
      const versionColumns = modeColumns.map(([label, cell, kind], at) => [label, at === 0 ? () => '' : cell, kind]);
      const rows = mode.versions.map(version => el('tr', {className: 'stats-version-row', hidden: true},
        versionColumns.map(([, cell, kind]) => el('td', {className: kind || ''}, cell(version)))));
      const line = modes.querySelectorAll('tbody tr')[i];
      line.querySelector('.stats-mode-cell').append(el('span', {className: 'stats-under'}, opener(`${TEXT.variants} (${n(mode.versions.length)})`, rows)));
      line.after(...rows);
    });
    const pairs = table(pairColumns, ranked(data && data.pairs), PAIRS, '', row => row.map_key ? mapAddress(row) : null);
    return [modes, note, el('h2', {className: 'stats-heading'}, TEXT.mostPlayed), pairs];
  }

  function boardsView() {
    heading(TEXT.stats, document.getElementById('stats-eyebrow').textContent, '');
    let board = BOARDS[asked('board')] ? asked('board') : 'players';
    let key = BOARDS[board].includes(asked(BY_PERIOD.includes(board) ? 'period' : 'by')) ? asked(BY_PERIOD.includes(board) ? 'period' : 'by') : BOARDS[board][0];
    // Smallest first: a second click on the sorted heading, and back.
    let up = asked('order') === 'asc' && (board === 'players' || board === 'clans');
    // How many pages of the players are open ("More players"): a sort
    // shows as many again, another board starts over.
    let pages = 1;
    const tabs = el('div', {className: 'slot-filter', role: 'group', ariaLabel: TEXT.stats});
    const keys = el('div', {className: 'slot-filter stats-keys', role: 'group'});
    const note = el('small', {className: 'stats-note'});
    const body = el('div', {className: 'stats-board'});
    const playerColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.player, row => playerLink(row.player), 'l'],
      [TEXT.rating, row => n(row.rating), '', 'rating'],
      [TEXT.matches, row => n(row.totals.matches), '', 'matches'], [TEXT.wins, row => n(row.totals.wins), '', 'wins'],
      [TEXT.winRate, row => rate(row.totals.win_rate), '', 'win_rate'], [TEXT.kills, row => n(row.totals.kills), '', 'kills'],
      [TEXT.kd, row => kd(row.totals.kd), '', 'kd'], [TEXT.scoreAvg, row => n(row.totals.score_avg), '', 'score_avg'],
      [TEXT.headshotRate, row => headshotRate(row.totals), '', 'headshot_rate'], [TEXT.accuracy, row => aim(row.totals), '', 'accuracy']];
    const clanColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.clan, row => link({clan: row.tag}, `[${row.tag}] ${row.name}`), 'l'],
      [TEXT.level, row => n(row.level)], [TEXT.members, row => n(row.members)],
      [TEXT.matches, row => n(row.totals.matches), '', 'matches'], [TEXT.wins, row => n(row.totals.wins), '', 'wins'],
      [TEXT.winRate, row => rate(row.totals.win_rate)], [TEXT.kills, row => n(row.totals.kills), '', 'kills'],
      [TEXT.kd, row => kd(row.totals.kd)], [TEXT.scoreAvg, row => n(row.totals.score_avg)],
      [TEXT.headshotRate, row => headshotRate(row.totals)]];
    const bestColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.player, row => playerLink(row.player), 'l'],
      [TEXT.score, row => n(row.score), 'sorted'], [TEXT.kills, row => n(row.kills)], [TEXT.deaths, row => n(row.deaths)],
      [TEXT.match, row => link({match: row.match}, `${row.match}`)], [TEXT.ended, row => when(row.ended_at), 'l'],
      [TEXT.map, mapLink, 'l'], [TEXT.mode, row => name(row.mode), 'l'], [TEXT.level, row => n(row.level)]];
    const latestColumns = [...MATCH_COLUMNS, [TEXT.points, points], [TEXT.players, row => n(row.players)]];
    // A ranked list: its rows numbered.
    const ranked = columns => columns.map(([label, cell, ...rest]) => [label, row => cell(row, row.rank), ...rest]);
    let shown = 0;
    async function draw() {
      const drawing = ++shown;
      for (const button of tabs.querySelectorAll('button')) button.setAttribute('aria-pressed', String(button.dataset.board === board));
      // The best scores' periods, and the maps' and modes', are buttons of a
      // row of their own; a list of players or clans sorts by its columns' headings.
      const periodic = BY_PERIOD.includes(board);
      note.textContent = KEY_NOTE[key] ? TEXT[KEY_NOTE[key]] : '';
      keys.hidden = board !== 'best' && !periodic;
      keys.replaceChildren(...(board === 'best' || periodic ? BOARDS[board] : []).map(value => {
        const button = el('button', {type: 'button'}, TEXT[periodic ? PERIODS[value] : value]);
        button.setAttribute('aria-pressed', String(value === key));
        button.addEventListener('click', () => { key = value; draw(); });
        return button;
      }));
      const url = new URL(location.href);
      url.searchParams.set('board', board);
      if (key && !periodic) url.searchParams.set('by', key); else url.searchParams.delete('by');
      if (periodic && key !== BOARDS[board][0]) url.searchParams.set('period', key); else url.searchParams.delete('period');
      if (up) url.searchParams.set('order', 'asc'); else url.searchParams.delete('order');
      history.replaceState(null, '', url);
      keepLanguage();
      if (periodic) {
        const view = board === 'maps' ? mapsBoard : modesBoard;
        // A period asked anew: the board as it was until the new one is here.
        if (!body.firstChild || body.dataset.board !== board) body.replaceChildren(el('div', {className: 'stats-plays'}, ...view(null)));
        body.dataset.board = board;
        let data;
        try {
          data = await api(board, {period: key});
        } catch (error) {
          if (drawing === shown) body.replaceChildren(notice(String(error)));
          return;
        }
        if (drawing === shown) body.replaceChildren(el('div', {className: `stats-plays stats-${board}`}, ...view(data || {maps: [], modes: [], pairs: []})));
        return;
      }
      body.dataset.board = board;
      if (board === 'latest') return body.replaceChildren(paged(latestColumns, before => api('matches', {limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches, openMatch));
      const [path, params, columns, list, size] = {
        players: ['top', {by: key, limit: TOP, ...(up ? {order: 'asc'} : {})}, playerColumns, data => data.players, TOP],
        clans: ['clans', {by: key, limit: TOP, ...(up ? {order: 'asc'} : {})}, clanColumns, data => data.clans, 20],
        best: ['best', {period: key}, bestColumns, data => data.rows, 20],
      }[board];
      const sorted = board === 'best' ? '' : key;
      const sort = board === 'best' ? null : {up, ask: value => { up = value === key && !up; key = value; draw(); }};
      body.replaceChildren(table(ranked(columns), null, pages > 1 ? body.rows : Math.min(size, body.rows || size), sorted, null, sort));
      const rows = [];
      let data;
      try {
        for (let page = 0; page < pages; page++) {
          data = await api(path, page ? {...params, after: rows.length} : params);
          rows.push(...(data ? list(data) : []));
          if (drawing !== shown || !data || !data.more) break;
        }
      } catch (error) {
        if (drawing === shown) body.replaceChildren(notice(String(error)));
        return;
      }
      if (drawing !== shown) return;
      rows.forEach((row, i) => { row.rank = i; });
      body.rows = rows.length || 1;
      // An empty list that sorts keeps its headings: the way to another.
      const drawn = table(ranked(columns), rows, 0, sorted, {players: openPlayer, clans: row => ({clan: row.tag}), best: row => ({match: row.match})}[board], sort);
      body.replaceChildren(...(rows.length ? [drawn] : sort ? [drawn, notice(TEXT.nobody)] : [notice(TEXT.nobody)]));
      // The players go on past their first page: the next one under the
      // list, in the same order, while one follows.
      if (board !== 'players' || !data || !data.more) return;
      const more = el('button', {type: 'button', className: 'quiet'}, TEXT.morePlayers);
      more.addEventListener('click', async () => {
        more.disabled = true;
        let next;
        try {
          next = await api(path, {...params, after: rows.length});
        } catch (error) {
          return more.replaceWith(notice(String(error)));
        }
        if (drawing !== shown) return;
        pages += 1;
        const added = next ? next.players : [];
        added.forEach((row, i) => { row.rank = rows.length + i; });
        rows.push(...added);
        drawn.add(added);
        body.rows = rows.length;
        if (next && next.more) more.disabled = false; else more.remove();
      });
      body.append(more);
    }
    for (const value of Object.keys(BOARDS)) {
      const button = el('button', {type: 'button'}, TEXT[value]);
      button.dataset.board = value;
      button.addEventListener('click', () => { board = value; key = BOARDS[board][0] || ''; up = false; pages = 1; draw(); });
      tabs.append(button);
    }
    tabs.append(note);
    const today = leaders(), all = el('div', {}, ...allTime(null));
    root.replaceChildren(finder(), today, all, tabs, keys, body);
    api('totals', {}).then(data => data ? all.replaceChildren(...allTime(data)) : all.replaceChildren(), () => all.replaceChildren());
    draw();
  }

  // Each of the header's languages keeps what is shown.
  function keepLanguage() {
    for (const language of document.querySelectorAll('.lang a')) {
      const other = new URL(location.href);
      other.searchParams.set('lang', new URL(language.href).searchParams.get('lang'));
      language.href = other;
    }
  }
  keepLanguage();
  const view = asked('id') ? playerView({id: asked('id')})
    : asked('player') ? playerView({name: asked('player')})
    : asked('match') ? matchView(asked('match'))
    : asked('clan') ? clanView(asked('clan'))
    : boardsView();
  Promise.resolve(view).catch(error => root.replaceChildren(notice(typeof error === 'string' ? error : TEXT.failed), finder()));
})();
