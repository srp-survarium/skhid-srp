package survarium.watch
{
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.FocusManager;
   import survarium.constants.MatchPhase;
   import survarium.controls.NotificationListScrollContent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsText;
   import survarium.rooms.RoomsWindow;

   // One match of the Watch window's lists (WatchWindow), research/watch-redesign.html's card: the
   // map's render, then three lines (its name and mode; the score or the player's result; its age,
   // its code, selectable, a save icon (ReplayAsks) when it may be watched, and the friends in it)
   // and at the right one Watch, or why it cannot be watched. A card is made once per match shown
   // and updated in place (show) as its row changes, so a press on its controls outlives the
   // refreshes of the list.
   public class MatchCard extends Sprite
   {
      public static const H:int = 66;

      private static const THUMB_W:int = 100;

      private static const THUMB_H:int = 56;

      private static const TEXT_X:int = 120;

      private static const BUTTON_W:int = 120;

      // the reason's column at the right, in place of Watch
      private static const REASON_W:int = 190;

      private static const GREEN:uint = 0x8FD18F;

      private static const RED:uint = 0xE0483E;

      // a result's word by its number (0 defeat, 1 victory, 2 draw)
      private static const RESULT_KEYS:Array = ["defeat","victory","st_srp_player_matches_draw"];

      private var _look:RoomsLook;

      private var _w:Number;

      private var _watch:Function;

      private var _route:Function;

      // the person before the friends' names
      private var _friendMark:Shape;

      // the row shown (WatchRows' object, `kind` live, recent or played)
      public var row:Object;

      // the clock's field of a live match being played, ticked by the window
      public var clock:TextField;

      // the card's text fields by name, and those this show used (the others are hidden)
      private var _fields:Object = {};

      private var _used:Object;

      private var _button:*;

      private var _why:TextField;

      private var _saveIcon:Sprite;

      private var _saveDrawn:String;

      // the controls the window routes presses to; they go with the card
      public var routed:Array = [];

      // `watch`: the card's Watch was pressed (its row). `route`: the window routes presses on a
      // control in the list (WatchWindow.routeInList).
      public function MatchCard(look:RoomsLook, row:Object, w:Number, watch:Function, route:Function)
      {
         super();
         this._look = look;
         this._w = w;
         this._watch = watch;
         this._route = route;
         graphics.beginFill(0x000000,0.01);
         graphics.drawRect(0,0,w,H - 1);
         graphics.endFill();
         graphics.beginFill(0x1E2830,1);
         graphics.drawRect(0,H - 1,w,1);
         graphics.endFill();
         // the map's render by the rooms' pool (the level and weather of its id); a map the pool no
         // longer has: its mode's colour
         var entry:Object = RoomsWindow.me != null ? RoomsWindow.me.entry(int(row.map)) : null;
         var thumb:DisplayObject = null;
         if(entry != null)
         {
            thumb = look.mapRender(int(entry.level),int(entry.weather),THUMB_W,THUMB_H);
         }
         else
         {
            var plain:Shape = new Shape();
            plain.graphics.beginFill(RoomsLook.modeColour(int(row.mode)),0.35);
            plain.graphics.drawRect(0,0,THUMB_W,THUMB_H);
            plain.graphics.endFill();
            thumb = plain;
         }
         thumb.name = "srpWatchThumb";
         thumb.x = 8;
         thumb.y = int((H - 1 - THUMB_H) / 2);
         addChild(thumb);
      }

      // The card as `row` says now; `reason` is why it cannot be watched (a dictionary key), null:
      // Watch.
      public function show(row:Object, reason:String) : void
      {
         this.row = row;
         this._used = {};
         this.clock = null;
         var right:Number = this._w - 10;
         right = reason == null ? this.showWatch(right) : this.showWhy(reason,right);
         // line 1: the map, then its mode and teams or players
         var map:TextField = this.field(WatchWindow.mapName(row),TEXT_X,5,right - TEXT_X,16,RoomsLook.LIGHT,"srpWatchMap");
         var after:Number = TEXT_X + Math.min(map.width,map.textWidth + 4) + 8;
         this.field(this.modeLine(row),after,8,right - after,13,RoomsLook.DIM,"srpWatchMode");
         // line 2: the score or result
         this.secondLine(row,right);
         // line 3: age or watchers, the code, the friends
         this.thirdLine(row,right,reason == null);
         for(var name:String in this._fields)
         {
            if(this._used[name] !== true)
            {
               var tf:TextField = this._fields[name] as TextField;
               tf.text = "";
               tf.visible = false;
            }
         }
      }

      // Watch at the right; the x left of it.
      private function showWatch(right:Number) : Number
      {
         if(this._why != null && this._why.parent == this)
         {
            removeChild(this._why);
         }
         if(this._button == null)
         {
            this._button = this.button(LocalizationData.getName("st_srp_replay_watch"));
         }
         if(this._button.parent != this)
         {
            addChild(this._button);
         }
         this._button.x = int(right - this._button.width);
         this._button.y = int((H - 1 - this._button.height) / 2);
         return this._button.x - 14;
      }

      // Why not, at the right; the x left of it.
      private function showWhy(reason:String, right:Number) : Number
      {
         if(this._button != null && this._button.parent == this)
         {
            removeChild(this._button);
         }
         if(this._why == null)
         {
            this._why = this._look.text(this,"",right - REASON_W,0,REASON_W,13,RoomsLook.GREY,"right");
            this._why.name = "srpWatchReason";
            this._why.wordWrap = true;
            this._why.multiline = true;
         }
         if(this._why.parent != this)
         {
            addChild(this._why);
         }
         this._why.text = LocalizationData.getName(reason);
         this._why.height = this._why.textHeight + 6;
         this._why.y = int((H - 1 - this._why.height) / 2);
         return right - REASON_W - 14;
      }

      private function modeLine(row:Object) : String
      {
         var parts:Array = [RoomsText.modeName(int(row.mode))];
         if(row.kind == "live")
         {
            parts.push(row.a + " " + LocalizationData.getName("st_srp_matches_versus") + " " + row.b);
         }
         else if(row.kind == "recent")
         {
            parts.push(LocalizationData.format("st_srp_watch_players",row.players));
         }
         return WatchWindow.joined(parts);
      }

      private function secondLine(row:Object, right:Number) : void
      {
         var x:Number = TEXT_X;
         if(row.kind == "live")
         {
            if(String(row.score) != "")
            {
               x = this.next(this.field(String(row.score),x,25,right - x,15,RoomsLook.LIGHT,"srpWatchScore"));
            }
            var playing:Boolean = int(row.phase) == MatchPhase.PLAYING;
            x = this.next(this.field(playing ? LocalizationData.getName("st_srp_watch_live_word") : WatchWindow.phaseWord(row),x,26,right - x,13,
               playing ? RED : RoomsLook.GREY,"srpWatchState"));
            this.clock = this.field("",x,26,Math.max(10,right - x),13,RoomsLook.LIGHT,"srpWatchTime");
            return;
         }
         if(row.kind == "played")
         {
            x = this.next(this.field(LocalizationData.getName(RESULT_KEYS[int(row.result)]),x,25,right - x,15,
               int(row.result) == 1 ? GREEN : int(row.result) == 2 ? RoomsLook.LIGHT : RoomsLook.GREY,"srpWatchResult"));
            this.field(LocalizationData.format("st_srp_player_matches_kd",row.kills,row.deaths),x,27,right - x,13,RoomsLook.GREY,"srpWatchKd");
            return;
         }
         if(String(row.score) != "")
         {
            x = this.next(this.field(String(row.score),x,25,right - x,15,RoomsLook.LIGHT,"srpWatchScore"));
         }
         this.field(RoomsText.clock(int(row.length)),x,27,right - x,13,RoomsLook.GREY,"srpWatchLength");
      }

      private function thirdLine(row:Object, right:Number, saves:Boolean) : void
      {
         var x:Number = TEXT_X;
         if(row.kind == "live")
         {
            var note:String = int(row.spectators) > 0 ? LocalizationData.format("st_srp_live_spectators",row.spectators) : "";
            if(note != "")
            {
               x = this.next(this.field(note,x,45,right - x,12,RoomsLook.DIM,"srpWatchNote"));
            }
            this.showSave(false);
         }
         else
         {
            x = this.next(this.field(WatchWindow.ageText(int(row.minutes)),x,45,right - x,12,RoomsLook.DIM,"srpWatchAge"));
            x = this.next(this.codeField(row,x,right));
            if(saves)
            {
               x = this.saveIcon(uint(row.match),x,46,right) + 6;
            }
            else
            {
               this.showSave(false);
            }
         }
         if(this._friendMark != null)
         {
            this._friendMark.visible = row.friends.length > 0;
         }
         if(row.friends.length > 0)
         {
            var names:Array = [];
            for each(var friend:Object in row.friends)
            {
               names.push(friend.name);
            }
            // the friend's mark of the top menu's badge, then their names
            if(this._friendMark == null)
            {
               this._friendMark = RoomsLook.person(RoomsLook.YELLOW,11);
               this._friendMark.name = "srpWatchFriendMark";
               addChild(this._friendMark);
            }
            this._friendMark.visible = true;
            this._friendMark.x = x;
            this._friendMark.y = 49;
            x += this._friendMark.width + 5;
            this.field(LocalizationData.format("st_srp_watch_friends",names.join(", ")),x,45,Math.max(20,right - x),12,RoomsLook.YELLOW,"srpWatchFriends");
         }
      }

      // The x after a field and a gap.
      private function next(tf:TextField) : Number
      {
         return tf.x + Math.min(tf.width,tf.textWidth + 4) + 10;
      }

      // The match's code, selectable for Ctrl+C (the game's System.setClipboard takes nothing, a text
      // field's own Ctrl+C reaches the system's clipboard); typing changes nothing.
      private function codeField(row:Object, x:Number, right:Number) : TextField
      {
         var code:String = NotificationListScrollContent.matchNumberText(uint(row.match));
         var tf:TextField = this.field(code,x,45,right - x,12,RoomsLook.GREY,"srpWatchNumber");
         tf.width = Math.max(10,Math.min(tf.width,tf.textWidth + 6));
         if(tf.type != TextFieldType.INPUT)
         {
            tf.type = TextFieldType.INPUT;
            tf.selectable = true;
            tf.mouseEnabled = true;
            tf.addEventListener(Event.CHANGE,this.onCodeChange);
            tf.addEventListener(MouseEvent.MOUSE_DOWN,this.onCodePress);
            this._route(tf);
            this.routed.push(tf);
         }
         return tf;
      }

      private function onCodeChange(e:Event) : void
      {
         TextField(e.currentTarget).text = NotificationListScrollContent.matchNumberText(uint(this.row.match));
      }

      private function onCodePress(e:MouseEvent) : void
      {
         var tf:TextField = e.currentTarget as TextField;
         FocusManager.setFocus(tf);
         tf.setSelection(0,tf.length);
      }

      // An arrow into a tray: a press saves the replay (ReplayAsks). Saving: dim, with its word;
      // saved: gold, filled, with its word. Its x end.
      private function saveIcon(match:uint, x:Number, y:Number, right:Number) : Number
      {
         this.showSave(true);
         var state:String = ReplayAsks.stateOf(match);
         var icon:Sprite = this._saveIcon;
         icon.x = x;
         icon.y = y;
         if(state != this._saveDrawn)
         {
            this.paintSave(state);
         }
         if(state == ReplayAsks.SAVE)
         {
            return x + 12;
         }
         var word:TextField = this.field(LocalizationData.getName(state == ReplayAsks.SAVING ? "st_srp_replay_saving" : "st_srp_replay_saved"),x + 18,45,
            Math.max(10,right - x - 18),12,state == ReplayAsks.SAVING ? RoomsLook.DIM : RoomsLook.GOLD,"srpWatchSaveState");
         return this.next(word) - 10;
      }

      private function showSave(on:Boolean) : void
      {
         if(!on)
         {
            if(this._saveIcon != null && this._saveIcon.parent == this)
            {
               removeChild(this._saveIcon);
            }
            return;
         }
         if(this._saveIcon == null)
         {
            var icon:Sprite = new Sprite();
            icon.name = "srpWatchSave";
            icon.mouseChildren = false;
            icon.addEventListener(MouseEvent.CLICK,this.onSavePress);
            icon.addEventListener(ButtonEvent.CLICK,this.onSavePress);
            this._saveIcon = icon;
            this._saveDrawn = null;
            this._route(icon);
            this.routed.push(icon);
         }
         if(this._saveIcon.parent != this)
         {
            addChild(this._saveIcon);
         }
      }

      private function paintSave(state:String) : void
      {
         this._saveDrawn = state;
         var icon:Sprite = this._saveIcon;
         var colour:uint = state == ReplayAsks.SAVING ? RoomsLook.DIM : state == ReplayAsks.SAVED ? 0x17130A : RoomsLook.GREY;
         icon.graphics.clear();
         icon.graphics.beginFill(0x000000,0.01);
         icon.graphics.drawRect(-3,-3,18,18);
         icon.graphics.endFill();
         if(state == ReplayAsks.SAVED)
         {
            icon.graphics.beginFill(RoomsLook.GOLD,1);
            icon.graphics.drawRoundRect(-2,-2,16,16,4,4);
            icon.graphics.endFill();
         }
         icon.graphics.lineStyle(1,colour);
         icon.graphics.moveTo(6,0);
         icon.graphics.lineTo(6,7);
         icon.graphics.moveTo(3,4);
         icon.graphics.lineTo(6,7);
         icon.graphics.lineTo(9,4);
         icon.graphics.moveTo(1,8);
         icon.graphics.lineTo(1,11);
         icon.graphics.lineTo(11,11);
         icon.graphics.lineTo(11,8);
         icon.buttonMode = state == ReplayAsks.SAVE;
      }

      private function onSavePress(e:Event) : void
      {
         ReplayAsks.save(uint(this.row.match));
      }

      // A text cut with dots to its column, made once and placed anew.
      private function field(value:String, x:Number, y:Number, w:Number, size:Number, colour:uint, name:String) : TextField
      {
         var tf:TextField = this._fields[name] as TextField;
         if(tf == null)
         {
            tf = this._look.text(this,"",x,y,Math.max(1,w),size,colour);
            tf.name = name;
            this._fields[name] = tf;
         }
         tf.x = x;
         tf.y = y;
         tf.width = Math.max(1,w);
         tf.visible = true;
         RoomsLook.fitName(tf,value,"");
         if(value == "")
         {
            tf.text = "";
         }
         tf.textColor = colour;
         this._used[name] = true;
         return tf;
      }

      // The game's red framed button (the rooms window's main action), as wide as its label wants.
      private function button(label:String) : *
      {
         var b:* = null;
         try
         {
            b = new (getDefinitionByName("SrvButtonSkinnedRed") as Class)();
            if(b.textField == null)
            {
               b = null;
            }
         }
         catch(e:Error)
         {
            b = null;
         }
         if(b == null)
         {
            b = new (getDefinitionByName("SrvButtonSkinned") as Class)();
         }
         b.name = "srpWatchWatch";
         b.label = label;
         addChild(b);
         var w:Number = Math.max(BUTTON_W,WatchWindow.measure(this._look,label,14) + 36);
         try
         {
            b.setSize(w,32);
         }
         catch(e2:Error)
         {
            b.width = w;
            b.height = 32;
         }
         b.addEventListener(ButtonEvent.CLICK,this.onWatchPress);
         this._route(b);
         this.routed.push(b);
         return b;
      }

      private function onWatchPress(e:Event) : void
      {
         this._watch(this.row);
      }
   }
}
