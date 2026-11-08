package survarium.watch
{
   import flash.display.Bitmap;
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import flash.text.TextFormat;
   import flash.ui.Keyboard;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.FocusManager;
   import survarium.constants.MatchPhase;
   import survarium.controls.BottomPanel;
   import survarium.controls.Modals;
   import survarium.controls.NotificationListScrollContent;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsText;
   import survarium.rooms.RoomsWindow;

   // Watch, the one place to watch matches (research/watch-hub-proposal.md): the bottom bar's Watch
   // tab (BottomPanel) opens it over the lobby in the rooms window's frame and look (RoomsLook),
   // with tabs Live (the matches being played, friends' first; a match's roster to pick whom to
   // watch), Recent (the server's last matches, friends' only on a tick), My matches, and a
   // player's matches (opened from the player pop-up), and a box for a match's number. Every
   // Watch asks through the History's prompt (NotificationListScrollContent.replayBegin), whose
   // answers set the cards' chips too. The lobby server's rows: WatchRows.
   public class WatchWindow extends Sprite
   {
      public static const LIVE_START:uint = 2240000000;

      public static const LIVE_COUNT:int = 20;

      public static const ROSTER_ASK:uint = 0xFFFFFF06;

      public static const ROSTER_START:uint = 2241000000;

      public static const ROSTER_COUNT:int = 24;

      // + 0: the player the asker was shown last (its last `93`), + 1: the asker itself
      public static const MATCHES_START:uint = 2230000000;

      public static const MATCHES_COUNT:int = 10;

      // + 0: all, + 1: only those with a friend of the asker
      public static const RECENT_START:uint = 2250000000;

      public static const RECENT_COUNT:int = 20;

      public static const LIVE:String = "live";

      public static const RECENT:String = "recent";

      public static const MINE:String = "mine";

      public static const PLAYER:String = "player";

      // the lists asked again this often while shown (live, recent); a list not answered yet once a
      // second, at most ASKS times (recent, a player's)
      private static const LIVE_EVERY:int = 5000;

      private static const RECENT_EVERY:int = 30000;

      private static const ASKS:int = 5;

      // a watch answered `watching` this long before the game loads a match: back on its tab after
      private static const RESUME_MS:int = 120000;

      // the lobby's windows that hide ours as they open (the rooms window's list)
      private static const WINDOWS:Array = ["inventory_list","shop_list","ranking","charWindow","clanWindow","questWindow","modifierWindow"];

      private static const TOOL_Y:Number = 50;

      private static const LIST_Y:Number = 92;

      private static const SECTION_H:int = 26;

      public static var me:WatchWindow;

      public var look:RoomsLook;

      private var _panel:BottomPanel;

      private var _shown:Boolean = false;

      private var _tab:String = LIVE;

      // the Player tab's nickname (null: no such tab), and whether it is the asker's own
      private var _player:String = null;

      private var _playerOwn:Boolean = false;

      // each list as last answered, and when (0: no answer yet)
      private var _live:Array = [];

      private var _liveAt:int = 0;

      private var _delay:int = -1;

      private var _recent:Array = [];

      private var _recentAt:int = 0;

      private var _friendsOnly:Boolean = false;

      private var _mine:Array = [];

      private var _mineAt:int = 0;

      private var _played:Array = [];

      private var _playedAt:int = 0;

      // the live match whose roster is shown (its L1 row; null: the list), its players, the place
      // a friend's Spectate looks for
      private var _roster:Object = null;

      private var _players:Array = [];

      private var _focus:int = -1;

      // a friend's Spectate waiting for the live list to say where it plays (lower case), and when
      private var _spectate:String = null;

      private var _spectateAt:int = 0;

      private var _asks:int = 0;

      // the shown tab's list answered since it was asked anew (askNow)
      private var _gotAnswer:Boolean = false;

      private var _askedAt:int = -100000;

      private var _openedAt:int = 0;

      private var _timer:Timer;

      private var _box:TextField;

      private var _boxText:String = "";

      private var _clocks:Array = [];

      // the last watch asked from here (its match and tab), the tab to come back to once it plays,
      // and whether the game is away in it
      private var _pendingMatch:uint = 0;

      private var _pendingTab:String = null;

      private var _resume:String = null;

      private var _resumeAt:int = 0;

      private var _away:Boolean = false;

      private var _windowsSeen:Object = {};

      private var _roomsSeen:Boolean = false;

      public function WatchWindow(panel:BottomPanel)
      {
         super();
         name = "srpWatchWindow";
         visible = false;
         this._panel = panel;
         // the font of the bottom bar's tabs (the game's own), as the rooms window's
         var sample:* = new (getDefinitionByName("ButtonChatTabBottom") as Class)();
         var tabText:TextField = sample["textField"] as TextField;
         var format:TextFormat = tabText != null ? tabText.getTextFormat() : new TextFormat();
         var backdrop:Sprite = new Sprite();
         backdrop.mouseEnabled = false;
         backdrop.mouseChildren = false;
         addChild(backdrop);
         var content:Sprite = new Sprite();
         addChild(content);
         this.look = new RoomsLook(content,backdrop,format.font,format.bold);
         content.addEventListener(MouseEvent.MOUSE_DOWN,this.look.onContentDown);
         addEventListener(MouseEvent.MOUSE_WHEEL,this.look.onWheel,false,0,true);
         addEventListener(Event.ENTER_FRAME,this.onFrame,false,0,true);
         this._timer = new Timer(1000,0);
         this._timer.addEventListener(TimerEvent.TIMER,this.onTimer,false,0,true);
         NotificationListScrollContent.chipListener = this.chipped;
         if(panel.parent != null)
         {
            panel.parent.addChild(this);
         }
         this.layout();
      }

      // The lobby server's rows for the window: taken (true) when they are these rows, whether or
      // not it is shown (PlayerRanking's rows).
      public static function take(rows:Array) : Boolean
      {
         var first:String = null;
         try
         {
            first = rows && rows.length > 0 && rows[0] ? String(rows[0].nickname) : "";
            var tag:String = first.substr(0,3);
            if(tag != "L0;" && tag != "L2;" && tag != "M0;" && tag != "P1;")
            {
               return false;
            }
            if(WatchWindow.me != null)
            {
               WatchWindow.me.answer(tag.substr(0,2),rows);
            }
         }
         catch(e:Error)
         {
         }
         return true;
      }

      // Opens the window on `tab`; PLAYER with the player's nickname.
      public static function open(tab:String, player:String = null) : void
      {
         if(WatchWindow.me != null)
         {
            WatchWindow.me.showTab(tab,player);
         }
      }

      // A friend's Spectate (the friends list): Live, and that friend's match's roster once the
      // list says where it plays.
      public static function spectate(name:String) : void
      {
         if(WatchWindow.me == null)
         {
            return;
         }
         WatchWindow.me._spectate = name.toLowerCase();
         WatchWindow.me._spectateAt = getTimer();
         WatchWindow.me.showTab(LIVE,null);
         WatchWindow.me.findSpectated();
      }

      // The game loads a match (LobbyMenu.set_loading_level): a watch asked from here playing?
      public static function away() : void
      {
         var win:WatchWindow = WatchWindow.me;
         if(win != null && win._resume != null && getTimer() - win._resumeAt < RESUME_MS)
         {
            win._away = true;
         }
      }

      // Back in the lobby (LobbyMenu.show_match_making(false)): from a watch, its tab again.
      public static function back() : void
      {
         var win:WatchWindow = WatchWindow.me;
         if(win == null || !win._away)
         {
            return;
         }
         win._away = false;
         var tab:String = win._resume;
         win._resume = null;
         win.showTab(tab,win._player);
      }

      public function get shown() : Boolean
      {
         return this._shown;
      }

      public function get tab() : String
      {
         return this._tab;
      }

      public function showTab(tab:String, player:String) : void
      {
         if(tab == PLAYER)
         {
            if(player == null || player == "")
            {
               return;
            }
            this._playerOwn = player == PlayerInfo.nickname;
            if(player != this._player)
            {
               this._played = [];
               this._playedAt = 0;
            }
            this._player = player;
         }
         this._tab = tab;
         this._roster = tab == LIVE ? this._roster : null;
         this.look._scrollPx = 0;
         if(!this._shown)
         {
            this.show(true);
         }
         else
         {
            this.toTop();
            this.askNow();
            this.render();
         }
      }

      public function show(value:Boolean) : void
      {
         if(value == this._shown)
         {
            if(value)
            {
               this.toTop();
            }
            return;
         }
         this._shown = value;
         visible = value;
         this._panel.watchShown(value);
         if(!value)
         {
            this.releaseFocus();
            this._timer.stop();
            this._roster = null;
            this._spectate = null;
            Modals.leave(this);
            return;
         }
         if(RoomsWindow.me != null && RoomsWindow.me.shown)
         {
            RoomsWindow.me.show(false);
         }
         Modals.open(this,Modals.LOBBY,function():void
         {
            show(false);
         },function():Boolean
         {
            return _shown;
         });
         this._openedAt = getTimer();
         this.toTop();
         this.layout();
         this.askNow();
         this._timer.reset();
         this._timer.start();
         this.render();
      }

      private function toTop() : void
      {
         if(parent == null)
         {
            return;
         }
         parent.setChildIndex(this,parent.numChildren - 1);
         var lobby:LobbyMenu = parent as LobbyMenu;
         if(lobby != null)
         {
            lobby.layerRoomsWindow(this);
         }
      }

      // The rooms window's frame: the design's 1280 x 720 fitted whole, between the top menu and
      // the bottom bar.
      public function layout() : void
      {
         try
         {
            var k:Number = Math.min(MainScene.stageWidth / RoomsWindow.DESIGN_W,MainScene.stageHeight / RoomsWindow.DESIGN_H);
            scaleX = k;
            scaleY = k;
            var w:int = int(RoomsWindow.DESIGN_W - RoomsWindow.LEFT - RoomsWindow.RIGHT);
            var h:int = int((this._panel.y - RoomsWindow.BOTTOM_GAP) / k - RoomsWindow.TOP);
            if(h < RoomsWindow.MIN_H)
            {
               h = RoomsWindow.MIN_H;
            }
            if(w != this.look._w || h != this.look._h)
            {
               this.look._w = w;
               this.look._h = h;
               this.render();
            }
            x = int(RoomsWindow.LEFT * k);
            y = int(this._panel.y - RoomsWindow.BOTTOM_GAP - this.look._h * k);
         }
         catch(e:Error)
         {
         }
      }

      private function onFrame(e:Event) : void
      {
         // the rooms window or a lobby window opening hides ours
         var rooms:Boolean = RoomsWindow.me != null && RoomsWindow.me.shown;
         if(rooms && !this._roomsSeen && this._shown)
         {
            this.show(false);
         }
         this._roomsSeen = rooms;
         var lobby:LobbyMenu = parent as LobbyMenu;
         if(lobby == null)
         {
            return;
         }
         for each(var key:String in WINDOWS)
         {
            var w:DisplayObject = null;
            try
            {
               w = lobby[key] as DisplayObject;
            }
            catch(err:Error)
            {
            }
            var open:Boolean = w != null && w.visible && w.stage != null;
            if(open && this._windowsSeen[key] != true && this._shown)
            {
               this.show(false);
            }
            this._windowsSeen[key] = open;
         }
      }

      private function call(name:String, a:uint, b:uint) : void
      {
         try
         {
            ExternalInterface.call(name,a,b);
         }
         catch(e:Error)
         {
         }
      }

      // The shown tab's list asked now (a tab chosen, the window opened).
      private function askNow() : void
      {
         this._asks = 0;
         this._gotAnswer = false;
         this.ask();
      }

      private function ask() : void
      {
         this._askedAt = getTimer();
         this._asks++;
         if(this._tab == LIVE)
         {
            if(this._roster != null)
            {
               this.call("request_players_range",ROSTER_START,ROSTER_COUNT);
            }
            this.call("request_players_range",LIVE_START,LIVE_COUNT);
         }
         else if(this._tab == RECENT)
         {
            this.call("request_players_range",RECENT_START + (this._friendsOnly ? 1 : 0),RECENT_COUNT);
         }
         else
         {
            this.call("request_players_range",MATCHES_START + (this._tab == MINE || this._playerOwn ? 1 : 0),MATCHES_COUNT);
         }
      }

      private function onTimer(e:TimerEvent) : void
      {
         var now:int = getTimer();
         var since:int = now - this._askedAt;
         if(this._tab == LIVE)
         {
            var every:int = this._roster != null && this._players.length == 0 ? 1000 : LIVE_EVERY;
            if(since >= every - 100)
            {
               this.ask();
            }
            if(this._liveAt == 0 && now - this._openedAt >= 6000 && this._roster == null)
            {
               this.render();
            }
            if(this._spectate != null && now - this._spectateAt > 15000)
            {
               this._spectate = null;
            }
            this.tick(now);
         }
         else if(!this._gotAnswer && this._asks < ASKS)
         {
            this.ask();
         }
         else if(!this._gotAnswer && this._asks == ASKS && since >= 1000)
         {
            this._asks++;
            this.render();
         }
         else if(this._tab == RECENT && since >= RECENT_EVERY)
         {
            this.ask();
         }
      }

      private function answer(tag:String, rows:Array) : void
      {
         var names:Array = [];
         for each(var r:Object in rows)
         {
            names.push(r ? String(r.nickname) : "");
         }
         if(tag == "L0")
         {
            var head:Array = WatchRows.head(names[0],"L0");
            this._delay = head != null && head[2].length > 0 && WatchRows.count(head[2][0]) >= 0 ? WatchRows.count(head[2][0]) : -1;
            this._live = this.listed(names,WatchRows.live);
            // friends' matches first, each part in the server's order
            var friends:Array = [];
            var others:Array = [];
            for each(var row:Object in this._live)
            {
               (row.friends.length > 0 ? friends : others).push(row);
            }
            this._live = friends.concat(others);
            this._liveAt = getTimer();
            this._gotAnswer = this._gotAnswer || this._tab == LIVE;
            this.findSpectated();
         }
         else if(tag == "L2")
         {
            this.rostered(names);
            return;
         }
         else if(tag == "M0")
         {
            this._recent = this.listed(names,WatchRows.recent);
            this._recentAt = getTimer();
            this._gotAnswer = this._gotAnswer || this._tab == RECENT;
         }
         else
         {
            // P1;<n>;<nickname>: the shown tab's player, or the asker for My matches
            var nick:String = names[0].split(";").slice(2).join(";");
            var list:Array = this.listed(names,WatchRows.played);
            if(this._tab == PLAYER && nick == this._player)
            {
               this._played = list;
               this._playedAt = getTimer();
               this._gotAnswer = true;
            }
            else if(this._tab == MINE && (PlayerInfo.nickname == null || PlayerInfo.nickname == "" || nick == PlayerInfo.nickname))
            {
               this._mine = list;
               this._mineAt = getTimer();
               this._gotAnswer = true;
            }
            else
            {
               return;
            }
         }
         if(this._shown)
         {
            this.render();
         }
      }

      // The page's rows parsed by `parse` (its head and the friends' rows left), each with the
      // friends the page names in its match.
      private function listed(names:Array, parse:Function) : Array
      {
         var list:Array = [];
         var byMatch:Object = {};
         for(var i:int = 1; i < names.length; i++)
         {
            var row:Object = parse(names[i]);
            if(row != null)
            {
               list.push(row);
               byMatch[String(row.match)] = row;
               continue;
            }
            var friend:Object = WatchRows.player(names[i],"F1");
            if(friend != null && byMatch[String(friend.match)] != null)
            {
               byMatch[String(friend.match)].friends.push(friend);
            }
         }
         return list;
      }

      private function rostered(names:Array) : void
      {
         if(this._roster == null)
         {
            return;
         }
         var list:Array = [];
         for each(var name:String in names)
         {
            var player:Object = WatchRows.player(name,"L2");
            if(player != null && player.match == uint(this._roster.match))
            {
               list.push(player);
            }
         }
         if(list.length > 0)
         {
            this._players = list;
            if(this._shown)
            {
               this.render();
            }
         }
      }

      // A friend's Spectate: the roster of the live match the list names it in, the friend marked.
      private function findSpectated() : void
      {
         if(this._spectate == null)
         {
            return;
         }
         for each(var row:Object in this._live)
         {
            for each(var friend:Object in row.friends)
            {
               if(String(friend.name).toLowerCase() == this._spectate)
               {
                  this._spectate = null;
                  this.pick(row,int(friend.place));
                  return;
               }
            }
         }
      }

      private function pick(row:Object, focus:int = -1) : void
      {
         this._roster = row;
         this._players = [];
         this._focus = focus;
         this.call("victory_screen_request_player_data",ROSTER_ASK,uint(row.match));
         this.askNow();
         this.render();
      }

      private function onBack(e:Event) : void
      {
         this._roster = null;
         this._focus = -1;
         this.askNow();
         this.render();
      }

      private function pickTab(which:String) : void
      {
         if(which == this._tab && this._roster == null)
         {
            return;
         }
         RoomsLook.clickSound();
         this._tab = which;
         this._roster = null;
         this.look._scrollPx = 0;
         this.askNow();
         this.render();
      }

      // Asks to watch `match` with `ask` (the History's prompt); the tab to come back to.
      private function watch(match:uint, ask:uint) : void
      {
         var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
         this._pendingMatch = match;
         this._pendingTab = this._tab;
         if(history != null)
         {
            history.replayBegin(match,ask);
         }
      }

      // An answer set a chip (NotificationListScrollContent): `watching` for the match asked from
      // here is the tab to come back to; the cards drawn again.
      private function chipped(hex:String, status:String) : void
      {
         if(status == "watching" && this._pendingTab != null && hex == WatchWindow.hex(this._pendingMatch))
         {
            this._resume = this._pendingTab;
            this._resumeAt = getTimer();
         }
         if(this._shown)
         {
            this.render();
         }
      }

      private static function hex(match:uint) : String
      {
         return NotificationListScrollContent.matchNumberText(match).substr(1);
      }

      private function typed() : void
      {
         var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
         var match:uint = NotificationListScrollContent.matchNumber(this._box.text);
         if(match != 0)
         {
            this._pendingMatch = match;
            this._pendingTab = this._tab;
         }
         if(history != null)
         {
            history.replayTyped(this._box.text);
         }
      }

      private function onClose(e:Event) : void
      {
         this.show(false);
      }

      private function onNumberWatch(e:Event) : void
      {
         this.typed();
      }

      private function onBoxKey(e:KeyboardEvent) : void
      {
         if(e.keyCode == Keyboard.ENTER)
         {
            this.typed();
         }
      }

      private function onFriendsOnly() : void
      {
         this._friendsOnly = !this._friendsOnly;
         this._recent = [];
         this._recentAt = 0;
         this.askNow();
         this.render();
      }

      private function releaseFocus() : void
      {
         if(stage != null && stage.focus != null && contains(stage.focus))
         {
            stage.focus = null;
         }
      }

      // How wide `text` is in the window's font at `size`.
      public static function measure(look:RoomsLook, text:String, size:Number) : Number
      {
         var box:Sprite = new Sprite();
         return look.text(box,text,0,0,2000,size,0).textWidth;
      }

      // A live match's word for its phase, or why it cannot be watched.
      public static function phaseWord(row:Object) : String
      {
         if(int(row.watch) == WatchRows.WATCH_FULL)
         {
            return LocalizationData.getName("st_srp_matches_full");
         }
         if(int(row.watch) == WatchRows.WATCH_PRIVATE)
         {
            return LocalizationData.getName("st_srp_live_private");
         }
         if(int(row.phase) == MatchPhase.GATHERING)
         {
            return LocalizationData.getName("st_srp_matches_waiting");
         }
         if(int(row.phase) == MatchPhase.COUNTDOWN)
         {
            return LocalizationData.getName("st_srp_matches_starting");
         }
         if(int(row.phase) == WatchRows.PHASE_OVER)
         {
            return LocalizationData.getName("st_srp_live_over");
         }
         return LocalizationData.getName("st_srp_watch_live_word");
      }

      // The live card's quiet line: its spectators, and why it cannot be watched when that is so.
      public static function liveNote(row:Object) : String
      {
         var parts:Array = [];
         if(int(row.phase) == MatchPhase.PLAYING && int(row.watch) != WatchRows.WATCH_MAY)
         {
            parts.push(WatchWindow.phaseWord(row));
         }
         if(int(row.spectators) > 0)
         {
            parts.push(LocalizationData.format("st_srp_live_spectators",row.spectators));
         }
         return parts.join("  ·  ");
      }

      public static function ageText(minutes:int) : String
      {
         if(minutes < 60)
         {
            return LocalizationData.format("st_srp_player_matches_minutes",Math.max(1,minutes));
         }
         if(minutes < 48 * 60)
         {
            return LocalizationData.format("st_srp_player_matches_hours",int(minutes / 60));
         }
         return LocalizationData.format("st_srp_player_matches_days",int(minutes / (24 * 60)));
      }

      // A recorded match's chip: what the last answer said, else what its row says.
      private static function chipOf(row:Object) : String
      {
         var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
         var said:String = history != null ? history.chipOf(WatchWindow.hex(uint(row.match))) : null;
         if(said != null || row.kind == "live")
         {
            return said;
         }
         return ["st_srp_replay_chip_not_recorded",null,"st_srp_replay_chip_ready","st_srp_replay_chip_gone","st_srp_replay_chip_old"][int(row.replay)];
      }

      private static function watchable(row:Object, chip:String) : Boolean
      {
         if(row.kind == "live")
         {
            return int(row.watch) == WatchRows.WATCH_MAY;
         }
         if(chip == "st_srp_replay_chip_not_recorded" || chip == "st_srp_replay_chip_gone" || chip == "st_srp_replay_chip_old")
         {
            return false;
         }
         return chip != null || int(row.replay) == WatchRows.REPLAY_KEPT || int(row.replay) == WatchRows.REPLAY_READY;
      }

      // ---- drawing ----

      private function render() : void
      {
         if(!this._shown)
         {
            return;
         }
         var focused:Boolean = stage != null && this._box != null && stage.focus == this._box;
         if(this._box != null)
         {
            this._boxText = this._box.text;
         }
         this.releaseFocus();
         while(this.look._content.numChildren > 0)
         {
            this.look._content.removeChildAt(0);
         }
         while(this.look._backdrop.numChildren > 0)
         {
            this.look._backdrop.removeChildAt(0);
         }
         this.look._routes = [];
         this.look._scrollInner = null;
         this.look._scrollThumb = null;
         this._clocks = [];
         this._rosterPresses = [];
         this.look.frame();
         this.drawTabs();
         this.look.closeButton(this.onClose);
         this.drawNumberBox();
         if(this._tab == LIVE && this._roster != null)
         {
            this.drawRoster();
         }
         else
         {
            this.drawTool();
            this.drawList();
         }
         if(focused && stage != null)
         {
            stage.focus = this._box;
            this._box.setSelection(this._box.length,this._box.length);
         }
      }

      private function drawTabs() : void
      {
         var tabs:Array = [[LIVE,LocalizationData.getName("st_srp_watch_live") + (this._live.length > 0 ? " (" + this._live.length + ")" : "")],
            [RECENT,LocalizationData.getName("st_srp_watch_recent")],[MINE,LocalizationData.getName("st_srp_watch_mine")]];
         if(this._player != null)
         {
            tabs.push([PLAYER,LocalizationData.format("st_srp_watch_player",this._player)]);
         }
         var tabW:Number = Math.min(200,int((this.look._w - 120) / tabs.length));
         var x:Number = int((this.look._w - tabs.length * tabW) / 2);
         for(var i:int = 0; i < tabs.length; i++)
         {
            var tab:Sprite = this.look.headTab("srpWatchTab_" + tabs[i][0],tabs[i][1],x + i * tabW,tabW,tabs[i][0] == this._tab,function(name:String):void
            {
               pickTab(name.substr("srpWatchTab_".length));
            });
            RoomsLook.fitName(tab.getChildAt(0) as TextField,tabs[i][1],"");
         }
      }

      // The match number's box and its Watch, at the toolbar's right: a number as the cards and the
      // History show it, `#` optional; Enter watches too.
      private function drawNumberBox() : void
      {
         var watchLabel:String = LocalizationData.getName("st_srp_replay_watch");
         var bw:Number = Math.max(100,WatchWindow.measure(this.look,watchLabel,14) + 36);
         var right:Number = this.look._w - 14;
         var b:* = this.look.button(this.look._content,watchLabel,right - bw,TOOL_Y,bw,this.onNumberWatch,null,30);
         b.name = "srpWatchNumberWatch";
         var tf:TextField = this.look.text(this.look._content,this._boxText,b.x - 10 - 120,TOOL_Y + 3,120,16,RoomsLook.GOLD);
         tf.name = "srpWatchNumberBox";
         tf.height = 24;
         tf.background = true;
         tf.backgroundColor = 0x070A0D;
         tf.border = true;
         tf.borderColor = 0x3A4854;
         tf.type = TextFieldType.INPUT;
         tf.restrict = "#0-9a-fA-F ";
         tf.maxChars = 10;
         tf.selectable = true;
         tf.mouseEnabled = true;
         tf.tabEnabled = true;
         tf.addEventListener(KeyboardEvent.KEY_DOWN,this.onBoxKey,false,0,true);
         tf.addEventListener(MouseEvent.MOUSE_DOWN,function(e:MouseEvent):void
         {
            FocusManager.setFocus(e.currentTarget as TextField);
         });
         this.look.route(tf,function(control:DisplayObject, e:MouseEvent):void
         {
            FocusManager.setFocus(control as TextField);
         });
         this._box = tf;
         var hint:String = LocalizationData.getName("st_srp_replay_number_hint");
         var hw:Number = WatchWindow.measure(this.look,hint,13) + 6;
         this.look.text(this.look._content,hint,tf.x - 8 - hw,TOOL_Y + 5,hw,13,RoomsLook.GREY).name = "srpWatchNumberHint";
      }

      // The toolbar's left: what the list is (live), the friends-only tick (recent).
      private function drawTool() : void
      {
         var room:Number = this._box.x - 200;
         if(this._tab == LIVE)
         {
            var line:String = LocalizationData.format("st_srp_watch_live_count",this._live.length);
            if(this._delay > 0)
            {
               line += "  ·  " + LocalizationData.format("st_srp_watch_delay",this._delay);
            }
            RoomsLook.fitName(this.look.text(this.look._content,"",14,TOOL_Y + 5,room,14,RoomsLook.LIGHT),line,"");
         }
         else if(this._tab == RECENT)
         {
            var tick:Sprite = new Sprite();
            tick.name = "srpWatchFriendsOnly";
            tick.x = 14;
            tick.y = TOOL_Y - 10;
            var box:DisplayObject = this.look.box(this._friendsOnly);
            tick.addChild(box);
            var label:TextField = this.look.text(tick,LocalizationData.getName("st_srp_watch_friends_only"),box.width + 6,14,room,14,RoomsLook.LIGHT);
            label.width = label.textWidth + 6;
            tick.graphics.beginFill(0,0.01);
            tick.graphics.drawRect(0,10,label.x + label.width,30);
            tick.graphics.endFill();
            tick.buttonMode = true;
            tick.mouseChildren = false;
            tick.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
            {
               onFriendsOnly();
            });
            this.look._content.addChild(tick);
            this.look.route(tick,function(control:DisplayObject, e:MouseEvent):void
            {
               onFriendsOnly();
            });
         }
      }

      private function drawList() : void
      {
         var w:Number = this.look._w - 28 - 10;
         var inner:Sprite = this.look.scrollBox(14,LIST_Y,w,this.look._h - LIST_Y - 10);
         var rows:Array = this._tab == LIVE ? this._live : this._tab == RECENT ? this._recent : this._tab == MINE ? this._mine : this._played;
         var y:Number = 0;
         if(rows.length == 0)
         {
            this.look.text(inner,this.emptyText(),0,14,w,15,RoomsLook.GREY).name = "srpWatchEmpty";
         }
         var friendsFirst:Boolean = this._tab == LIVE && rows.length > 0 && rows[0].friends.length > 0;
         for(var i:int = 0; i < rows.length; i++)
         {
            if(friendsFirst && (i == 0 || rows[i].friends.length == 0 && rows[i - 1].friends.length > 0))
            {
               this.look.text(inner,LocalizationData.getName(i == 0 ? "st_srp_watch_friends_playing" : "st_srp_watch_others").toUpperCase(),
                  4,y + 6,w,12,i == 0 ? RoomsLook.GOLD : RoomsLook.DIM).name = i == 0 ? "srpWatchSectionFriends" : "srpWatchSectionOthers";
               y += SECTION_H;
            }
            var card:MatchCard = this.card(rows[i],w);
            card.name = "srpWatchCard" + i;
            card.y = y;
            inner.addChild(card);
            y += MatchCard.H;
         }
         this.look.scrollDone(y);
      }

      private function emptyText() : String
      {
         var now:int = getTimer();
         var asking:Boolean = this._tab == LIVE ? this._liveAt == 0 : !this._gotAnswer;
         if(asking && (this._tab == LIVE ? now - this._openedAt >= 6000 : this._asks > ASKS))
         {
            return LocalizationData.getName("st_srp_matches_no_answer");
         }
         if(asking)
         {
            return RoomsLook.DOTS;
         }
         if(this._tab == LIVE)
         {
            return LocalizationData.getName("st_srp_matches_none");
         }
         if(this._tab == RECENT)
         {
            return LocalizationData.getName(this._friendsOnly ? "st_srp_watch_none_friends" : "st_srp_watch_none_recent");
         }
         return LocalizationData.getName(this._tab == MINE ? "st_srp_watch_none_mine" : "st_srp_player_matches_none");
      }

      // A row's card, with its buttons: live Watch and Players; recorded Watch (a player's from its
      // place), and My deaths on the asker's own.
      private function card(row:Object, w:Number) : MatchCard
      {
         var chip:String = WatchWindow.chipOf(row);
         var may:Boolean = WatchWindow.watchable(row,chip);
         var match:uint = uint(row.match);
         var actions:Array = [];
         var ask:uint = row.kind == "played" ? WatchRows.watchAsk(int(row.place)) : NotificationListScrollContent.REPLAY_WATCH_ALL;
         actions.push([LocalizationData.getName("st_srp_replay_watch"),"srpWatchWatch",may,function(e:Event):void
         {
            watch(match,ask);
         }]);
         if(row.kind == "live")
         {
            actions.push([LocalizationData.getName("st_srp_live_players"),"srpWatchPlayers",may,function(e:Event):void
            {
               pick(row);
            }]);
         }
         else if(this._tab == MINE)
         {
            actions.push([LocalizationData.getName("st_srp_replay_my_deaths"),"srpWatchDeaths",may,function(e:Event):void
            {
               watch(match,NotificationListScrollContent.REPLAY_WATCH_MY_DEATHS);
            }]);
         }
         var card:MatchCard = new MatchCard(this.look,row,w,chip,actions);
         for each(var b:* in card.presses)
         {
            this.routeInList(b);
         }
         if(card.clock != null)
         {
            this._clocks.push({"field":card.clock,"row":row});
            this.putTime(card.clock,row,getTimer());
         }
         return card;
      }

      // A button in the scrolled list: a press the content got itself clicks it, where the list
      // shows it (not scrolled away under its mask).
      private function routeInList(b:*) : void
      {
         this.look.route(b,function(control:DisplayObject, e:MouseEvent):void
         {
            var inner:Sprite = look._scrollInner;
            if(inner != null && inner.mask != null && !inner.mask.hitTestPoint(e.stageX,e.stageY,false))
            {
               return;
            }
            if(Object(control).enabled === false)
            {
               return;
            }
            RoomsLook.clickSound();
            control.dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
         });
      }

      private function putTime(field:TextField, row:Object, now:int) : void
      {
         var total:int = int(row.played) + int(Math.max(0,now - this._liveAt) / 1000);
         field.text = int(row.phase) == MatchPhase.PLAYING ? RoomsText.clock(total) : "";
      }

      private function tick(now:int) : void
      {
         for each(var clock:Object in this._clocks)
         {
            this.putTime(clock.field as TextField,clock.row,now);
         }
      }

      // A live match's roster: both teams side by side, each player a button that watches from its
      // place; a friend's Spectate marks the friend.
      private function drawRoster() : void
      {
         var row:Object = this._roster;
         var back:* = this.look.button(this.look._content,LocalizationData.getName("st_srp_live_back"),14,TOOL_Y,
            Math.max(100,WatchWindow.measure(this.look,LocalizationData.getName("st_srp_live_back"),14) + 36),this.onBack,null,30);
         back.name = "srpWatchBack";
         RoomsLook.fitName(this.look.text(this.look._content,"",back.x + back.width + 14,TOOL_Y + 5,this._box.x - 200 - back.width,14,RoomsLook.LIGHT),
            LocalizationData.format("st_srp_live_pick",RoomsText.levelName(int(row.map))),"");
         var colW:Number = int((this.look._w - 28 - 20) / 2);
         var thumb:Bitmap = this.look.mapRender(int(row.map),1,colW * 2 + 20,90);
         thumb.x = 14;
         thumb.y = LIST_Y;
         thumb.alpha = 0.5;
         this.look._content.addChild(thumb);
         var rows:Array = [0,0];
         var match:uint = uint(row.match);
         if(this._players.length == 0)
         {
            this.look.text(this.look._content,RoomsLook.DOTS,14,LIST_Y + 100,200,15,RoomsLook.GREY);
         }
         for each(var player:Object in this._players)
         {
            var team:int = int(player.team);
            var b:* = this.look.button(this.look._content,player.name + "   " + player.kills + " / " + player.deaths,14 + team * (colW + 20),
               LIST_Y + 100 + rows[team] * 36,colW,this.watchPlace(match,int(player.place)),null,32);
            b.name = "srpWatchPlayer" + player.place;
            rows[team]++;
            if(int(player.place) == this._focus)
            {
               RoomsLook.skinTint(b,RoomsLook.YELLOW_TINT);
               b.addEventListener(MouseEvent.ROLL_OUT,function(e:Event):void
               {
                  RoomsLook.skinTint(e.currentTarget,RoomsLook.YELLOW_TINT);
               });
            }
         }
      }

      // A roster button's press: watch `match` from `place`. Held strongly: the button's listener
      // is weak.
      private var _rosterPresses:Array = [];

      private function watchPlace(match:uint, place:int) : Function
      {
         var press:Function = function(e:Event):void
         {
            watch(match,WatchRows.watchAsk(place));
         };
         this._rosterPresses.push(press);
         return press;
      }
   }
}
