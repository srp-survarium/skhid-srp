package survarium.rooms
{
    import flash.display.DisplayObject;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.KeyboardEvent;
    import flash.events.MouseEvent;
    import flash.events.TimerEvent;
    import flash.geom.Matrix;
    import flash.display.Shape;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import flash.utils.Timer;
    import flash.utils.getDefinitionByName;
    import flash.utils.getTimer;
    import scaleform.gfx.MouseEventEx;
    import survarium.controls.BottomPanel;
    import survarium.controls.Modals;
    import survarium.controls.PlayerInfo;
    import survarium.player.LocalPlayerProfile;
    import survarium.controls.TopMenu;
    import survarium.constants.JoinHold;
    import survarium.constants.RoomCode;
    import survarium.constants.RoomPhase;
    import survarium.constants.ChatChannel;
    import survarium.constants.ChatTab;
    import survarium.data.LocalizationData;

    // Rooms, the players' "servers" (srp's own; the wire is lobby.md 2.16.3,
    // contract S1, RoomsWire), the only way PvP matches are made. The top
    // menu's tab in place of Map (TopMenu's) opens one window over the
    // lobby, in three views: the list of the rooms (RoomsList), the making
    // of one (RoomsCreate), and the asker's own room with its vote
    // (RoomsRoom); a member sees two tabs, My room (its room) and All rooms
    // (the list, browsed while it stays in its room; the owner's of
    // 2026-10-22). The Play button goes straight into a room (StatusPanel's).
    // The window keeps the state the server answers, polls it, and draws the
    // view in its content (RoomsLook); the game's classes ask it what they
    // show: TopMenu its tab, StatusPanel its Play button, LobbyMenu Esc and
    // the results, BottomPanel makes it and places it.
    public class RoomsWindow extends Sprite
    {
        // The window, made with the bottom bar's Matches tab (BottomPanel).
        public static var me:RoomsWindow;
        // The top menu, whose tab of the map is ours (TopMenu).
        public static var topMenu:TopMenu;

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
        // As the lobby's other menus: ours opens on top, and when one of
        // them opens (equipment, shop, ranking, character, clan, quests,
        // upgrades) ours is hidden; its tab opens it again.
        private static const WINDOWS:Array = ["inventory_list", "shop_list", "ranking", "charWindow",
            "clanWindow", "questWindow", "modifierWindow"];

        internal var look:RoomsLook;
        internal var listView:RoomsList;
        internal var createView:RoomsCreate;
        internal var roomView:RoomsRoom;

        private var _panel:BottomPanel;
        // The lobby, from its first set_play_button_enabled on.
        private var _lobby:LobbyMenu;
        internal var _gear:int = 0;
        internal var _shown:Boolean = false;
        internal var _view:String = "list";
        private var _timer:Timer;
        private var _now:int = 0;
        internal var _askedAt:int = -100000;
        // The last STATE asked, and whether one was ever answered: a game
        // started anew may be in a room the server kept for it, and must
        // learn it (live, 2026-10-20: one played matches it did not know of;
        // the server seats it in none until then).
        private var _stateAskedAt:int = -100000;
        private var _stateKnown:Boolean = false;
        private var _listAt:int = -100000;
        internal var _fastUntil:int = 0;
        internal var _rooms:Array = [];
        internal var _total:int = 0;
        internal var _room:Object = null;
        internal var _roomAt:int = 0;
        internal var _code:int = RoomCode.NONE;
        internal var _pool:Array = [];
        internal var _poolTotal:int = -1;
        // The create window's start (RoomsCreate.formOf): the player's last
        // room made by hand, or the defaults (`S1;F`), fitted by the server
        // to the pool and the gear. Asked as the lobby comes up (the pool
        // in), at each lobby sign-in and when the gear changes, so New room
        // opens filled and nothing is applied after it is drawn (the
        // owner's of 2026-10-25); every answer replaces it (the server
        // answers in order: the last is the newest), and a room made from
        // the window keeps it as the server does. Null until answered: the
        // window opens with the defaults, and stays so while it is up.
        internal var _form:Object = null;
        // Asked for and not answered yet: asked again every 3 s.
        private var _formWanted:Boolean = true;
        private var _formAskedAt:int = -100000;
        // The account whose lobby this is (its name, LobbyMenu.set_account_name).
        private var _account:String = null;
        internal var _steps:Array = [];
        // The game's own Play enabled, as the lobby last set it.
        private var _nativePlay:Boolean = true;
        private var _tabText:String = "";
        private var _poolAskedAt:int = -100000;
        private var _roomSig:String = "";
        // The full room whose queue we wait in, and our place (1 the first);
        // 0: none.
        internal var _queueKey:int = 0;
        internal var _queuePlace:int = 0;
        // Play was pressed and its answer has not come yet: Play stays grey
        // (until then; then while in a room).
        private var _playAskedAt:int = 0;

        // When Play (quickPlay) opened the window (0: it did not, or that is
        // settled): a refusal of the kit (code 15) within PLAY_OPEN_MS closes
        // it again, so a refused Play leaves no rooms view behind
        // (presets.md). A poll's plain answer before Play's own keeps it;
        // a room, another code, the window closed, or a Join or New room in
        // it end it.
        private static const PLAY_OPEN_MS:int = 6000;

        internal var _openedByPlayAt:int = 0;
        // The list as last drawn, but its times: a change redraws it.
        private var _listSig:String = "";
        private var _windowsSeen:Object = {};
        // A state answered while a chain (PIN, pool bits, then its create or
        // join) is on its way: the chain goes on (a poll asked before it may
        // answer meanwhile). One with nothing left is done; the button
        // pressed is given back by the redraw.
        internal var _stepsAt:int = 0;
        internal var _busyAt:int = 0;
        private var _redraw:Boolean = false;
        // The last answer, and (with _askedAt) whether the server still
        // answers.
        private var _answeredAt:int = 0;
        // Enter and the arrows in the list (once on the stage).
        private var _keys:Boolean = false;
        // The members of the room chosen in the list, or of our own (srp's
        // `S1;G`), and the room asked last and when.
        internal var _seen:Object = null;
        private var _membersKey:int = 0;
        private var _membersAt:int = 0;
        // The room whose chat tab is up (ChatTab.ROOM); 0: none.
        private var _chatKey:int = 0;
        // Who waits for the friends' rooms (FriendListRender's menu).
        private var _friendRooms:Function = null;
        // A join (or Play) waiting for its kit preset offer (`S1;O`,
        // presets.md): {room, play, at}; and the offer shown, the popup over
        // the view: {kind, preset, level, gear, max, room, play}.
        internal var _offerAsk:Object = null;
        internal var _offer:Object = null;

        // PlayerRanking.fillPlayers, first: rows of the rooms' range are
        // the window's answers (true: taken).
        public static function take(rows:Array):Boolean
        {
            if (!RoomsWire.owns(rows))
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

        // Made by BottomPanel, under the lobby with the bottom bar.
        public function RoomsWindow(panel:BottomPanel)
        {
            super();
            _panel = panel;
            // The font of the bottom bar's tabs (the game's own), for ours.
            var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
            var sample:* = new tabClass();
            var tabText:TextField = sample["textField"] as TextField;
            var format:TextFormat = tabText != null ? tabText.getTextFormat() : new TextFormat();
            addEventListener(Event.ENTER_FRAME, onFrame, false, 0, true);
            tabLabel();
            name = "srpRoomsWindow";
            visible = false;
            var backdrop:Sprite = new Sprite();
            backdrop.mouseEnabled = false;
            backdrop.mouseChildren = false;
            addChild(backdrop);
            var content:Sprite = new Sprite();
            addChild(content);
            look = new RoomsLook(content, backdrop, format.font, format.bold);
            listView = new RoomsList(this);
            createView = new RoomsCreate(this);
            roomView = new RoomsRoom(this);
            // Presses Scaleform gives the content itself, not the control
            // under them, go to that control (RoomsLook.route).
            content.addEventListener(MouseEvent.MOUSE_DOWN, look.onContentDown);
            // A right-click on a member: its equipment (`onContentClick`).
            content.addEventListener(MouseEvent.CLICK, onContentClick);
            addEventListener(MouseEvent.MOUSE_WHEEL, look.onWheel, false, 0, true);
            if (panel.parent != null)
                panel.parent.addChild(this);
            _timer = new Timer(1000, 0);
            _timer.addEventListener(TimerEvent.TIMER, onTimer, false, 0, true);
            _timer.start();
            // Tells the server this movie has rooms (its Play goes to one),
            // and finds a room the player is in already.
            ask(RoomsWire.STATE, 1);
            // The account signed in before the window was made, if any: a
            // sign-in to another one drops its form (`signedIn`).
            var known:String = PlayerInfo.nickname;
            _account = known != null && known != "" ? known : null;
            layout();
        }

        public function get shown():Boolean
        {
            return _shown;
        }

        // In a room (LobbyMenu.show_victory_screen opens its window).
        public function get inRoom():Boolean
        {
            return _room != null;
        }

        // In a room with a PIN, whose matches take no drop card
        // (LobbyMenu.fill_victory_screen; a PIN is the room's for life).
        public function get inPinRoom():Boolean
        {
            return _room != null && _room.pin;
        }

        // LobbyMenu.set_equipment_level: the player's own level.
        public function setGear(level:int):void
        {
            if (level == _gear)
                return;
            _gear = level;
            // The form's max level is fitted to the gear: the server's again.
            wantForm();
        }

        // LobbyMenu.set_account_name: the sign-in's account data (`8F`, every
        // lobby sign-in; again when the ranked count changes). The form is
        // asked anew; another account's (a sign-in to another account in
        // the same game) is dropped at once: its window opens with the
        // defaults until this one's form is in.
        public function signedIn(account:String):void
        {
            if (_account != null && account != _account)
            {
                _form = null;
                createView.notMade();
            }
            _account = account;
            wantForm();
        }

        // TopMenu.fill_windows: the tabs are made anew; ours named again.
        public function retitle():void
        {
            _tabText = "";
            tabLabel();
        }

        // TopMenu.changeActiveMenu: ours chosen opens the window; another
        // closes it (as the game's own windows do).
        public function tabChosen(ours:Boolean):void
        {
            if (ours != _shown)
                show(ours);
        }

        // LobbyMenu.set_play_button_enabled, after the game's own: in a room
        // or a room's queue Play is our Leave (on); otherwise the game's own.
        public function playEnabled(lobby:LobbyMenu, on:Boolean):void
        {
            _lobby = lobby;
            _nativePlay = on;
            // The lobby is live: the presets' reasons may now come (their
            // fallback counts from here).
            LocalPlayerProfile.lobbyLive();
            applyPlay();
        }

        // RoomsList's empty-list hint: whether Play is lit now (the game's
        // own state and ours).
        internal function playLit():Boolean
        {
            try
            {
                var lobby:LobbyMenu = _lobby != null ? _lobby : parent as LobbyMenu;
                return lobby != null && lobby.statusPanel.playBtn.enabled && !leavesRoom() && !leavesQueue();
            }
            catch (e:Error)
            {
            }
            return false;
        }

        // LocalPlayerProfile, a presets list taken: whether the kits can play
        // may have changed: Play and the list's Join again.
        public function kitsChanged():void
        {
            applyPlay();
            if (_shown && _view == "list" && _offer == null)
                render();
        }

        // StatusPanel's Play in a room ("Leave room"): out of the room.
        public function leaveRoom():void
        {
            _playAskedAt = getTimer();
            applyPlay();
            ask(RoomsWire.LEAVE, 1);
            _fastUntil = getTimer() + 5000;
        }

        // StatusPanel's Play in a room's queue ("Leave queue"): out of it.
        public function leaveQueue():void
        {
            _playAskedAt = getTimer();
            applyPlay();
            ask(RoomsWire.UNQUEUE, 1);
            _fastUntil = getTimer() + 5000;
        }

        // The end of StatusPanel.play_button_clicked: Play may have put us in
        // a room; ask at once and often for a while.
        public function played():void
        {
            _fastUntil = getTimer() + 10000;
            _askedAt = -100000;
            ask(RoomsWire.STATE, 1);
        }

        // StatusPanel.show_match_making: the game's search panel is not shown
        // to a room's member (or right after Play), whom the server sends to
        // its room's match through the game's queue state.
        public function hidesSearch():Boolean
        {
            return _room != null || _playAskedAt != 0 || getTimer() < _fastUntil;
        }

        // The window is a modal (Modals, docs/notes/esc.md) while shown, but
        // not under the results (Esc is theirs then): Esc hides it. Its join
        // popup is a modal over it: Esc is the popup's Exit (no join).
        private var _offerModal:Object = {};

        private function enterModals():void
        {
            Modals.open(this, Modals.LOBBY, function():void
            {
                show(false);
            }, function():Boolean
            {
                return _shown && !resultsUp();
            });
        }

        private function enterOfferModal():void
        {
            Modals.open(_offerModal, Modals.LOBBY, function():void
            {
                if (_offer != null)
                    onOfferExit(null);
            }, function():Boolean
            {
                return _offer != null && _shown && !resultsUp();
            });
        }

        private function setTab(text:String):void
        {
            if (text == _tabText)
                return;
            _tabText = text;
            if (topMenu != null)
                topMenu.setRoomsTab(text);
        }

        private function onFrame(e:Event):void
        {
            // The Clan tab's lock came or went: ours follows.
            if (topMenu != null)
                topMenu.followClanTab(_tabText != "" ? _tabText : say("st_srp_rooms_servers"));
            watchWindows();
            // Our icon and the tab's yellow while we are in a room or queued.
            if (topMenu != null)
                topMenu.paintRoomsTab(marked());
            overPanels();
            roomView.alertNear();
            applyPlay(true);
            syncChatTab();
        }

        // LobbyMenu.onVsRemoved: the results are closing. The top menu's
        // tab, left as it was under them (`show`), follows the window: Rooms
        // for a room it opened in meanwhile (let in from a queue).
        public function resultsClosed():void
        {
            if (topMenu != null)
                topMenu.selectRoomsTab(_shown);
        }

        // LobbyMenu.show_victory_screen: a match is over. The room's chat
        // lines of the match did not reach the lobby (its game was in the
        // match): the tab is put up anew and given the room's last lines.
        public function matchOver():void
        {
            dropChatTab();
        }

        // Each frame: the chat's "Room Chat" tab, right of the general
        // chat's, while we are in a room (the owner's of 2026-10-20: "each
        // room has its own chat", "and you should be able to type in one").
        // Another room's (a switch) is a new tab, with that room's last
        // lines; asked once a room, so a tab the game refuses is not asked
        // for each frame.
        private function syncChatTab():void
        {
            var key:int = _room != null ? int(_room.key) : 0;
            if (key == _chatKey || _lobby == null)
                return;
            dropChatTab();
            if (key == 0)
                return;
            _chatKey = key;
            try
            {
                _lobby.add_tab({"id": ChatTab.ROOM, "name": say("st_srp_rooms_room_chat"), "channels": [ChatChannel.ROOM],
                    "channel_to_send": ChatChannel.ROOM, "closable": false, "save_history": true});
                ask(RoomsWire.CHAT, 1);
            }
            catch (e:Error)
            {
            }
        }

        // A friend's right-click menu (FriendListRender): which friends are
        // members of a room; `answer` gets them, {account, key, players,
        // size, pin} each.
        public function askFriendRooms(answer:Function):void
        {
            _friendRooms = answer;
            // Not `ask`: the state's poll keeps its own pace.
            RoomsWire.send(RoomsWire.FRIEND_ROOMS, 1);
        }

        // `S1;Y;<count>`, then `<account>;<key>;<players>;<size>;<pin>` a
        // friend in a room.
        private function takeFriendRooms(rows:Array):void
        {
            var answer:Function = _friendRooms;
            _friendRooms = null;
            if (answer == null)
                return;
            var rooms:Array = [];
            for (var i:int = 1; i < rows.length; i++)
            {
                var f:Array = RoomsWire.fields(rows[i]);
                if (f.length >= 5)
                    rooms.push({"account": Number(f[0]), "key": int(f[1]), "players": int(f[2]), "size": int(f[3]),
                        "pin": f[4] == "1"});
            }
            answer(rooms);
        }

        // "Join room" in a friend's menu: the window opens on the list with
        // the friend's room chosen, joined as the list's Join does (Queue for
        // a full one); a room with a PIN is chosen and its PIN asked first.
        public function joinFriend(room:Object):void
        {
            if (!_shown)
                show(true);
            // A member sees its room's view: the list is where a join is.
            _view = "list";
            _code = RoomCode.NONE;
            listView.joinFriend(room);
        }

        // The list now (a member's window asks for none of itself).
        internal function askList():void
        {
            _listAt = getTimer();
            ask(RoomsWire.LIST, RoomsWire.LIST_ASK_ROWS);
        }

        private function dropChatTab():void
        {
            if (_chatKey == 0 || _lobby == null)
                return;
            _chatKey = 0;
            try
            {
                _lobby.remove_tab(ChatTab.ROOM);
            }
            catch (e:Error)
            {
            }
        }

        // While open, ours is the lobby's top child but for the chat
        // (LobbyMenu.layerRoomsWindow): the lobby adds its own parts again
        // over it, and their hidden areas then take the clicks meant for our
        // window. The game's windows that open over it close it first (the
        // top menu).
        private function overPanels():void
        {
            if (!_shown || parent == null)
                return;
            var lobby:LobbyMenu = parent as LobbyMenu;
            if (lobby != null)
                lobby.layerRoomsWindow(this);
        }

        // `each`: the frame's call, which leaves the game's own Play enabled as
        // the game sets it (only our Leave is kept on). Grey from a press
        // until its answer (6 s at most).
        internal function applyPlay(each:Boolean = false):void
        {
            // Before the game's first word on Play, the lobby we are in.
            var lobby:LobbyMenu = _lobby != null ? _lobby : parent as LobbyMenu;
            if (lobby == null)
                return;
            try
            {
                var leaving:Boolean = leavesRoom();
                var queued:Boolean = !leaving && leavesQueue();
                var asking:Boolean = _playAskedAt != 0 && getTimer() - _playAskedAt < 6000;
                lobby.statusPanel.showRoomsPlay(leaving, queued, asking, each, _nativePlay,
                    say(leaving ? "st_srp_rooms_leave_room" : "st_srp_rooms_leave_queue"));
            }
            catch (e:Error)
            {
            }
        }

        internal function say(key:String):String
        {
            return LocalizationData.getName(key);
        }

        internal function levelName(level:int):String
        {
            return RoomsText.levelName(level);
        }

        internal function weatherName(weather:int):String
        {
            return RoomsText.weatherName(weather);
        }

        internal function modeName(mode:int):String
        {
            return RoomsText.modeName(mode);
        }

        public function quickPlay():void
        {
            _code = RoomCode.NONE;
            _steps = [];
            createView.notMade();
            _openedByPlayAt = _shown ? 0 : Math.max(1, getTimer());
            if (!_shown)
                show(true);
            _fastUntil = getTimer() + 10000;
            _playAskedAt = getTimer();
            applyPlay();
            // From no room: whether the room Play takes offers a kit preset first.
            if (_room == null && _queueKey == 0)
            {
                _offer = null;
                _offerAsk = {"room": null, "play": true, "at": getTimer()};
                ask(RoomsWire.OFFER, 1);
            }
            else
                ask(RoomsWire.PLAY, 1);
        }

        // The results screen is up (over our window, after a match): Esc and
        // the list's keys are its, not ours.
        internal function resultsUp():Boolean
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

        public function show(value:Boolean):void
        {
            if (!value)
            {
                releaseInputFocus();
                _offer = null;
                _openedByPlayAt = 0;
            }
            _shown = value;
            visible = value;
            if (value)
                enterModals();
            else
            {
                Modals.leave(this);
                Modals.leave(_offerModal);
            }
            // Enter in the list joins the room chosen (once on the stage).
            if (value && !_keys && stage != null)
            {
                _keys = true;
                stage.addEventListener(KeyboardEvent.KEY_DOWN, listView.onKey, false, 0, false);
            }
            // The top menu's tab follows: chosen while the window is open. Not
            // under the results: any change of that tab closes them
            // (LobbyMenu.closeVS), and a window the server opens (let in
            // from a queue) or hides then is not the player's choice. The
            // tab follows as they close (resultsClosed).
            if (topMenu != null && !resultsUp())
                topMenu.selectRoomsTab(value);
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

        public function layout():void
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
                if (w != look._w || h != look._h)
                {
                    look._w = w;
                    look._h = h;
                    render();
                }
                x = int(LEFT * k);
                y = int(_panel.y - BOTTOM_GAP - look._h * k);
            }
            catch (e:Error)
            {
            }
        }

        // In a room, Play is "Leave room", red, and takes us out of it (the
        // owner's of 2026-10-19); but when our row says we play the room's
        // match, yet we are here (the game started anew), it is Play and
        // takes us back to our seat.
        public function leavesRoom():Boolean
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
        public function leavesQueue():Boolean
        {
            return _room == null && _queueKey != 0;
        }

        // The Rooms tab is yellow while we are in a room or a room's queue
        // (the owner's of 2026-10-20).
        public function marked():Boolean
        {
            return _room != null || _queueKey != 0;
        }

        internal function ask(start:int, count:int):void
        {
            _askedAt = getTimer();
            if (start == RoomsWire.STATE)
                _stateAskedAt = _askedAt;
            RoomsWire.send(start, count);
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
            if (_offerAsk != null && _now - _offerAsk.at >= 6000)
                _offerAsk = null;
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
                ask(RoomsWire.POOL, RoomsWire.POOL_ASK_ROWS);
                return;
            }
            // The create window's start, once the pool is in, until answered.
            if (_formWanted && poolIn() && _steps.length == 0 && _now - _formAskedAt >= 3000)
            {
                askForm();
                return;
            }
            if (_steps.length == 0)
            {
                if (_room != null || fast || _queueKey != 0 || (_shown && _view == "room"))
                {
                    if (_now - _askedAt >= (fast ? 1000 : 2000))
                        ask(RoomsWire.STATE, 1);
                    // The All rooms tab: the list too, at its own pace.
                    else if (_room != null && _shown && _view == "list" && _now - _listAt >= 3000)
                        askList();
                }
                else if (_shown && _view == "list" && _now - _listAt >= 3000)
                {
                    _listAt = _now;
                    ask(RoomsWire.LIST, RoomsWire.LIST_ASK_ROWS);
                }
                // Outside a room too: asked again until answered, then every
                // 5 s, so a room the server holds us in always shows.
                else if (_now - _stateAskedAt >= (_stateKnown ? 5000 : 2000))
                    ask(RoomsWire.STATE, 1);
                // The members at their own pace, whatever the state's poll
                // (in a room, queued or not).
                if (_shown && (_view == "list" || _view == "room"))
                    askMembers(false);
            }
            tabLabel();
            if (_shown && _view == "room")
                roomView.banner();
        }

        // Every page of the pool is in.
        private function poolIn():Boolean
        {
            return _poolTotal >= 0 && _pool.length >= _poolTotal;
        }

        // The form is wanted anew: asked now if the pool is in (it names
        // the form's tiles), else once it is.
        private function wantForm():void
        {
            _formWanted = true;
            if (poolIn() && _steps.length == 0)
                askForm();
        }

        private function askForm():void
        {
            _formAskedAt = getTimer();
            ask(RoomsWire.SETTINGS, 1);
        }

        internal function next():void
        {
            if (_steps.length > 0)
            {
                _stepsAt = getTimer();
                ask(int(_steps.shift()), 1);
            }
        }

        private function settle():void
        {
            if (_busyAt != 0 && _steps.length == 0)
            {
                _busyAt = 0;
                _redraw = true;
            }
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

        private function answerRows(rows:Array):void
        {
            if (rows == null || rows.length == 0 || rows[0] == null)
                return;
            var head:Array = RoomsWire.fields(rows[0]);
            if (head[0] != "S1")
                return;
            var kind:String = head[1];
            if (kind == "R" || kind == "N")
                _stateKnown = true;
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
                    var f:Array = RoomsWire.fields(rows[i]);
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
                // A squad's soldier has Join and New room greyed: drawn
                // anew when that changes (squads.md).
                sig += LocalPlayerProfile.squadSoldier() ? ";soldier" : "";
                if (_view == "list")
                {
                    if (sig != _listSig)
                        render();
                    else
                        listView.listTimes();
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
                    var p:Array = RoomsWire.fields(rows[j]);
                    if (p.length < 6 || hasEntry(int(p[0])))
                        continue;
                    _pool.push({"index": int(p[0]), "map": int(p[1]), "level": int(p[2]), "weather": int(p[3]),
                        "mode": int(p[4]), "extra": p[5] == "1"});
                }
                if (_pool.length < _poolTotal && rows.length > 1)
                    ask(RoomsWire.POOL + page + 1, RoomsWire.POOL_ASK_ROWS);
                else
                {
                    // A create window opened before the pool: its tiles as
                    // the defaults have them (none were shown to choose).
                    createView.defaultPool();
                    // The pool names the form's tiles: the form now.
                    if (_formWanted)
                        askForm();
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
                    var fe:Array = RoomsWire.fields(rows[fb]);
                    if (fe[0] == "E" && fe.length > 2)
                        bits[int(fe[1])] = uint(fe[2]);
                }
                // Kept for the next New room; a window up now stays as it
                // opened.
                if (poolIn())
                {
                    _form = createView.formOf(head, bits);
                    _formWanted = false;
                }
            }
            else if (kind == "K" || kind == "B")
            {
                next();
            }
            else if (kind == "O")
                takeOffer(head);
            else if (kind == "G")
                takeMembers(head, rows);
            else if (kind == "Y")
                takeFriendRooms(rows);
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
                if (code == RoomCode.NONE && _code < RoomCode.NONE)
                    code = _code;
                var newCode:Boolean = code != _code;
                _code = code;
                applyPlay();
                // A create refused: no room made, nothing kept.
                if (code > RoomCode.NONE)
                    createView.notMade();
                if (_openedByPlayAt != 0 && code == RoomCode.KIT_CANT_PLAY
                    && getTimer() - _openedByPlayAt < PLAY_OPEN_MS)
                {
                    _openedByPlayAt = 0;
                    _code = RoomCode.NONE;
                    applyPlay();
                    if (_shown)
                        show(false);
                    return;
                }
                if (code != RoomCode.NONE)
                    _openedByPlayAt = 0;
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
                _openedByPlayAt = 0;
                _queueKey = 0;
                _queuePlace = 0;
                var was:Object = _room;
                _room = {"key": int(head[2]), "level": int(head[3]), "max": int(head[4]), "size": int(head[5]),
                    "wait": int(head[6]), "phase": int(head[7]), "secs": int(head[8]), "owner": head[9] == "1",
                    "pool": int(head[10]), "serial": int(head[12]), "pin": head[13] == "1", "next": int(head[14]), "min": int(head[15]),
                    "cards": [], "members": [], "last": 0, "switchTo": null, "joinIn": -1, "joinWhy": JoinHold.NONE, "teams": null};
                var squads:String = "";
                _roomAt = getTimer();
                // A new vote (another room, or after its match): its alert
                // may sound again (`alertNear`).
                if (was == null || was.key != _room.key || was.phase != RoomPhase.VOTE)
                    roomView._alerted = false;
                for (var k:int = 1; k < rows.length; k++)
                {
                    var r:Array = RoomsWire.fields(rows[k]);
                    if (r[0] == "C")
                        _room.cards.push({"map": int(r[2]), "votes": int(r[3]), "mine": r[4] == "1"});
                    else if (r[0] == "P")
                        _room.members.push({"gear": int(r[2]), "owner": r[3] == "1", "me": r[4] == "1",
                            "inMatch": r[5] == "1", "fit": int(r[6]), "away": r[7] == "1", "squad": 0,
                            "name": r.slice(8).join(";")});
                    else if (r[0] == "J")
                    {
                        // `J;<seconds>`, or `J;-1;<why>`: it cannot go in now.
                        _room.joinIn = int(r[1]);
                        _room.joinWhy = r.length > 2 ? int(r[2]) : JoinHold.NONE;
                    }
                    else if (r[0] == "W")
                        _room.switchTo = {"key": int(r[1]), "max": int(r[2]), "players": int(r[3]),
                            "size": int(r[4]), "name": r.slice(5).join(";")};
                    // A private room's teams, a digit a member (its owner's).
                    else if (r[0] == "A")
                        _room.teams = String(r[1]);
                    // Its squads' marks, a character a member (0: none).
                    else if (r[0] == "S")
                        squads = String(r[1]);
                    else if (r[0] == "L")
                        _room.last = int(r[1]);
                }
                // Why a join from the All rooms tab (or a room made) left us
                // here, said once; our own word (a PIN not of 4 digits) stays
                // while we stay.
                RoomsLook.markSquads(_room.members, squads);
                var roomCode:int = head.length > 16 ? int(head[16]) : RoomCode.NONE;
                if (roomCode == RoomCode.NONE && _code < RoomCode.NONE && was != null && was.key == _room.key)
                    roomCode = _code;
                var codeChanged:Boolean = roomCode != _code;
                _code = roomCode;
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
                    // Ours: the room a create on its way made (Back pressed
                    // meanwhile too), whose settings the server now keeps.
                    if (_room.owner)
                        createView.made();
                    _view = "room";
                    if (!_shown)
                        show(true);
                }
                if (_shown && ((_view == "room" && changed) || (_view != "room" && codeChanged)))
                    render();
            }
        }

        // A level of 0 is any level.
        internal function levelText(level:int):String
        {
            return level == RoomsWire.LEVEL_ANY ? say("st_srp_rooms_any") : String(level);
        }

        // A listed room's state: waiting (for how many more), the countdown,
        // or its match and how long it has been played.
        internal function phaseText(room:Object):String
        {
            if (room.phase == RoomPhase.VOTE)
                return say("st_srp_rooms_voting") + " " + RoomsText.clock(room.secs);
            if (room.phase == RoomPhase.COUNTDOWN)
                return say("st_srp_rooms_countdown") + " " + RoomsText.clock(room.secs);
            if (room.phase == RoomPhase.IN_MATCH)
                return say("st_srp_rooms_in_match") + " " + RoomsText.clock(room.secs);
            return say("st_srp_rooms_waiting_short");
        }

        internal function hasEntry(index:int):Boolean
        {
            for (var i:int = 0; i < _pool.length; i++)
                if (_pool[i].index == index)
                    return true;
            return false;
        }

        internal function entry(id:int):Object
        {
            for (var i:int = 0; i < _pool.length; i++)
                if (_pool[i].map == id)
                    return _pool[i];
            return null;
        }

        // The room chosen in the list; in the room's window, our own (its
        // whole names, for a right-click on a member there).
        internal function askMembers(now:Boolean):void
        {
            var room:Object = _view == "room" ? _room : listView.chosenRoom();
            if (room == null)
                return;
            var t:int = getTimer();
            if (!now && room.key == _membersKey && t - _membersAt < 3000)
                return;
            _membersKey = room.key;
            _membersAt = t;
            // Not `ask`: the state's poll keeps its own pace.
            RoomsWire.send(RoomsWire.MEMBERS + room.key, 1);
        }

        // `S1;G;<key>;<members>`, then two rows a member: `P;<i>;<gear>;
        // <owner>;<me>;<in match>;<fit>;<away>;<side>;<account>;<squad>` and
        // its whole name (`squad`: its squad's mark, 0 none). Drawn when it is the chosen room's and something
        // changed (only its box: the view under the mouse keeps its clicks
        // and the PIN typed).
        private function takeMembers(head:Array, rows:Array):void
        {
            var key:int = int(head[2]);
            // Only the room asked last (the list's choice or our own).
            if (key != _membersKey)
                return;
            var list:Array = [];
            var sig:String = "";
            for (var i:int = 1; i + 1 < rows.length; i += 2)
            {
                var f:Array = RoomsWire.fields(rows[i]);
                if (f[0] != "P" || f.length < 10 || rows[i + 1] == null)
                    continue;
                var name:String = String(rows[i + 1].nickname);
                sig += String(rows[i].nickname) + "|" + name + "|";
                list.push({"gear": int(f[2]), "owner": f[3] == "1", "me": f[4] == "1", "inMatch": f[5] == "1",
                    "fit": int(f[6]), "away": f[7] == "1", "side": int(f[8]), "account": uint(f[9]),
                    "squad": f.length > 10 ? int(f[10]) : 0, "name": name});
            }
            if (_seen != null && _seen.key == key && _seen.sig == sig)
                return;
            _seen = {"key": key, "sig": sig, "members": list};
            listView.drawMembers();
        }

        // A removed input can remain Scaleform's native character target even
        // after its AS3 object is gone. Relinquish ownership before removing it.
        private function releaseInputFocus():void
        {
            if (stage != null && stage.focus != null && contains(stage.focus))
                stage.focus = null;
        }

        private function clear():void
        {
            releaseInputFocus();
            while (look._content.numChildren > 0)
                look._content.removeChildAt(0);
            while (look._backdrop.numChildren > 0)
                look._backdrop.removeChildAt(0);
            look._routes = [];
            listView._stateCells = {};
            listView._listHits = [];
            roomView._roomHits = [];
            listView._joinPin = null;
            if (createView._makePin != null)
                createView._makePinText = createView._makePin.text;
            createView._makePin = null;
        }

        internal function render():void
        {
            if (!_shown)
                return;
            var focused:TextField = stage != null ? stage.focus as TextField : null;
            var editingPin:Boolean = focused != null && (focused == listView._joinPin || focused == createView._makePin);
            var selectionStart:int = editingPin ? focused.selectionBeginIndex : 0;
            var selectionEnd:int = editingPin ? focused.selectionEndIndex : 0;
            if (listView._joinPin != null)
                listView._pinText = listView._joinPin.text;
            clear();
            // The title bar: the view's name, then hide and close.
            var title:String = _view == "room" && _room != null ? say("st_srp_rooms_room") + " " + _room.key
                : _view == "create" ? say("st_srp_rooms_new") : say("st_srp_rooms_servers");
            look._scrollInner = null;
            look._scrollThumb = null;
            // The shop's look: a near black body with a thin edge, and a slim
            // steel header with the title in the middle and two small square
            // buttons at its right.
            var body:Shape = new Shape();
            var bg:* = body.graphics;
            // The inventory's frame and header, by its colours: a 1 px edge,
            // a near-black band with the bold title in its middle, and a
            // darker line of 3 px under it.
            bg.beginFill(0x3A3E44, 1);
            bg.drawRect(0, 0, look._w, look._h);
            bg.endFill();
            bg.beginFill(0x131517, 0.97);
            bg.drawRect(1, 1, look._w - 2, look._h - 2);
            bg.endFill();
            var m:Matrix = new Matrix();
            m.createGradientBox(look._w, RoomsLook.HEAD_H, Math.PI / 2, 0, 0);
            bg.beginGradientFill("linear", [0x0F1114, 0x0B0D10], [1, 1], [0, 255], m);
            bg.drawRect(1, 1, look._w - 2, RoomsLook.HEAD_H);
            bg.endFill();
            bg.beginFill(0x040507, 1);
            bg.drawRect(1, RoomsLook.HEAD_H + 1, look._w - 2, 3);
            bg.endFill();
            look._backdrop.addChild(body);
            // A member: the tabs in place of the title.
            if (_room != null)
                drawTabs();
            else
                look.text(look._content, title, 0, 10, look._w, 15, 0xD5D8DA, "center");
            look.closeButton(onHide);
            if (_view == "room" && _room != null)
                roomView.render();
            else if (_view == "create")
                createView.render();
            else
                listView.render();
            if (_offer != null)
                drawOffer();
            // Preserve typing across a redraw only if this view still has a PIN.
            // Joining or hiding the room leaves focus released.
            var pin:TextField = _view == "create" ? createView._makePin : listView._joinPin;
            if (editingPin && pin != null && _offer == null && stage != null)
            {
                stage.focus = pin;
                pin.setSelection(selectionStart, selectionEnd);
            }
        }

        // The header's two tabs while we are in a room (the owner's of
        // 2026-10-22: "you still stay in the room, but you can look at other
        // rooms"): My room, our room's view; All rooms, the list (a room made
        // from it too). Chosen as the list's rows are: a dark gold fill and a
        // gold bar, here along its bottom.
        private function drawTabs():void
        {
            var tabW:Number = 160;
            var x:Number = int((look._w - 2 * tabW) / 2);
            var tabs:Array = [["myRoom", "st_srp_rooms_my_room", _view == "room"],
                ["allRooms", "st_srp_rooms_all_rooms", _view != "room"]];
            for (var i:int = 0; i < tabs.length; i++)
            {
                var on:Boolean = tabs[i][2];
                var tab:Sprite = new Sprite();
                tab.x = x + i * tabW;
                tab.y = 1;
                tab.graphics.beginFill(on ? 0x3C2C10 : 0x000000, on ? 0.6 : 0.01);
                tab.graphics.drawRect(0, 0, tabW, RoomsLook.HEAD_H);
                tab.graphics.endFill();
                if (on)
                {
                    tab.graphics.beginFill(RoomsLook.GOLD, 1);
                    tab.graphics.drawRect(0, RoomsLook.HEAD_H - 3, tabW, 3);
                    tab.graphics.endFill();
                }
                look.text(tab, say(tabs[i][1]), 0, 9, tabW, 15, on ? 0xFFFFFF : RoomsLook.GREY, "center");
                tab.name = tabs[i][0];
                tab.buttonMode = true;
                tab.mouseChildren = false;
                tab.addEventListener(MouseEvent.CLICK, onTab, false, 0, true);
                look._content.addChild(tab);
                look.route(tab, function(control:DisplayObject, e:MouseEvent):void
                {
                    pickTab(control.name);
                });
            }
        }

        private function onTab(e:MouseEvent):void
        {
            var ex:MouseEventEx = e as MouseEventEx;
            if (ex != null && ex.buttonIdx != MouseEventEx.LEFT_BUTTON)
                return;
            pickTab(DisplayObject(e.currentTarget).name);
        }

        private function pickTab(which:String):void
        {
            var view:String = which == "myRoom" ? "room" : "list";
            if (_room == null || view == _view)
                return;
            RoomsLook.clickSound();
            _view = view;
            _code = RoomCode.NONE;
            look._scrollPx = 0;
            if (view == "list")
                askList();
            render();
            askMembers(true);
        }

        // A join (or a full room's queue) from the list: first whether it
        // offers one of our kit presets (presets.md); the server answers
        // `S1;O` (takeOffer).
        internal function askOffer(room:Object):void
        {
            _offer = null;
            _offerAsk = {"room": room, "play": false, "at": getTimer()};
            ask(RoomsWire.OFFER + int(room.key), 1);
        }

        // `S1;O;<key>;<kind>;<preset>;<level>;<gear>;<max>;<about>`: kind 0 joins as
        // asked; 1 (the room allows more than our kits), 2 (our gear is over
        // its max) and 3 (our kits can not play, preset n can) show the popup.
        // `about` 1: Play would queue, the room is of the other bracket
        // (the owner's of 2026-10-30): "Queue" (Play as it is) for the
        // popup's first button. So for a full room's Queue offered a higher
        // loadout (the owner's of 2026-11-01): its queue as it is.
        private function takeOffer(head:Array):void
        {
            var asked:Object = _offerAsk;
            if (asked == null)
                return;
            _offerAsk = null;
            var kind:int = int(head[3]);
            if (kind != 1 && kind != 2 && kind != 3)
            {
                goOn(asked, 0);
                return;
            }
            var full:Boolean = asked.room != null && asked.room.players >= asked.room.size * 2;
            _offer = {"kind": kind, "preset": int(head[4]), "level": int(head[5]), "gear": int(head[6]),
                "max": int(head[7]), "room": asked.room, "play": asked.play,
                "queue": (asked.play && head.length > 8 && head[8] == "1") || (full && kind == 1)};
            _busyAt = 0;
            _playAskedAt = 0;
            applyPlay();
            if (!_shown)
                show(true);
            else
                render();
            enterOfferModal();
        }

        // The join (or Play) asked, with `preset` put on first (0: none).
        private function goOn(asked:Object, preset:int):void
        {
            if (!asked.play)
            {
                listView.joinWith(asked.room, preset);
                return;
            }
            _fastUntil = getTimer() + 10000;
            _playAskedAt = getTimer();
            applyPlay();
            ask(preset > 0 ? RoomsWire.PRESET + (preset - 1) * RoomsWire.PRESET_STEP : RoomsWire.PLAY, 1);
        }

        // The popup over the view: why, then "Enter anyway" (or "Exit") and
        // "Switch and enter". The view under it takes no press meanwhile.
        private function drawOffer():void
        {
            look._routes = [];
            var cover:Sprite = new Sprite();
            cover.graphics.beginFill(0x000000, 0.6);
            cover.graphics.drawRect(0, 0, look._w, look._h);
            cover.graphics.endFill();
            look._content.addChild(cover);
            var w:Number = 560;
            var h:Number = 150;
            var x:Number = int((look._w - w) / 2);
            var y:Number = int((look._h - h) / 2);
            var box:Shape = new Shape();
            box.graphics.beginFill(0x3A3E44, 1);
            box.graphics.drawRect(x, y, w, h);
            box.graphics.endFill();
            box.graphics.beginFill(0x131517, 1);
            box.graphics.drawRect(x + 1, y + 1, w - 2, h - 2);
            box.graphics.endFill();
            look._content.addChild(box);
            var text:String = _offer.kind == 3 ? LocalizationData.format("st_srp_rooms_offer_unfit", _offer.preset, _offer.level)
                : LocalizationData.format(_offer.kind == 1 ? "st_srp_rooms_offer_higher" : "st_srp_rooms_offer_fits",
                    _offer.max, _offer.gear, _offer.preset, _offer.level);
            var tf:TextField = look.text(look._content, "", x + 20, y + 18, w - 40, 15, RoomsLook.LIGHT, "center");
            tf.multiline = true;
            tf.wordWrap = true;
            tf.height = 48;
            tf.text = text;
            var bw:Number = 200;
            var by:Number = y + h - 50;
            // Play that would queue, or a full room's queue: "Queue" goes on
            // as it is; else "Enter anyway" (the kits as they are) or "Exit"
            // (nothing).
            var asIs:Boolean = _offer.queue || _offer.kind == 1;
            var first:String = _offer.queue ? "st_srp_rooms_queue" : _offer.kind == 1 ? "st_srp_rooms_enter_anyway" : "st_srp_rooms_exit";
            look.button(look._content, say(first), x + w / 2 - bw - 8, by, bw,
                asIs ? onOfferAsIs : onOfferExit, null, 32);
            look.button(look._content, say("st_srp_rooms_switch_enter"), x + w / 2 + 8, by, bw, onOfferSwitch, null, 32, true);
        }

        private function onOfferAsIs(e:Event):void
        {
            var offer:Object = _offer;
            _offer = null;
            if (offer != null)
                goOn(offer, 0);
            render();
        }

        private function onOfferSwitch(e:Event):void
        {
            var offer:Object = _offer;
            _offer = null;
            if (offer != null)
                goOn(offer, int(offer.preset));
            render();
        }

        private function onOfferExit(e:Event):void
        {
            _offer = null;
            render();
        }

        internal function message(y:Number):void
        {
            // A kit that can not play: no word (Play and Join are greyed).
            if (_code != RoomCode.NONE && _code != RoomCode.KIT_CANT_PLAY)
                look.text(look._content, _code < RoomCode.NONE ? say("st_srp_rooms_pin_4_digits") : RoomsText.codeText(_code), 14, y, look._w - 28, 15, 0xFF8A7A);
        }

        private function tabLabel():void
        {
            if (_room == null && _queueKey != 0 && !_shown)
            {
                setTab(say("st_srp_rooms_queue") + " " + _queuePlace);
                return;
            }
            if (_room == null || _shown)
            {
                setTab(say("st_srp_rooms_servers"));
                return;
            }
            var left:int = _room.secs - int((getTimer() - _roomAt) / 1000);
            var tail:String = _room.phase == RoomPhase.COUNTDOWN || _room.phase == RoomPhase.VOTE ? RoomsText.clock(left)
                : _room.phase == RoomPhase.IN_MATCH ? say("st_srp_rooms_in_match") : _room.members.length + "/" + _room.size * 2;
            setTab(say("st_srp_rooms_room") + " " + tail);
        }

        internal function onHide(e:Event):void
        {
            show(false);
        }

        // The pressed button off until the answer redraws the view (the room,
        // or what went wrong).
        internal function busy(e:Event):void
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

        // A PIN of other than 4 digits: said, nothing sent.
        internal function pinRefused():void
        {
            _code = RoomCode.PIN_NOT_4_DIGITS;
            render();
        }

        private function onContentClick(e:MouseEvent):void
        {
            try
            {
                var ex:MouseEventEx = e as MouseEventEx;
                if (ex == null || ex.buttonIdx != MouseEventEx.RIGHT_BUTTON)
                    return;
                var hits:Array = _view == "room" ? roomView._roomHits : listView._listHits;
                for (var i:int = hits.length - 1; i >= 0; i--)
                {
                    var hit:Object = hits[i];
                    var control:DisplayObject = hit.control as DisplayObject;
                    if (control.stage == null || !control.hitTestPoint(e.stageX, e.stageY, false))
                        continue;
                    var nick:String = hit.place != null ? roomView.wholeName(int(hit.place), String(hit.name)) : String(hit.name);
                    LobbyMenu.showPlayerInfoMenu(nick, uint(hit.player), uint(hit.how), this);
                    return;
                }
            }
            catch (err:Error)
            {
            }
        }
    }
}
