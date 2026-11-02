// The stats page (servers/web-server/src/wiki.rs): public matches from the
// match history's JSON (/api/history/..., docs/notes/match_history.md), asked
// for as the page is viewed. The address says what it shows: ?player=<name>,
// ?match=<number>, ?clan=<tag>, else the leaderboards (?board=, ?by=).
// Each view is drawn once empty, in its final shape, then with its data, so
// nothing moves while it loads; long lists grow by "Older matches".
(() => {
  const root = document.getElementById('stats');
  if (!root) return;
  const lang = document.documentElement.lang === 'uk' ? 'uk' : 'en';
  const TEXT = {
    en: {
      stats: 'Stats', player: 'Player', clan: 'Clan', match: 'Match', matches: 'Matches', find: 'Find',
      playerName: 'Player name', clanTag: 'Clan tag', matchNumber: 'Match number',
      players: 'Players', clans: 'Clans', best: 'Best matches', latest: 'Latest matches',
      hour: 'Hour', day: 'Day', rank: '#', level: 'Level', experience: 'Experience', rating: 'Rating',
      wins: 'Wins', losses: 'Losses', winRate: 'Win rate', kills: 'Kills', deaths: 'Deaths', kd: 'K/D',
      assists: 'Assists', score: 'Score', scoreAvg: 'Avg score', headshots: 'Headshots', grenadeKills: 'Grenade kills',
      meleeKills: 'Melee kills', batteries: 'Batteries', captures: 'Captures', artefacts: 'Artefacts', members: 'Members', commander: 'Commander',
      overall: 'Overall', inMatches: 'In matches', last30: 'Last 30 days', ended: 'Ended', duration: 'Duration',
      map: 'Map', mode: 'Mode', result: 'Result', place: 'Place', team: 'Team', points: 'Points',
      win: 'Win', loss: 'Loss', draw: 'Draw', left: 'Left', won: 'won', older: 'Older matches',
      noMatches: 'No public matches yet.', noPlayer: 'No player named “{}”.', noClan: 'No clan tagged [{}].',
      noMatch: 'No public match {}.', noHistory: 'This server keeps no match history.',
      failed: 'Could not load the stats. Reload the page.', nobody: 'Nobody yet.',
    },
    uk: {
      stats: 'Статистика', player: 'Гравець', clan: 'Клан', match: 'Матч', matches: 'Матчі', find: 'Знайти',
      playerName: 'Ім’я гравця', clanTag: 'Тег клану', matchNumber: 'Номер матчу',
      players: 'Гравці', clans: 'Клани', best: 'Найкращі матчі', latest: 'Останні матчі',
      hour: 'Година', day: 'День', rank: '#', level: 'Рівень', experience: 'Досвід', rating: 'Рейтинг',
      wins: 'Перемоги', losses: 'Поразки', winRate: 'Частка перемог', kills: 'Вбивства', deaths: 'Смерті', kd: 'В/С',
      assists: 'Допомоги', score: 'Очки', scoreAvg: 'Сер. очки', headshots: 'У голову', grenadeKills: 'Гранатами',
      meleeKills: 'Врукопаш', batteries: 'Батареї', captures: 'Захоплення', artefacts: 'Артефакти', members: 'Учасники', commander: 'Командир',
      overall: 'Загалом', inMatches: 'У матчах', last30: 'Останні 30 днів', ended: 'Завершено', duration: 'Тривалість',
      map: 'Мапа', mode: 'Режим', result: 'Результат', place: 'Місце', team: 'Команда', points: 'Бали',
      win: 'Перемога', loss: 'Поразка', draw: 'Нічия', left: 'Вийшов', won: 'перемогла', older: 'Старіші матчі',
      noMatches: 'Публічних матчів ще немає.', noPlayer: 'Гравця «{}» немає.', noClan: 'Клану [{}] немає.',
      noMatch: 'Публічного матчу {} немає.', noHistory: 'Цей сервер не зберігає історію матчів.',
      failed: 'Не вдалося завантажити статистику. Оновіть сторінку.', nobody: 'Поки нікого.',
    },
  }[lang];
  const PAGE = 15, TOP = 50;
  const BOARDS = {
    players: ['rating', 'matches', 'wins', 'kills', 'score', 'headshots', 'batteries', 'artefacts'],
    clans: ['score', 'matches', 'wins', 'kills', 'headshots', 'batteries', 'artefacts'],
    best: ['day', 'hour'],
    latest: [],
  };
  const KEY_TEXT = {rating: 'rating', matches: 'matches', wins: 'wins', kills: 'kills', score: 'score',
    headshots: 'headshots', batteries: 'batteries', artefacts: 'artefacts', day: 'day', hour: 'hour'};
  const numbers = new Intl.NumberFormat(lang, {maximumFractionDigits: 2});
  const pair = new Intl.NumberFormat(lang, {minimumFractionDigits: 2, maximumFractionDigits: 2});
  const percent = new Intl.NumberFormat(lang, {style: 'percent', maximumFractionDigits: 1});
  const dates = new Intl.DateTimeFormat(lang, {dateStyle: 'medium', timeStyle: 'short'});
  const query = new URLSearchParams(location.search);
  const asked = name => (query.get(name) || '').trim();

  // DOM pieces: text only, never markup from the data.
  function el(tag, props = {}, ...children) {
    const node = Object.assign(document.createElement(tag), props);
    node.append(...children.flat().filter(child => child !== null && child !== undefined && child !== false));
    return node;
  }
  const address = params => `/wiki/stats?${new URLSearchParams({...params, lang})}`;
  const link = (params, text) => el('a', {href: address(params)}, text);
  const n = value => value === null || value === undefined ? '—' : numbers.format(value);
  const kd = value => value === null || value === undefined ? '—' : pair.format(value);
  const rate = value => value === null || value === undefined ? '—' : percent.format(value);
  // A map's or a mode's name: null for one the game data no longer has.
  const name = value => value ?? '—';
  const when = seconds => dates.format(new Date(seconds * 1000));
  const duration = seconds => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  const say = (template, value) => template.replace('{}', value);
  function playerLink(player) {
    return el('span', {}, player.clan ? [link({clan: player.clan.tag}, `[${player.clan.tag}]`), ' '] : [],
      link({player: player.name}, player.name));
  }

  // A heading's look: the page's title, the line over it, the breadcrumb's last part.
  function heading(title, eyebrow, crumb) {
    document.getElementById('stats-title').textContent = title;
    document.getElementById('stats-eyebrow').textContent = eyebrow;
    document.getElementById('stats-crumb').textContent = crumb ? ` / ${crumb}` : '';
    document.title = `${title} · ${document.title.split(' · ').pop()}`;
  }

  // Panels of label/value pairs; values null while loading.
  function panels(...groups) {
    return el('div', {className: 'weapon-details stats-panels'}, groups.map(([title, rows]) =>
      el('section', {}, el('h2', {}, title), el('dl', {}, rows.map(([label, value]) =>
        el('div', {}, el('dt', {}, label), el('dd', {}, value === null ? ' ' : value)))))));
  }

  // A table: columns [label, cell(row), className]; rows null while loading
  // (`placeholder` empty rows of the same height).
  function table(columns, rows, placeholder = PAGE, sorted = '') {
    const body = el('tbody');
    const head = el('thead', {}, el('tr', {}, columns.map(([label, , kind, key]) =>
      el('th', {className: `${kind || ''}${key && key === sorted ? ' sorted' : ''}`, scope: 'col'}, label))));
    const wrap = el('div', {className: 'table-scroll'}, el('table', {className: 'stats-table'}, head, body));
    wrap.add = list => body.append(...list.map(row => el('tr', {}, columns.map(([, cell, kind, key]) =>
      el('td', {className: `${kind || ''}${key && key === sorted ? ' sorted' : ''}`}, cell(row))))));
    if (rows) wrap.add(rows);
    else for (let i = 0; i < placeholder; i++) body.append(el('tr', {}, columns.map(([, , kind]) => el('td', {className: kind || ''}, ' '))));
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
  function paged(columns, load, empty) {
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
        const real = table(columns, []);
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

  function result(row) {
    if (row.left) return TEXT.left;
    if (row.winner === 'draw') return TEXT.draw;
    if (row.winner === null) return '—';
    return row.won ? TEXT.win : TEXT.loss;
  }
  const matchLink = row => link({match: row.id}, `${row.id}`);
  const MATCH_COLUMNS = [
    [TEXT.ended, row => when(row.ended_at), 'l'], [TEXT.match, matchLink], [TEXT.map, row => name(row.map), 'l'],
    [TEXT.mode, row => name(row.mode), 'l'], [TEXT.level, row => n(row.level)],
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
    return [[TEXT.kills, v('kills')], [TEXT.deaths, v('deaths')], [TEXT.kd, totals ? kd(totals.kd) : null], [TEXT.assists, v('assists')],
      [TEXT.headshots, v('headshots')], [TEXT.grenadeKills, v('grenade_kills')], [TEXT.meleeKills, v('melee_kills')],
      [TEXT.batteries, v('batteries')], [TEXT.captures, v('captures')], [TEXT.artefacts, v('artefacts')]];
  }
  function record(totals) {
    const v = key => totals ? n(totals[key]) : null;
    return [[TEXT.matches, v('matches')], [TEXT.wins, v('wins')], [TEXT.losses, v('losses')],
      [TEXT.winRate, totals ? (totals.matches ? rate(totals.win_rate) : '—') : null],
      [TEXT.scoreAvg, totals ? (totals.matches ? n(totals.score_avg) : '—') : null]];
  }

  async function playerView(name) {
    heading(name, TEXT.player, TEXT.player);
    const head = (card, days) => panels(
      [TEXT.overall, [[TEXT.level, card ? n(card.level) : null], [TEXT.experience, card ? n(card.experience) : null],
        [TEXT.rating, card ? n(card.rating) : null],
        [TEXT.clan, card ? (card.player.clan ? link({clan: card.player.clan.tag}, `[${card.player.clan.tag}]`) : '—') : null], ...record(card && card.totals)]],
      [TEXT.inMatches, combat(card && card.totals)],
      [TEXT.last30, days ? lastDays(days) : blank([TEXT.matches, TEXT.winRate, TEXT.kd, TEXT.scoreAvg])]);
    const columns = [...MATCH_COLUMNS, [TEXT.result, result, 'l'], [TEXT.place, row => n(row.place)],
      [TEXT.score, row => n(row.score)], [TEXT.kills, row => n(row.kills)], [TEXT.deaths, row => n(row.deaths)],
      [TEXT.kd, row => kd(row.kd)], [TEXT.assists, row => n(row.assists)], [TEXT.headshots, row => n(row.headshots)],
      [TEXT.batteries, row => n(row.batteries)], [TEXT.captures, row => n(row.captures)], [TEXT.artefacts, row => n(row.artefacts)],
      [TEXT.rating, row => n(row.rating)]];
    root.replaceChildren(head(null, null), el('h2', {className: 'stats-heading'}, TEXT.matches), table(columns, null));
    const card = await api('player', {name});
    if (!card) return root.replaceChildren(notice(say(TEXT.noPlayer, name)), finder());
    heading(card.player.name, TEXT.player, TEXT.player);
    const days = await api('player/days', {id: card.player.id, days: 30}).catch(() => null);
    root.replaceChildren(head(card, days ? days.days : []), el('h2', {className: 'stats-heading'}, TEXT.matches),
      paged(columns, before => api('player/matches', {id: card.player.id, limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches));
  }

  async function matchView(id) {
    heading(`${TEXT.match} ${id}`, TEXT.match, `${TEXT.match} ${id}`);
    const columns = [[TEXT.place, row => n(row.place)], [TEXT.player, row => [playerLink(row.player), row.left ? el('small', {}, ` · ${TEXT.left}`) : null], 'l'],
      [TEXT.level, row => n(row.level)], [TEXT.score, row => n(row.score)], [TEXT.kills, row => n(row.kills)],
      [TEXT.deaths, row => n(row.deaths)], [TEXT.kd, row => kd(row.kd)], [TEXT.assists, row => n(row.assists)],
      [TEXT.headshots, row => n(row.headshots)], [TEXT.grenadeKills, row => n(row.grenade_kills)],
      [TEXT.meleeKills, row => n(row.melee_kills)], [TEXT.batteries, row => n(row.batteries)],
      [TEXT.captures, row => n(row.captures)], [TEXT.artefacts, row => n(row.artefacts)], [TEXT.rating, row => n(row.rating)]];
    const view = data => {
      const m = data && data.match;
      const winner = !m ? null : m.winner === 'draw' ? TEXT.draw : m.winner === null ? '—' : `${TEXT.team} ${m.winner + 1} ${TEXT.won}`;
      const teams = [0, 1].map(team => {
        const rows = data ? data.players.filter(row => row.team === team) : null;
        const title = `${TEXT.team} ${team + 1}${m ? ` · ${n(m.team_points[team])}` : ''}${m && m.winner === team ? ` · ${TEXT.win}` : ''}`;
        return [el('h2', {className: 'stats-heading'}, title), table(columns, rows, 6)];
      });
      return [panels([TEXT.match, [[TEXT.ended, m ? when(m.ended_at) : null], [TEXT.duration, m ? duration(m.duration_s) : null],
        [TEXT.map, m ? name(m.map) : null], [TEXT.mode, m ? name(m.mode) : null]]],
      [TEXT.result, [[TEXT.result, winner], [TEXT.points, m ? m.team_points.map(n).join(' : ') : null],
        [TEXT.level, m ? n(m.level) : null], [TEXT.players, m ? n(m.players) : null]]]), ...teams.flat()];
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
      [TEXT.headshots, row => n(row.totals.headshots)], [TEXT.batteries, row => n(row.totals.batteries)],
      [TEXT.artefacts, row => n(row.totals.artefacts)]];
    const matchColumns = [...MATCH_COLUMNS, [TEXT.points, row => row.team_points.map(n).join(' : ')], [TEXT.players, row => n(row.players)]];
    const head = data => panels(
      [TEXT.clan, [[TEXT.level, data ? n(data.clan.level) : null], [TEXT.members, data ? n(data.clan.members) : null],
        [TEXT.commander, data ? (data.clan.leader ? link({player: data.clan.leader.name}, data.clan.leader.name) : '—') : null],
        ...record(data && data.totals)]],
      [TEXT.inMatches, combat(data && data.totals)]);
    root.replaceChildren(head(null), el('h2', {className: 'stats-heading'}, TEXT.members), table(memberColumns, null, 8));
    const data = await api('clan', {tag});
    if (!data) return root.replaceChildren(notice(say(TEXT.noClan, tag)), finder());
    heading(`[${data.clan.tag}] ${data.clan.name}`, TEXT.clan, TEXT.clan);
    root.replaceChildren(head(data), el('h2', {className: 'stats-heading'}, TEXT.members), table(memberColumns, data.members),
      el('h2', {className: 'stats-heading'}, TEXT.matches),
      paged(matchColumns, before => api('matches', {clan: data.clan.id, limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches));
  }

  // The search: a plain form, so it works as the address does.
  function finder() {
    const field = (label, name, props = {}) => el('label', {}, label, el('input', {name, autocomplete: 'off', ...props}));
    const form = el('form', {className: 'catalogue-tools stats-find', action: '/wiki/stats', method: 'get'},
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

  function boardsView() {
    heading(TEXT.stats, document.getElementById('stats-eyebrow').textContent, '');
    let board = BOARDS[asked('board')] ? asked('board') : 'players';
    let key = BOARDS[board].includes(asked('by')) ? asked('by') : BOARDS[board][0];
    const tabs = el('div', {className: 'slot-filter', role: 'group', ariaLabel: TEXT.stats});
    const keys = el('div', {className: 'slot-filter stats-keys', role: 'group'});
    const body = el('div', {className: 'stats-board'});
    const playerColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.player, row => playerLink(row.player), 'l'],
      [TEXT.level, row => n(row.level)], [TEXT.rating, row => n(row.rating), '', 'rating'],
      [TEXT.matches, row => n(row.totals.matches), '', 'matches'], [TEXT.wins, row => n(row.totals.wins), '', 'wins'],
      [TEXT.winRate, row => rate(row.totals.win_rate)], [TEXT.kills, row => n(row.totals.kills), '', 'kills'],
      [TEXT.kd, row => kd(row.totals.kd)], [TEXT.score, row => n(row.totals.score), '', 'score'],
      [TEXT.scoreAvg, row => n(row.totals.score_avg)], [TEXT.headshots, row => n(row.totals.headshots), '', 'headshots'],
      [TEXT.batteries, row => n(row.totals.batteries), '', 'batteries'],
      [TEXT.artefacts, row => n(row.totals.artefacts), '', 'artefacts']];
    const clanColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.clan, row => link({clan: row.tag}, `[${row.tag}] ${row.name}`), 'l'],
      [TEXT.level, row => n(row.level)], [TEXT.members, row => n(row.members)],
      [TEXT.matches, row => n(row.totals.matches), '', 'matches'], [TEXT.wins, row => n(row.totals.wins), '', 'wins'],
      [TEXT.winRate, row => rate(row.totals.win_rate)], [TEXT.kills, row => n(row.totals.kills), '', 'kills'],
      [TEXT.kd, row => kd(row.totals.kd)], [TEXT.score, row => n(row.totals.score), '', 'score'],
      [TEXT.scoreAvg, row => n(row.totals.score_avg)], [TEXT.headshots, row => n(row.totals.headshots), '', 'headshots'],
      [TEXT.batteries, row => n(row.totals.batteries), '', 'batteries'],
      [TEXT.artefacts, row => n(row.totals.artefacts), '', 'artefacts']];
    const bestColumns = [[TEXT.rank, (row, i) => n(i + 1)], [TEXT.player, row => playerLink(row.player), 'l'],
      [TEXT.score, row => n(row.score), 'sorted'], [TEXT.kills, row => n(row.kills)], [TEXT.deaths, row => n(row.deaths)],
      [TEXT.match, row => link({match: row.match}, `${row.match}`)], [TEXT.ended, row => when(row.ended_at), 'l'],
      [TEXT.map, row => name(row.map), 'l'], [TEXT.mode, row => name(row.mode), 'l'], [TEXT.level, row => n(row.level)]];
    const latestColumns = [...MATCH_COLUMNS, [TEXT.points, row => row.team_points.map(n).join(' : ')], [TEXT.players, row => n(row.players)]];
    // A ranked list: its rows numbered.
    const ranked = columns => columns.map(([label, cell, ...rest]) => [label, row => cell(row, row.rank), ...rest]);
    let shown = 0;
    async function draw() {
      const drawing = ++shown;
      for (const button of tabs.children) button.setAttribute('aria-pressed', String(button.dataset.board === board));
      keys.replaceChildren(...BOARDS[board].map(value => {
        const button = el('button', {type: 'button'}, TEXT[KEY_TEXT[value]]);
        button.setAttribute('aria-pressed', String(value === key));
        button.addEventListener('click', () => { key = value; draw(); });
        return button;
      }));
      const url = new URL(location.href);
      url.searchParams.set('board', board);
      if (key) url.searchParams.set('by', key); else url.searchParams.delete('by');
      history.replaceState(null, '', url);
      keepLanguage();
      if (board === 'latest') return body.replaceChildren(paged(latestColumns, before => api('matches', {limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches));
      const [path, params, columns, list, size] = {
        players: ['top', {by: key, limit: TOP}, playerColumns, data => data.players, TOP],
        clans: ['clans', {by: key, limit: TOP}, clanColumns, data => data.clans, 20],
        best: ['best', {period: key}, bestColumns, data => data.rows, 20],
      }[board];
      const sorted = board === 'best' ? '' : key;
      body.replaceChildren(table(ranked(columns), null, Math.min(size, body.rows || size), sorted));
      let rows;
      try {
        const data = await api(path, params);
        rows = data ? list(data) : [];
      } catch (error) {
        if (drawing === shown) body.replaceChildren(notice(String(error)));
        return;
      }
      if (drawing !== shown) return;
      rows.forEach((row, i) => { row.rank = i; });
      body.rows = rows.length || 1;
      body.replaceChildren(rows.length ? table(ranked(columns), rows, 0, sorted) : notice(TEXT.nobody));
    }
    for (const value of Object.keys(BOARDS)) {
      const button = el('button', {type: 'button'}, TEXT[value]);
      button.dataset.board = value;
      button.addEventListener('click', () => { board = value; key = BOARDS[board][0] || ''; draw(); });
      tabs.append(button);
    }
    root.replaceChildren(finder(), tabs, keys, body);
    draw();
  }

  // The language link keeps what is shown.
  function keepLanguage() {
    const language = document.querySelector('a.lang');
    if (!language) return;
    const other = new URL(language.href);
    for (const [name, value] of new URLSearchParams(location.search)) if (name !== 'lang') other.searchParams.set(name, value);
    language.href = other;
  }
  keepLanguage();
  const view = asked('player') ? playerView(asked('player'))
    : asked('match') ? matchView(asked('match'))
    : asked('clan') ? clanView(asked('clan'))
    : boardsView();
  Promise.resolve(view).catch(error => root.replaceChildren(notice(typeof error === 'string' ? error : TEXT.failed), finder()));
})();
