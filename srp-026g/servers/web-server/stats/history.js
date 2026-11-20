// The stats page (servers/web-server/src/history.rs): public matches from the
// match history's JSON (/api/history/..., docs/notes/match_history.md), asked
// for as the page is viewed. The address says what it shows: ?player=<name>,
// ?match=<number>, ?clan=<tag>, else the leaderboards (?board=, ?by=).
// Each view is drawn once empty, in its final shape, then with its data, so
// nothing moves while it loads; long lists grow by "Older matches".
(() => {
  const root = document.getElementById('stats');
  if (!root) return;
  // The page's language: its links' and the names the API gives (the game's eleven). The page's
  // own words (TEXT) are written in two: Ukrainian for Ukrainian, English for every other.
  const lang = document.documentElement.lang || 'en';
  const written = lang === 'uk' ? 'uk' : 'en';
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
      win: 'Win', loss: 'Loss', draw: 'Draw', left: 'Left', won: 'won', older: 'Older matches', hours: 'Hours played',
      replay: 'Replay', replayPreparing: 'Preparing…', replayFile: 'Download',
      replayNo: {incompatible: 'Recorded by an older game version', busy: 'Busy, try again in a minute', other: 'No replay'},
      today: 'Daily leaders', allTime: 'All time', shots: 'Shots fired',
      accuracy: 'Accuracy', weapons: 'Weapons', weapon: 'Weapon', armour: 'Armour', kit: 'Kit', favourites: 'Favourite equipment',
      accuracyNote: '1000+ shots', averageNote: '20+ matches', headshotRate: 'HS %',
      noMatches: 'No public matches yet.', noPlayer: 'No player with public matches named “{}”.', noClan: 'No clan tagged [{}].',
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
      win: 'Перемога', loss: 'Поразка', draw: 'Нічия', left: 'Вийшов', won: 'перемогла', older: 'Старіші матчі', hours: 'Годин у грі',
      replay: 'Повтор', replayPreparing: 'Готуємо…', replayFile: 'Завантажити',
      replayNo: {incompatible: 'Записано старішою версією гри', busy: 'Зайнято, спробуйте за хвилину', other: 'Повтору немає'},
      today: 'Лідери дня', allTime: 'За весь час', shots: 'Постріли',
      accuracy: 'Точність', weapons: 'Зброя', weapon: 'Зброя', armour: 'Броня', kit: 'Комплект', favourites: 'Улюблене спорядження',
      accuracyNote: '1000+ пострілів', averageNote: '20+ матчів', headshotRate: 'У голову %',
      noMatches: 'Публічних матчів ще немає.', noPlayer: 'Гравця з публічними матчами на ім’я «{}» немає.', noClan: 'Клану [{}] немає.',
      noMatch: 'Публічного матчу {} немає.', noHistory: 'Цей сервер не зберігає історію матчів.',
      failed: 'Не вдалося завантажити статистику. Оновіть сторінку.', nobody: 'Поки нікого.',
    },
  }[written];
  const PAGE = 15, TOP = 50;
  // An empty cell's text, so it has a line's height.
  const BLANK = '\u00a0';
  const BOARDS = {
    players: ['rating', 'matches', 'wins', 'win_rate', 'kills', 'kd', 'score_avg', 'headshot_rate', 'accuracy'],
    clans: ['matches', 'wins', 'kills'],
    best: ['day', 'hour'],
    latest: [],
  };
  // What a list by a key leaves out, said beside the boards.
  const KEY_NOTE = {accuracy: 'accuracyNote', score_avg: 'averageNote', win_rate: 'averageNote', kd: 'averageNote',
    headshot_rate: 'averageNote'};
  const numbers = new Intl.NumberFormat(written, {maximumFractionDigits: 2});
  const pair = new Intl.NumberFormat(written, {minimumFractionDigits: 2, maximumFractionDigits: 2});
  const percent = new Intl.NumberFormat(written, {style: 'percent', maximumFractionDigits: 1});
  const hours = new Intl.NumberFormat(written, {minimumFractionDigits: 1, maximumFractionDigits: 1});
  // Accuracy in whole percents: shots count pellets and kits, not aimed bullets.
  const whole = new Intl.NumberFormat(written, {style: 'percent', maximumFractionDigits: 0});
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
      link({player: player.name}, player.name));
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
  // An item's icon as the lobby draws it, beside its name (a link with
  // `href`); an empty frame of its size while loading, in an empty row,
  // and for an item without one.
  const itemCell = (item, href) => el('span', {className: 'stats-item-cell'},
    item && item.icon ? el('img', {className: 'stats-item', src: item.icon, alt: '', width: 54, height: 26, loading: 'lazy'})
      : el('span', {className: 'stats-item'}),
    item ? (href ? el('a', {href}, name(item.name)) : name(item.name)) : BLANK);
  function favourites(data) {
    // An item's place in the wiki: a weapon's card, an armour piece's; none
    // for one the game data no longer has.
    const wiki = (page, anchor) => item => item.key ? `/wiki/${page}?${new URLSearchParams({lang})}#${anchor}${item.key}` : null;
    const list = (label, items, where) => {
      const box = table([[label, item => itemCell(item, where(item)), 'l', '', () => itemCell(null)],
        [TEXT.matches, item => n(item.matches)]], items, FAVOURITES, '', where);
      if (items) box.blank(FAVOURITES - items.length);
      return box;
    };
    return [el('h2', {className: 'stats-heading'}, TEXT.favourites),
      el('div', {className: 'stats-favourites'}, list(TEXT.weapon, data && data.weapons, wiki('weapons', 'wpn_')), list(TEXT.armour, data && data.armour, wiki('equipment', '')))];
  }
  const matchLink = row => link({match: row.id}, `${row.id}`);
  // A map's name, leading to it in the wiki's Maps page in the match's mode;
  // plain for a map the game data no longer has.
  const mapAddress = row => `/wiki/maps?${new URLSearchParams({lang})}#${row.map_key}/${row.rules}`;
  const mapLink = row => row.map_key ? el('a', {href: mapAddress(row)}, row.map) : name(row.map);
  // A map's picture as the rooms draw it, beside its name and mode; an
  // empty frame of its size while loading, and for a map without one.
  // The picture leads where the name does.
  const mapPicture = row => {
    if (!row || !row.map_icon) return el('span', {className: 'stats-map'});
    const picture = el('img', {className: 'stats-map', src: row.map_icon, alt: '', width: 64, height: 36, loading: 'lazy'});
    return row.map_key ? el('a', {className: 'stats-map-link', href: mapAddress(row), ariaLabel: row.map, tabIndex: -1}, picture) : picture;
  };
  const mapCell = row => el('span', {className: 'stats-map-cell'}, mapPicture(row),
    el('span', {}, row ? [mapLink(row), el('small', {className: 'stats-under'}, name(row.mode))] : twoLines()));
  // Where a click on a match's row goes, and on a player's.
  const openMatch = row => ({match: row.id});
  const openPlayer = row => ({player: row.player.name});
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
    return [[TEXT.kills, v('kills')], [TEXT.deaths, v('deaths')], [TEXT.kd, totals ? kd(totals.kd) : null],
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

  async function playerView(name) {
    heading(name, TEXT.player, TEXT.player);
    const head = (card, days) => panels(
      [TEXT.overall, [[TEXT.level, card ? n(card.level) : null], [TEXT.experience, card ? n(card.experience) : null],
        [TEXT.rating, card ? n(card.rating) : null],
        [TEXT.clan, card ? (card.player.clan ? link({clan: card.player.clan.tag}, `[${card.player.clan.tag}]`) : '—') : null], ...record(card && card.totals)]],
      [TEXT.inMatches, combat(card && card.totals)],
      [TEXT.last30, days ? lastDays(days) : blank([TEXT.matches, TEXT.winRate, TEXT.kd, TEXT.scoreAvg])]);
    const columns = [...MATCH_COLUMNS, [TEXT.weapons, row => kitWeapons(row), 'l kits'], [TEXT.result, result, 'l'], [TEXT.place, row => n(row.place)],
      [TEXT.score, row => n(row.score)], [TEXT.kills, row => n(row.kills)], [TEXT.deaths, row => n(row.deaths)],
      [TEXT.kd, row => kd(row.kd)], [TEXT.accuracy, aim], [TEXT.assists, row => n(row.assists)], [TEXT.headshotRate, headshotRate],
      [TEXT.rating, row => n(row.rating)]];
    root.replaceChildren(head(null, null), ...favourites(null), el('h2', {className: 'stats-heading'}, TEXT.matches), table(columns, null));
    // A player without a public match is not found, as an unknown name.
    const card = await api('player', {name});
    if (!card) return root.replaceChildren(notice(say(TEXT.noPlayer, name)), finder());
    heading(card.player.name, TEXT.player, TEXT.player);
    const [days, equipment] = await Promise.all([api('player/days', {id: card.player.id, days: 30}).catch(() => null),
      api('player/equipment', {id: card.player.id, limit: FAVOURITES}).catch(() => null)]);
    root.replaceChildren(head(card, days ? days.days : []), ...favourites(equipment || {weapons: [], armour: []}),
      el('h2', {className: 'stats-heading'}, TEXT.matches),
      paged(columns, before => api('player/matches', {id: card.player.id, limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches, openMatch));
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

  async function matchView(id) {
    heading(`${TEXT.match} ${id}`, TEXT.match, `${TEXT.match} ${id}`);
    const columns = [[TEXT.place, row => n(row.place)], [TEXT.player, playerKit, 'l', '', twoLines],
      [TEXT.level, row => n(row.level)], [TEXT.score, row => n(row.score)], [TEXT.kills, row => n(row.kills)],
      [TEXT.deaths, row => n(row.deaths)], [TEXT.kd, row => kd(row.kd)], [TEXT.accuracy, aim], [TEXT.assists, row => n(row.assists)],
      [TEXT.headshotRate, headshotRate], [TEXT.grenadeKills, row => n(row.grenade_kills)],
      [TEXT.meleeKills, row => n(row.melee_kills)], [TEXT.captures, row => n(row.captures)], [TEXT.rating, row => n(row.rating)]];
    const view = data => {
      const m = data && data.match;
      // A free-for-all: everyone its own side, one list by place.
      const free = m && m.winner === 'free_for_all';
      const winner = !m ? null : m.winner === 'draw' ? TEXT.draw
        : free ? data.players.filter(row => row.outcome === 'won').map(row => row.player.name).join(', ') || '—'
        : m.winner === null ? '—' : el('span', {className: `stats-side stats-team${m.winner}`}, `${TEXT.team} ${m.winner + 1} ${TEXT.won}`);
      const teams = free
        ? [[el('h2', {className: 'stats-heading'}, TEXT.players), table(columns, [...data.players].sort((a, b) => a.place - b.place), 0, '', openPlayer)]]
        : [0, 1].map(team => {
          const rows = data ? data.players.filter(row => row.team === team) : null;
          const title = `${TEXT.team} ${team + 1}${m ? ` · ${n(m.team_points[team])}` : ''}${m && m.winner === team ? ` · ${TEXT.win}` : ''}`;
          return [el('h2', {className: `stats-heading stats-team stats-team${team}`}, title), table(columns, rows, 6, '', openPlayer)];
        });
      return [panels([TEXT.match, [[TEXT.ended, m ? when(m.ended_at) : null], [TEXT.duration, m ? duration(m.duration_s) : null],
        [TEXT.map, el('span', {className: 'stats-map-cell'}, m ? mapLink(m) : BLANK, mapPicture(m))],
        [TEXT.mode, m ? name(m.mode) : null]]],
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
        [TEXT.commander, data ? (data.clan.leader ? link({player: data.clan.leader.name}, data.clan.leader.name) : '—') : null],
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
  // (the day is UTC's), and everything counted added up.
  const LEADERS = [['kills', 'kills'], ['assists', 'assists'], ['score', 'score'], ['wins', 'wins'], ['played_s', 'hours']];
  const LEADERS_EACH = 5;
  function leaders(data) {
    const value = (key, amount) => key === 'played_s' ? hours.format(amount / 3600) : n(amount);
    return [el('h2', {className: 'stats-heading'}, TEXT.today),
      el('div', {className: 'stats-leaders'}, LEADERS.map(([key, label]) => {
        const rows = data && (data.leaders[key] || []).map((row, rank) => ({...row, rank}));
        const box = table([[TEXT.rank, row => n(row.rank + 1)], [TEXT.player, row => playerLink(row.player), 'l'],
          [TEXT[label], row => value(key, row.value)]], rows, LEADERS_EACH, '', openPlayer);
        if (rows) box.blank(LEADERS_EACH - rows.length);
        return box;
      }))];
  }
  // Everything added up: its heading, then one row in the tables' look.
  function allTime(data) {
    const box = table([[TEXT.players, row => n(row.players)], [TEXT.matches, row => n(row.matches)], [TEXT.kills, row => n(row.kills)],
      [TEXT.assists, row => n(row.assists)], [TEXT.shots, row => n(row.shots)], [TEXT.accuracy, aim],
      [TEXT.hours, row => hours.format(row.played_s / 3600)]], data && [data], 1);
    box.classList.add('stats-all');
    return [el('h2', {className: 'stats-heading'}, TEXT.allTime), box];
  }

  function boardsView() {
    heading(TEXT.stats, document.getElementById('stats-eyebrow').textContent, '');
    let board = BOARDS[asked('board')] ? asked('board') : 'players';
    let key = BOARDS[board].includes(asked('by')) ? asked('by') : BOARDS[board][0];
    // Smallest first: a second click on the sorted heading, and back.
    let up = asked('order') === 'asc' && (board === 'players' || board === 'clans');
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
      // The best scores' periods are buttons of a row of their own; a list
      // of players or clans sorts by its columns' headings.
      note.textContent = KEY_NOTE[key] ? TEXT[KEY_NOTE[key]] : '';
      keys.hidden = board !== 'best';
      keys.replaceChildren(...(board === 'best' ? BOARDS.best : []).map(value => {
        const button = el('button', {type: 'button'}, TEXT[value]);
        button.setAttribute('aria-pressed', String(value === key));
        button.addEventListener('click', () => { key = value; draw(); });
        return button;
      }));
      const url = new URL(location.href);
      url.searchParams.set('board', board);
      if (key) url.searchParams.set('by', key); else url.searchParams.delete('by');
      if (up) url.searchParams.set('order', 'asc'); else url.searchParams.delete('order');
      history.replaceState(null, '', url);
      keepLanguage();
      if (board === 'latest') return body.replaceChildren(paged(latestColumns, before => api('matches', {limit: PAGE, ...(before ? {before} : {})}), TEXT.noMatches, openMatch));
      const [path, params, columns, list, size] = {
        players: ['top', {by: key, limit: TOP, ...(up ? {order: 'asc'} : {})}, playerColumns, data => data.players, TOP],
        clans: ['clans', {by: key, limit: TOP, ...(up ? {order: 'asc'} : {})}, clanColumns, data => data.clans, 20],
        best: ['best', {period: key}, bestColumns, data => data.rows, 20],
      }[board];
      const sorted = board === 'best' ? '' : key;
      const sort = board === 'best' ? null : {up, ask: value => { up = value === key && !up; key = value; draw(); }};
      body.replaceChildren(table(ranked(columns), null, Math.min(size, body.rows || size), sorted, null, sort));
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
      // An empty list that sorts keeps its headings: the way to another.
      const drawn = table(ranked(columns), rows, 0, sorted, {players: openPlayer, clans: row => ({clan: row.tag}), best: row => ({match: row.match})}[board], sort);
      body.replaceChildren(...(rows.length ? [drawn] : sort ? [drawn, notice(TEXT.nobody)] : [notice(TEXT.nobody)]));
    }
    for (const value of Object.keys(BOARDS)) {
      const button = el('button', {type: 'button'}, TEXT[value]);
      button.dataset.board = value;
      button.addEventListener('click', () => { board = value; key = BOARDS[board][0] || ''; up = false; draw(); });
      tabs.append(button);
    }
    tabs.append(note);
    const today = el('div', {}, leaders(null)), all = el('div', {}, ...allTime(null));
    root.replaceChildren(finder(), today, all, tabs, keys, body);
    api('today', {}).then(data => today.replaceChildren(...leaders(data || {leaders: {}})), () => today.replaceChildren());
    api('totals', {}).then(data => data ? all.replaceChildren(...allTime(data)) : all.replaceChildren(), () => all.replaceChildren());
    draw();
  }

  // Each of the header's languages keeps what is shown.
  function keepLanguage() {
    for (const language of document.querySelectorAll('.lang a')) {
      const other = new URL(language.href);
      for (const [name, value] of new URLSearchParams(location.search)) if (name !== 'lang') other.searchParams.set(name, value);
      language.href = other;
    }
  }
  keepLanguage();
  const view = asked('player') ? playerView(asked('player'))
    : asked('match') ? matchView(asked('match'))
    : asked('clan') ? clanView(asked('clan'))
    : boardsView();
  Promise.resolve(view).catch(error => root.replaceChildren(notice(typeof error === 'string' ? error : TEXT.failed), finder()));
})();
