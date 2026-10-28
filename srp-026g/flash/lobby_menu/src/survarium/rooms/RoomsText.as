package survarium.rooms
{
    import survarium.data.LocalizationData;
    import survarium.constants.Language;

    // The rooms' words, in the game's six languages (English, Ukrainian,
    // Russian, German, Polish, Serbian Latin; the game's language is told by its own word
    // for Friends), and the names of the maps, modes and weathers the
    // server's pool names by number.
    public class RoomsText
    {
        private static const TEXT:Object = {
            "servers": ["Rooms", "Кімнати", "Комнаты", "Räume", "Pokoje", "Sobe"],
            "room": ["Room", "Кімната", "Комната", "Raum", "Pokój", "Soba"],
            // The window's tabs while in a room (the owner's of 2026-10-22).
            "myRoom": ["My room", "Моя кімната", "Моя комната", "Mein Raum", "Mój pokój", "Moja soba"],
            "allRooms": ["All rooms", "Усі кімнати", "Все комнаты", "Alle Räume", "Wszystkie pokoje", "Sve sobe"],
            "create": ["Create", "Створити", "Создать", "Erstellen", "Utwórz", "Napravi"],
            "new": ["Private room", "Приватна кімната", "Приватная комната", "Privater Raum", "Pokój prywatny", "Privatna soba"],
            "back": ["Back", "Назад", "Назад", "Zurück", "Wstecz", "Nazad"],
            "join": ["Join", "Увійти", "Войти", "Beitreten", "Dołącz", "Pridruži se"],
            // Play greyed (presets.md, the reasons): no word of it; New room
            // greyed too (a maintenance): no word of either.
            "noneQuiet": ["No rooms yet.", "Кімнат ще немає.", "Комнат пока нет.", "Noch keine Räume.", "Brak pokojów.", "Još nema soba."],
            "noneNoPlay": ["No rooms yet. Make one.", "Кімнат ще немає. Створіть.", "Комнат пока нет. Создайте.",
                "Noch keine Räume. Erstelle einen.", "Brak pokojów. Utwórz.", "Još nema soba. Napravi je."],
            "none": ["No rooms yet. Make one, or press Play.", "Кімнат ще немає. Створіть або натисніть «Грати».",
                "Комнат пока нет. Создайте или нажмите «Играть».", "Noch keine Räume. Erstelle einen oder drücke Spielen.",
                "Brak pokojów. Utwórz lub naciśnij Graj.", "Još nema soba. Napravi je ili pritisni Igraj."],
            "name": ["Name", "Назва", "Название", "Name", "Nazwa", "Naziv"],
            "level": ["Level", "Рівень", "Уровень", "Stufe", "Poziom", "Nivo"],
            "max": ["Max level", "Макс. рівень", "Макс. уровень", "Max. Stufe", "Maks. poziom", "Najviši nivo"],
            "any": ["Any", "Будь-який", "Любой", "Alle", "Dowolny", "Bilo koji"],
            "sec": ["s", "с", "с", "s", "s", "s"],
            "min": ["min", "хв", "мин", "Min.", "min", "min"],
            "players": ["Players", "Гравці", "Игроки", "Spieler", "Gracze", "Igrači"],
            "map": ["Map", "Карта", "Карта", "Karte", "Mapa", "Mapa"],
            "state": ["State", "Стан", "Состояние", "Status", "Stan", "Stanje"],
            "size": ["Team size", "Розмір команд", "Размер команд", "Teamgröße", "Rozmiar drużyn", "Veličina tima"],
            "wait": ["Time to start", "Час до старту", "Время до старта", "Zeit bis zum Start", "Czas do startu", "Vreme do početka"],
            "pin": ["PIN", "PIN", "PIN", "PIN", "PIN", "PIN"],
            "pinHint": ["4 digits required", "Потрібні 4 цифри", "Нужны 4 цифры", "4 Ziffern erforderlich", "Wymagane 4 cyfry", "Potrebne su 4 cifre"],
            "minPlayers": ["Players to start", "Гравців для старту", "Игроков для старта", "Spieler zum Start",
                "Graczy do startu", "Igrača za početak"],
            "voting": ["Map vote", "Голосування", "Голосование", "Kartenwahl", "Głosowanie", "Glasanje za mapu"],
            "voteNow": ["Vote for the next map: click a card. The match starts on the winner when the time is up.",
                "Голосуйте за наступну карту: клацніть картку. Коли час вийде, матч почнеться на переможниці.",
                "Голосуйте за следующую карту: нажмите карточку. Когда время выйдет, матч начнётся на победителе.",
                "Stimme für die nächste Karte: klicke eine Karte. Ist die Zeit um, beginnt das Match auf der Gewinnerin.",
                "Zagłosuj na następną mapę: kliknij kartę. Gdy czas minie, mecz zacznie się na zwycięskiej.", "Glasaj za sledeću mapu klikom na karticu. Po isteku vremena meč počinje na pobedničkoj mapi."],
            "nextMap": ["Next", "Далі", "Далее", "Als Nächstes", "Następna", "Sledeća"],
            "pin4": ["The PIN is 4 digits.", "PIN — це 4 цифри.", "PIN — это 4 цифры.", "Die PIN hat 4 Ziffern.",
                "PIN ma 4 cyfry.", "PIN ima 4 cifre."],
            "poolHint": ["The vote draws 3 cards from these.", "Голосування бере 3 картки з них.",
                "Голосование берёт 3 карточки из них.", "Die Wahl zieht 3 Karten daraus.", "Głosowanie losuje z nich 3 karty.", "Za glasanje se nasumično biraju 3 od ovih mapa."],
            "pool": ["Pool", "Пул", "Пул", "Pool", "Pula", "Izbor mapa"],
            "all": ["All", "Усі", "Все", "Alle", "Wszystkie", "Sve"],
            "nothing": ["None", "Жодної", "Ни одной", "Keine", "Żadna", "Nijedna"],
            "waiting": ["Waiting for players", "Чекаємо гравців", "Ждём игроков", "Warten auf Spieler", "Czekamy na graczy", "Čekanje igrača"],
            // A private room (a PIN; the owner's of 2026-10-26): no size, no
            // players to start, no countdown; its owner's Start.
            "startMatch": ["Start", "Почати", "Начать", "Starten", "Rozpocznij", "Pokreni"],
            "startHint": ["Press Start when everyone is in.", "Натисніть «Почати», коли всі зайдуть.",
                "Нажмите «Начать», когда все зайдут.", "Drücke Starten, wenn alle da sind.",
                "Naciśnij Rozpocznij, gdy wszyscy wejdą.", "Pritisni Pokreni kada svi uđu."],
            "waitingOwner": ["Waiting for the creator to start.", "Чекаємо, поки творець почне.",
                "Ждём, пока создатель начнёт.", "Warten, bis der Ersteller startet.",
                "Czekamy, aż twórca rozpocznie.", "Čekanje da kreator pokrene meč."],
            // Its two teams, arranged by its owner with « and ».
            "team1": ["Team 1", "Команда 1", "Команда 1", "Team 1", "Drużyna 1", "Tim 1"],
            "team2": ["Team 2", "Команда 2", "Команда 2", "Team 2", "Drużyna 2", "Tim 2"],
            // A team's free places, one line after its last member.
            "openSlots": ["%N open", "вільно: %N", "свободно: %N", "%N frei", "wolne: %N", "Slobodno: %N"],
            "teamsOwner": ["Move players between the teams with « and ».", "Переміщуйте гравців між командами кнопками « і ».",
                "Перемещайте игроков между командами кнопками « и ».", "Verschiebe Spieler mit « und » zwischen den Teams.",
                "Przenoś graczy między drużynami przyciskami « i ».", "Premesti igrače između timova pomoću « i »."],
            "teamsLater": ["The teams can be changed after the match.", "Команди можна змінити після матчу.",
                "Команды можно изменить после матча.", "Die Teams lassen sich nach dem Match ändern.",
                "Drużyny można zmienić po meczu.", "Timovi se mogu promeniti posle meča."],
            "teamsMember": ["The creator arranges the teams.", "Команди розставляє творець.", "Команды расставляет создатель.",
                "Der Ersteller stellt die Teams zusammen.", "Drużyny ustawia twórca.", "Kreator sobe sastavlja timove."],
            "privateRoom": ["Private room: up to 20 players; you set the teams and start.",
                "Приватна кімната: до 20 гравців; команди й старт за вами.",
                "Приватная комната: до 20 игроков; команды и старт за вами.",
                "Privater Raum: bis 20 Spieler; du stellst die Teams und startest.",
                "Pokój prywatny: do 20 graczy; ty ustawiasz drużyny i start.", "Privatna soba: do 20 igrača; ti sastavljaš timove i pokrećeš meč."],
            "starts": ["Match starts in", "Матч почнеться через", "Матч начнётся через", "Match beginnt in", "Mecz za", "Meč počinje za"],
            "playing": ["Match in progress", "Триває матч", "Идёт матч", "Match läuft", "Trwa mecz", "Meč je u toku"],
            "vote": ["Vote for the next map", "Голосуйте за наступну карту", "Голосуйте за следующую карту",
                "Stimme für die nächste Karte", "Głosuj na następną mapę", "Glasaj za sledeću mapu"],
            "teams": ["Teams are drawn when the match starts.", "Команди формуються на початку матчу.",
                "Команды формируются в начале матча.", "Teams werden beim Matchstart gebildet.",
                "Drużyny są losowane na starcie meczu.", "Timovi se biraju na početku meča."],
            "code": ["code", "код", "код", "Code", "kod", "kod"],
            "leave": ["Leave", "Вийти", "Выйти", "Verlassen", "Opuść", "Napusti"],
            "leaveRoom": ["Leave room", "Вийти з кімнати", "Выйти из комнаты", "Raum verlassen", "Opuść pokój", "Napusti sobu"],
            // The chat's tab, right of the general chat's (the owner's of
            // 2026-10-20): each 108 px wide at most in the tab's Calibri Bold
            // 15 px, which has every letter of them.
            "roomChat": ["Room Chat", "Чат кімнати", "Чат комнаты", "Raum-Chat", "Czat pokoju", "Čet sobe"],
            // The room's own lines in its chat, after the player's name (the
            // server says them in English).
            "joinedRoom": ["joined the room", "приєднався до кімнати", "вошёл в комнату", "ist dem Raum beigetreten",
                "dołączył do pokoju", "se pridružio sobi"],
            "leftRoom": ["left the room", "вийшов з кімнати", "покинул комнату", "hat den Raum verlassen",
                "opuścił pokój", "je napustio sobu"],
            // A friend's right-click menu, while the friend is in a room.
            "joinRoom": ["Join room", "Увійти в кімнату", "Войти в комнату", "Raum beitreten", "Dołącz do pokoju", "Uđi u sobu"],
            "queueRoom": ["Queue for room", "Черга в кімнату", "Очередь в комнату", "Raum-Warteschlange",
                "Kolejka do pokoju", "Stani u red za sobu"],
            "joinMatch": ["Join match", "Увійти в матч", "Войти в матч", "Match beitreten", "Dołącz do meczu", "Pridruži se meču"],
            "joinIn": ["Into the match in %S s, or now with Join match", "У матч за %S с, або зараз: «Увійти в матч»",
                "В матч через %S с, или сейчас: «Войти в матч»", "Ins Match in %S s, oder jetzt: Match beitreten",
                "Do meczu za %S s albo teraz: Dołącz do meczu", "Ulazak za %S s, ili odmah preko Pridruži se meču"],
            "switchTo": ["Switch room", "Змінити кімнату", "Сменить комнату", "Raum wechseln", "Zmień pokój", "Promeni sobu"],
            // Why to switch (the owner's of 2026-10-20: it was not clear):
            // nobody's gear there is over the room's cap. Short: it shares
            // the footer with two buttons (at most 364 px at 1280 wide).
            "switchWhy": ["Fairer match: opponents' gear up to level %L", "Чесніший бій: спорядження суперників до рівня %L",
                "Бой честнее: снаряжение соперников до уровня %L", "Fairer: Ausrüstung der Gegner bis Stufe %L",
                "Uczciwszy mecz: sprzęt rywali do poziomu %L", "Ravnopravniji meč: oprema protivnika do nivoa %L"],
            "switchWho": ["%N's room · %P players", "Кімната %N · гравців: %P", "Комната %N · игроков: %P",
                "Raum von %N · %P Spieler", "Pokój %N · graczy: %P", "Soba igrača %N · %P igrača"],
            "countdown": ["Starts", "Старт", "Старт", "Start", "Start", "Početak"],
            "inMatch": ["Playing", "Грають", "Играют", "Läuft", "Gra", "U igri"],
            "experimental": ["exp.", "експ.", "эксп.", "exp.", "eksp.", "eksp."],
            "ownerTag": ["owner", "власник", "владелец", "Besitzer", "właściciel", "vlasnik"],
            "queue": ["Queue", "Черга", "Очередь", "Warteschlange", "Kolejka", "Red"],
            "inQueue": ["In queue", "У черзі", "В очереди", "In der Warteschlange", "W kolejce", "U redu"],
            "leaveQueue": ["Leave queue", "Вийти з черги", "Выйти из очереди", "Warteschlange verlassen",
                "Opuść kolejkę", "Napusti red"],
            "modes": ["Modes", "Режими", "Режимы", "Modi", "Tryby", "Režimi"],
            "open": ["open", "вільно", "свободно", "frei", "wolne", "slobodno"],
            "voteHint": ["3 cards from the pool. Most votes wins; a tie is drawn at random. Click to change your vote.",
                "3 картки з пулу. Перемагає більшість голосів; нічия — випадково. Клацніть, щоб змінити голос.",
                "3 карточки из пула. Побеждает большинство; ничья — случайно. Нажмите, чтобы сменить голос.",
                "3 Karten aus dem Pool. Die meisten Stimmen gewinnen; Gleichstand: Zufall. Klicke, um umzustimmen.",
                "3 karty z puli. Wygrywa większość; remis losowo. Kliknij, aby zmienić głos.", "3 mape iz izbora. Pobeđuje mapa sa najviše glasova; nerešen ishod se bira nasumično. Klikni da promeniš glas."],
            "unfitKit": ["Your kit cannot play (no weapon or too little ammo): fix it, or you leave the room at the start.",
                "Ваш комплект не може грати (немає зброї або мало набоїв): виправте, інакше на старті ви вийдете з кімнати.",
                "Ваш комплект не может играть (нет оружия или мало патронов): исправьте, иначе на старте вы выйдете из комнаты.",
                "Deine Ausrüstung kann nicht spielen (keine Waffe oder zu wenig Munition): sonst verlässt du beim Start den Raum.",
                "Twój zestaw nie może grać (brak broni lub za mało amunicji): popraw, inaczej na starcie opuścisz pokój.", "Komplet nije spreman (nema oružja ili dovoljno municije): popravi ga ili ćeš napustiti sobu na početku meča."],
            "unfitGear": ["Your equipment is above the max level: lower it, or you leave the room at the start.",
                "Ваше спорядження вище макс. рівня: знизьте, інакше на старті ви вийдете з кімнати.",
                "Ваше снаряжение выше макс. уровня: снизьте, иначе на старте вы выйдете из комнаты.",
                "Deine Ausrüstung liegt über der max. Stufe: senke sie, sonst verlässt du beim Start den Raum.",
                "Twój ekwipunek przekracza maks. poziom: obniż go, inaczej na starcie opuścisz pokój.", "Tvoja oprema prelazi najviši nivo: smanji ga ili ćeš napustiti sobu na početku meča."],
            // Why a member waiting in its room cannot go into the match now
            // (`J;-1;<why>`, the owner's of 2026-10-20).
            "joinWhy1": ["Another player is joining the match; you go in after.",
                "Інший гравець заходить у матч; ви — після нього.", "Другой игрок заходит в матч; вы — после него.",
                "Ein anderer Spieler betritt das Match; du kommst danach.", "Inny gracz dołącza do meczu; ty wchodzisz po nim.", "Drugi igrač se pridružuje meču; ulaziš posle njega."],
            "joinWhy2": ["A player's game is loading; joining opens in a moment.",
                "Гра одного з гравців завантажується; вхід відкриється за мить.",
                "Игра одного из игроков загружается; вход откроется через мгновение.",
                "Das Spiel eines Spielers lädt; Beitreten gleich wieder möglich.",
                "Gra jednego z graczy się wczytuje; dołączenie za chwilę.", "Jednom igraču se učitava igra; pridruživanje će uskoro biti dostupno."],
            "joinWhy3": ["Under 30 s left: you'll play the next match.", "Лишилося менше 30 с: ви зіграєте наступний матч.",
                "Осталось меньше 30 с: вы сыграете следующий матч.", "Weniger als 30 s übrig: du spielst das nächste Match.",
                "Zostało mniej niż 30 s: zagrasz następny mecz.", "Ostalo je manje od 30 s: igraćeš sledeći meč."],
            "joinWhy4": ["The match is full.", "Матч заповнений.", "Матч заполнен.", "Das Match ist voll.", "Mecz jest pełny.", "Meč je pun."],
            "joinWhy5": ["Server maintenance soon: no new joins.", "Скоро технічні роботи: нові входи закрито.",
                "Скоро технические работы: новые входы закрыты.", "Bald Wartung: keine neuen Beitritte.",
                "Wkrótce przerwa techniczna: brak nowych wejść.", "Održavanje servera uskoro: nema novih ulazaka."],
            "joinWhy6": ["The match is ending: you'll play the next one.", "Матч завершується: ви зіграєте наступний.",
                "Матч завершается: вы сыграете следующий.", "Das Match endet: du spielst das nächste.",
                "Mecz się kończy: zagrasz następny.", "Meč se završava: igraćeš sledeći."],
            "joinWhy7": ["This match has reached its player limit. You'll join the next match.", "Цей матч досяг ліміту гравців. Ви приєднаєтеся до наступного матчу.", "Этот матч достиг лимита игроков. Вы присоединитесь к следующему матчу.", "Dieses Match hat sein Spielerlimit erreicht. Du spielst im nächsten Match.", "Ten mecz osiągnął limit graczy. Dołączysz do następnego meczu.", "Ovaj meč je dostigao ograničenje broja igrača. Pridružićeš se sledećem meču."],
            "lateHint": ["Who joins now sees the room, then goes into the match.", "Хто зайде зараз, побачить кімнату, потім потрапить у матч.",
                "Кто зайдёт сейчас, увидит комнату, потом попадёт в матч.", "Wer jetzt beitritt, sieht den Raum, dann geht es ins Match.",
                "Kto dołączy teraz, zobaczy pokój, potem trafi do meczu.", "Ko se sada pridruži, prvo vidi sobu, zatim ulazi u meč."],
            "waitingShort": ["Waiting", "Очікування", "Ожидание", "Warten", "Oczekiwanie", "Čekanje"],
            "code0": ["", "", "", "", "", ""],
            "code2": ["That room is gone.", "Цієї кімнати вже немає.", "Этой комнаты больше нет.",
                "Diesen Raum gibt es nicht mehr.", "Ten pokój już nie istnieje.", "Ta soba više ne postoji."],
            "code3": ["Wrong PIN.", "Невірний PIN.", "Неверный PIN.", "Falsche PIN.", "Błędny PIN.", "Pogrešan PIN."],
            "code4": ["That room is full.", "Кімната заповнена.", "Комната заполнена.", "Der Raum ist voll.", "Pokój jest pełny.", "Ta soba je puna."],
            "code5": ["You are in a match.", "Ви в матчі.", "Вы в матче.", "Du bist in einem Match.", "Jesteś w meczu.", "U meču si."],
            "code6": ["Too many rooms now.", "Забагато кімнат.", "Слишком много комнат.", "Zu viele Räume.", "Za dużo pokojów.", "Trenutno ima previše soba."],
            "code7": ["Update your game.", "Оновіть гру.", "Обновите игру.", "Aktualisiere dein Spiel.", "Zaktualizuj grę.", "Ažuriraj igru."],
            "code8": ["Pick at least one map.", "Виберіть хоча б одну карту.", "Выберите хотя бы одну карту.",
                "Wähle mindestens eine Karte.", "Wybierz co najmniej jedną mapę.", "Izaberi bar jednu mapu."],
            "code9": ["Maintenance soon.", "Скоро технічні роботи.", "Скоро технические работы.", "Bald Wartung.", "Wkrótce przerwa techniczna.", "Održavanje uskoro."],
            "code10": ["The owner removed you from that room.", "Власник прибрав вас із кімнати.",
                "Владелец убрал вас из комнаты.", "Der Besitzer hat dich entfernt.", "Właściciel usunął cię z pokoju.", "Vlasnik te je uklonio iz te sobe."],
            "code11": ["Leave your clan team first.", "Спершу вийдіть із кланової команди.", "Сначала выйдите из клановой команды.",
                "Verlasse zuerst dein Clanteam.", "Najpierw opuść drużynę klanu.", "Prvo napusti tim klana."],
            "code12": ["Your equipment is above the max level.", "Ваше спорядження вище макс. рівня.",
                "Ваше снаряжение выше макс. уровня.", "Deine Ausrüstung liegt über der max. Stufe.",
                "Twój ekwipunek przekracza maks. poziom.", "Tvoja oprema prelazi najviši dozvoljeni nivo."],
            "code13": ["Your kit could not play (no weapon or too little ammo): you were taken out of the room.",
                "Ваш комплект не міг грати (немає зброї або мало набоїв): вас вивели з кімнати.",
                "Ваш комплект не мог играть (нет оружия или мало патронов): вас вывели из комнаты.",
                "Deine Ausrüstung konnte nicht spielen (keine Waffe oder zu wenig Munition): du wurdest aus dem Raum genommen.",
                "Twój zestaw nie mógł grać (brak broni lub za mało amunicji): usunięto cię z pokoju.", "Komplet nije bio spreman (nema oružja ili dovoljno municije): uklonjen si iz sobe."],
            "code16": ["Press Play to enter a public room.", "Натисніть «Грати», щоб увійти у відкриту кімнату.", "Нажмите «Играть», чтобы войти в открытую комнату.", "Drücke Spielen, um einem öffentlichen Raum beizutreten.", "Naciśnij Graj, aby wejść do pokoju publicznego.", "Pritisni Igraj da uđeš u javnu sobu."],
            "code14": ["The loadout could not be chosen.", "Не вдалося обрати комплект.", "Не удалось выбрать комплект.",
                "Das Loadout ließ sich nicht wählen.", "Nie udało się wybrać zestawu.", "Nije moguće izabrati komplet."],
            // A join's kit preset (presets.md): {gear} the kits' level now, {max}
            // the room's, {n} and {level} the preset's.
            "offerHigher": ["Room max level {max}, your gear {gear}. Loadout {n} is level {level}.",
                "Макс. рівень кімнати {max}, ваше спорядження {gear}. Комплект {n}: рівень {level}.",
                "Макс. уровень комнаты {max}, ваше снаряжение {gear}. Комплект {n}: уровень {level}.",
                "Max. Stufe des Raums {max}, deine Ausrüstung {gear}. Loadout {n}: Stufe {level}.",
                "Maks. poziom pokoju {max}, twój ekwipunek {gear}. Zestaw {n}: poziom {level}.", "Najviši nivo sobe {max}, tvoja oprema {gear}. Komplet {n} je nivoa {level}."],
            "offerFits": ["Your gear {gear} is over the room's max level {max}. Loadout {n} is level {level}.",
                "Ваше спорядження {gear} вище макс. рівня кімнати {max}. Комплект {n}: рівень {level}.",
                "Ваше снаряжение {gear} выше макс. уровня комнаты {max}. Комплект {n}: уровень {level}.",
                "Deine Ausrüstung {gear} liegt über der max. Stufe {max}. Loadout {n}: Stufe {level}.",
                "Twój ekwipunek {gear} przekracza maks. poziom {max}. Zestaw {n}: poziom {level}.", "Tvoja oprema {gear} prelazi najviši nivo sobe {max}. Komplet {n} je nivoa {level}."],
            // The kits can not play; loadout {n} can, level {level}.
            "offerUnfit": ["Your loadout can't play. Loadout {n} is level {level}.",
                "Ваш комплект не для бою. Комплект {n}: рівень {level}.",
                "Ваш комплект не для боя. Комплект {n}: уровень {level}.",
                "Dein Loadout ist nicht spielbar. Loadout {n}: Stufe {level}.",
                "Twój zestaw nie nadaje się do gry. Zestaw {n}: poziom {level}.", "Tvoj komplet nije spreman. Komplet {n} je nivoa {level}."],
            "enterAnyway": ["Enter anyway", "Увійти так", "Войти так", "Trotzdem rein", "Wejdź mimo to", "Ipak uđi"],
            "switchEnter": ["Switch and enter", "Змінити й увійти", "Сменить и войти", "Wechseln, rein", "Zmień i wejdź", "Promeni i uđi"],
            "exit": ["Exit", "Вийти", "Выйти", "Abbrechen", "Wyjdź", "Izađi"]
        };
        private static const FRIENDS:Array = ["Friends", "Друзі", "Друзья", "Freunde", "Znajomi", "Prijatelji"];
        private static const LEVELS:Array = [
            null,
            ["Rudnya", "Рудня", "Рудня", "Rudnya", "Rudnia", "Rudnja"],
            ["School", "Школа", "Школа", "Schule", "Szkoła", "Škola"],
            ["Vostok Radar Station", "РЛС \"Восток\"", "РЛС \"Восток\"", "RS Vostok", "Stacja Radarowa \"Vostok\"", "Radarska stanica Vostok"],
            ["Chemical Plant", "Хімзавод", "Химзавод", "Chemiebetrieb", "Zakłady chemiczne", "Hemijska fabrika"],
            ["Tarakanovsky Fort", "Тараканівський форт", "Таракановский форт", "Tarakanowski Fort", "Fort Tarakanów", "Tarakanovska tvrđava"],
            ["Mamayev Kurgan", "Мамаїв курган", "Мамаев курган", "Mamaew kurgan", "Kurhan Mamaja", "Mamajev kurgan"],
            ["Vector Laboratory", "Лабораторія \"Вектор\"", "Лаборатория \"Вектор\"", "Labor Vektor", "Laboratorium \"Wektor\"", "Laboratorija Vektor"],
            ["Cologne", "Кельн", "Кельн", "Köln", "Most w Kolonii", "Keln"],
            ["London", "Лондон", "Лондон", "London", "Londyn", "London"],
            // The ports of revision 23 (BottomPanel's own table).
            ["Vostok Radar Station (v0.21)", "РЛС \"Восток\" (v0.21)", "РЛС \"Восток\" (v0.21)", "RS Vostok (v0.21)",
                "Stacja Radarowa \"Vostok\" (v0.21)", "Radarska stanica Vostok (v0.21)"],
            ["Mamayev Kurgan (v0.21)", "Мамаїв курган (v0.21)", "Мамаев курган (v0.21)", "Mamaew kurgan (v0.21)",
                "Kurhan Mamaja (v0.21)", "Mamajev kurgan (v0.21)"],
            ["Chemical Plant (v0.11)", "Хімзавод (v0.11)", "Химзавод (v0.11)", "Chemiebetrieb (v0.11)",
                "Zakłady chemiczne (v0.11)", "Hemijska fabrika (v0.11)"],
            ["Vostok Radar Station (v0.11)", "РЛС \"Восток\" (v0.11)", "РЛС \"Восток\" (v0.11)", "RS Vostok (v0.11)",
                "Stacja Radarowa \"Vostok\" (v0.11)", "Radarska stanica Vostok (v0.11)"]
        ];
        private static const MODES:Object = {
            "2": ["Protective device", "Захисний пристрій", "Защитное устройство", "Schutzeinrichtung", "Urządzenie ochronne", "Zaštitni uređaj"],
            "3": ["Protective device: chargers", "Захисний пристрій: зарядники", "Защитное устройство: зарядники",
                "Schutzeinrichtung: Ladestationen", "Urządzenie ochronne: ładowarki", "Zaštitni uređaj: punjači"],
            "4": ["Team Deathmatch", "Командний бій", "Командный бой", "Kommandokampf", "Walka drużynowa", "Timska borba"],
            "5": ["Research", "Дослідження", "Исследование", "Forschung", "Badania", "Istraživanje"],
            "6": ["Protective device: blackout", "Захисний пристрій: блекаут", "Защитное устройство: блэкаут",
                "Schutzeinrichtung: Blackout", "Urządzenie ochronne: blackout", "Zaštitni uređaj: zamračenje"]
        };
        // maps_dict's map_weather_id: 1 day, 2 rain, 3 evening, 4 night, 5 prerain.
        private static const WEATHER:Object = {
            "1": ["day", "день", "день", "Tag", "dzień", "dan"],
            "2": ["rain", "дощ", "дождь", "Regen", "deszcz", "kiša"],
            "3": ["evening", "вечір", "вечер", "Abend", "wieczór", "veče"],
            "4": ["night", "ніч", "ночь", "Nacht", "noc", "noć"],
            "5": ["before rain", "перед дощем", "перед дождём", "vor dem Regen", "przed deszczem", "pred kišu"]
        };

        // The game's language: the index of its own word for Friends.
        public static function language():int
        {
            var lang:int = Language.EN;
            var friends:String = LocalizationData.getName("btn_friends");
            for (var i:int = 1; i < FRIENDS.length; i++)
                if (friends == FRIENDS[i])
                    lang = i;
            return lang;
        }

        // The words before the window is made: the game's language is known
        // by then from its own texts.
        public static function sayFirst(key:String):String
        {
            return String(TEXT[key][language()]);
        }

        public static function say(key:String, lang:int):String
        {
            var words:Array = TEXT[key] as Array;
            return words != null ? String(words[lang]) : key;
        }

        public static function known(key:String):Boolean
        {
            return TEXT[key] != null;
        }

        // A room's own line in its chat ("<name> joined the room", "<name>
        // left the room": the server's English) in the game's language; any
        // other text as it is.
        public static function notice(text:String):String
        {
            var lang:int = language();
            var keys:Array = ["joinedRoom", "leftRoom"];
            for (var i:int = 0; i < keys.length; i++)
            {
                var words:String = " " + TEXT[keys[i]][Language.EN];
                var at:int = text.length - words.length;
                if (at > 0 && text.substr(at) == words)
                    return text.substr(0, at) + " " + TEXT[keys[i]][lang];
            }
            return text;
        }

        public static function levelName(level:int, lang:int):String
        {
            var names:Array = level >= 0 && level < LEVELS.length ? LEVELS[level] as Array : null;
            return names != null ? String(names[lang]) : say("level", lang) + " " + level;
        }

        public static function weatherName(weather:int, lang:int):String
        {
            var names:Array = WEATHER[String(weather)] as Array;
            return names != null ? String(names[lang]) : "";
        }

        public static function modeName(mode:int, lang:int):String
        {
            var names:Array = MODES[String(mode)] as Array;
            return names != null ? String(names[lang]) : "";
        }

        public static function clock(secs:int):String
        {
            if (secs < 0)
                secs = 0;
            var s:int = secs % 60;
            return int(secs / 60) + ":" + (s < 10 ? "0" : "") + s;
        }
    }
}
