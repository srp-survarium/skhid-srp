package survarium.live
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.MatchPhase;
   import survarium.controls.NotificationListScrollContent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsText;

   // The bottom bar's Matches tab (BottomPanel): the running matches, each with Watch (as a
   // spectator, anyone in view) and Players, a picker of whom to watch. Both ask a replay
   // session's `3D` (NotificationListScrollContent's prompt): for a running match the lobby server
   // makes it a live spectator's. The lobby server's rows (PlayerRanking's), polled while open:
   // - `3B 00 <LIST_START> <LIST_COUNT>`: `L0;<n>`, then n rows
   //   `L1;<match 8 hex>;<map id>;<mode>;<phase>;<players a>;<players b>;<score a|->;<score b|->;<played s>;<spectators>;<watch>`
   //   (watch: 1 may, 0 full, 2 not public);
   // - the picker: `3D 00 <ROSTER_ASK> <match>` chooses the match, then `3B 00 <ROSTER_START> <ROSTER_COUNT>`:
   //   rows `L2;<match 8 hex>;<place>;<team 0|1>;<kills>;<deaths>;<name>`.
   public class LiveWindow extends Sprite
   {
      public static const LIST_START:uint = 2240000000;

      public static const LIST_COUNT:int = 20;

      public static const ROSTER_ASK:uint = 0xFFFFFF06;

      public static const ROSTER_START:uint = 2241000000;

      public static const ROSTER_COUNT:int = 24;

      public static const W:int = 400;

      private static const ROW_H:int = 46;

      private static const ROWS_SHOWN:int = 6;

      private static const TOP:int = 34;

      // a watch row's word
      private static const WATCH_MAY:int = 1;

      private static const WATCH_FULL:int = 0;

      private static const WATCH_PRIVATE:int = 2;

      // the list is asked again this often while open, the picker's roster once a second until it comes
      private static const LIST_EVERY:int = 5000;

      // the open window: the one the answers go to
      public static var me:LiveWindow;

      private var _font:String;

      // the view: the list (_roster null) or one match's picker (_roster its L1 row)
      private var _roster:Object;

      private var _matches:Array = [];

      private var _players:Array = [];

      // when the shown list came (its played seconds count from then), 0: no answer yet
      private var _listAt:int = 0;

      private var _askedAt:int = 0;

      private var _openedAt:int = 0;

      private var _scroll:int = 0;

      private var _timer:Timer;

      private var _title:TextField;

      private var _back:Button;

      private var _body:Sprite = new Sprite();

      private var _note:TextField;

      // the shown rows' time fields and their rows, ticked each second
      private var _clocks:Array = [];

      public function LiveWindow(font:String)
      {
         super();
         name = "srpLiveWindow";
         this._font = font;
         this._title = this.text("",13,0xCCCCCC);
         this._title.x = 12;
         this._title.y = 8;
         addChild(this._title);
         this._note = this.text("",12,0x8A9096);
         this._note.x = 12;
         this._note.y = TOP + 12;
         addChild(this._note);
         this._body.y = TOP;
         addChild(this._body);
         this._back = this.button(LocalizationData.getName("st_srp_live_back"),"srpLiveBack");
         this._back.x = W - 8 - this._back.width;
         this._back.y = 6;
         this._back.addEventListener(ButtonEvent.CLICK,this.onBack,false,0,true);
         addChild(this._back);
         addEventListener(MouseEvent.MOUSE_WHEEL,this.onWheel,false,0,true);
         this.draw();
      }

      // L0/L2 rows: taken (true) when they are these rows, whether or not the window is open.
      public static function take(rows:Array) : Boolean
      {
         var first:String = null;
         try
         {
            first = rows && rows.length > 0 && rows[0] ? String(rows[0].nickname) : "";
            if(first.indexOf("L0;") == 0)
            {
               if(LiveWindow.me)
               {
                  LiveWindow.me.listed(rows);
               }
               return true;
            }
            if(first.indexOf("L2;") == 0)
            {
               if(LiveWindow.me)
               {
                  LiveWindow.me.rostered(rows);
               }
               return true;
            }
         }
         catch(e:Error)
         {
         }
         return false;
      }

      public function get shown() : Boolean
      {
         return LiveWindow.me == this;
      }

      public function show() : void
      {
         LiveWindow.me = this;
         this._roster = null;
         this._openedAt = getTimer();
         this._scroll = 0;
         this.ask();
         if(!this._timer)
         {
            this._timer = new Timer(1000,0);
            this._timer.addEventListener(TimerEvent.TIMER,this.onTimer,false,0,true);
         }
         this._timer.reset();
         this._timer.start();
         this.draw();
      }

      public function hide() : void
      {
         if(this._timer)
         {
            this._timer.stop();
         }
         if(LiveWindow.me == this)
         {
            LiveWindow.me = null;
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

      private function ask() : void
      {
         this._askedAt = getTimer();
         if(this._roster)
         {
            this.call("request_players_range",ROSTER_START,ROSTER_COUNT);
         }
         else
         {
            this.call("request_players_range",LIST_START,LIST_COUNT);
         }
      }

      private function onTimer(e:TimerEvent) : void
      {
         var now:int = getTimer();
         var every:int = this._roster && this._players.length == 0 ? 1000 : LIST_EVERY;
         if(now - this._askedAt >= every - 100)
         {
            this.ask();
         }
         if(!this._roster)
         {
            this.drawClocks(now);
         }
         if(this._listAt == 0 && !this._roster && now - this._openedAt >= 6000)
         {
            this._note.text = LocalizationData.getName("st_srp_matches_no_answer");
         }
      }

      private static function hex(text:String) : Boolean
      {
         return /^[0-9a-f]{8}$/.test(text);
      }

      private static function count(text:String) : int
      {
         return /^[0-9]{1,6}$/.test(text) ? int(text) : -1;
      }

      // One L1 row, parsed; null if it is not one.
      private static function parseMatch(text:String) : Object
      {
         var f:Array = text.split(";");
         var n:Array = [];
         if(f.length != 12 || f[0] != "L1" || !hex(f[1]))
         {
            return null;
         }
         for each(var i:int in [2,3,4,5,6,9,10,11])
         {
            if(count(f[i]) < 0)
            {
               return null;
            }
         }
         if((f[7] == "-") != (f[8] == "-") || f[7] != "-" && (count(f[7]) < 0 || count(f[8]) < 0) || count(f[11]) > 2 || count(f[4]) > MatchPhase.PLAYING)
         {
            return null;
         }
         return {
            "match":uint(parseInt(f[1],16)),
            "map":count(f[2]),
            "mode":count(f[3]),
            "phase":count(f[4]),
            "a":count(f[5]),
            "b":count(f[6]),
            "score":f[7] == "-" ? "" : f[7] + ":" + f[8],
            "played":count(f[9]),
            "spectators":count(f[10]),
            "watch":count(f[11])
         };
      }

      // One L2 row of `match`, parsed; null if it is not one.
      private static function parsePlayer(text:String, match:uint) : Object
      {
         var f:Array = text.split(";");
         if(f.length < 7 || f[0] != "L2" || !hex(f[1]) || uint(parseInt(f[1],16)) != match)
         {
            return null;
         }
         if(count(f[2]) < 0 || count(f[2]) > 0xFE || count(f[3]) < 0 || count(f[3]) > 1 || count(f[4]) < 0 || count(f[5]) < 0)
         {
            return null;
         }
         return {
            "place":count(f[2]),
            "team":count(f[3]),
            "kills":count(f[4]),
            "deaths":count(f[5]),
            "name":f.slice(6).join(";")
         };
      }

      public function listed(rows:Array) : void
      {
         var list:Array = [];
         var row:Object = null;
         for(var i:int = 1; i < rows.length; i++)
         {
            row = rows[i] ? LiveWindow.parseMatch(String(rows[i].nickname)) : null;
            if(row)
            {
               list.push(row);
            }
         }
         this._matches = list;
         this._listAt = getTimer();
         if(!this._roster)
         {
            this.draw();
         }
      }

      public function rostered(rows:Array) : void
      {
         var list:Array = [];
         var row:Object = null;
         if(!this._roster)
         {
            return;
         }
         for(var i:int = 0; i < rows.length; i++)
         {
            row = rows[i] ? LiveWindow.parsePlayer(String(rows[i].nickname),uint(this._roster.match)) : null;
            if(row)
            {
               list.push(row);
            }
         }
         if(list.length > 0)
         {
            this._players = list;
            this.draw();
         }
      }

      private function watch(match:uint, ask:uint) : void
      {
         var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
         if(history != null)
         {
            history.replayBegin(match,ask);
         }
      }

      private function pick(match:Object) : void
      {
         this._roster = match;
         this._players = [];
         this.call("victory_screen_request_player_data",ROSTER_ASK,uint(match.match));
         this.ask();
         this.draw();
      }

      private function onBack(e:ButtonEvent) : void
      {
         this._roster = null;
         this.ask();
         this.draw();
      }

      private function onWheel(e:MouseEvent) : void
      {
         var last:int = Math.max(0,this._matches.length - ROWS_SHOWN);
         var old:int = this._scroll;
         if(this._roster)
         {
            return;
         }
         this._scroll = Math.max(0,Math.min(last,this._scroll + (e.delta > 0 ? -1 : 1)));
         if(this._scroll != old)
         {
            this.draw();
            e.stopImmediatePropagation();
         }
      }

      private function draw() : void
      {
         var rows:int = 0;
         var height:int = 0;
         while(this._body.numChildren > 0)
         {
            this._body.removeChildAt(0);
         }
         this._clocks = [];
         this._back.visible = this._roster != null;
         if(this._roster)
         {
            this._title.text = LocalizationData.format("st_srp_live_pick",RoomsText.levelName(int(this._roster.map)));
            rows = this.drawRoster();
         }
         else
         {
            this._title.text = LocalizationData.getName("st_srp_matches_title").toUpperCase();
            rows = this.drawList();
         }
         this._note.visible = rows == 0;
         if(rows == 0)
         {
            this._note.text = this._listAt == 0 && !this._roster || this._roster ? "…" : LocalizationData.getName("st_srp_matches_none");
         }
         height = TOP + Math.max(1,rows) * (this._roster ? 30 : ROW_H) + 8;
         graphics.clear();
         graphics.lineStyle(1,0x2A3036,1);
         graphics.beginFill(0x0B0F12,0.96);
         graphics.drawRect(0,0,W,height);
         graphics.endFill();
         // its owner keeps its foot on the bar
         dispatchEvent(new Event(Event.RESIZE));
      }

      private function drawList() : int
      {
         var shown:int = Math.min(ROWS_SHOWN,this._matches.length - this._scroll);
         for(var i:int = 0; i < shown; i++)
         {
            this._body.addChild(this.matchRow(this._matches[this._scroll + i],i));
         }
         return shown;
      }

      private function matchRow(row:Object, index:int) : Sprite
      {
         var line:Sprite = new Sprite();
         var map:TextField = this.text(RoomsText.levelName(int(row.map)),13,0xCCCCCC);
         var label:TextField = this.text(RoomsText.modeName(int(row.mode)) + "  ·  " + row.a + " " + LocalizationData.getName("st_srp_matches_versus") + " " + row.b,11,0x8A9096);
         var score:TextField = this.text(String(row.score),13,0x9CA5A5);
         var word:TextField = this.text(this.wordOf(row),11,0xF2C14E);
         var time:TextField = this.text("",11,0x8A9096);
         time.name = "srpLiveTime";
         var watchButton:Button = this.button(LocalizationData.getName("st_srp_replay_watch"),"srpLiveWatch");
         var playersButton:Button = this.button(LocalizationData.getName("st_srp_live_players"),"srpLivePlayers");
         line.name = "srpLiveMatch" + index;
         line.y = index * ROW_H;
         line.graphics.beginFill(index % 2 == 0 ? 0x161B20 : 0x11161A,1);
         line.graphics.drawRect(6,2,W - 12,ROW_H - 4);
         line.graphics.endFill();
         watchButton.x = W - 12 - watchButton.width;
         playersButton.x = watchButton.x - 4 - playersButton.width;
         watchButton.y = playersButton.y = int((ROW_H - watchButton.height) / 2);
         watchButton.enabled = int(row.watch) == WATCH_MAY;
         playersButton.enabled = watchButton.enabled;
         watchButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            watch(uint(row.match),NotificationListScrollContent.REPLAY_WATCH_ALL);
         });
         playersButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            pick(row);
         });
         score.x = 12;
         score.y = 6;
         time.x = 12;
         time.y = 24;
         map.x = 72;
         map.y = 5;
         label.x = 72;
         label.y = 24;
         word.x = int(playersButton.x - 8 - word.width);
         word.y = 24;
         // the map and the mode line cut at what follows them
         map.autoSize = TextFieldAutoSize.NONE;
         map.width = Math.max(20,playersButton.x - 8 - map.x);
         label.autoSize = TextFieldAutoSize.NONE;
         label.width = Math.max(20,word.x - 6 - label.x);
         for each(var part:* in [score,time,map,label,word,playersButton,watchButton])
         {
            line.addChild(part);
         }
         this._clocks.push({"field":time,"row":row});
         this.putTime(time,row,getTimer());
         return line;
      }

      private function wordOf(row:Object) : String
      {
         if(int(row.watch) == WATCH_FULL)
         {
            return LocalizationData.getName("st_srp_matches_full");
         }
         if(int(row.watch) == WATCH_PRIVATE)
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
         return int(row.spectators) > 0 ? LocalizationData.format("st_srp_live_spectators",row.spectators) : "";
      }

      private function putTime(field:TextField, row:Object, now:int) : void
      {
         var total:int = int(row.played) + int(Math.max(0,now - this._listAt) / 1000);
         var minutes:int = int(total / 60);
         var seconds:int = total - minutes * 60;
         field.text = int(row.phase) == MatchPhase.PLAYING ? (minutes < 10 ? "0" : "") + minutes + ":" + (seconds < 10 ? "0" : "") + seconds : "";
      }

      private function drawClocks(now:int) : void
      {
         for each(var clock:Object in this._clocks)
         {
            this.putTime(clock.field as TextField,clock.row,now);
         }
      }

      // Both teams side by side, each player a button: watch from that place.
      private function drawRoster() : int
      {
         var rows:Array = [0,0];
         var match:uint = uint(this._roster.match);
         for each(var player:Object in this._players)
         {
            var team:int = int(player.team);
            var button:Button = this.button(player.name + "  " + player.kills + "/" + player.deaths,"srpLivePlayer" + player.place);
            button.width = W / 2 - 18;
            button.x = 12 + team * (W / 2 - 6);
            button.y = rows[team] * 30 + 4;
            rows[team]++;
            (function(place:int):void
            {
               button.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
               {
                  watch(match,(NotificationListScrollContent.REPLAY_WATCH_ALL & 0xFF000000) | uint(place) << 16);
               });
            })(int(player.place));
            this._body.addChild(button);
         }
         return Math.max(rows[0],rows[1]);
      }

      private function button(label:String, name:String) : Button
      {
         var button:Button = new (getDefinitionByName("SvButton2SmallFr") as Class)() as Button;
         button.name = name;
         button.label = label;
         button.width = Math.max(70,Math.min(140,int(label.length * 7.3 + 18)));
         return button;
      }

      private function text(value:String, size:Number, colour:uint) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = new TextFormat(this._font,size,colour,true);
         field.text = value;
         return field;
      }
   }
}
