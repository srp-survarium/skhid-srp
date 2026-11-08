package survarium.watch
{
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.system.System;
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
   import survarium.controls.TopMenu;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsText;
   import survarium.rooms.RoomsWindow;

   // Watch, the one place to watch matches (research/watch-redesign.html): the top menu's Watch tab
   // (TopMenu, between Rooms and Ranking, with the live matches' count) opens it over the lobby in
   // the rooms window's frame and look (RoomsLook); opening one closes the other. Two tabs: Live
   // (the matches being played, friends' first; chips Everyone, Friends) and Replays (chips Mine,
   // Friends, Everyone), and a search box: a match's code watches it, a player's name lists that
   // player's matches. Each card has one Watch, or why it cannot be watched. Every watch asks
   // through the History's prompt (NotificationListScrollContent.replayBegin). The lobby server's
   // rows: WatchRows.
   public class WatchWindow extends Sprite
   {
      public static const LIVE_START:uint = 2240000000;

      public static const LIVE_COUNT:int = 20;

      // + 0: the player whose matches the asker chose last (its last `93`, or SEARCH_ASK's), + 1:
      // the asker itself
      public static const MATCHES_START:uint = 2230000000;

      public static const MATCHES_COUNT:int = 10;

      // + 0: all, + 1: only those with a friend of the asker
      public static const RECENT_START:uint = 2250000000;

      public static const RECENT_COUNT:int = 20;

      // `3D 00 <SEARCH_ASK> <account>` (the lobby server's PLAYER_MATCHES_ASK): the account whose
      // matches the next MATCHES_START lists, the one a search by name found (find_players)
      public static const SEARCH_ASK:uint = 0xFFFFFF07;

      public static const LIVE:String = "live";

      public static const REPLAYS:String = "replays";

      // the chips (Live: EVERYONE, FRIENDS; Replays: MINE, FRIENDS, EVERYONE), and the lists by
      // what they hold: these, LIVE and PLAYER (a searched player's matches)
      public static const EVERYONE:String = "everyone";

      public static const FRIENDS:String = "friends";

      public static const MINE:String = "mine";

      public static const PLAYER:String = "player";

      // a search by name: find_players asked, its account's matches asked, listed, no such
      // player, no answer
      private static const FINDING:String = "finding";

      private static const ASKED:String = "asked";

      private static const LISTED:String = "listed";

      private static const NOT_FOUND:String = "not_found";

      private static const NO_ANSWER:String = "no_answer";

      // the shown list asked again this often (live, recent); one not answered yet once a second,
      // at most ASKS times; the live count (the top menu's badge) while Live is not shown, outside
      // matches
      private static const LIVE_EVERY:int = 5000;

      private static const RECENT_EVERY:int = 30000;

      private static const BADGE_EVERY:int = 30000;

      private static const ASKS:int = 5;

      // the first asks after the lobby comes up (the badge, the asker's matches for History's
      // rows), and after a match (the asker's matches again)
      private static const FIRST_ASK_MS:int = 3000;

      private static const BACK_ASK_MS:int = 2000;

      // a watch answered `watching` this long before the game loads a match: back on its view after
      private static const RESUME_MS:int = 120000;

      // a friend's Spectate waits this long for its match to be listed in play
      private static const SPECTATE_MS:int = 60000;

      // a copy's note this long
      private static const COPIED_MS:int = 4000;

      // the lobby's windows that hide ours as they open (the rooms window's list)
      private static const WINDOWS:Array = ["inventory_list","shop_list","ranking","charWindow","clanWindow","questWindow","modifierWindow"];

      private static const TOOL_Y:Number = 50;

      private static const LIST_Y:Number = 92;

      private static const SEARCH_W:Number = 300;

      private static const CHIP_H:Number = 28;

      private static const RED:uint = 0xE0483E;

      public static var me:WatchWindow;

      // The top menu, whose Watch tab is ours (TopMenu).
      public static var topMenu:TopMenu;

      // The game's clipboard: the copy icon's (a harness may stand in for it).
      public static var clipboard:Function = function(text:String):void
      {
         System.setClipboard(text);
      };

      public var look:RoomsLook;

      private var _panel:BottomPanel;

      private var _shown:Boolean = false;

      private var _tab:String = LIVE;

      private var _liveChip:String = EVERYONE;

      private var _replaysChip:String = MINE;

      // the search by name ({name, state, at, asks, id}), null none
      private var _search:Object = null;

      // each list ({rows, at}: at 0 not answered yet) by its name (EVERYONE..., LIVE, PLAYER)
      private var _lists:Object = {};

      // when each list was asked last
      private var _askedAt:Object = {};

      // the recent list (FRIENDS, EVERYONE) and the player's (MINE, PLAYER) asked last: the one an
      // M0 or P1 answer is
      private var _recentAsked:String = EVERYONE;

      private var _playedAsked:String = MINE;

      private var _delay:int = -1;

      // a friend's Spectate waiting for the live list to say where it plays (lower case), and when
      private var _spectate:String = null;

      private var _spectateAt:int = 0;

      private var _asks:int = 0;

      // the shown list answered since it was asked anew (askNow)
      private var _gotAnswer:Boolean = false;

      private var _openedAt:int = 0;

      private var _mineDue:int = 0;

      private var _timer:Timer;

      private var _box:TextField;

      private var _boxText:String = "";

      private var _clocks:Array = [];

      // the copy icon's last press ({hex, ok, at}), and the code's field it showed (the clipboard
      // refused)
      private var _copied:Object = null;

      private var _codeField:TextField;

      // the last watch asked from here (its match and view), the view to come back to once it
      // plays, and whether the game is away in it
      private var _pendingMatch:uint = 0;

      private var _pendingView:Object = null;

      private var _resume:Object = null;

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
         NotificationListScrollContent.answerListener = this.answered;
         var now:int = getTimer();
         this._askedAt[LIVE] = now - BADGE_EVERY + FIRST_ASK_MS;
         this._mineDue = now + FIRST_ASK_MS;
         this._timer = new Timer(1000,0);
         this._timer.addEventListener(TimerEvent.TIMER,this.onTimer,false,0,true);
         this._timer.start();
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
            if(tag != "L0;" && tag != "M0;" && tag != "P1;")
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

      // Opens the window on `tab` (LIVE, REPLAYS).
      public static function open(tab:String) : void
      {
         if(WatchWindow.me != null)
         {
            WatchWindow.me.showTab(tab);
         }
      }

      // The player pop-up's Matches: Replays, the player's name searched.
      public static function openPlayer(name:String) : void
      {
         if(WatchWindow.me != null && name != null && name != "")
         {
            WatchWindow.me.showTab(REPLAYS);
            WatchWindow.me.searchPlayer(name);
         }
      }

      // A friend's Spectate (the friends list): Live, its friends' matches, and that friend's match
      // watched from the friend's place once the list says where it plays.
      public static function spectate(name:String) : void
      {
         var win:WatchWindow = WatchWindow.me;
         if(win == null)
         {
            return;
         }
         win._spectate = name.toLowerCase();
         win._spectateAt = getTimer();
         win._liveChip = FRIENDS;
         win.showTab(LIVE);
         win.findSpectated();
      }

      // LobbyMenu.fill_players_search: the players a search by name found, when it asked (true).
      public static function found(players:Array) : Boolean
      {
         var win:WatchWindow = WatchWindow.me;
         if(win == null || win._search == null || win._search.state != FINDING)
         {
            return false;
         }
         win.foundPlayers(players != null ? players : []);
         return true;
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

      // Back in the lobby (LobbyMenu.show_match_making(false)): the asker's matches asked again
      // (History's rows of the match just played); from a watch, its view again.
      public static function back() : void
      {
         var win:WatchWindow = WatchWindow.me;
         if(win == null)
         {
            return;
         }
         win._mineDue = getTimer() + BACK_ASK_MS;
         if(!win._away)
         {
            return;
         }
         win._away = false;
         var view:Object = win._resume;
         win._resume = null;
         win._liveChip = view.liveChip;
         win._replaysChip = view.replaysChip;
         win._search = view.search;
         win.setBox(view.search != null ? view.search.name : "");
         if(win._search != null)
         {
            win.searchPlayer(win._search.name);
         }
         win.showTab(view.tab);
      }

      public function get shown() : Boolean
      {
         return this._shown;
      }

      public function get tab() : String
      {
         return this._tab;
      }

      // The chip chosen on the shown tab; PLAYER while a searched player's matches are shown.
      public function get chip() : String
      {
         if(this._tab == LIVE)
         {
            return this._liveChip;
         }
         return this._search != null ? PLAYER : this._replaysChip;
      }

      // The matches being played, as last listed (the top menu's badge).
      public function get liveCount() : int
      {
         return this.rowsOf(LIVE).length;
      }

      public function showTab(tab:String) : void
      {
         this._tab = tab;
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
         if(topMenu != null)
         {
            topMenu.selectWatchTab(value);
         }
         if(!value)
         {
            this.releaseFocus();
            this._spectate = null;
            this._copied = null;
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
         this.render();
      }

      // TopMenu: its Watch tab chosen (open) or another (closed).
      public function tabChosen(ours:Boolean) : void
      {
         if(ours != this._shown)
         {
            this.show(ours);
         }
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
         if(topMenu != null)
         {
            topMenu.paintWatchTab(this.liveCount);
         }
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

      // The list the window shows now.
      private function shownList() : String
      {
         if(this._tab == LIVE)
         {
            return LIVE;
         }
         return this._search != null ? PLAYER : this._replaysChip;
      }

      private function rowsOf(list:String) : Array
      {
         return this._lists[list] != null ? this._lists[list].rows : [];
      }

      private function answeredAt(list:String) : int
      {
         return this._lists[list] != null ? int(this._lists[list].at) : 0;
      }

      // The shown list asked now (a tab or chip chosen, the window opened).
      private function askNow() : void
      {
         this._asks = 1;
         this._gotAnswer = false;
         var list:String = this.shownList();
         if(list == PLAYER)
         {
            if(this._search.state == ASKED || this._search.state == LISTED)
            {
               this.askPlayer();
            }
            return;
         }
         this.ask(list);
      }

      private function ask(list:String) : void
      {
         this._askedAt[list] = getTimer();
         if(list == LIVE)
         {
            this.call("request_players_range",LIVE_START,LIVE_COUNT);
         }
         else if(list == MINE || list == PLAYER)
         {
            this._playedAsked = list;
            this.call("request_players_range",MATCHES_START + (list == MINE ? 1 : 0),MATCHES_COUNT);
         }
         else
         {
            this._recentAsked = list;
            this.call("request_players_range",RECENT_START + (list == FRIENDS ? 1 : 0),RECENT_COUNT);
         }
      }

      private function since(list:String, now:int) : int
      {
         return this._askedAt[list] != null ? now - int(this._askedAt[list]) : int.MAX_VALUE;
      }

      private function onTimer(e:TimerEvent) : void
      {
         var now:int = getTimer();
         // nothing asked in a match (the lobby is not shown then)
         var free:Boolean = !MainScene.activeMatchMaking;
         if(free && this._mineDue != 0 && now >= this._mineDue)
         {
            this._mineDue = 0;
            this.ask(MINE);
         }
         var liveEvery:int = this._shown && this._tab == LIVE ? LIVE_EVERY : BADGE_EVERY;
         if(free && this.since(LIVE,now) >= liveEvery - 100)
         {
            this.ask(LIVE);
         }
         if(this._copied != null && now - int(this._copied.at) >= COPIED_MS)
         {
            this._copied = null;
            this.render();
         }
         if(!this._shown)
         {
            return;
         }
         var list:String = this.shownList();
         if(list == LIVE)
         {
            if(this.answeredAt(LIVE) == 0 && now - this._openedAt >= 6000 && now - this._openedAt < 7000)
            {
               this.render();
            }
            if(this._spectate != null && now - this._spectateAt > SPECTATE_MS)
            {
               this._spectate = null;
            }
            this.tick(now);
         }
         else if(list == PLAYER)
         {
            this.searchTick(now);
         }
         else if(!this._gotAnswer && this._asks < ASKS)
         {
            this._asks++;
            this.ask(list);
         }
         else if(!this._gotAnswer && this._asks == ASKS && this.since(list,now) >= 1000)
         {
            this._asks++;
            this.render();
         }
         else if(list != MINE && this.since(list,now) >= RECENT_EVERY)
         {
            this.ask(list);
         }
      }

      private function answer(tag:String, rows:Array) : void
      {
         var names:Array = [];
         for each(var r:Object in rows)
         {
            names.push(r ? String(r.nickname) : "");
         }
         var list:String = null;
         if(tag == "L0")
         {
            var head:Array = WatchRows.head(names[0],"L0");
            this._delay = head != null && head[2].length > 0 && WatchRows.count(head[2][0]) >= 0 ? WatchRows.count(head[2][0]) : -1;
            var live:Array = this.listed(names,WatchRows.live);
            // friends' matches first, each part in the server's order
            var friends:Array = [];
            var others:Array = [];
            for each(var row:Object in live)
            {
               (row.friends.length > 0 ? friends : others).push(row);
            }
            list = LIVE;
            this._lists[LIVE] = {"rows":friends.concat(others),"at":getTimer()};
         }
         else if(tag == "M0")
         {
            list = this._recentAsked;
            this._lists[list] = {"rows":this.listed(names,WatchRows.recent),"at":getTimer()};
         }
         else
         {
            // P1;<n>;<nickname>: the searched player's, or the asker's own (My matches)
            var nick:String = names[0].split(";").slice(2).join(";");
            var own:String = PlayerInfo.nickname;
            if(this._search != null && (this._search.state == ASKED || this._search.state == LISTED)
               && nick.toLowerCase() == String(this._search.name).toLowerCase())
            {
               list = PLAYER;
               this._search.state = LISTED;
            }
            else if(own != null && own != "" ? nick == own : this._playedAsked == MINE)
            {
               list = MINE;
            }
            else
            {
               return;
            }
            this._lists[list] = {"rows":this.listed(names,WatchRows.played),"at":getTimer()};
         }
         if(list == this.shownList())
         {
            this._gotAnswer = true;
         }
         if(list == LIVE)
         {
            this.findSpectated();
         }
         if(this._shown)
         {
            this.render();
         }
      }

      // The page's rows parsed by `parse` (its head and the friends' rows left), each with the
      // friends the page names in its match; what each recorded one says of its replay told to
      // the History's rows.
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
               if(row.kind != "live")
               {
                  NotificationListScrollContent.noteReplay(WatchWindow.hex(uint(row.match)),WatchWindow.stateReason(int(row.replay)));
               }
               continue;
            }
            var friend:Object = WatchRows.friend(names[i]);
            if(friend != null && byMatch[String(friend.match)] != null)
            {
               byMatch[String(friend.match)].friends.push(friend);
            }
         }
         return list;
      }

      // A friend's Spectate: the live match the list names it in, watched from its place.
      private function findSpectated() : void
      {
         if(this._spectate == null)
         {
            return;
         }
         for each(var row:Object in this.rowsOf(LIVE))
         {
            for each(var friend:Object in row.friends)
            {
               if(String(friend.name).toLowerCase() == this._spectate)
               {
                  var reason:String = WatchWindow.reasonOf(row);
                  // counting down: watched once it plays
                  if(reason == "st_srp_watch_reason_starts")
                  {
                     return;
                  }
                  this._spectate = null;
                  if(reason == null)
                  {
                     this.watch(uint(row.match),WatchRows.watchAsk(int(friend.place)));
                  }
                  return;
               }
            }
         }
      }

      private function pickTab(which:String) : void
      {
         if(which == this._tab)
         {
            return;
         }
         RoomsLook.clickSound();
         this._tab = which;
         this.look._scrollPx = 0;
         this.askNow();
         this.render();
      }

      private function pickChip(which:String) : void
      {
         RoomsLook.clickSound();
         if(this._tab == LIVE)
         {
            this._liveChip = which;
         }
         else
         {
            this._replaysChip = which;
            this._search = null;
            this.setBox("");
         }
         this.look._scrollPx = 0;
         this.askNow();
         this.render();
      }

      // The search box: a match's code (8 hex digits, `#` optional) watches it; else a player's
      // name lists that player's matches.
      private function typed() : void
      {
         var text:String = this._box.text.replace(/^\s+|\s+$/g,"");
         var match:uint = WatchWindow.codeOf(text);
         if(match != 0)
         {
            this.watch(match,NotificationListScrollContent.REPLAY_WATCH_ALL);
            return;
         }
         if(text.length > 0)
         {
            this.searchPlayer(text);
         }
      }

      // A match's code as the cards show it, `#` optional; 0 if `text` is not one.
      public static function codeOf(text:String) : uint
      {
         var digits:String = text.replace(/^#/,"");
         return /^[0-9a-fA-F]{8}$/.test(digits) ? uint(parseInt(digits,16)) : 0;
      }

      // Replays, `name`'s matches: the asker's own are Mine; another's account is found first
      // (find_players, answered in `found`).
      private function searchPlayer(name:String) : void
      {
         this._tab = REPLAYS;
         this.look._scrollPx = 0;
         var own:String = PlayerInfo.nickname;
         if(own != null && own != "" && own.toLowerCase() == name.toLowerCase())
         {
            this._search = null;
            this.setBox("");
            this._replaysChip = MINE;
            this.askNow();
            this.render();
            return;
         }
         this.setBox(name);
         this._search = {"name":name,"state":FINDING,"at":getTimer(),"asks":1};
         this._lists[PLAYER] = null;
         try
         {
            ExternalInterface.call("find_players",name);
         }
         catch(e:Error)
         {
         }
         this.render();
      }

      // find_players' answer: the player named so (any case), or the one player found; its
      // account's matches asked.
      private function foundPlayers(players:Array) : void
      {
         var want:String = String(this._search.name).toLowerCase();
         var pick:Object = null;
         for each(var player:Object in players)
         {
            if(player != null && String(player.name).toLowerCase() == want)
            {
               pick = player;
            }
         }
         if(pick == null && players.length == 1)
         {
            pick = players[0];
         }
         if(pick == null)
         {
            this._search.state = NOT_FOUND;
         }
         else
         {
            this._search.name = String(pick.name);
            this._search.id = uint(pick.id);
            this._search.state = ASKED;
            this._search.asks = 1;
            this.setBox(this._search.name);
            this.askPlayer();
         }
         this.render();
      }

      // The search box's text (kept over a drawing anew, render).
      private function setBox(text:String) : void
      {
         this._boxText = text;
         if(this._box != null)
         {
            this._box.text = text;
         }
      }

      private function askPlayer() : void
      {
         this._search.at = getTimer();
         this.call("victory_screen_request_player_data",SEARCH_ASK,uint(this._search.id));
         this.ask(PLAYER);
      }

      private function searchTick(now:int) : void
      {
         var search:Object = this._search;
         if(search.state == FINDING && now - int(search.at) >= 6000)
         {
            search.state = NO_ANSWER;
            this.render();
         }
         else if(search.state == ASKED && now - int(search.at) >= 1500)
         {
            if(int(search.asks) >= ASKS)
            {
               search.state = NO_ANSWER;
               this.render();
               return;
            }
            search.asks = int(search.asks) + 1;
            this.askPlayer();
         }
      }

      // Asks to watch `match` with `ask` (the History's prompt); the view to come back to.
      private function watch(match:uint, ask:uint) : void
      {
         var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
         this._pendingMatch = match;
         this._pendingView = {
            "tab":this._tab,
            "liveChip":this._liveChip,
            "replaysChip":this._replaysChip,
            "search":this._search
         };
         if(history != null)
         {
            history.replayBegin(match,ask);
         }
      }

      // A card's Watch: a recorded match from the player's place (a player's list) or anyone's;
      // a live one from the place of a friend in it, else the server's pick.
      private function watchRow(row:Object) : void
      {
         var place:int = WatchRows.PLACE_ANY;
         if(row.kind == "played")
         {
            place = int(row.place);
         }
         else if(row.kind == "live" && row.friends.length > 0)
         {
            place = int(row.friends[0].place);
         }
         this.watch(uint(row.match),WatchRows.watchAsk(place));
      }

      // An answer about the match asked for (NotificationListScrollContent): `watching` for the
      // one asked from here is the view to come back to; the cards drawn again.
      private function answered(hex:String, status:String) : void
      {
         if(status == "watching" && this._pendingView != null && hex == WatchWindow.hex(this._pendingMatch))
         {
            this._resume = this._pendingView;
            this._resumeAt = getTimer();
         }
         if(this._shown)
         {
            this.render();
         }
      }

      // The copy icon: the code on the game's clipboard, or, refused, shown selected for Ctrl+C.
      private function copy(hex:String) : void
      {
         var ok:Boolean = true;
         try
         {
            WatchWindow.clipboard("#" + hex);
         }
         catch(e:Error)
         {
            ok = false;
         }
         this._copied = {
            "hex":hex,
            "ok":ok,
            "at":getTimer()
         };
         this.render();
         if(!ok && this._codeField != null && stage != null)
         {
            stage.focus = this._codeField;
            this._codeField.setSelection(0,this._codeField.length);
         }
      }

      private static function hex(match:uint) : String
      {
         return NotificationListScrollContent.matchNumberText(match).substr(1);
      }

      private function onClose(e:Event) : void
      {
         this.show(false);
      }

      private function onBoxKey(e:KeyboardEvent) : void
      {
         if(e.keyCode == Keyboard.ENTER)
         {
            this.typed();
         }
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

      // The parts that are not empty, with dots between.
      public static function joined(parts:Array) : String
      {
         return parts.filter(function(part:String, i:int, all:Array):Boolean
         {
            return part != null && part != "";
         }).join("  ·  ");
      }

      // A match's map name: the rooms' pool's (its level), else the History's row of it.
      public static function mapName(row:Object) : String
      {
         var entry:Object = RoomsWindow.me != null ? RoomsWindow.me.entry(int(row.map)) : null;
         if(entry != null)
         {
            return RoomsText.levelName(int(entry.level));
         }
         var history:String = NotificationListScrollContent.mapNameOf(uint(row.match));
         return history != null ? history : LocalizationData.getName("st_srp_watch_map_retired");
      }

      // A live match's word for its phase (in play: LIVE).
      public static function phaseWord(row:Object) : String
      {
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
            return "";
         }
         return LocalizationData.getName("st_srp_watch_live_word");
      }

      // Why a row's match cannot be watched (a dictionary key), null when it can: a live one not
      // in play yet or over, full or closed to spectators; a recorded one's replay as last said.
      public static function reasonOf(row:Object) : String
      {
         if(row.kind != "live")
         {
            return NotificationListScrollContent.replayReason(WatchWindow.hex(uint(row.match)));
         }
         if(int(row.phase) == MatchPhase.GATHERING || int(row.phase) == MatchPhase.COUNTDOWN)
         {
            return "st_srp_watch_reason_starts";
         }
         if(int(row.phase) == WatchRows.PHASE_OVER)
         {
            return "st_srp_watch_reason_over";
         }
         if(int(row.watch) == WatchRows.WATCH_FULL)
         {
            return "st_srp_watch_reason_full";
         }
         if(int(row.watch) == WatchRows.WATCH_PRIVATE)
         {
            return "st_srp_watch_reason_private";
         }
         return null;
      }

      // A row's replay state's reason (WatchRows.REPLAY_*), null when it may be watched.
      private static function stateReason(replay:int) : String
      {
         if(replay == WatchRows.REPLAY_OLD)
         {
            return NotificationListScrollContent.REASON_OLD;
         }
         if(replay == WatchRows.REPLAY_KEPT || replay == WatchRows.REPLAY_READY)
         {
            return null;
         }
         return NotificationListScrollContent.REASON_GONE;
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
         this._codeField = null;
         this.look.frame();
         this.drawTabs();
         this.look.closeButton(this.onClose);
         this.drawSearch();
         this.drawChips();
         this.drawList();
         if(focused && stage != null)
         {
            stage.focus = this._box;
            this._box.setSelection(this._box.length,this._box.length);
         }
      }

      private function drawTabs() : void
      {
         var count:int = this.liveCount;
         var tabs:Array = [[LIVE,LocalizationData.getName("st_srp_watch_live") + (count > 0 ? "  " + count : "")],
            [REPLAYS,LocalizationData.getName("st_srp_watch_replays")]];
         var tabW:Number = 220;
         var x:Number = int((this.look._w - tabs.length * tabW) / 2);
         for(var i:int = 0; i < tabs.length; i++)
         {
            var tab:Sprite = this.look.headTab("srpWatchTab_" + tabs[i][0],tabs[i][1],x + i * tabW,tabW,tabs[i][0] == this._tab,function(name:String):void
            {
               pickTab(name.substr("srpWatchTab_".length));
            });
            var label:TextField = tab.getChildAt(0) as TextField;
            RoomsLook.fitName(label,tabs[i][1],"");
            if(tabs[i][0] == LIVE && count > 0)
            {
               // the red dot of live matches, left of the label
               var dot:Shape = new Shape();
               dot.name = "srpWatchLiveDot";
               dot.graphics.beginFill(RED,1);
               dot.graphics.drawCircle(0,0,4);
               dot.graphics.endFill();
               dot.x = int((tabW - label.textWidth) / 2) - 10;
               dot.y = label.y + int(label.height / 2) - 1;
               tab.addChild(dot);
            }
         }
      }

      // The chips at the toolbar's left (Live: and how far behind a watch is).
      private function drawChips() : void
      {
         var chips:Array = this._tab == LIVE ? [[EVERYONE,"st_srp_watch_chip_everyone"],[FRIENDS,"st_srp_watch_chip_friends"]]
            : [[MINE,"st_srp_watch_chip_mine"],[FRIENDS,"st_srp_watch_chip_friends"],[EVERYONE,"st_srp_watch_chip_everyone"]];
         var x:Number = 14;
         for each(var c:Array in chips)
         {
            x = this.chipAt(c[0],LocalizationData.getName(c[1]),x,this.chip == c[0]) + 8;
         }
         if(this._tab == LIVE && this._delay > 0)
         {
            var room:Number = this._box.x - 20 - (x + 8);
            RoomsLook.fitName(this.look.text(this.look._content,"",x + 8,TOOL_Y + 6,Math.max(20,room),13,RoomsLook.DIM),
               LocalizationData.format("st_srp_watch_delay",this._delay),"");
            this.look._content.getChildAt(this.look._content.numChildren - 1).name = "srpWatchDelay";
         }
      }

      // A chip: a rounded tag, filled gold when chosen; its x end.
      private function chipAt(id:String, label:String, x:Number, on:Boolean) : Number
      {
         var chip:Sprite = new Sprite();
         chip.name = "srpWatchChip_" + id;
         chip.x = x;
         chip.y = TOOL_Y + 1;
         var tf:TextField = this.look.text(chip,label,14,5,400,13,on ? 0xFFFFFF : RoomsLook.GREY);
         tf.width = int(tf.textWidth + 6);
         var w:Number = int(tf.width + 24);
         chip.graphics.lineStyle(1,on ? RoomsLook.GOLD : 0x3A4854);
         chip.graphics.beginFill(on ? 0x3C2C10 : 0x0B0E11,1);
         chip.graphics.drawRoundRect(0,0,w,CHIP_H,CHIP_H,CHIP_H);
         chip.graphics.endFill();
         chip.buttonMode = true;
         chip.mouseChildren = false;
         chip.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
         {
            pickChip(id);
         });
         this.look._content.addChild(chip);
         this.look.route(chip,function(control:DisplayObject, e:MouseEvent):void
         {
            pickChip(id);
         });
         return x + w;
      }

      // The search box at the toolbar's right: its hint while empty, Enter or the magnifier searches.
      private function drawSearch() : void
      {
         var x:Number = this.look._w - 14 - SEARCH_W;
         var hint:TextField = this.look.text(this.look._content,"",x + 8,TOOL_Y + 5,SEARCH_W - 40,14,RoomsLook.DIM);
         hint.name = "srpWatchSearchHint";
         RoomsLook.fitName(hint,LocalizationData.getName("st_srp_watch_search_hint"),"");
         var tf:TextField = this.look.text(this.look._content,this._boxText,x,TOOL_Y + 2,SEARCH_W,15,RoomsLook.GOLD);
         tf.name = "srpWatchSearch";
         tf.height = 26;
         tf.border = true;
         tf.borderColor = 0x3A4854;
         tf.type = TextFieldType.INPUT;
         tf.maxChars = 32;
         tf.selectable = true;
         tf.mouseEnabled = true;
         tf.tabEnabled = true;
         hint.visible = tf.text == "";
         tf.addEventListener(Event.CHANGE,function(e:Event):void
         {
            hint.visible = tf.text == "";
         });
         tf.addEventListener(KeyboardEvent.KEY_DOWN,this.onBoxKey,false,0,true);
         tf.addEventListener(MouseEvent.MOUSE_DOWN,function(e:MouseEvent):void
         {
            FocusManager.setFocus(e.currentTarget as TextField);
         });
         this.look.route(tf,function(control:DisplayObject, e:MouseEvent):void
         {
            FocusManager.setFocus(control as TextField);
         });
         // its back under it (the field's own background would hide the hint)
         var back:Shape = new Shape();
         back.graphics.beginFill(0x070A0D,1);
         back.graphics.drawRect(x,TOOL_Y + 2,SEARCH_W,26);
         back.graphics.endFill();
         this.look._content.addChildAt(back,this.look._content.getChildIndex(hint));
         this._box = tf;
         var go:Sprite = new Sprite();
         go.name = "srpWatchSearchGo";
         go.x = x + SEARCH_W - 26;
         go.y = TOOL_Y + 2;
         go.graphics.beginFill(0x000000,0.01);
         go.graphics.drawRect(0,0,26,26);
         go.graphics.endFill();
         go.graphics.lineStyle(2,RoomsLook.GREY);
         go.graphics.drawCircle(11,11,5.5);
         go.graphics.moveTo(15,15);
         go.graphics.lineTo(20,20);
         go.buttonMode = true;
         go.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
         {
            typed();
         });
         this.look._content.addChild(go);
         this.look.route(go,function(control:DisplayObject, e:MouseEvent):void
         {
            typed();
         });
      }

      // The rows the shown list holds (Live's Friends: the matches with a friend in them).
      private function shownRows() : Array
      {
         var rows:Array = this.rowsOf(this.shownList());
         if(this._tab == LIVE && this._liveChip == FRIENDS)
         {
            rows = rows.filter(function(row:Object, i:int, all:Array):Boolean
            {
               return row.friends.length > 0;
            });
         }
         return rows;
      }

      private function drawList() : void
      {
         var w:Number = this.look._w - 28 - 10;
         var inner:Sprite = this.look.scrollBox(14,LIST_Y,w,this.look._h - LIST_Y - 10);
         var rows:Array = this.shownRows();
         var y:Number = 0;
         if(rows.length == 0)
         {
            this.look.text(inner,this.emptyText(),0,14,w,15,RoomsLook.GREY).name = "srpWatchEmpty";
         }
         for(var i:int = 0; i < rows.length; i++)
         {
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
         var list:String = this.shownList();
         if(list == PLAYER)
         {
            var state:String = this._search.state;
            if(state == NOT_FOUND)
            {
               return LocalizationData.format("st_srp_watch_no_player",this._search.name);
            }
            if(state == NO_ANSWER)
            {
               return LocalizationData.getName("st_srp_matches_no_answer");
            }
            return state == LISTED ? LocalizationData.getName("st_srp_player_matches_none") : RoomsLook.DOTS;
         }
         var now:int = getTimer();
         var asking:Boolean = list == LIVE ? this.answeredAt(LIVE) == 0 : !this._gotAnswer;
         if(asking && (list == LIVE ? now - this._openedAt >= 6000 : this._asks > ASKS))
         {
            return LocalizationData.getName("st_srp_matches_no_answer");
         }
         if(asking)
         {
            return RoomsLook.DOTS;
         }
         if(list == LIVE)
         {
            return LocalizationData.getName(this._liveChip == FRIENDS ? "st_srp_watch_none_live_friends" : "st_srp_matches_none");
         }
         if(list == MINE)
         {
            return LocalizationData.getName("st_srp_watch_none_mine");
         }
         return LocalizationData.getName(list == FRIENDS ? "st_srp_watch_none_friends" : "st_srp_watch_none_recent");
      }

      // A row's card: Watch or why not, and the copy icon by its code.
      private function card(row:Object, w:Number) : MatchCard
      {
         var hexed:String = WatchWindow.hex(uint(row.match));
         var copied:Object = this._copied != null && this._copied.hex == hexed ? this._copied : null;
         var card:MatchCard = new MatchCard(this.look,row,w,WatchWindow.reasonOf(row),copied,function(e:Event):void
         {
            watchRow(row);
         },function():void
         {
            copy(hexed);
         });
         for each(var b:* in card.presses)
         {
            this.routeInList(b);
         }
         if(card.codeField != null)
         {
            this._codeField = card.codeField;
         }
         if(card.clock != null)
         {
            this._clocks.push({"field":card.clock,"row":row});
            this.putTime(card.clock,row,getTimer());
         }
         return card;
      }

      // A control in the scrolled list: a press the content got itself clicks it, where the list
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
         var total:int = int(row.played) + int(Math.max(0,now - this.answeredAt(LIVE)) / 1000);
         field.text = int(row.phase) == MatchPhase.PLAYING ? RoomsText.clock(total) : "";
      }

      private function tick(now:int) : void
      {
         for each(var clock:Object in this._clocks)
         {
            this.putTime(clock.field as TextField,clock.row,now);
         }
      }
   }
}
