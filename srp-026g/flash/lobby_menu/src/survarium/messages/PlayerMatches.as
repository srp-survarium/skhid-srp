package survarium.messages
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.NotificationListScrollContent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsText;

   // The player pop-up's Matches tab (MessagePlayerInfo): the shown player's recent public matches,
   // each with Watch on that player's view (a replay session from the player's place,
   // NotificationListScrollContent's prompt). The lobby server answers
   // `3B 00 <MATCHES_START> <MATCHES_COUNT>` about the player the asker was shown last (the subject
   // of its last `93`), `3B 00 <MATCHES_START + 1> <MATCHES_COUNT>` about the asker itself (its own
   // pop-up has no `93`): `P1;<n>;<nickname>`, then n rows
   // `P2;<match 8 hex>;<place>;<map id>;<mode>;<result>;<kills>;<deaths>;<minutes since its end>;<replay kept 1|0>`
   // (PlayerRanking's rows; taken only when the nickname is the pop-up's).
   public class PlayerMatches extends Sprite
   {
      public static const MATCHES_START:uint = 2230000000;

      public static const MATCHES_COUNT:int = 10;

      // a result's word by its number (0 defeat, 1 victory, 2 draw)
      private static const RESULT_KEYS:Array = ["defeat","victory","st_srp_player_matches_draw"];

      private static const ROW_H:int = 30;

      // asks again once a second until an answer, at most this many times
      private static const ASKS:int = 5;

      // the pop-up whose tab is open: the one the answers go to
      public static var open:PlayerMatches;

      // the panel's state: asking (no answer yet), listed, or no answer
      private static const ASKING:int = 0;

      private static const LISTED:int = 1;

      private static const NO_ANSWER:int = 2;

      private var _nickname:String;

      // the asker's own pop-up (MATCHES_START + 1)
      private var _own:Boolean;

      private var _width:Number;

      private var _font:String;

      private var _state:int = ASKING;

      private var _asks:int = 0;

      private var _timer:Timer;

      private var _rows:Sprite = new Sprite();

      private var _note:TextField;

      public function PlayerMatches(nickname:String, own:Boolean, width:Number, height:Number, font:String)
      {
         super();
         name = "srpPlayerMatches";
         this._nickname = nickname;
         this._own = own;
         this._width = width;
         this._font = font;
         graphics.beginFill(0x0B0F12,0.96);
         graphics.drawRect(0,0,width,height);
         graphics.endFill();
         this._note = this.text(LocalizationData.getName("st_srp_player_matches_asking"),12,0x8A9096);
         this._note.x = 10;
         this._note.y = 10;
         addChild(this._note);
         addChild(this._rows);
         addEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved,false,0,true);
      }

      // `P1`/`P2` rows: taken (true) when they are these rows, whether or not a tab is open for them.
      public static function take(rows:Array) : Boolean
      {
         var first:String = null;
         try
         {
            first = rows && rows.length > 0 && rows[0] ? String(rows[0].nickname) : "";
            if(first.indexOf("P1;") != 0)
            {
               return false;
            }
            if(PlayerMatches.open)
            {
               PlayerMatches.open.answer(rows);
            }
         }
         catch(e:Error)
         {
         }
         return true;
      }

      public function show() : void
      {
         PlayerMatches.open = this;
         this._asks = 0;
         this.ask();
         if(!this._timer)
         {
            this._timer = new Timer(1000,0);
            this._timer.addEventListener(TimerEvent.TIMER,this.onTimer,false,0,true);
         }
         this._timer.reset();
         this._timer.start();
      }

      public function hide() : void
      {
         if(this._timer)
         {
            this._timer.stop();
         }
         if(PlayerMatches.open == this)
         {
            PlayerMatches.open = null;
         }
      }

      private function onRemoved(e:Event) : void
      {
         this.hide();
      }

      private function ask() : void
      {
         this._asks++;
         try
         {
            ExternalInterface.call("request_players_range",PlayerMatches.MATCHES_START + (this._own ? 1 : 0),PlayerMatches.MATCHES_COUNT);
         }
         catch(e:Error)
         {
         }
      }

      private function onTimer(e:TimerEvent) : void
      {
         if(this._state != ASKING)
         {
            this._timer.stop();
         }
         else if(this._asks >= ASKS)
         {
            this._timer.stop();
            this._state = NO_ANSWER;
            this._note.text = LocalizationData.getName("st_srp_replay_no_answer");
         }
         else
         {
            this.ask();
         }
      }

      // One match row, parsed; null if it is not one.
      private static function parse(text:String) : Object
      {
         var f:Array = text.split(";");
         var n:Array = [];
         var i:int = 0;
         if(f.length != 10 || f[0] != "P2" || !/^[0-9a-f]{8}$/.test(f[1]))
         {
            return null;
         }
         for(i = 2; i < 10; i++)
         {
            if(!/^[0-9]{1,6}$/.test(f[i]))
            {
               return null;
            }
            n.push(int(f[i]));
         }
         if(n[0] > 0xFE || n[3] > 2 || n[7] > 1)
         {
            return null;
         }
         return {
            "match":uint(parseInt(f[1],16)),
            "place":n[0],
            "map":n[1],
            "mode":n[2],
            "result":n[3],
            "kills":n[4],
            "deaths":n[5],
            "minutes":n[6],
            "replay":n[7] == 1
         };
      }

      private function answer(rows:Array) : void
      {
         var head:Array = String(rows[0].nickname).split(";");
         var list:Array = [];
         var row:Object = null;
         var i:int = 0;
         if(head.length < 3 || head.slice(2).join(";") != this._nickname)
         {
            return;
         }
         for(i = 1; i < rows.length; i++)
         {
            row = rows[i] ? PlayerMatches.parse(String(rows[i].nickname)) : null;
            if(row)
            {
               list.push(row);
            }
         }
         this._state = LISTED;
         this.draw(list);
      }

      private function draw(list:Array) : void
      {
         var i:int = 0;
         while(this._rows.numChildren > 0)
         {
            this._rows.removeChildAt(0);
         }
         this._note.visible = list.length == 0;
         this._note.text = LocalizationData.getName("st_srp_player_matches_none");
         for(i = 0; i < list.length; i++)
         {
            this._rows.addChild(this.rowOf(list[i],i));
         }
      }

      private function rowOf(row:Object, index:int) : Sprite
      {
         var line:Sprite = new Sprite();
         var place:TextField = this.text(RoomsText.levelName(int(row.map)) + "  ·  " + RoomsText.modeName(int(row.mode)),12,0xCCCCCC);
         var result:TextField = this.text(LocalizationData.getName(RESULT_KEYS[row.result]),12,row.result == 1 ? 0x5FA65A : 0xA0A4A8);
         var kd:TextField = this.text(LocalizationData.format("st_srp_player_matches_kd",row.kills,row.deaths),12,0xA0A4A8);
         var age:TextField = this.text(PlayerMatches.ageText(int(row.minutes)),11,0x8A9096);
         var watch:Button = null;
         line.name = "srpPlayerMatch" + index;
         line.y = 8 + index * ROW_H;
         line.graphics.beginFill(index % 2 == 0 ? 0x161B20 : 0x11161A,1);
         line.graphics.drawRect(4,0,this._width - 8,ROW_H - 2);
         line.graphics.endFill();
         if(row.replay)
         {
            watch = new (getDefinitionByName("SvButton2SmallFr") as Class)() as Button;
            watch.name = "srpPlayerMatchWatch";
            watch.label = LocalizationData.getName("st_srp_replay_watch");
            watch.width = Math.max(70,Math.min(120,int(watch.label.length * 7.3 + 18)));
            watch.x = int(this._width - 10 - watch.width);
            watch.y = 2;
            watch.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
            {
               var history:NotificationListScrollContent = NotificationListScrollContent.replayHolder;
               if(history != null)
               {
                  history.replayBegin(uint(row.match),PlayerMatches.watchAsk(int(row.place)));
               }
            });
            line.addChild(watch);
         }
         // columns 80 px apart, wider where a language's word needs it
         result.x = 10;
         kd.x = Math.max(90,result.x + result.width + 10);
         place.x = Math.max(170,kd.x + kd.width + 10);
         age.x = int((watch ? watch.x : this._width - 10) - 8 - age.width);
         place.width = Math.max(20,age.x - 8 - place.x);
         for each(var field:TextField in [result,kd,place,age])
         {
            field.y = int((ROW_H - 2 - field.height) / 2);
            line.addChild(field);
         }
         return line;
      }

      // A replay session from `place`'s view, from the start (NotificationListScrollContent.REPLAY_WATCH_ALL's layout).
      public static function watchAsk(place:int) : uint
      {
         return (NotificationListScrollContent.REPLAY_WATCH_ALL & 0xFF000000) | uint(place) << 16;
      }

      private static function ageText(minutes:int) : String
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
