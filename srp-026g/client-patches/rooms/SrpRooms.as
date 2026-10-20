package
{
    import flash.display.Bitmap;
    import flash.display.BitmapData;
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.KeyboardEvent;
    import flash.events.MouseEvent;
    import flash.events.TimerEvent;
    import flash.external.ExternalInterface;
    import flash.geom.ColorTransform;
    import flash.geom.Matrix;
    import flash.geom.Point;
    import flash.geom.Rectangle;
    import flash.text.TextField;
    import flash.text.TextFieldType;
    import flash.text.TextFormat;
    import flash.utils.Timer;
    import flash.utils.getDefinitionByName;
    import flash.utils.getTimer;
    import scaleform.clik.events.ButtonEvent;
    import scaleform.gfx.FocusManager;
    import survarium.controls.PlayerInfo;
    import survarium.data.LocalizationData;

    // Rooms, the players' "servers" (srp's own; the wire is lobby.md 2.16.3,
    // contract S1), the only way PvP matches are made. The top menu's tab
    // in place of Map opens one window over the lobby, in three views: the
    // list of the rooms, the making of one, and the asker's own room with
    // its vote; the Play button goes straight into a room. The server
    // answers every request with ranking rows (`91`) at the start asked;
    // PlayerRanking.fillPlayers hands them here first.
    public class SrpRooms extends Sprite
    {
        public static const FIRST:int = 2060000000;
        public static const LIST:int = FIRST;
        public static const STATE:int = FIRST + 1;
        public static const LEAVE:int = FIRST + 2;
        public static const PLAY:int = FIRST + 6;
        public static const UNQUEUE:int = FIRST + 7;
        public static const JOIN_MATCH:int = FIRST + 8;
        // The create window's start: the player's last room made by hand,
        // kept by the server (the owner's of 2026-10-20).
        public static const SETTINGS:int = FIRST + 9;
        public static const QUEUE:int = FIRST + 200000;
        public static const VOTE:int = FIRST + 3;
        public static const POOL:int = FIRST + 10;
        public static const KICK:int = FIRST + 100;
        public static const PIN:int = FIRST + 10000;
        public static const JOIN:int = FIRST + 100000;
        public static const BITS:int = FIRST + 1000000;
        public static const CREATE:int = FIRST + 10000000;
        public static const LAST:int = FIRST + 19999999;
        public static const WAITS:Array = [15, 20, 25, 30, 35, 40, 45, 50, 55, 60];
        // The vote's own time before the wait (srp's rooms::VOTE).
        public static const VOTE_SECS:int = 15;
        public static const GOLD:uint = 0xE0A64E;
        public static const RED:uint = 0xE0533F;
        // A member of a room sees where it is (the owner's of 2026-10-19:
        // players did not know they were in one): the top menu's tab and
        // the switch to a better room in this, Play turned red (it leaves).
        public static const YELLOW:uint = 0xFFD23F;
        // The colours as transforms, for drawn parts (the tab's icon, the
        // game's buttons): yellow and red over their own shading.
        private static const YELLOW_TINT:ColorTransform = new ColorTransform(1, 0.82, 0.25, 1, 60, 45, 0, 0);
        private static const RED_TINT:ColorTransform = new ColorTransform(1, 0.3, 0.25, 1, 70, 0, 0, 0);
        public static const LIGHT:uint = 0xE8EEF2;
        public static const GREY:uint = 0x93A3AF;
        public static const DIM:uint = 0x5F707C;
        // The design canvas's units: a 1280 wide screen, the window 40 in from
        // each side and 79 down (the top bar is 71).
        public static const DESIGN_W:Number = 1280;
        public static const DESIGN_H:Number = 720;
        public static const MIN_H:int = 560;
        // In design units: the area of the game's own pair of windows, from
        // the Character window's top left to the Inventory's bottom right
        // (the owner's of 2026-10-17): clear of the side panels, the
        // counters under Play and the bottom bar's buttons.
        public static const LEFT:Number = 75;
        public static const RIGHT:Number = 68;
        public static const TOP:Number = 90;
        public static const BOTTOM_GAP:Number = 12;

        private static const TEXT:Object = {
            "servers": ["Rooms", "Кімнати", "Комнаты", "Räume", "Pokoje"],
            "room": ["Room", "Кімната", "Комната", "Raum", "Pokój"],
            "create": ["Create", "Створити", "Создать", "Erstellen", "Utwórz"],
            "new": ["New room", "Нова кімната", "Новая комната", "Neuer Raum", "Nowy pokój"],
            "back": ["Back", "Назад", "Назад", "Zurück", "Wstecz"],
            "join": ["Join", "Увійти", "Войти", "Beitreten", "Dołącz"],
            "none": ["No rooms yet. Make one, or press Play.", "Кімнат ще немає. Створіть або натисніть «Грати».",
                "Комнат пока нет. Создайте или нажмите «Играть».", "Noch keine Räume. Erstelle einen oder drücke Spielen.",
                "Brak pokojów. Utwórz lub naciśnij Graj."],
            "name": ["Name", "Назва", "Название", "Name", "Nazwa"],
            "level": ["Level", "Рівень", "Уровень", "Stufe", "Poziom"],
            "max": ["Max level", "Макс. рівень", "Макс. уровень", "Max. Stufe", "Maks. poziom"],
            "any": ["Any", "Будь-який", "Любой", "Alle", "Dowolny"],
            "sec": ["s", "с", "с", "s", "s"],
            "min": ["min", "хв", "мин", "Min.", "min"],
            "players": ["Players", "Гравці", "Игроки", "Spieler", "Gracze"],
            "map": ["Map", "Карта", "Карта", "Karte", "Mapa"],
            "state": ["State", "Стан", "Состояние", "Status", "Stan"],
            "size": ["Team size", "Розмір команд", "Размер команд", "Teamgröße", "Rozmiar drużyn"],
            "wait": ["Time to start", "Час до старту", "Время до старта", "Zeit bis zum Start", "Czas do startu"],
            "pin": ["PIN", "PIN", "PIN", "PIN", "PIN"],
            "pinHint": ["4 digits; empty: an open room", "4 цифри; порожньо: відкрита кімната",
                "4 цифры; пусто: открытая комната", "4 Ziffern; leer: offener Raum", "4 cyfry; puste: pokój otwarty"],
            "minPlayers": ["Players to start", "Гравців для старту", "Игроков для старта", "Spieler zum Start",
                "Graczy do startu"],
            "voting": ["Map vote", "Голосування", "Голосование", "Kartenwahl", "Głosowanie"],
            "voteNow": ["Vote for the next map: click a card. The match starts on the winner when the time is up.",
                "Голосуйте за наступну карту: клацніть картку. Коли час вийде, матч почнеться на переможниці.",
                "Голосуйте за следующую карту: нажмите карточку. Когда время выйдет, матч начнётся на победителе.",
                "Stimme für die nächste Karte: klicke eine Karte. Ist die Zeit um, beginnt das Match auf der Gewinnerin.",
                "Zagłosuj na następną mapę: kliknij kartę. Gdy czas minie, mecz zacznie się na zwycięskiej."],
            "nextMap": ["Next", "Далі", "Далее", "Als Nächstes", "Następna"],
            "pin4": ["The PIN is 4 digits.", "PIN — це 4 цифри.", "PIN — это 4 цифры.", "Die PIN hat 4 Ziffern.",
                "PIN ma 4 cyfry."],
            "poolHint": ["The vote draws 3 cards from these.", "Голосування бере 3 картки з них.",
                "Голосование берёт 3 карточки из них.", "Die Wahl zieht 3 Karten daraus.", "Głosowanie losuje z nich 3 karty."],
            "pool": ["Pool", "Пул", "Пул", "Pool", "Pula"],
            "all": ["All", "Усі", "Все", "Alle", "Wszystkie"],
            "nothing": ["None", "Жодної", "Ни одной", "Keine", "Żadna"],
            "waiting": ["Waiting for players", "Чекаємо гравців", "Ждём игроков", "Warten auf Spieler", "Czekamy na graczy"],
            "starts": ["Match starts in", "Матч почнеться через", "Матч начнётся через", "Match beginnt in", "Mecz za"],
            "playing": ["Match in progress", "Триває матч", "Идёт матч", "Match läuft", "Trwa mecz"],
            "vote": ["Vote for the next map", "Голосуйте за наступну карту", "Голосуйте за следующую карту",
                "Stimme für die nächste Karte", "Głosuj na następną mapę"],
            "teams": ["Teams are drawn when the match starts.", "Команди формуються на початку матчу.",
                "Команды формируются в начале матча.", "Teams werden beim Matchstart gebildet.",
                "Drużyny są losowane na starcie meczu."],
            "code": ["code", "код", "код", "Code", "kod"],
            "leave": ["Leave", "Вийти", "Выйти", "Verlassen", "Opuść"],
            "leaveRoom": ["Leave room", "Вийти з кімнати", "Выйти из комнаты", "Raum verlassen", "Opuść pokój"],
            "joinMatch": ["Join match", "Увійти в матч", "Войти в матч", "Match beitreten", "Dołącz do meczu"],
            "joinIn": ["Into the match in %S s, or now with Join match", "У матч за %S с, або зараз: «Увійти в матч»",
                "В матч через %S с, или сейчас: «Войти в матч»", "Ins Match in %S s, oder jetzt: Match beitreten",
                "Do meczu za %S s albo teraz: Dołącz do meczu"],
            "switchTo": ["Switch room", "Змінити кімнату", "Сменить комнату", "Raum wechseln", "Zmień pokój"],
            // Short: they share the footer with two buttons.
            "switchWhy": ["A place free: room for gear up to level %L", "Є місце: кімната для рівня спорядження до %L",
                "Есть место: комната для снаряжения до %L", "Platz frei: Raum bis Ausrüstungsstufe %L",
                "Wolne miejsce: pokój do poziomu %L"],
            "switchWho": ["%N's room · %P players", "Кімната %N · гравців: %P", "Комната %N · игроков: %P",
                "Raum von %N · %P Spieler", "Pokój %N · graczy: %P"],
            "countdown": ["Starts", "Старт", "Старт", "Start", "Start"],
            "inMatch": ["Playing", "Грають", "Играют", "Läuft", "Gra"],
            "experimental": ["exp.", "експ.", "эксп.", "exp.", "eksp."],
            "ownerTag": ["owner", "власник", "владелец", "Besitzer", "właściciel"],
            "queue": ["Queue", "Черга", "Очередь", "Warteschlange", "Kolejka"],
            "inQueue": ["In queue", "У черзі", "В очереди", "In der Warteschlange", "W kolejce"],
            "leaveQueue": ["Leave queue", "Вийти з черги", "Выйти из очереди", "Warteschlange verlassen",
                "Opuść kolejkę"],
            "modes": ["Modes", "Режими", "Режимы", "Modi", "Tryby"],
            "open": ["open", "вільно", "свободно", "frei", "wolne"],
            "voteHint": ["3 cards from the pool. Most votes wins; a tie is drawn at random. Click to change your vote.",
                "3 картки з пулу. Перемагає більшість голосів; нічия — випадково. Клацніть, щоб змінити голос.",
                "3 карточки из пула. Побеждает большинство; ничья — случайно. Нажмите, чтобы сменить голос.",
                "3 Karten aus dem Pool. Die meisten Stimmen gewinnen; Gleichstand: Zufall. Klicke, um umzustimmen.",
                "3 karty z puli. Wygrywa większość; remis losowo. Kliknij, aby zmienić głos."],
            "unfitKit": ["Your kit cannot play (no weapon or too little ammo): fix it, or you leave the room at the start.",
                "Ваш комплект не може грати (немає зброї або мало набоїв): виправте, інакше на старті ви вийдете з кімнати.",
                "Ваш комплект не может играть (нет оружия или мало патронов): исправьте, иначе на старте вы выйдете из комнаты.",
                "Deine Ausrüstung kann nicht spielen (keine Waffe oder zu wenig Munition): sonst verlässt du beim Start den Raum.",
                "Twój zestaw nie może grać (brak broni lub za mało amunicji): popraw, inaczej na starcie opuścisz pokój."],
            "unfitGear": ["Your equipment is above the max level: lower it, or you leave the room at the start.",
                "Ваше спорядження вище макс. рівня: знизьте, інакше на старті ви вийдете з кімнати.",
                "Ваше снаряжение выше макс. уровня: снизьте, иначе на старте вы выйдете из комнаты.",
                "Deine Ausrüstung liegt über der max. Stufe: senke sie, sonst verlässt du beim Start den Raum.",
                "Twój ekwipunek przekracza maks. poziom: obniż go, inaczej na starcie opuścisz pokój."],
            // Why a member waiting in its room cannot go into the match now
            // (`J;-1;<why>`, the owner's of 2026-10-20).
            "joinWhy1": ["Another player is joining the match; you go in after.",
                "Інший гравець заходить у матч; ви — після нього.", "Другой игрок заходит в матч; вы — после него.",
                "Ein anderer Spieler betritt das Match; du kommst danach.", "Inny gracz dołącza do meczu; ty wchodzisz po nim."],
            "joinWhy2": ["A player's game is loading; joining opens in a moment.",
                "Гра одного з гравців завантажується; вхід відкриється за мить.",
                "Игра одного из игроков загружается; вход откроется через мгновение.",
                "Das Spiel eines Spielers lädt; Beitreten gleich wieder möglich.",
                "Gra jednego z graczy się wczytuje; dołączenie za chwilę."],
            "joinWhy3": ["Under 30 s left: you'll play the next match.", "Лишилося менше 30 с: ви зіграєте наступний матч.",
                "Осталось меньше 30 с: вы сыграете следующий матч.", "Weniger als 30 s übrig: du spielst das nächste Match.",
                "Zostało mniej niż 30 s: zagrasz następny mecz."],
            "joinWhy4": ["The match is full.", "Матч заповнений.", "Матч заполнен.", "Das Match ist voll.", "Mecz jest pełny."],
            "joinWhy5": ["Server maintenance soon: no new joins.", "Скоро технічні роботи: нові входи закрито.",
                "Скоро технические работы: новые входы закрыты.", "Bald Wartung: keine neuen Beitritte.",
                "Wkrótce przerwa techniczna: brak nowych wejść."],
            "joinWhy6": ["The match is ending: you'll play the next one.", "Матч завершується: ви зіграєте наступний.",
                "Матч завершается: вы сыграете следующий.", "Das Match endet: du spielst das nächste.",
                "Mecz się kończy: zagrasz następny."],
            "lateHint": ["Who joins now sees the room, then goes into the match.", "Хто зайде зараз, побачить кімнату, потім потрапить у матч.",
                "Кто зайдёт сейчас, увидит комнату, потом попадёт в матч.", "Wer jetzt beitritt, sieht den Raum, dann geht es ins Match.",
                "Kto dołączy teraz, zobaczy pokój, potem trafi do meczu."],
            "waitingShort": ["Waiting", "Очікування", "Ожидание", "Warten", "Oczekiwanie"],
            "code0": ["", "", "", "", ""],
            "code2": ["That room is gone.", "Цієї кімнати вже немає.", "Этой комнаты больше нет.",
                "Diesen Raum gibt es nicht mehr.", "Ten pokój już nie istnieje."],
            "code3": ["Wrong PIN.", "Невірний PIN.", "Неверный PIN.", "Falsche PIN.", "Błędny PIN."],
            "code4": ["That room is full.", "Кімната заповнена.", "Комната заполнена.", "Der Raum ist voll.", "Pokój jest pełny."],
            "code5": ["You are in a match.", "Ви в матчі.", "Вы в матче.", "Du bist in einem Match.", "Jesteś w meczu."],
            "code6": ["Too many rooms now.", "Забагато кімнат.", "Слишком много комнат.", "Zu viele Räume.", "Za dużo pokojów."],
            "code7": ["Update your game.", "Оновіть гру.", "Обновите игру.", "Aktualisiere dein Spiel.", "Zaktualizuj grę."],
            "code8": ["Pick at least one map.", "Виберіть хоча б одну карту.", "Выберите хотя бы одну карту.",
                "Wähle mindestens eine Karte.", "Wybierz co najmniej jedną mapę."],
            "code9": ["Maintenance soon.", "Скоро технічні роботи.", "Скоро технические работы.", "Bald Wartung.", "Wkrótce przerwa techniczna."],
            "code10": ["The owner removed you from that room.", "Власник прибрав вас із кімнати.",
                "Владелец убрал вас из комнаты.", "Der Besitzer hat dich entfernt.", "Właściciel usunął cię z pokoju."],
            "code11": ["Leave your clan team first.", "Спершу вийдіть із кланової команди.", "Сначала выйдите из клановой команды.",
                "Verlasse zuerst dein Clanteam.", "Najpierw opuść drużynę klanu."],
            "code12": ["Your equipment is above the max level.", "Ваше спорядження вище макс. рівня.",
                "Ваше снаряжение выше макс. уровня.", "Deine Ausrüstung liegt über der max. Stufe.",
                "Twój ekwipunek przekracza maks. poziom."],
            "code13": ["Your kit could not play (no weapon or too little ammo): you were taken out of the room.",
                "Ваш комплект не міг грати (немає зброї або мало набоїв): вас вивели з кімнати.",
                "Ваш комплект не мог играть (нет оружия или мало патронов): вас вывели из комнаты.",
                "Deine Ausrüstung konnte nicht spielen (keine Waffe oder zu wenig Munition): du wurdest aus dem Raum genommen.",
                "Twój zestaw nie mógł grać (brak broni lub za mało amunicji): usunięto cię z pokoju."]
        };
        private static const FRIENDS:Array = ["Friends", "Друзі", "Друзья", "Freunde", "Znajomi"];
        private static const LEVELS:Array = [
            null,
            ["Rudnya", "Рудня", "Рудня", "Rudnya", "Rudnia"],
            ["School", "Школа", "Школа", "Schule", "Szkoła"],
            ["Vostok Radar Station", "РЛС \"Восток\"", "РЛС \"Восток\"", "RS Vostok", "Stacja Radarowa \"Vostok\""],
            ["Chemical Plant", "Хімзавод", "Химзавод", "Chemiebetrieb", "Zakłady chemiczne"],
            ["Tarakanovsky Fort", "Тараканівський форт", "Таракановский форт", "Tarakanowski Fort", "Fort Tarakanów"],
            ["Mamayev Kurgan", "Мамаїв курган", "Мамаев курган", "Mamaew kurgan", "Kurhan Mamaja"],
            ["Vector Laboratory", "Лабораторія \"Вектор\"", "Лаборатория \"Вектор\"", "Labor Vektor", "Laboratorium \"Wektor\""],
            ["Cologne", "Кельн", "Кельн", "Köln", "Most w Kolonii"],
            ["London", "Лондон", "Лондон", "London", "Londyn"],
            // The ports of revision 23 (BottomPanel's own table).
            ["Vostok Radar Station (v0.21)", "РЛС \"Восток\" (v0.21)", "РЛС \"Восток\" (v0.21)", "RS Vostok (v0.21)",
                "Stacja Radarowa \"Vostok\" (v0.21)"],
            ["Mamayev Kurgan (v0.21)", "Мамаїв курган (v0.21)", "Мамаев курган (v0.21)", "Mamaew kurgan (v0.21)",
                "Kurhan Mamaja (v0.21)"],
            ["Chemical Plant (v0.11)", "Хімзавод (v0.11)", "Химзавод (v0.11)", "Chemiebetrieb (v0.11)",
                "Zakłady chemiczne (v0.11)"],
            ["Vostok Radar Station (v0.11)", "РЛС \"Восток\" (v0.11)", "РЛС \"Восток\" (v0.11)", "RS Vostok (v0.11)",
                "Stacja Radarowa \"Vostok\" (v0.11)"]
        ];
        // The render a level is drawn with: the v0.21 ports with the map they
        // are an older look of, the v0.11 ones with their one loading image.
        private static const RENDER_LEVEL:Object = {"10": 3, "11": 6};
        private static const RENDER_ONE:Object = {"12": "Level_v011e", "13": "Level_v011e"};
        private static const MODES:Object = {
            "2": ["Protective device", "Захисний пристрій", "Защитное устройство", "Schutzeinrichtung", "Urządzenie ochronne"],
            "3": ["Protective device: chargers", "Захисний пристрій: зарядники", "Защитное устройство: зарядники",
                "Schutzeinrichtung: Ladestationen", "Urządzenie ochronne: ładowarki"],
            "4": ["Team Deathmatch", "Командний бій", "Командный бой", "Kommandokampf", "Walka drużynowa"]
        };
        // maps_dict's map_weather_id: 1 day, 2 rain, 3 evening, 4 night, 5 prerain.
        private static const WEATHER:Object = {
            "1": ["day", "день", "день", "Tag", "dzień"],
            "2": ["rain", "дощ", "дождь", "Regen", "deszcz"],
            "3": ["evening", "вечір", "вечер", "Abend", "wieczór"],
            "4": ["night", "ніч", "ночь", "Nacht", "noc"],
            "5": ["before rain", "перед дощем", "перед дождём", "vor dem Regen", "przed deszczem"]
        };
        private static const RENDER:Object = {"1": "day", "2": "rain", "3": "evn", "4": "night", "5": "prerain"};
        private static var _renders:Object = {};

        public static var me:SrpRooms;

        private var _panel:*;
        private var _lobby:*;
        private var _font:String;
        private var _bold:Object;
        private var _lang:int = 0;
        private var _gear:int = 0;
        private var _shown:Boolean = false;
        private var _view:String = "list";
        private var _content:Sprite;
        private var _w:Number = 900;
        private var _h:Number = 500;
        private var _timer:Timer;
        private var _now:int = 0;
        private var _askedAt:int = -100000;
        private var _listAt:int = -100000;
        private var _fastUntil:int = 0;
        private var _rooms:Array = [];
        private var _total:int = 0;
        private var _room:Object = null;
        private var _roomAt:int = 0;
        private var _code:int = 0;
        private var _pool:Array = [];
        private var _poolTotal:int = -1;
        private var _off:Object = {};
        private var _steps:Array = [];
        private var _size:int = 8;
        // The players a side the room needs before its vote: 1 v 1 to 3 v 3.
        private var _minSide:int = 1;
        private var _max:int = 0;
        private var _wait:int = 30;
        private var _pinText:String = "";
        private var _joinPin:TextField;
        private var _makePin:TextField;
        private var _backdrop:Sprite;
        // Controls placed straight in the content, each with what a press on
        // it does: Scaleform gave such presses to the content itself (the
        // left of the make view took none: PIN, sliders, modes).
        private var _routes:Array = [];
        // What was typed in it, kept across redraws (a tile clicked, a mode).
        private var _makePinText:String = "";
        private var _nativePlay:Boolean = true;
        private var _tabText:String = "";
        private var _poolAskedAt:int = -100000;
        private var _roomSig:String = "";
        // The full room whose queue we wait in, and our place (1 the first);
        // 0: none.
        private var _queueKey:int = 0;
        private var _queuePlace:int = 0;
        // Play was pressed and its answer has not come yet: Play stays grey
        // (until then; then while in a room).
        private var _playAskedAt:int = 0;
        // The list as last drawn, but its times: a change redraws it.
        private var _listSig:String = "";
        // Its rows' state cells by room key (and the side panel's), so the
        // times change in place.
        private var _stateCells:Object = {};

        // ------------------------------------------------------ the hooks

        // TopMenu's tab of the map (its index 4, which the game leaves off)
        // is ours: Servers, and the countdown while the window is hidden.
        public static const TOP_INDEX:int = 4;
        private static var _top:*;
        private var _syncing:Boolean = false;

        // The end of TopMenu.fill_windows: the tabs are made; ours named.
        public static function topMenu(menu:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                _top = menu;
                if (me != null)
                {
                    me._tabText = "";
                    me.tabLabel();
                }
                else
                    topLabel(say0("servers"));
            }
            catch (err:Error)
            {
            }
        }

        // The end of TopMenu.changeActiveMenu: ours chosen opens the window;
        // another closes it (as the game's own windows do).
        public static function topChanged(menu:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                _top = menu;
                if (me == null || me._syncing)
                    return;
                var ours:Boolean = false;
                try
                {
                    ours = menu.tabPage.selectedIndex == TOP_INDEX;
                }
                catch (e:Error)
                {
                }
                if (ours != me._shown)
                    me.show(ours);
            }
            catch (err:Error)
            {
            }
        }

        // Ours is on as the Clan tab is: the lobby locks its tabs for a moment
        // while it loads, and ours then waits with them.
        private static var _topOn:Boolean = true;

        private static function topLabel(text:String):void
        {
            if (_top == null)
                return;
            try
            {
                var clan:Object = _top.tabPage.dataProvider[3];
                _topOn = clan == null || clan.enabled !== false;
                _top.tabPage.dataProvider[TOP_INDEX] = {"name": text, "icon": 6, "enabled": _topOn};
                _top.tabPage.dataProvider.invalidate();
            }
            catch (e:Error)
            {
            }
        }

        // The words before the window is made: the game's language is known
        // by then from its own texts.
        private static function say0(key:String):String
        {
            var lang:int = 0;
            var friends:String = LocalizationData.getName("btn_friends");
            for (var i:int = 1; i < FRIENDS.length; i++)
                if (friends == FRIENDS[i])
                    lang = i;
            return String(TEXT[key][lang]);
        }

        // The end of BottomPanel.createMatchesTab.
        public static function attach(panel:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                hideMatchesTab(panel);
                if (me != null)
                    return;
                me = new SrpRooms(panel);
            }
            catch (err:Error)
            {
            }
        }

        // The Matches tab (Skhid's list of running matches and their JOIN) is
        // gone: every PvP match is a room's, listed and joined in this window
        // (the owner's of 2026-10-17). The tab is still made, as this window
        // is attached when it is, and hidden at once.
        private static function hideMatchesTab(panel:*):void
        {
            try
            {
                var tab:DisplayObject = panel.matchesTab as DisplayObject;
                if (tab != null && tab.visible)
                    tab.visible = false;
            }
            catch (e:Error)
            {
            }
        }

        // The end of BottomPanel.placeMatchesTab: the window follows the bar.
        public static function place(panel:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                hideMatchesTab(panel);
                if (me != null)
                    me.layout();
            }
            catch (err:Error)
            {
            }
        }

        // The end of BottomPanel.resizeTab: the window follows the screen.
        public static function resize(panel:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                hideMatchesTab(panel);
                if (me != null)
                    me.layout();
            }
            catch (err:Error)
            {
            }
        }

        // The start of PlayerRanking.fillPlayers: true takes the rows.
        public static function take(rows:Array):Boolean
        {
            if (rows == null || rows.length == 0 || rows[0] == null)
                return false;
            var place:Number = Number(rows[0].place);
            if (place < FIRST || place > LAST)
                return false;
            if (me != null)
            {
                try
                {
                    me.answer(rows);
                }
                catch (e:Error)
                {
                }
            }
            return true;
        }

        // The start of LobbyMenu.show_victory_screen (a match's results): a
        // player in a room gets the room's window opened first, so the
        // results screen goes over it and closing them shows the room (the
        // owner's of 2026-10-20). Out of a room: nothing.
        public static function afterMatch():void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me != null && me._room != null && !me._shown)
                    me.show(true);
            }
            catch (err:Error)
            {
            }
        }

        // The results screen is up (over our window, after a match): Esc and
        // the list's keys are its, not ours.
        private function resultsUp():Boolean
        {
            try
            {
                var screen:DisplayObject = _lobby != null ? _lobby["victory_screen"] as DisplayObject : null;
                return screen != null && screen.visible && screen.stage != null;
            }
            catch (err:Error)
            {
            }
            return false;
        }

        // The end of LobbyMenu.set_play_button_enabled: in a room or a room's
        // queue Play is our Leave (on); otherwise the game's own word.
        public static function playState(lobby:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me == null)
                    return;
                me._lobby = lobby;
                try
                {
                    me._nativePlay = lobby.statusPanel.playBtn.enabled;
                }
                catch (e:Error)
                {
                }
                me.applyPlay();
            }
            catch (err:Error)
            {
            }
        }

        // The start of StatusPanel.play_button_clicked (the Play button; a
        // JOIN and a match against bots call the game's Play themselves):
        // with rooms on, Play goes straight into a room, no search shown.
        public static function playPressed():Boolean
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                // Rooms are srp's only PvP (always on): the game's own Play,
                // its search, is never run from this button.
                if (me == null)
                    return false;
                // In a room it is "Leave room" (red): out of the room.
                if (me.leavesRoom())
                {
                    me._playAskedAt = getTimer();
                    me.applyPlay();
                    me.ask(LEAVE, 1);
                    me._fastUntil = getTimer() + 5000;
                    return true;
                }
                // In a room's queue it is "Leave queue" (red): out of it.
                if (me.leavesQueue())
                {
                    me._playAskedAt = getTimer();
                    me.applyPlay();
                    me.ask(UNQUEUE, 1);
                    me._fastUntil = getTimer() + 5000;
                    return true;
                }
                me.quickPlay();
                return true;
            }
            catch (err:Error)
            {
                return false;
            }
        }

        private function quickPlay():void
        {
            _code = 0;
            _steps = [];
            if (!_shown)
                show(true);
            _fastUntil = getTimer() + 10000;
            _playAskedAt = getTimer();
            applyPlay();
            ask(PLAY, 1);
        }

        // The end of StatusPanel.play_button_clicked: Play may have put us in
        // a room; ask at once and often for a while.
        public static function played(status:*):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me == null)
                    return;
                me._fastUntil = getTimer() + 10000;
                me._askedAt = -100000;
                me.ask(STATE, 1);
            }
            catch (err:Error)
            {
            }
        }

        // The start of StatusPanel.show_match_making: the game's search panel
        // is not shown to a room's member (or right after Play), whom the
        // server sends to its room's match through the game's queue state.
        public static function searchHidden(show:Boolean):Boolean
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me == null || !show)
                    return false;
                return me._room != null || me._playAskedAt != 0 || getTimer() < me._fastUntil;
            }
            catch (err:Error)
            {
            }
            return false;
        }

        // The start of LobbyMenu.has_active_window: our window counts, so Esc
        // closes it before it opens the game's menu.
        public static function escapable():Boolean
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                return me != null && me._shown && !me.resultsUp();
            }
            catch (err:Error)
            {
                return false;
            }
        }

        // The start of LobbyMenu.close_active_window: Esc hides our window.
        public static function closeWindow():Boolean
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me == null || !me._shown || MainScene.activePop != null || me.resultsUp())
                    return false;
                me.show(false);
                return true;
            }
            catch (err:Error)
            {
                return false;
            }
        }

        // The end of LobbyMenu.set_equipment_level: the player's own level.
        public static function gear(value:uint):void
        {
            // A hook: nothing of it may throw into the game's code.
            try
            {
                if (me != null)
                    me._gear = int(value);
            }
            catch (err:Error)
            {
            }
        }

        // ------------------------------------------------------ the tab

        public function SrpRooms(panel:*)
        {
            super();
            _panel = panel;
            var friends:String = LocalizationData.getName("btn_friends");
            for (var i:int = 1; i < FRIENDS.length; i++)
                if (friends == FRIENDS[i])
                    _lang = i;
            // The font of the bottom bar's tabs (the game's own), for ours.
            var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
            var sample:* = new tabClass();
            var tabText:TextField = sample["textField"] as TextField;
            var format:TextFormat = tabText != null ? tabText.getTextFormat() : new TextFormat();
            addEventListener(Event.ENTER_FRAME, onFrame, false, 0, true);
            tabLabel();
            _font = format.font;
            _bold = format.bold;
            name = "srpRoomsWindow";
            visible = false;
            _backdrop = new Sprite();
            _backdrop.mouseEnabled = false;
            _backdrop.mouseChildren = false;
            addChild(_backdrop);
            _content = new Sprite();
            addChild(_content);
            // Presses Scaleform gives the content itself, not the control
            // under them, go to that control here (see route()).
            _content.addEventListener(MouseEvent.MOUSE_DOWN, onContentDown);
            addEventListener(MouseEvent.MOUSE_WHEEL, onWheel, false, 0, true);
            if (panel.parent != null)
                panel.parent.addChild(this);
            _timer = new Timer(1000, 0);
            _timer.addEventListener(TimerEvent.TIMER, onTimer, false, 0, true);
            _timer.start();
            // Tells the server this movie has rooms (its Play goes to one),
            // and finds a room the player is in already.
            ask(STATE, 1);
            layout();
        }

        private function setTab(text:String):void
        {
            if (text == _tabText)
                return;
            _tabText = text;
            topLabel(text);
        }

        private function onFrame(e:Event):void
        {
            // The Clan tab's lock came or went: ours follows.
            try
            {
                if (_top != null)
                {
                    var clan:Object = _top.tabPage.dataProvider[3];
                    if ((clan == null || clan.enabled !== false) != _topOn)
                        topLabel(_tabText != "" ? _tabText : say("servers"));
                }
            }
            catch (err:Error)
            {
            }
            watchWindows();
            topIcon();
            overPanels();
            alertNear();
            applyPlay(true);
        }

        // While open, ours is the lobby's top child: the lobby adds its own
        // parts again over it (side panels, chat), and their hidden areas
        // then take the clicks meant for our window. The game's windows that
        // open over it close it first (the top menu).
        // The chat stays over ours (the owner's of 2026-10-19).
        private static const PANELS:Array = ["squadList", "questList"];

        private function overPanels():void
        {
            if (!_shown || parent == null)
                return;
            for (var i:int = 0; i < PANELS.length; i++)
            {
                var panel:DisplayObject = null;
                try
                {
                    panel = parent[PANELS[i]] as DisplayObject;
                }
                catch (err:Error)
                {
                }
                if (panel != null && panel.parent == parent
                    && parent.getChildIndex(panel) > parent.getChildIndex(this))
                    parent.setChildIndex(this, parent.getChildIndex(panel));
            }
            var chat:DisplayObject = null;
            try
            {
                chat = parent["chat"] as DisplayObject;
            }
            catch (e:Error)
            {
            }
            if (chat != null && chat.parent == parent && parent.getChildIndex(chat) < parent.getChildIndex(this))
                parent.setChildIndex(chat, parent.getChildIndex(this));
        }

        // Our own icon on the top tab: the map's art (frame 6) is drawn
        // darker than the others. A stack of three servers in the game
        // icon's place, as bright as the tab makes its icon (its alpha and
        // colour transform, copied each frame).
        private var _topIcon:Shape;

        private function topIcon():void
        {
            if (_top == null)
                return;
            try
            {
                var tab:* = _top.getRenderer(TOP_INDEX);
                var art:MovieClip = tab.icon as MovieClip;
                if (tab == null || art == null)
                    return;
                if (_topIcon == null || _topIcon.parent != tab)
                {
                    _topIcon = new Shape();
                    var g:* = _topIcon.graphics;
                    // One tone, as the skull of the Clan tab: three servers,
                    // each a small square and a bar to its right.
                    g.beginFill(0x4E5457, 1);
                    for (var i:int = 0; i < 3; i++)
                    {
                        g.drawRect(-13, -12 + i * 9, 6, 6);
                        g.drawRect(-4, -12 + i * 9, 17, 6);
                    }
                    g.endFill();
                    tab.addChild(_topIcon);
                }
                var box:Rectangle = art.getBounds(tab);
                if (box.width > 0)
                {
                    _topIcon.x = Math.round(box.x + box.width / 2);
                    _topIcon.y = Math.round(box.y + box.height / 2);
                }
                art.visible = false;
                _topIcon.alpha = art.alpha;
                _topIcon.transform.colorTransform = marked() ? YELLOW_TINT : art.transform.colorTransform;
                topColour(tab.textField as TextField);
            }
            catch (e:Error)
            {
            }
        }

        // The tab's label: yellow while we are in a room, its own colour
        // otherwise (the tab sets it again on each state: ours each frame).
        private var _topText:TextField;
        private var _topTextColour:uint;

        private function topColour(field:TextField):void
        {
            if (field == null)
                return;
            if (field != _topText)
            {
                _topText = field;
                _topTextColour = field.textColor;
            }
            if (marked())
            {
                if (field.textColor != YELLOW)
                    field.textColor = YELLOW;
            }
            else if (field.textColor == YELLOW)
                field.textColor = _topTextColour;
        }

        private function show(value:Boolean):void
        {
            _shown = value;
            visible = value;
            // Enter in the list joins the room chosen (once on the stage).
            if (value && !_keys && stage != null)
            {
                _keys = true;
                stage.addEventListener(KeyboardEvent.KEY_DOWN, onKey, false, 0, false);
            }
            // The top menu's tab follows: chosen while the window is open.
            if (_top != null)
            {
                _syncing = true;
                try
                {
                    var at:int = _top.tabPage.selectedIndex;
                    if (value && at != TOP_INDEX)
                        _top.selectActiveWindow(TOP_INDEX);
                    else if (!value && at == TOP_INDEX)
                        _top.selectActiveWindow(-1);
                }
                catch (e:Error)
                {
                }
                _syncing = false;
            }
            if (value)
            {
                if (_room != null)
                    _view = "room";
                else if (_view == "room")
                    _view = "list";
                toTop();
                _askedAt = -100000;
                _listAt = -100000;
                onTimer(null);
            }
            layout();
            render();
        }

        // As the lobby's other menus: ours opens on top, and when one of
        // them opens (equipment, shop, ranking, character, clan, quests,
        // upgrades) ours is hidden; its tab opens it again.
        private static const WINDOWS:Array = ["inventory_list", "shop_list", "ranking", "charWindow",
            "clanWindow", "questWindow", "modifierWindow"];
        private var _windowsSeen:Object = {};

        private function toTop():void
        {
            if (parent != null)
                parent.setChildIndex(this, parent.numChildren - 1);
            overPanels();
        }

        // Each frame: a lobby window that has just opened hides ours.
        private function watchWindows():void
        {
            if (_lobby == null || parent == null)
                return;
            for (var i:int = 0; i < WINDOWS.length; i++)
            {
                var w:DisplayObject = null;
                try
                {
                    w = _lobby[WINDOWS[i]] as DisplayObject;
                }
                catch (e:Error)
                {
                }
                var open:Boolean = w != null && w.visible && w.stage != null;
                if (open && _windowsSeen[WINDOWS[i]] != true && _shown)
                    show(false);
                _windowsSeen[WINDOWS[i]] = open;
            }
        }

        private function layout():void
        {
            try
            {
                // The design's 1280 x 720, fitted whole on a wider or taller
                // screen.
                var k:Number = Math.min(MainScene.stageWidth / DESIGN_W, MainScene.stageHeight / DESIGN_H);
                scaleX = k;
                scaleY = k;
                var w:int = int(DESIGN_W - LEFT - RIGHT);
                var h:int = int((_panel.y - BOTTOM_GAP) / k - TOP);
                // The make form's height at least.
                if (h < MIN_H)
                    h = MIN_H;
                if (w != _w || h != _h)
                {
                    _w = w;
                    _h = h;
                    render();
                }
                x = int(LEFT * k);
                y = int(_panel.y - BOTTOM_GAP - _h * k);
            }
            catch (e:Error)
            {
            }
        }

        // In a room, Play is "Leave room", red, and takes us out of it (the
        // owner's of 2026-10-19); but when our row says we play the room's
        // match, yet we are here (the game started anew), it is Play and
        // takes us back to our seat.
        private function leavesRoom():Boolean
        {
            if (_room == null)
                return false;
            for (var i:int = 0; i < _room.members.length; i++)
                if (_room.members[i].me && _room.members[i].inMatch)
                    return false;
            return true;
        }

        // In no room but in a full room's queue: Play is "Leave queue", red,
        // and takes us out of the queue (the owner's of 2026-10-20).
        private function leavesQueue():Boolean
        {
            return _room == null && _queueKey != 0;
        }

        // The Rooms tab is yellow while we are in a room or a room's queue
        // (the owner's of 2026-10-20).
        private function marked():Boolean
        {
            return _room != null || _queueKey != 0;
        }

        // A button's art tinted, its label not: the text stays white, as
        // PLAY's (the owner's of 2026-10-20). Every part of the button but
        // the one that holds its text field takes the tint.
        private static function skinTint(btn:*, tint:ColorTransform):void
        {
            var box:DisplayObjectContainer = btn as DisplayObjectContainer;
            if (box == null)
                return;
            box.transform.colorTransform = new ColorTransform();
            var field:DisplayObject = null;
            try
            {
                field = btn.textField as DisplayObject;
            }
            catch (err:Error)
            {
            }
            for (var i:int = 0; i < box.numChildren; i++)
            {
                var part:DisplayObject = box.getChildAt(i);
                if (field != null && (part == field
                    || (part is DisplayObjectContainer && DisplayObjectContainer(part).contains(field))))
                    continue;
                part.transform.colorTransform = tint;
            }
        }

        // The game's own label of Play, while ours is on it, and ours.
        private var _playLabel:String = null;
        private var _ourPlayLabel:String = null;

        // `each`: the frame's call, which leaves the game's own Play enabled as
        // the game sets it (only our Leave is kept on).
        private function applyPlay(each:Boolean = false):void
        {
            if (_lobby == null)
                return;
            try
            {
                var btn:* = _lobby.statusPanel.playBtn;
                var leaving:Boolean = leavesRoom();
                var queued:Boolean = !leaving && leavesQueue();
                // Grey from a press until its answer (6 s at most). Leave room
                // and Leave queue need none of the game's Play conditions (a
                // squad's member, a kit the game refuses): on whatever the
                // game's Play is. Play: the game's own word.
                var asking:Boolean = _playAskedAt != 0 && getTimer() - _playAskedAt < 6000;
                if (leaving || queued || !each)
                {
                    var on:Boolean = !asking && (leaving || queued || _nativePlay);
                    if (btn.enabled != on)
                        btn.enabled = on;
                }
                // In the case of the game's own label (its Play is in capitals).
                var native:String = _playLabel != null ? _playLabel : String(btn.label);
                var label:String = say(leaving ? "leaveRoom" : "leaveQueue");
                if (native != "" && native == native.toUpperCase())
                    label = label.toUpperCase();
                if (leaving || queued)
                {
                    // The game sets its own label again (its language): ours
                    // over it each time.
                    if (btn.label != label)
                    {
                        if (_playLabel == null)
                            _playLabel = btn.label;
                        btn.label = label;
                    }
                    _ourPlayLabel = label;
                    skinTint(btn, RED_TINT);
                }
                else if (_playLabel != null)
                {
                    if (btn.label == _ourPlayLabel)
                        btn.label = _playLabel;
                    _playLabel = null;
                    _ourPlayLabel = null;
                    skinTint(btn, new ColorTransform());
                }
            }
            catch (e:Error)
            {
            }
        }

        // ------------------------------------------------------ the wire

        private function ask(start:int, count:int):void
        {
            _askedAt = getTimer();
            try
            {
                ExternalInterface.call("request_players_range", start, count);
            }
            catch (e:Error)
            {
            }
        }

        private function onTimer(e:TimerEvent):void
        {
            _now = getTimer();
            // A chain or a pressed button with no answer for 6 s: given up,
            // the buttons back.
            if ((_steps.length > 0 && _now - _stepsAt > 6000) || (_busyAt != 0 && _now - _busyAt > 6000))
            {
                _steps = [];
                _busyAt = 0;
                if (_shown)
                    render();
            }
            // No answer to Play (or Leave) within 6 s: the button is back. Silence never drops the
            // room: during a match the lobby may not answer, and the next
            // answer says where we are.
            if (_playAskedAt != 0 && _now - _playAskedAt >= 6000)
            {
                _playAskedAt = 0;
                applyPlay();
            }
            var fast:Boolean = _now < _fastUntil;
            // The pool names and draws every card: fetched once, whatever the view.
            if (_poolTotal < 0 && _steps.length == 0 && _now - _poolAskedAt >= 3000)
            {
                _poolAskedAt = _now;
                ask(POOL, 31);
                return;
            }
            if (_steps.length == 0)
            {
                if (_room != null || fast || _queueKey != 0 || (_shown && _view == "room"))
                {
                    if (_now - _askedAt >= (fast ? 1000 : 2000))
                        ask(STATE, 1);
                }
                else if (_shown && _view == "list" && _now - _listAt >= 3000)
                {
                    _listAt = _now;
                    ask(LIST, 32);
                }
            }
            tabLabel();
            if (_shown && _view == "room")
                banner();
        }

        private function next():void
        {
            if (_steps.length > 0)
            {
                _stepsAt = getTimer();
                ask(int(_steps.shift()), 1);
            }
        }

        // A state answered while a chain (PIN, pool bits, then its create or
        // join) is on its way: the chain goes on (a poll asked before it may
        // answer meanwhile). One with nothing left is done; the button
        // pressed is given back by the redraw.
        private var _stepsAt:int = 0;
        private var _busyAt:int = 0;
        private var _redraw:Boolean = false;

        private function settle():void
        {
            if (_busyAt != 0 && _steps.length == 0)
            {
                _busyAt = 0;
                _redraw = true;
            }
        }

        private function fields(row:Object):Array
        {
            if (row == null || row.nickname == null)
                return [];
            return String(row.nickname).split(";");
        }

        private function answer(rows:Array):void
        {
            _answeredAt = getTimer();
            _redraw = false;
            answerRows(rows);
            if (_redraw && _shown)
                render();
            _redraw = false;
        }

        // The last answer, and (with _askedAt) whether the server still
        // answers.
        private var _answeredAt:int = 0;

        private function answerRows(rows:Array):void
        {
            if (rows == null || rows.length == 0 || rows[0] == null)
                return;
            var head:Array = fields(rows[0]);
            if (head[0] != "S1")
                return;
            var kind:String = head[1];
            // Where we are now is known: Play follows it.
            if (_playAskedAt != 0 && (kind == "R" || kind == "N"))
            {
                _playAskedAt = 0;
                applyPlay();
            }
            if (kind == "L")
            {
                _total = int(head[2]);
                _rooms = [];
                for (var i:int = 1; i < rows.length; i++)
                {
                    var f:Array = fields(rows[i]);
                    _rooms.push({"key": int(f[0]), "level": int(f[1]), "max": int(f[2]), "size": int(f[3]),
                        "players": int(f[4]), "phase": int(f[5]), "secs": int(f[6]), "map": int(f[7]),
                        "pin": f[8] == "1", "name": f.slice(9).join(";")});
                }
                var sig:String = "";
                for (var q:int = 0; q < _rooms.length; q++)
                {
                    var r0:Object = _rooms[q];
                    sig += [r0.key, r0.level, r0.max, r0.size, r0.players, r0.phase, r0.map, r0.pin, r0.name].join(",") + "|";
                }
                sig += _total;
                if (_view == "list")
                {
                    if (sig != _listSig)
                        render();
                    else
                        listTimes();
                }
                _listSig = sig;
            }
            else if (kind == "M")
            {
                _poolTotal = int(head[2]);
                var page:int = int(head[3]);
                if (page == 0)
                    _pool = [];
                for (var j:int = 1; j < rows.length; j++)
                {
                    var p:Array = fields(rows[j]);
                    if (p.length < 6 || hasEntry(int(p[0])))
                        continue;
                    _pool.push({"index": int(p[0]), "map": int(p[1]), "level": int(p[2]), "weather": int(p[3]),
                        "mode": int(p[4]), "extra": p[5] == "1"});
                }
                if (_pool.length < _poolTotal && rows.length > 1)
                    ask(POOL + page + 1, 31);
                else
                {
                    defaultPool();
                    // Settings that came before the pool: now they can be read.
                    if (_saved != null)
                        applySaved();
                }
                if (_view == "create" || _view == "room")
                    render();
            }
            else if (kind == "F")
            {
                // `S1;F;<saved>;<size>;<min side>;<max level>;<wait>;<modes off>`,
                // then `E;<chunk>;<bits>`: a pool entry's tile is in by its bit.
                var bits:Array = [];
                for (var fb:int = 1; fb < rows.length; fb++)
                {
                    var fe:Array = fields(rows[fb]);
                    if (fe[0] == "E" && fe.length > 2)
                        bits[int(fe[1])] = uint(fe[2]);
                }
                _saved = {"size": int(head[3]), "min": int(head[4]), "max": int(head[5]), "wait": int(head[6]),
                    "modesOff": head.length > 7 && head[7] != "" ? String(head[7]).split(",") : [], "bits": bits};
                if (_poolTotal >= 0 && _pool.length >= _poolTotal)
                    applySaved();
                if (_shown && _view == "create")
                    render();
            }
            else if (kind == "K" || kind == "B")
            {
                next();
            }
            else if (kind == "N")
            {
                settle();
                var had:Boolean = _room != null;
                _room = null;
                _roomSig = "";
                var code:int = int(head[2]);
                var queueKey:int = int(head[3]);
                var queuePlace:int = int(head[4]);
                var newQueue:Boolean = queueKey != _queueKey || queuePlace != _queuePlace;
                _queueKey = queueKey;
                _queuePlace = queuePlace;
                // Our own word (a PIN not of 4 digits) stays until the
                // server has one of its own.
                if (code == 0 && _code < 0)
                    code = _code;
                var newCode:Boolean = code != _code;
                _code = code;
                applyPlay();
                var wasRoom:Boolean = _view == "room";
                if (had || wasRoom)
                    _view = "list";
                // Only when something shown changed: this answers every poll,
                // and a view drawn anew under the mouse loses its clicks, its
                // drag and the PIN being typed.
                if (_shown && (had || wasRoom || newCode || newQueue))
                    render();
            }
            else if (kind == "R")
            {
                settle();
                _queueKey = 0;
                _queuePlace = 0;
                var was:Object = _room;
                _room = {"key": int(head[2]), "level": int(head[3]), "max": int(head[4]), "size": int(head[5]),
                    "wait": int(head[6]), "phase": int(head[7]), "secs": int(head[8]), "owner": head[9] == "1",
                    "pool": int(head[10]), "serial": int(head[12]), "pin": head[13] == "1", "next": int(head[14]), "min": int(head[15]),
                    "cards": [], "members": [], "last": 0, "switchTo": null, "joinIn": -1, "joinWhy": 0};
                _roomAt = getTimer();
                // A new vote (another room, or after its match): its alert
                // may sound again (`alertNear`).
                if (was == null || was.key != _room.key || was.phase != 3)
                    _alerted = false;
                for (var k:int = 1; k < rows.length; k++)
                {
                    var r:Array = fields(rows[k]);
                    if (r[0] == "C")
                        _room.cards.push({"map": int(r[2]), "votes": int(r[3]), "mine": r[4] == "1"});
                    else if (r[0] == "P")
                        _room.members.push({"gear": int(r[2]), "owner": r[3] == "1", "me": r[4] == "1",
                            "inMatch": r[5] == "1", "fit": int(r[6]), "away": r[7] == "1",
                            "name": r.slice(8).join(";")});
                    else if (r[0] == "J")
                    {
                        // `J;<seconds>`, or `J;-1;<why>`: it cannot go in now.
                        _room.joinIn = int(r[1]);
                        _room.joinWhy = r.length > 2 ? int(r[2]) : 0;
                    }
                    else if (r[0] == "W")
                        _room.switchTo = {"key": int(r[1]), "max": int(r[2]), "players": int(r[3]),
                            "size": int(r[4]), "name": r.slice(5).join(";")};
                    else if (r[0] == "L")
                        _room.last = int(r[1]);
                }
                _code = 0;
                applyPlay();
                var parts:Array = [];
                for (var q:int = 0; q < rows.length; q++)
                    parts.push(String(rows[q].nickname));
                head[8] = "";
                parts[0] = head.join(";");
                var sig:String = parts.join("|");
                var changed:Boolean = sig != _roomSig;
                _roomSig = sig;
                // A room that is new to us (made, joined, or Play put us there)
                // opens its window.
                if (was == null || was.key != _room.key)
                {
                    _view = "room";
                    if (!_shown)
                        show(true);
                }
                if (_shown && _view == "room" && changed)
                    render();
            }
        }

        // ------------------------------------------------------ the views

        private function say(key:String):String
        {
            var words:Array = TEXT[key] as Array;
            return words != null ? String(words[_lang]) : key;
        }

        private function levelName(level:int):String
        {
            var names:Array = level >= 0 && level < LEVELS.length ? LEVELS[level] as Array : null;
            return names != null ? String(names[_lang]) : say("level") + " " + level;
        }

        private function weatherName(weather:int):String
        {
            var names:Array = WEATHER[String(weather)] as Array;
            return names != null ? String(names[_lang]) : "";
        }

        private function modeName(mode:int):String
        {
            var names:Array = MODES[String(mode)] as Array;
            return names != null ? String(names[_lang]) : "";
        }

        private function hasEntry(index:int):Boolean
        {
            for (var i:int = 0; i < _pool.length; i++)
                if (_pool[i].index == index)
                    return true;
            return false;
        }

        private function entry(id:int):Object
        {
            for (var i:int = 0; i < _pool.length; i++)
                if (_pool[i].map == id)
                    return _pool[i];
            return null;
        }

        // A map's render for its weather, made small once and kept.
        // A map's render scaled to a tile or card, kept for the next time.
        private function render_(level:int, weather:int, w:int, h:int):Bitmap
        {
            var key:String = level + "_" + weather + "_" + w + "x" + h;
            var small:BitmapData = _renders[key] as BitmapData;
            if (small == null)
            {
                small = new BitmapData(w, h, false, 0x05080B);
                try
                {
                    var cls:Class = renderClass(level, weather);
                    var big:BitmapData = new cls() as BitmapData;
                    var scale:Number = Math.max(w / big.width, h / big.height);
                    var m:Matrix = new Matrix();
                    m.scale(scale, scale);
                    m.translate((w - big.width * scale) / 2, (h - big.height * scale) / 2);
                    small.draw(big, m, null, null, null, true);
                    big.dispose();
                }
                catch (e:Error)
                {
                }
                _renders[key] = small;
            }
            return new Bitmap(small);
        }

        // The library bitmap of a level in a weather; its day one when that
        // weather has none.
        private function renderClass(level:int, weather:int):Class
        {
            var one:String = RENDER_ONE[String(level)] as String;
            if (one != null)
                return getDefinitionByName(one) as Class;
            var base:int = RENDER_LEVEL[String(level)] != null ? int(RENDER_LEVEL[String(level)]) : level;
            try
            {
                return getDefinitionByName("Level" + base + "_" + RENDER[String(weather)]) as Class;
            }
            catch (e:Error)
            {
            }
            return getDefinitionByName("Level" + base + "_day") as Class;
        }

        private function text(box:Sprite, value:String, x:Number, y:Number, w:Number, size:Number,
            color:uint, align:String = "left"):TextField
        {
            var tf:TextField = new TextField();
            var format:TextFormat = new TextFormat(_font, size, color, _bold);
            format.align = align;
            tf.defaultTextFormat = format;
            tf.embedFonts = true;
            tf.selectable = false;
            tf.mouseEnabled = false;
            tf.multiline = false;
            tf.wordWrap = false;
            tf.x = x;
            tf.y = y;
            tf.width = w;
            tf.height = size + 8;
            tf.text = value;
            box.addChild(tf);
            return tf;
        }

        private function button(box:Sprite, label:String, x:Number, y:Number, w:Number, handler:Function,
            data:Object = null, h:Number = 32, primary:Boolean = false):*
        {
            // The game's framed button with a label (the game menu's look); the
            // red one for the main action, where it has a label too.
            var b:* = null;
            if (primary)
            {
                try
                {
                    var red:Class = getDefinitionByName("SrvButtonSkinnedRed") as Class;
                    b = new red();
                    if (b.textField == null)
                        b = null;
                }
                catch (e:Error)
                {
                    b = null;
                }
            }
            if (b == null)
            {
                var cls:Class = getDefinitionByName("SrvButtonSkinned") as Class;
                b = new cls();
            }
            b.label = label;
            b.x = x;
            b.y = y;
            box.addChild(b);
            try
            {
                b.setSize(w, h);
            }
            catch (e2:Error)
            {
                b.width = w;
                b.height = h;
            }
            b.data = data;
            b.addEventListener(ButtonEvent.CLICK, handler, false, 0, true);
            routeButton(b);
            return b;
        }

        // As tall as the game's close button (37 x 38) that sits in it.
        private static const HEAD_H:Number = 38;
        private static const FOOT_H:Number = 46;

        // The strip along the window's bottom, as the shop's under its list;
        // `buttons` are [label, handler, data] laid out centred in it, or at
        // `at` (x of the first) when given.
        private function footer(buttons:Array, at:Number = -1):Array
        {
            var made:Array = [];
            var y:Number = _h - FOOT_H;
            var strip:Shape = new Shape();
            strip.graphics.beginFill(0x07090A, 1);
            strip.graphics.drawRect(1, y, _w - 2, 1);
            strip.graphics.endFill();
            strip.graphics.beginFill(0x2A2F34, 1);
            strip.graphics.drawRect(1, y + 1, _w - 2, 1);
            strip.graphics.endFill();
            strip.graphics.beginFill(0x15181B, 1);
            strip.graphics.drawRect(1, y + 2, _w - 2, FOOT_H - 3);
            strip.graphics.endFill();
            _content.addChild(strip);
            var bw:Number = 170;
            var gap:Number = 12;
            var x:Number = at >= 0 ? at : (_w - buttons.length * bw - (buttons.length - 1) * gap) / 2;
            // Centred in the strip by the height the button really takes (the
            // game's keeps its own whatever is asked).
            for (var i:int = 0; i < buttons.length; i++)
            {
                // 32 high as All and None: squeezed lower, the game's button
                // keeps its label where its full height puts it.
                var b:* = button(_content, buttons[i][0], x + i * (bw + gap), y, bw, buttons[i][1], buttons[i][2], 32);
                b.validateNow();
                b.y = y + 1 + Math.round((FOOT_H - 1 - b.height) / 2);
                made.push(b);
            }
            return made;
        }

        // The game's own window close button (the inventory's, SquareVloseBtnChIL),
        // at its size in the header's right end; drawn when it is missing.
        private function closeButton(handler:Function):void
        {
            try
            {
                var cls:Class = getDefinitionByName("SquareVloseBtnChIL") as Class;
                var b:* = new cls();
                // At its own size (37 x 38), in the header's right end.
                b.x = _w - 38;
                // Its art sits high in its box: 2 lower centres it.
                b.y = 2;
                _content.addChild(b);
                b.addEventListener(ButtonEvent.CLICK, handler, false, 0, true);
                routeButton(b);
                return;
            }
            catch (e:Error)
            {
            }
            headButton("×", _w - (HEAD_H - 2) - 2, handler);
        }

        // A small square button of the header, as the shop's ×.
        private function headButton(glyph:String, x:Number, handler:Function):void
        {
            var size:Number = HEAD_H - 4;
            var b:Sprite = new Sprite();
            b.x = x;
            b.y = 3;
            var face:Shape = new Shape();
            face.graphics.beginFill(0x4A535B, 1);
            face.graphics.drawRect(0, 0, size, size);
            face.graphics.endFill();
            face.graphics.beginFill(0x0D1012, 1);
            face.graphics.drawRect(1, 1, size - 2, size - 2);
            face.graphics.endFill();
            b.addChild(face);
            var over:Shape = new Shape();
            over.graphics.beginFill(0xFFFFFF, 0.12);
            over.graphics.drawRect(1, 1, size - 2, size - 2);
            over.graphics.endFill();
            over.visible = false;
            b.addChild(over);
            text(b, glyph, 0, glyph == "×" ? 0 : -1, size, 17, 0xE6E9EB, "center");
            b.buttonMode = true;
            b.mouseChildren = false;
            // Strong: a closure held weakly may be collected at once.
            b.addEventListener(MouseEvent.ROLL_OVER, function(e:Event):void { over.visible = true; });
            b.addEventListener(MouseEvent.ROLL_OUT, function(e:Event):void { over.visible = false; });
            b.addEventListener(MouseEvent.CLICK, handler, false, 0, true);
            _content.addChild(b);
        }

        // A scrolled box: its rows go into the sprite it gives; done() sets
        // their height. The wheel and the bar's thumb move it.
        private var _scrollInner:Sprite;
        private var _scrollThumb:Sprite;
        private var _scrollTop:Number = 0;
        private var _scrollViewH:Number = 0;
        private var _scrollTotal:Number = 0;
        private var _scrollBarX:Number = 0;
        private var _scrollPx:Number = 0;
        private var _scrollGrab:Number = 0;

        // The rows scroll inside a holder at the box's place, its mask there
        // too: a mask drawn in the window's coordinates was hit-tested by
        // Scaleform at the rows' offset, over the controls left of the pool,
        // and took their clicks.
        private function scrollBox(x:Number, y:Number, w:Number, h:Number):Sprite
        {
            var holder:Sprite = new Sprite();
            holder.x = x;
            holder.y = y;
            var mask:Shape = new Shape();
            mask.graphics.beginFill(0x000000, 1);
            mask.graphics.drawRect(0, 0, w, h);
            mask.graphics.endFill();
            holder.addChild(mask);
            var inner:Sprite = new Sprite();
            holder.addChild(inner);
            inner.mask = mask;
            _content.addChild(holder);
            _scrollInner = inner;
            _scrollTop = y;
            _scrollViewH = h;
            _scrollBarX = x + w + 4;
            return inner;
        }

        private function scrollDone(total:Number):void
        {
            _scrollTotal = total;
            if (total > _scrollViewH)
            {
                var track:Shape = new Shape();
                track.graphics.beginFill(0x0F151B, 1);
                track.graphics.drawRect(_scrollBarX, _scrollTop, 6, _scrollViewH);
                track.graphics.endFill();
                _content.addChild(track);
                _scrollThumb = new Sprite();
                _scrollThumb.buttonMode = true;
                var th:Number = Math.max(30, _scrollViewH * _scrollViewH / total);
                _scrollThumb.graphics.beginFill(0x4C6070, 1);
                _scrollThumb.graphics.drawRect(0, 0, 6, th);
                _scrollThumb.graphics.endFill();
                _scrollThumb.x = _scrollBarX;
                _scrollThumb.addEventListener(MouseEvent.MOUSE_DOWN, onThumb, false, 0, true);
                _content.addChild(_scrollThumb);
            }
            else
            {
                _scrollThumb = null;
            }
            applyScroll();
        }

        private function applyScroll():void
        {
            if (_scrollInner == null)
                return;
            var most:Number = Math.max(0, _scrollTotal - _scrollViewH);
            _scrollPx = Math.max(0, Math.min(most, _scrollPx));
            _scrollInner.y = -_scrollPx;
            if (_scrollThumb != null)
            {
                var room:Number = _scrollViewH - _scrollThumb.height;
                _scrollThumb.y = _scrollTop + (most > 0 ? room * _scrollPx / most : 0);
            }
        }

        private function onThumb(e:MouseEvent):void
        {
            if (stage == null || _scrollThumb == null)
                return;
            _scrollGrab = _content.mouseY - _scrollThumb.y;
            stage.addEventListener(MouseEvent.MOUSE_MOVE, onThumbMove);
            stage.addEventListener(MouseEvent.MOUSE_UP, onThumbUp);
        }

        private function onThumbMove(e:MouseEvent):void
        {
            if (_scrollThumb == null)
                return;
            var room:Number = _scrollViewH - _scrollThumb.height;
            var most:Number = Math.max(0, _scrollTotal - _scrollViewH);
            var at:Number = _content.mouseY - _scrollGrab - _scrollTop;
            _scrollPx = room > 0 ? most * at / room : 0;
            applyScroll();
        }

        private function onThumbUp(e:MouseEvent):void
        {
            // The listeners are the stage's: removed from it whatever ours is
            // now (the window may be off the stage meanwhile).
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_MOVE, onThumbMove);
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_UP, onThumbUp);
        }

        private function input(box:Sprite, x:Number, y:Number, value:String):TextField
        {
            var tf:TextField = text(box, value, x, y, 70, 16, GOLD);
            // Its own background and border: an empty field is a solid area
            // to click.
            tf.height = 24;
            tf.background = true;
            tf.backgroundColor = 0x070A0D;
            tf.border = true;
            tf.borderColor = 0x3A4854;
            tf.type = TextFieldType.INPUT;
            tf.restrict = "0-9";
            tf.maxChars = 4;
            tf.selectable = true;
            tf.mouseEnabled = true;
            tf.tabEnabled = true;
            tf.addEventListener(MouseEvent.MOUSE_DOWN, onPress, false, 0, true);
            route(tf, function(control:DisplayObject, e:MouseEvent):void
            {
                FocusManager.setFocus(control as TextField);
            });
            return tf;
        }

        // A control and what a press on it does, (control, event).
        private function route(control:DisplayObject, press:Function):void
        {
            _routes.push({"control": control, "press": press});
        }

        // A game button (CLIK) placed in the content: a press the content
        // got itself clicks it as the game's own press would, sound and all.
        private function routeButton(b:*):void
        {
            route(b, function(control:DisplayObject, e:MouseEvent):void
            {
                var button:* = control;
                if (button.enabled === false)
                    return;
                clickSound();
                control.dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
            });
        }

        // A press in the content: to the last control routed under it, unless
        // Scaleform gave it to that control (or to something inside it)
        // itself, which then has it (no double press).
        private function onContentDown(e:MouseEvent):void
        {
            for (var i:int = _routes.length - 1; i >= 0; i--)
            {
                var r:Object = _routes[i];
                var control:DisplayObject = r.control as DisplayObject;
                if (control.stage == null || !control.visible || !control.hitTestPoint(e.stageX, e.stageY, false))
                    continue;
                var at:DisplayObject = e.target as DisplayObject;
                while (at != null && at != control && at != _content)
                    at = at.parent;
                if (at != control)
                    r.press(control, e);
                return;
            }
        }

        private function onPress(e:MouseEvent):void
        {
            FocusManager.setFocus(e.currentTarget as TextField);
        }

        private function line(box:Sprite, x:Number, y:Number, w:Number):void
        {
            var s:Shape = new Shape();
            s.graphics.beginFill(0x26323C, 1);
            s.graphics.drawRect(x, y, w, 1);
            s.graphics.endFill();
            box.addChild(s);
        }

        private function clear():void
        {
            while (_content.numChildren > 0)
                _content.removeChildAt(0);
            while (_backdrop.numChildren > 0)
                _backdrop.removeChildAt(0);
            _routes = [];
            _stateCells = {};
            _sliders = {};
            _joinPin = null;
            if (_makePin != null)
                _makePinText = _makePin.text;
            _makePin = null;
        }

        private function render():void
        {
            if (!_shown)
                return;
            if (_joinPin != null)
                _pinText = _joinPin.text;
            clear();
            // The title bar: the view's name, then hide and close.
            var title:String = _view == "room" && _room != null ? say("room") + " " + _room.key
                : _view == "create" ? say("new") : say("servers");
            _scrollInner = null;
            _scrollThumb = null;
            // The shop's look: a near black body with a thin edge, and a slim
            // steel header with the title in the middle and two small square
            // buttons at its right.
            var body:Shape = new Shape();
            var bg:* = body.graphics;
            // The inventory's frame and header, by its colours: a 1 px edge,
            // a near-black band with the bold title in its middle, and a
            // darker line of 3 px under it.
            bg.beginFill(0x3A3E44, 1);
            bg.drawRect(0, 0, _w, _h);
            bg.endFill();
            bg.beginFill(0x131517, 0.97);
            bg.drawRect(1, 1, _w - 2, _h - 2);
            bg.endFill();
            var m:Matrix = new Matrix();
            m.createGradientBox(_w, HEAD_H, Math.PI / 2, 0, 0);
            bg.beginGradientFill("linear", [0x0F1114, 0x0B0D10], [1, 1], [0, 255], m);
            bg.drawRect(1, 1, _w - 2, HEAD_H);
            bg.endFill();
            bg.beginFill(0x040507, 1);
            bg.drawRect(1, HEAD_H + 1, _w - 2, 3);
            bg.endFill();
            _backdrop.addChild(body);
            text(_content, title, 0, 10, _w, 15, 0xD5D8DA, "center");
            closeButton(onHide);
            if (_view == "room" && _room != null)
                renderRoom();
            else if (_view == "create")
                renderCreate();
            else
                renderList();
        }

        private function message(y:Number):void
        {
            if (_code != 0)
                text(_content, _code < 0 ? say("pin4") : say("code" + _code), 14, y, _w - 28, 15, 0xFF8A7A);
        }

        // The list, after the design: the table of the rooms on the left, the
        // room chosen on the right with its map's render and JOIN.
        private var _selKey:int = 0;
        // The last row pressed, and when: a second press on it soon joins.
        private var _rowKey:int = 0;
        private var _rowAt:int = 0;
        private var _keys:Boolean = false;
        private static const SIDE_W:Number = 330;

        private function renderList():void
        {
            var tableW:Number = _w - SIDE_W - 42;
            text(_content, say("servers") + ": " + _total, 14, 52, 200, 16, LIGHT);
            message(52);
            if (_code != 0)
                _content.getChildAt(_content.numChildren - 1).x = 220;
            else if (_queueKey != 0)
            {
                // Waiting in a full room's queue: our place, and a way out.
                var queued:String = "";
                for (var qi:int = 0; qi < _rooms.length; qi++)
                    if (_rooms[qi].key == _queueKey)
                        queued = " · " + _rooms[qi].name;
                text(_content, say("inQueue") + ": " + _queuePlace + queued, 220, 52, 360, 16, GOLD);
                button(_content, say("leaveQueue"), 590, 46, 170, onUnqueue, null, 32);
            }
            var cols:Array = [0, 0.29, 0.37, 0.46, 0.74, 0.84];
            var heads:Array = ["name", "level", "max", "map", "players", "state"];
            // The headers at their rows' x: the rows are in the scrolled box
            // (at 14, tableW - 12 wide), the name 10 in.
            for (var h:int = 0; h < heads.length; h++)
                text(_content, say(heads[h]).toUpperCase(), 14 + (h == 0 ? 10 : (tableW - 12) * cols[h]), 84, 160,
                    12, DIM);
            line(_content, 14, 106, tableW - 12);
            var inner:Sprite = scrollBox(14, 108, tableW - 12, _h - FOOT_H - 108 - 8);
            var sel:Object = null;
            if (_rooms.length == 0)
                text(inner, say("none"), 0, 14, tableW - 20, 16, GREY);
            for (var i:int = 0; i < _rooms.length; i++)
            {
                var room:Object = _rooms[i];
                if (_selKey == 0)
                    _selKey = room.key;
                var on:Boolean = room.key == _selKey;
                if (on)
                    sel = room;
                var row:Sprite = new Sprite();
                row.y = i * 42;
                row.graphics.beginFill(on ? 0x3C2C10 : 0x000000, on ? 0.6 : 0.01);
                row.graphics.drawRect(0, 0, tableW - 12, 41);
                row.graphics.endFill();
                if (on)
                {
                    row.graphics.beginFill(GOLD, 1);
                    row.graphics.drawRect(0, 0, 3, 41);
                    row.graphics.endFill();
                }
                row.graphics.beginFill(0x1E2830, 1);
                row.graphics.drawRect(0, 41, tableW - 12, 1);
                row.graphics.endFill();
                var tw:Number = tableW - 12;
                text(row, (room.pin ? "PIN · " : "") + room.name, 10, 10, tw * 0.33, 17, on ? 0xFFFFFF : LIGHT);
                text(row, String(room.level), tw * cols[1], 10, tw * 0.09, 17, LIGHT);
                text(row, levelText(room.max), tw * cols[2], 10, tw * 0.09, 17, LIGHT);
                var e:Object = entry(room.map);
                // No card leads alone yet (a tie, no votes): the start draws one.
                text(row, e != null ? levelName(e.level) + " · " + weatherName(e.weather)
                    : room.phase == 3 ? say("voting") : "", tw * cols[3], 10, tw * 0.27, 15, GREY);
                text(row, room.players + " / " + room.size * 2, tw * cols[4], 10, tw * 0.10, 17, LIGHT);
                _stateCells[String(room.key)] = text(row, phaseText(room), tw * cols[5], 10, tw * 0.16, 15,
                    room.phase == 2 ? GOLD : room.phase == 3 || room.phase == 1 ? 0x8FD18F : GREY);
                row.buttonMode = true;
                row.mouseChildren = false;
                row.name = String(room.key);
                row.addEventListener(MouseEvent.CLICK, onRow, false, 0, true);
                inner.addChild(row);
                // A press the content got itself (as on the left side) picks
                // the row too; only where the list shows it (not scrolled away
                // under its mask).
                route(row, function(control:DisplayObject, e:MouseEvent):void
                {
                    var shown:DisplayObject = (control.parent as Sprite).mask;
                    if (shown != null && !shown.hitTestPoint(e.stageX, e.stageY, false))
                        return;
                    pickRow(int(control.name));
                });
            }
            scrollDone(_rooms.length * 42);
            var joinable:Object = null;
            for (var j:int = 0; j < _rooms.length; j++)
                if (_rooms[j].key == _selKey)
                    joinable = _rooms[j];
            var joinWord:String = joinable != null && joinable.players >= joinable.size * 2 ? say("queue") : say("join");
            footer(joinable != null ? [[say("new"), onNew, null], [joinWord, onJoin, joinable]]
                : [[say("new"), onNew, null]]);
            renderSide(sel, tableW + 28);
        }

        // The room chosen: its next map's render, what it is, and JOIN.
        private function renderSide(room:Object, x:Number):void
        {
            var panel:Shape = new Shape();
            panel.graphics.lineStyle(1, 0x26323C);
            panel.graphics.beginFill(0x080C10, 0.78);
            panel.graphics.drawRect(x, 52, SIDE_W, _h - FOOT_H - 60);
            panel.graphics.endFill();
            _content.addChild(panel);
            if (room == null)
                return;
            var e:Object = entry(room.map);
            if (e != null)
            {
                var pic:Bitmap = render_(e.level, e.weather, SIDE_W, 150);
                pic.x = x;
                pic.y = 52;
                _content.addChild(pic);
            }
            var y:Number = 214;
            text(_content, room.name, x + 16, y, SIDE_W - 32, 20, 0xFFFFFF);
            y += 30;
            if (e != null)
            {
                text(_content, levelName(e.level) + " · " + weatherName(e.weather) + " · " + modeName(e.mode),
                    x + 16, y, SIDE_W - 32, 15, GREY);
                y += 28;
            }
            var facts:Array = [[say("level"), String(room.level)], [say("max"), levelText(room.max)],
                [say("players"), room.players + " / " + room.size * 2], [say("state"), phaseText(room)]];
            for (var i:int = 0; i < facts.length; i++)
            {
                text(_content, facts[i][0], x + 16, y, 140, 15, DIM);
                var cell:TextField = text(_content, facts[i][1], x + 160, y, SIDE_W - 176, 15, LIGHT);
                if (i == facts.length - 1)
                    _stateCells["side"] = cell;
                y += 26;
            }
            if (room.pin)
            {
                y += 6;
                text(_content, say("pin"), x + 16, y + 4, 60, 15, DIM);
                _joinPin = input(_content, x + 160, y, _pinText);
            }
        }

        // The list's times, written into its cells as drawn.
        private function listTimes():void
        {
            for (var i:int = 0; i < _rooms.length; i++)
            {
                var room:Object = _rooms[i];
                var cell:TextField = _stateCells[String(room.key)] as TextField;
                if (cell != null)
                    cell.text = phaseText(room);
                if (room.key == _selKey && _stateCells["side"] != null)
                    TextField(_stateCells["side"]).text = phaseText(room);
            }
        }

        // The click the game's buttons make (ButtonClick: sound_play 0).
        private static function clickSound():void
        {
            gameSound(0);
        }

        private static function gameSound(id:int):void
        {
            try
            {
                ExternalInterface.call("sound_play", id);
            }
            catch (e:Error)
            {
            }
        }

        private function onRow(e:Event):void
        {
            pickRow(int(e.currentTarget.name));
        }

        // A row pressed: chosen; pressed again within 450 ms, joined (as
        // Join, Queue for a full room).
        private function pickRow(key:int):void
        {
            clickSound();
            var now:int = getTimer();
            if (key == _rowKey && now - _rowAt < 450)
            {
                _rowAt = 0;
                joinRoom(chosenRoom());
                return;
            }
            _rowKey = key;
            _rowAt = now;
            _selKey = key;
            render();
        }

        private function chosenRoom():Object
        {
            for (var i:int = 0; i < _rooms.length; i++)
                if (_rooms[i].key == _selKey)
                    return _rooms[i];
            return null;
        }

        // In the list: Enter joins the room chosen, Up and Down choose the
        // one above or below (scrolled into view). Not while something is
        // typed in another text field (a chat message); the PIN's is ours.
        private function onKey(e:KeyboardEvent):void
        {
            try
            {
                if (!_shown || _view != "list" || (e.keyCode != 13 && e.keyCode != 38 && e.keyCode != 40)
                    || resultsUp())
                    return;
                var focus:TextField = stage != null ? stage.focus as TextField : null;
                if (focus != null && focus != _joinPin && focus.text.length > 0)
                    return;
                if (e.keyCode == 13)
                {
                    joinRoom(chosenRoom());
                    return;
                }
                var at:int = -1;
                for (var i:int = 0; i < _rooms.length; i++)
                    if (_rooms[i].key == _selKey)
                        at = i;
                var next:int = Math.max(0, Math.min(_rooms.length - 1, at + (e.keyCode == 40 ? 1 : -1)));
                if (_rooms.length == 0 || next == at)
                    return;
                _selKey = _rooms[next].key;
                // Its row (42 high) in the scrolled box's view.
                var top:Number = next * 42;
                if (top < _scrollPx)
                    _scrollPx = top;
                else if (top + 42 > _scrollPx + _scrollViewH)
                    _scrollPx = top + 42 - _scrollViewH;
                clickSound();
                render();
            }
            catch (err:Error)
            {
            }
        }

        // A level of 0 is any level.
        private function levelText(level:int):String
        {
            return level == 0 ? say("any") : String(level);
        }

        // A listed room's state: waiting (for how many more), the countdown,
        // or its match and how long it has been played.
        private function phaseText(room:Object):String
        {
            if (room.phase == 3)
                return say("voting") + " " + clock(room.secs);
            if (room.phase == 1)
                return say("countdown") + " " + clock(room.secs);
            if (room.phase == 2)
                return say("inMatch") + " " + clock(room.secs);
            return say("waitingShort");
        }

        private function clock(secs:int):String
        {
            if (secs < 0)
                secs = 0;
            var s:int = secs % 60;
            return int(secs / 60) + ":" + (s < 10 ? "0" : "") + s;
        }

        // Making one: size, level, max level, wait, PIN, then the pool's tiles.
        private static const FORM_W:Number = 420;

        private function renderCreate():void
        {
            var y:Number = 50;
            text(_content, say("name").toUpperCase(), 14, y, 100, 12, DIM);
            text(_content, myName(), 110, y - 3, FORM_W - 110, 17, LIGHT);
            y += 30;
            text(_content, say("pin").toUpperCase(), 14, y + 4, 100, 12, DIM);
            _makePin = input(_content, 110, y, _makePinText);
            text(_content, say("pinHint"), 190, y + 4, FORM_W - 190, 14, GREY);
            y += 38;
            slider(y, say("size"), "size");
            y += SLIDER_H;
            slider(y, say("minPlayers"), "min");
            y += SLIDER_H;
            slider(y, say("max"), "max");
            y += SLIDER_H;
            slider(y, say("wait"), "wait");
            y += SLIDER_H + 12;
            // The modes: the two kinds of protective device apart, and team
            // deathmatch; a mode out takes all its entries out of the pool.
            text(_content, say("modes").toUpperCase(), 14, y, 260, 12, DIM);
            y += 22;
            for (var mi:int = 0; mi < MODE_ORDER.length; mi++)
            {
                var mode:int = MODE_ORDER[mi];
                var toggle:Sprite = new Sprite();
                toggle.x = 14;
                toggle.y = y - 14;
                var area:Shape = new Shape();
                area.graphics.beginFill(0x000000, 0.01);
                area.graphics.drawRect(0, 10, FORM_W - 28, 26);
                area.graphics.endFill();
                toggle.addChild(area);
                toggle.addChild(box(_modesOff[String(mode)] != true));
                text(toggle, modeName(mode), 26, 12, FORM_W - 40, 16, LIGHT);
                toggle.buttonMode = true;
                toggle.mouseChildren = false;
                toggle.name = String(mode);
                toggle.addEventListener(MouseEvent.CLICK, onModeToggle, false, 0, true);
                route(toggle, function(control:DisplayObject, e:MouseEvent):void
                {
                    flipMode(control.name);
                });
                _content.addChild(toggle);
                y += 30;
            }
            message(_h - FOOT_H - 30);
            var made:Array = footer([[say("back"), onBack, null], [say("create"), onCreate, null]]);
            // No map chosen: nothing to make, the button is off.
            made[1].enabled = poolChosen();
            renderPool();
        }

        private function myName():String
        {
            try
            {
                return String(PlayerInfo.nickname);
            }
            catch (e:Error)
            {
            }
            return "";
        }

        // The pool: a row a map, its checkbox and name, then a tile a weather
        // with that weather's render; a tile takes all the modes of its map
        // and weather.
        private function renderPool():void
        {
            var x0:Number = FORM_W + 24;
            var w:Number = _w - x0 - 14;
            var frame:Shape = new Shape();
            frame.graphics.lineStyle(1, 0x26323C);
            frame.graphics.beginFill(0x080C10, 0.78);
            frame.graphics.drawRect(x0, 46, w, _h - FOOT_H - 52);
            frame.graphics.endFill();
            _content.addChild(frame);
            var tiles:Array = poolTiles();
            var on:int = 0;
            for (var t:int = 0; t < tiles.length; t++)
                if (tiles[t].on)
                    on++;
            text(_content, say("pool").toUpperCase() + ": " + on + " / " + tiles.length,
                x0 + 16, 58, 260, 16, LIGHT);
            text(_content, say("poolHint"), x0 + 230, 60, w - 430, 14, GREY);
            button(_content, say("all"), x0 + w - 190, 54, 84, onAll, true);
            button(_content, say("nothing"), x0 + w - 100, 54, 84, onAll, false);
            // The maps in the pool's order, each with its weathers.
            var rows:Array = [];
            var at:Object = {};
            for (var i:int = 0; i < tiles.length; i++)
            {
                var tile:Object = tiles[i];
                var row:Object = at[String(tile.level)];
                if (row == null)
                {
                    row = {"level": tile.level, "tiles": []};
                    at[String(tile.level)] = row;
                    rows.push(row);
                }
                row.tiles.push(tile);
            }
            // The map's name on its own line, its weathers' tiles under it,
            // as wide as the pool lets three be.
            var top:Number = 94;
            var areaW:Number = w - 40;
            var cw:int = int((areaW - 4 - 2 * 10) / 3);
            var ch:int = int(cw * 9 / 16);
            var rowH:int = 28 + ch + 14;
            var area:Sprite = scrollBox(x0 + 16, top, areaW, _h - FOOT_H - 6 - top - 8);
            var y:Number = 0;
            for (var r:int = 0; r < rows.length; r++)
            {
                var map:Object = rows[r];
                var any:Boolean = false;
                for (var c:int = 0; c < map.tiles.length; c++)
                    if (map.tiles[c].on)
                        any = true;
                var check:Sprite = new Sprite();
                check.x = 0;
                check.y = y - 12;
                // The whole line takes the click, the gap between the box and
                // the name too.
                var line_hit:Shape = new Shape();
                line_hit.graphics.beginFill(0x000000, 0.01);
                line_hit.graphics.drawRect(0, 10, areaW - 4, 26);
                line_hit.graphics.endFill();
                check.addChild(line_hit);
                check.addChild(box(any));
                text(check, levelName(map.level), 24, 13, areaW - 40, 16, LIGHT);
                check.buttonMode = true;
                check.mouseChildren = false;
                check.name = String(map.level);
                check.addEventListener(MouseEvent.CLICK, onMapRow, false, 0, true);
                area.addChild(check);
                for (var k:int = 0; k < map.tiles.length; k++)
                {
                    var one:Object = map.tiles[k];
                    var chip:Sprite = new Sprite();
                    chip.x = 2 + k * (cw + 10);
                    chip.y = y + 28;
                    var pic:Bitmap = render_(one.level, one.weather, cw, ch);
                    pic.alpha = one.on ? 1 : 0.35;
                    chip.addChild(pic);
                    var edge:Shape = new Shape();
                    edge.graphics.beginFill(0x000000, 0.66);
                    edge.graphics.drawRect(0, ch - 22, cw, 22);
                    edge.graphics.endFill();
                    edge.graphics.lineStyle(2, one.on ? GOLD : 0x26323C);
                    edge.graphics.drawRect(0, 0, cw, ch);
                    chip.addChild(edge);
                    text(chip, weatherName(one.weather).toUpperCase() + (one.extra ? " · " + say("experimental").toUpperCase() : ""),
                        0, ch - 22 + 3, cw, 12, 0xFFFFFF, "center");
                    chip.buttonMode = true;
                    chip.mouseChildren = false;
                    chip.name = one.key;
                    chip.addEventListener(MouseEvent.CLICK, onTile, false, 0, true);
                    area.addChild(chip);
                }
                line(area, 0, y + rowH - 6, areaW - 4);
                y += rowH;
            }
            scrollDone(y);
        }

        // The game's own check box (its bitmaps), drawn when they are missing.
        private function box(on:Boolean):DisplayObject
        {
            try
            {
                var cls:Class = getDefinitionByName(on ? "check_checked" : "check_unchecked") as Class;
                var b:Bitmap = new Bitmap(new cls() as BitmapData);
                b.y = 14;
                return b;
            }
            catch (e:Error)
            {
            }
            var drawn:Shape = new Shape();
            drawn.graphics.lineStyle(1, 0x6D8799);
            drawn.graphics.beginFill(on ? GOLD : 0x0F151B, 1);
            drawn.graphics.drawRect(0, 14, 16, 16);
            drawn.graphics.endFill();
            return drawn;
        }

        private static const MODE_ORDER:Array = [2, 3, 4];
        private var _modesOff:Object = {};

        private function onModeToggle(e:Event):void
        {
            flipMode(e.currentTarget.name);
        }

        private function flipMode(mode:String):void
        {
            clickSound();
            _modesOff[mode] = _modesOff[mode] != true;
            render();
        }

        private function onMapRow(e:Event):void
        {
            clickSound();
            var level:String = e.currentTarget.name;
            var tiles:Array = poolTiles();
            var any:Boolean = false;
            var i:int;
            for (i = 0; i < tiles.length; i++)
                if (String(tiles[i].level) == level && tiles[i].on)
                    any = true;
            for (i = 0; i < tiles.length; i++)
                if (String(tiles[i].level) == level)
                    _off[tiles[i].key] = any;
            render();
        }

        // A slider after the design: the label and the gold value on one
        // line, a thin track gold up to a round gold knob, its range small
        // under it, a line between one slider and the next. Dragged or clicked; while dragged only the
        // slider is redrawn.
        private static const TRACK_X:Number = 14;
        private static const TRACK_W:Number = 406;
        private static const KNOB_R:Number = 8;
        private static const SLIDER_H:Number = 64;
        private var _sliders:Object = {};
        private var _drag:String = null;

        private function slider(y:Number, label:String, what:String):void
        {
            text(_content, label.toUpperCase(), TRACK_X, y + 4, 260, 12, DIM);
            var value:TextField = text(_content, "", TRACK_X + TRACK_W - 160, y - 2, 160, 18, GOLD, "right");
            var box:Sprite = new Sprite();
            box.x = TRACK_X;
            box.y = y + 24;
            box.name = what;
            box.buttonMode = true;
            var hit:Shape = new Shape();
            // Not alpha 0: Scaleform hit-tests no fully clear fill.
            hit.graphics.beginFill(0x000000, 0.01);
            hit.graphics.drawRect(-KNOB_R, -4, TRACK_W + 2 * KNOB_R, 28);
            hit.graphics.endFill();
            box.addChild(hit);
            var bar:Shape = new Shape();
            box.addChild(bar);
            box.addEventListener(MouseEvent.MOUSE_DOWN, onSlide, false, 0, true);
            route(box, function(control:DisplayObject, e:MouseEvent):void
            {
                startSlide(control.name, e.stageX);
            });
            _content.addChild(box);
            // Its range, small and dim right under the track, and a line under
            // the whole: each slider a block of its own.
            var list:Array = stops(what);
            text(_content, stopText(what, int(list[0])), TRACK_X, y + 40, 120, 11, 0x5E6E7A);
            text(_content, stopText(what, int(list[list.length - 1])), TRACK_X + TRACK_W - 120, y + 40, 120, 11,
                0x5E6E7A, "right");
            var rule:Shape = new Shape();
            rule.graphics.beginFill(0x232C34, 1);
            rule.graphics.drawRect(TRACK_X, y + SLIDER_H - 6, TRACK_W, 1);
            rule.graphics.endFill();
            _content.addChild(rule);
            _sliders[what] = {"box": box, "bar": bar, "value": value};
            drawSlider(what);
        }

        private function stopText(what:String, v:int):String
        {
            return what == "wait" ? startText(v) : what == "size" || what == "min" ? v + " v " + v
                : levelText(v);
        }

        // The time to start a wait makes: the vote's own 15 s and the wait
        // (the owner's of 2026-10-19: the vote runs until the match starts,
        // its time added to the wait; 30 s at the least).
        private function startText(wait:int):String
        {
            var all:int = wait + VOTE_SECS;
            if (all < 60)
                return all + " " + say("sec");
            return int(all / 60) + " " + say("min") + (all % 60 != 0 ? " " + (all % 60) + " " + say("sec") : "");
        }

        // A slider's places: the values it stops at, in order.
        private function stops(what:String):Array
        {
            var list:Array = [];
            if (what == "size")
                list = [6, 7, 8, 9, 10];
            else if (what == "min")
                list = [1, 2, 3];
            else if (what == "wait")
                list = WAITS.concat();
            else
            {
                // Any, then the player's own level up to 15: never below it.
                list.push(0);
                for (var m:int = Math.max(1, _gear); m <= 15; m++)
                    list.push(m);
            }
            return list;
        }

        private function valueOf(what:String):int
        {
            if (what == "max" && _max != 0 && _max < _gear)
                _max = 0;
            return what == "size" ? _size : what == "min" ? _minSide : what == "wait" ? _wait : _max;
        }

        private function drawSlider(what:String):void
        {
            var s:Object = _sliders[what];
            if (s == null)
                return;
            var list:Array = stops(what);
            var at:int = Math.max(0, list.indexOf(valueOf(what)));
            var kx:Number = list.length > 1 ? TRACK_W * at / (list.length - 1) : 0;
            var g:* = s.bar.graphics;
            g.clear();
            // Plain rectangles of one height: the gold part on the track
            // exactly (Scaleform's round rectangles come out uneven).
            g.beginFill(0x3A434B, 1);
            g.drawRect(0, 8, TRACK_W, 4);
            g.endFill();
            if (kx > 0)
            {
                g.beginFill(GOLD, 1);
                g.drawRect(0, 8, kx, 4);
                g.endFill();
            }
            g.beginFill(0x0B0E11, 1);
            g.drawCircle(kx, 10, KNOB_R + 2);
            g.endFill();
            g.beginFill(GOLD, 1);
            g.drawCircle(kx, 10, KNOB_R);
            g.endFill();
            var v:int = valueOf(what);
            s.value.text = stopText(what, v);
        }

        private function onSlide(e:MouseEvent):void
        {
            startSlide(e.currentTarget.name, e.stageX);
        }

        private function startSlide(what:String, stageX:Number):void
        {
            clickSound();
            _drag = what;
            slideTo(stageX);
            if (stage == null)
                return;
            stage.addEventListener(MouseEvent.MOUSE_MOVE, onSlideMove);
            stage.addEventListener(MouseEvent.MOUSE_UP, onSlideUp);
        }

        private function onSlideMove(e:MouseEvent):void
        {
            slideTo(e.stageX);
        }

        private function onSlideUp(e:MouseEvent):void
        {
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_MOVE, onSlideMove);
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_UP, onSlideUp);
            _drag = null;
        }

        private function slideTo(stageX:Number):void
        {
            var s:Object = _drag != null ? _sliders[_drag] : null;
            if (s == null)
                return;
            var local:Number = s.box.globalToLocal(new Point(stageX, 0)).x;
            var list:Array = stops(_drag);
            var at:int = Math.round(Math.max(0, Math.min(1, local / TRACK_W)) * (list.length - 1));
            var v:int = int(list[at]);
            if (_drag == "size")
                _size = v;
            else if (_drag == "min")
                _minSide = v;
            else if (_drag == "wait")
                _wait = v;
            else
                _max = v;
            drawSlider(_drag);
        }

        // The pool's entries by map and weather: a tile takes all its modes.
        private function poolTiles():Array
        {
            var tiles:Array = [];
            var at:Object = {};
            for (var i:int = 0; i < _pool.length; i++)
            {
                var e:Object = _pool[i];
                var key:String = e.level + "_" + e.weather;
                var tile:Object = at[key];
                if (tile == null)
                {
                    tile = {"key": key, "level": e.level, "weather": e.weather, "extra": e.extra,
                        "entries": [], "on": _off[key] != true};
                    at[key] = tile;
                    tiles.push(tile);
                }
                tile.entries.push(e);
            }
            return tiles;
        }

        // The create window as the server says it starts (`_saved`, the
        // player's last room made by hand, fitted to the pool and its
        // equipment now): the sliders, the modes, and a tile out unless an
        // entry of it is in. A value out of a slider's range leaves it as it is.
        private var _saved:Object = null;

        private function applySaved():void
        {
            var s:Object = _saved;
            _saved = null;
            if (stops("size").indexOf(s.size) >= 0)
                _size = s.size;
            if (stops("min").indexOf(s.min) >= 0)
                _minSide = s.min;
            if (stops("wait").indexOf(s.wait) >= 0)
                _wait = s.wait;
            if (s.max >= 0 && s.max <= 15)
                _max = s.max;
            _modesOff = {};
            for (var m:int = 0; m < s.modesOff.length; m++)
                _modesOff[String(s.modesOff[m])] = true;
            var on:Object = {};
            var i:int;
            for (i = 0; i < _pool.length; i++)
            {
                var p:Object = _pool[i];
                var chunk:uint = uint(s.bits[int(p.index / 20)]);
                if ((chunk & (1 << (p.index % 20))) != 0)
                    on[p.level + "_" + p.weather] = true;
            }
            _off = {};
            for (i = 0; i < _pool.length; i++)
            {
                var key:String = _pool[i].level + "_" + _pool[i].weather;
                if (on[key] != true)
                    _off[key] = true;
            }
        }


        // A new creator starts with the server's pool, the experimental maps out.
        private function defaultPool():void
        {
            _off = {};
            for (var i:int = 0; i < _pool.length; i++)
                if (_pool[i].extra)
                    _off[_pool[i].level + "_" + _pool[i].weather] = true;
        }

        // The asker's room, after the design: what it is, the banner with the
        // countdown, the vote (always there) and the members, each place of
        // the room a slot.
        private function renderRoom():void
        {
            var room:Object = _room;
            var info:String = say("level") + " " + String(room.level) + "  ·  " + say("max") + " "
                + levelText(room.max) + "  ·  " + room.size + " v " + room.size + "  ·  " + say("pool") + " "
                + room.pool + "  ·  " + say("code") + " " + room.key + (room.pin ? "  ·  PIN" : "");
            text(_content, info, 16, 50, _w - 32, 15, GREY);
            // The banner: the phase and its time.
            var band:Shape = new Shape();
            band.graphics.beginFill(room.phase == 0 ? 0x0A0E12 : 0x3C280C, 0.85);
            band.graphics.drawRect(1, 76, _w - 2, 50);
            band.graphics.endFill();
            _content.addChild(band);
            _bannerLabel = text(_content, "", 16, 90, 300, 15, GREY);
            _banner = text(_content, "", 260, 82, 200, 30, room.phase == 0 ? LIGHT : GOLD);
            _bannerHint = text(_content, "", 440, 91, _w - 456, 15, GREY);
            banner();
            var y:Number = 140;
            text(_content, say("vote").toUpperCase(), 16, y, 400, 16, LIGHT);
            text(_content, say("voteHint"), 360, y + 1, _w - 376, 14, GREY);
            y += 28;
            var cw:int = int((_w - 32 - 2 * 14) / 3);
            var ch:int = Math.min(190, int((_h - y) * 0.48));
            for (var i:int = 0; i < room.cards.length; i++)
            {
                var card:Object = room.cards[i];
                var e:Object = entry(card.map);
                var cell:Sprite = new Sprite();
                cell.x = 16 + i * (cw + 14);
                cell.y = y;
                if (e != null)
                    cell.addChild(render_(e.level, e.weather, cw, ch));
                var shade:Shape = new Shape();
                shade.graphics.beginFill(0x000000, 0.01);
                shade.graphics.drawRect(0, 0, cw, ch);
                shade.graphics.endFill();
                shade.graphics.beginFill(0x000000, 0.62);
                shade.graphics.drawRect(0, ch - 58, cw, 58);
                shade.graphics.endFill();
                var share:Number = room.members.length > 0 ? card.votes / room.members.length : 0;
                shade.graphics.beginFill(0xFFFFFF, 0.14);
                shade.graphics.drawRect(12, ch - 10, cw - 24, 5);
                shade.graphics.endFill();
                shade.graphics.beginFill(GOLD, 1);
                shade.graphics.drawRect(12, ch - 10, (cw - 24) * Math.min(1, share), 5);
                shade.graphics.endFill();
                var chosen:Boolean = room.phase == 1 && card.map == room.next;
                shade.graphics.lineStyle(card.mine || chosen ? 3 : 1, chosen ? 0xFFFFFF : card.mine ? GOLD : 0x26323C);
                shade.graphics.drawRect(0, 0, cw, ch);
                cell.addChild(shade);
                // The vote is shut during the wait: its map marked, the rest dim.
                if (room.phase == 1 && !chosen)
                    cell.alpha = 0.45;
                if (chosen)
                    text(cell, say("nextMap").toUpperCase(), cw - 120, 13, 108, 13, 0xFFFFFF, "right");
                if (e != null)
                {
                    var tag:Shape = new Shape();
                    tag.graphics.lineStyle(1, e.mode == 4 ? 0xB23A2A : 0x5A8FB5);
                    tag.graphics.beginFill(0x000000, 0.7);
                    tag.graphics.drawRect(10, 10, cw * 0.55, 24);
                    tag.graphics.endFill();
                    cell.addChild(tag);
                    text(cell, modeName(e.mode).toUpperCase(), 16, 13, cw * 0.55 - 12, 13, 0xFFFFFF);
                    text(cell, levelName(e.level), 12, ch - 54, cw - 80, 21, 0xFFFFFF);
                    text(cell, weatherName(e.weather).toUpperCase(), 12, ch - 30, cw - 80, 13, 0xCDD8E0);
                }
                text(cell, String(card.votes), cw - 70, ch - 52, 58, 24, GOLD, "right");
                cell.buttonMode = true;
                cell.mouseChildren = false;
                cell.name = String(i);
                cell.addEventListener(MouseEvent.CLICK, onVote, false, 0, true);
                route(cell, function(control:DisplayObject, e:MouseEvent):void
                {
                    clickSound();
                    ask(VOTE + Math.max(0, Math.min(2, int(control.name))), 1);
                });
                _content.addChild(cell);
            }
            y += ch + 16;
            // The players: one list, the sides drawn when the match starts.
            var panel:Shape = new Shape();
            panel.graphics.lineStyle(1, 0x26323C);
            panel.graphics.beginFill(0x080C10, 0.78);
            panel.graphics.drawRect(16, y, _w - 32, _h - FOOT_H - y - 8);
            panel.graphics.endFill();
            _content.addChild(panel);
            text(_content, say("players").toUpperCase(), 28, y + 10, 140, 15, LIGHT);
            text(_content, room.members.length + " / " + room.size * 2 + "   ·   " + say("teams"),
                160, y + 11, _w - 200, 14, GREY);
            line(_content, 17, y + 38, _w - 34);
            var colW:Number = (_w - 32 - 24) / 4;
            // Every place at once (20 at most): the rows share the panel's
            // height, 34 at most, no scrolling.
            var inner:Sprite = new Sprite();
            inner.x = 28;
            inner.y = y + 40;
            _content.addChild(inner);
            var slots:int = room.size * 2;
            var rowH:int = Math.min(34, int((_h - FOOT_H - y - 8 - 46) / Math.ceil(slots / 4)));
            var mid:int = int((rowH - 20) / 2);
            for (var m:int = 0; m < slots; m++)
            {
                var member:Object = m < room.members.length ? room.members[m] : null;
                var mx:Number = (m % 4) * colW;
                var my:Number = int(m / 4) * rowH;
                var under:Shape = new Shape();
                under.graphics.beginFill(0x1A232B, 1);
                under.graphics.drawRect(mx, my + rowH - 1, colW - 10, 1);
                under.graphics.endFill();
                inner.addChild(under);
                if (member == null)
                {
                    text(inner, say("open"), mx + 34, my + mid, colW - 50, 15, 0x4D5C67);
                    continue;
                }
                text(inner, String(member.gear), mx, my + mid + 1, 26, 14, member.fit == 2 ? RED : DIM, "right");
                // A red mark: this kit cannot play the next match as it is
                // now; the match starts without it and takes it out of the room.
                var nameX:Number = mx + 34;
                if (member.fit != 0)
                {
                    var mark:Shape = new Shape();
                    mark.graphics.beginFill(RED, 1);
                    mark.graphics.drawCircle(mx + 40, my + mid + 10, 7);
                    mark.graphics.endFill();
                    inner.addChild(mark);
                    text(inner, "!", mx + 33, my + mid + 1, 14, 12, 0xFFFFFF, "center");
                    nameX = mx + 52;
                }
                text(inner, member.name + (member.owner ? "  (" + say("ownerTag") + ")" : ""), nameX, my + mid, colW - 80 - (nameX - mx - 34), 16,
                    // Away (its game gone for now): grey until it is back, or
                    // out of the room after about 90 s.
                    member.away ? 0x5E6E7A : member.fit != 0 ? RED : member.me ? 0xFFFFFF
                        : (member.inMatch ? GOLD : LIGHT));
                // Only a private room (a PIN) turns a member out.
                if (room.owner && room.pin && !member.me)
                    button(inner, "×", mx + colW - 44, my + int((rowH - 28) / 2), 30, onKick, m, 28);
            }
            // A room of a lower max level that takes us: one click there
            // (the server offers it only while we play no match).
            var buttons:Array = [[say("leave"), onLeave, null]];
            if (room.joinIn >= 0)
                buttons.push([say("joinMatch"), onJoinMatch, null]);
            if (room.switchTo != null)
            {
                var to:Object = room.switchTo;
                buttons.push([say("switchTo"), onSwitch, to]);
                var made:Array = footer(buttons);
                made[made.length - 1].transform.colorTransform = YELLOW_TINT;
                // Two buttons leave room for what the switch is; three do not.
                if (buttons.length < 3)
                {
                // What the switch is, left of the buttons: the room's gear cap,
                // whose it is and how full.
                var room_w:Number = made[0].x - 28;
                var fy:Number = _h - FOOT_H;
                text(_content, say("switchWhy").split("%L").join(String(to.max)), 16, fy + 5, room_w, 13, GOLD);
                text(_content, say("switchWho").split("%N").join(to.name).split("%P").join(to.players + " / " + to.size * 2),
                    16, fy + 24, room_w, 12, GREY);
                }
            }
            else
                footer(buttons);
        }

        private var _banner:TextField;
        private var _bannerLabel:TextField;
        private var _bannerHint:TextField;

        private function myFit():int
        {
            if (_room != null)
                for (var i:int = 0; i < _room.members.length; i++)
                    if (_room.members[i].me)
                        return _room.members[i].fit;
            return 0;
        }

        // The vote's alert: the game's (6, its important message's), not
        // the match's own start (15), the owner's; once a vote, when 10 s
        // or less are left of it (the vote runs until the match starts, the
        // owner's of 2026-10-19), also with the window hidden.
        private var _alerted:Boolean = false;
        private static const ALERT_SECS:int = 10;

        private function alertNear():void
        {
            if (_room == null || _room.phase != 3 || _alerted)
                return;
            var left:int = _room.secs - int((getTimer() - _roomAt) / 1000);
            if (left <= ALERT_SECS)
            {
                _alerted = true;
                gameSound(6);
            }
        }

        // The countdown, kept moving between the server's answers.
        private function banner():void
        {
            if (_room == null || _banner == null || _banner.parent != _content)
                return;
            var passed:int = int((getTimer() - _roomAt) / 1000);
            var fit:int = myFit();
            var unfit:String = fit == 1 ? say("unfitKit") : fit == 2 ? say("unfitGear") : "";
            if (_room.phase == 3)
            {
                _bannerLabel.text = say("voting").toUpperCase();
                _banner.text = clock(_room.secs - passed);
                _bannerHint.text = unfit != "" ? unfit : say("voteNow");
            }
            else if (_room.phase == 1)
            {
                _bannerLabel.text = say("starts").toUpperCase();
                _banner.text = clock(_room.secs - passed);
                var e:Object = entry(_room.next);
                _bannerHint.text = unfit != "" ? unfit : (e != null ? say("nextMap") + ": " + levelName(e.level) + " · "
                    + weatherName(e.weather) + " · " + modeName(e.mode) : "");
            }
            else if (_room.phase == 2)
            {
                _bannerLabel.text = say("playing").toUpperCase();
                _banner.text = clock(_room.secs + passed);
                // Waiting to go into it: the seconds left, counted here; or
                // why it cannot go in now; or its own kit or gear.
                var why:String = "joinWhy" + _room.joinWhy;
                _bannerHint.text = _room.joinIn >= 0
                    ? say("joinIn").split("%S").join(String(Math.max(0, _room.joinIn - passed)))
                    : TEXT[why] != null ? say(why)
                    : unfit != "" ? unfit : say("lateHint");
            }
            else
            {
                _bannerLabel.text = say("waitingShort").toUpperCase();
                _banner.text = _room.members.length + " / " + _room.min;
                _bannerHint.text = say("waiting");
            }
        }

        private function tabLabel():void
        {
            if (_room == null && _queueKey != 0 && !_shown)
            {
                setTab(say("queue") + " " + _queuePlace);
                return;
            }
            if (_room == null || _shown)
            {
                setTab(say("servers"));
                return;
            }
            var left:int = _room.secs - int((getTimer() - _roomAt) / 1000);
            var tail:String = _room.phase == 1 || _room.phase == 3 ? clock(left)
                : _room.phase == 2 ? say("inMatch") : _room.members.length + "/" + _room.size * 2;
            setTab(say("room") + " " + tail);
        }

        // ------------------------------------------------------ the clicks

        private function onHide(e:Event):void
        {
            show(false);
        }

        // LEAVE in the room: the only way out of it (the cross only hides
        // the window).
        private function onLeave(e:Event):void
        {
            if (_view == "room" && _room != null)
                ask(LEAVE, 1);
        }

        // Into the room's match now, without waiting for the countdown.
        private function onJoinMatch(e:Event):void
        {
            if (_steps.length > 0)
                return;
            busy(e);
            ask(JOIN_MATCH, 1);
            _fastUntil = getTimer() + 5000;
        }

        // The room offered of a lower max level: joining it leaves ours.
        private function onSwitch(e:Event):void
        {
            var to:Object = e.currentTarget.data;
            if (to == null || _steps.length > 0)
                return;
            busy(e);
            _code = 0;
            ask(JOIN + to.key, 1);
            _fastUntil = getTimer() + 5000;
        }

        private function onNew(e:Event):void
        {
            _view = "create";
            _code = 0;
            _scrollPx = 0;
            _max = 0;
            _makePinText = "";
            if (_poolTotal < 0)
                ask(POOL, 31);
            // The settings of the player's last room made by hand (the
            // defaults with none): the window starts with them. Not the PIN.
            _saved = null;
            ask(SETTINGS, 1);
            render();
        }

        private function onBack(e:Event):void
        {
            _view = "list";
            _code = 0;
            _scrollPx = 0;
            render();
        }

        private function onUnqueue(e:Event):void
        {
            ask(UNQUEUE, 1);
        }

        private function onJoin(e:Event):void
        {
            var room:Object = e.currentTarget.data;
            if (room == null || _steps.length > 0)
                return;
            busy(e);
            joinRoom(room);
        }

        // Join (or Queue for a full room) as the button does; the PIN typed
        // goes first.
        private function joinRoom(room:Object):void
        {
            if (room == null || _steps.length > 0)
                return;
            _busyAt = getTimer();
            _stepsAt = _busyAt;
            // A full room: wait in its queue (the server joins one a place
            // has freed for meanwhile).
            var start:int = room.players >= room.size * 2 ? QUEUE : JOIN;
            _code = 0;
            _steps = [];
            if (room.pin)
            {
                var typed:String = _joinPin != null ? _joinPin.text : _pinText;
                if (typed.length != 4)
                {
                    pinRefused();
                    return;
                }
                var pin:int = int(typed);
                _steps.push(start + room.key);
                ask(PIN + pin, 1);
            }
            else
            {
                ask(start + room.key, 1);
            }
            _fastUntil = getTimer() + 5000;
        }

        private function onAll(e:Event):void
        {
            var on:Boolean = e.currentTarget.data == true;
            var tiles:Array = poolTiles();
            for (var i:int = 0; i < tiles.length; i++)
                _off[tiles[i].key] = !on;
            render();
        }

        private function onTile(e:Event):void
        {
            clickSound();
            var key:String = e.currentTarget.name;
            _off[key] = _off[key] != true;
            render();
        }

        // The pressed button off until the answer redraws the view (the room,
        // or what went wrong).
        private function busy(e:Event):void
        {
            _busyAt = getTimer();
            _stepsAt = _busyAt;
            try
            {
                e.currentTarget.enabled = false;
            }
            catch (err:Error)
            {
            }
        }

        // Whether the pool as chosen has an entry a create can name.
        private function poolChosen():Boolean
        {
            for (var i:int = 0; i < _pool.length; i++)
            {
                var p:Object = _pool[i];
                if (_off[p.level + "_" + p.weather] != true && _modesOff[String(p.mode)] != true && p.index < 160)
                    return true;
            }
            return false;
        }

        // A PIN of other than 4 digits: said, nothing sent.
        private function pinRefused():void
        {
            _code = -1;
            render();
        }

        private function onCreate(e:Event):void
        {
            // One at a time: its PIN, pool and create are still on their way.
            if (_steps.length > 0)
                return;
            busy(e);
            var typedPin:String = _makePin != null ? _makePin.text : "";
            if (typedPin.length > 0 && typedPin.length != 4)
            {
                pinRefused();
                return;
            }
            _code = 0;
            _steps = [];
            // The pool in pieces of 20 entries, each in by its bit.
            var chunks:Array = [0, 0, 0, 0, 0, 0, 0, 0];
            var any:Boolean = false;
            for (var i:int = 0; i < _pool.length; i++)
            {
                var p:Object = _pool[i];
                if (_off[p.level + "_" + p.weather] == true || _modesOff[String(p.mode)] == true || p.index >= 160)
                    continue;
                chunks[int(p.index / 20)] |= 1 << (p.index % 20);
                any = true;
            }
            var used:int = _pool.length > 0 ? int((_pool.length + 19) / 20) : 1;
            for (var c:int = 0; c < used && c < 8; c++)
                _steps.push(BITS + c * 1048576 + chunks[c]);
            var pinText:String = _makePin != null ? _makePin.text : "";
            var withPin:Boolean = pinText.length > 0;
            var wait:int = WAITS.indexOf(_wait);
            if (wait < 0)
                wait = 3;
            valueOf("max");
            var code:int = ((((_size - 6) * 16 + _max) * 10 + wait) * 2 + (withPin ? 1 : 0)) * 3 + (_minSide - 1);
            _steps.push(CREATE + code);
            if (withPin)
                _steps.unshift(PIN + int(pinText));
            if (!any)
            {
                _steps = [];
                render();
                return;
            }
            _fastUntil = getTimer() + 5000;
            next();
        }

        private function onVote(e:Event):void
        {
            clickSound();
            ask(VOTE + Math.max(0, Math.min(2, int(e.currentTarget.name))), 1);
        }

        private function onKick(e:Event):void
        {
            // The place in the list this state gave, and that state's serial:
            // a kick from an older list does nothing.
            if (_room != null)
                ask(KICK + int(e.currentTarget.data) + 20 * (int(_room.serial) % 495), 1);
        }

        private function onWheel(e:MouseEvent):void
        {
            _scrollPx += e.delta < 0 ? 60 : -60;
            applyScroll();
        }
    }
}
