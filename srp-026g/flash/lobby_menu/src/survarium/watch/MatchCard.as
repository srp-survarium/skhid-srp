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
   // its code with a copy icon and the friends in it) and at the right one Watch, or why it cannot
   // be watched. The row (WatchRows' object, `kind` live, recent or played) and what Watch and the
   // copy icon do come from the window.
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

      // the card's controls and what a press on each does, for the window's routes
      public var presses:Array = [];

      // the clock's field of a live match being played, ticked by the window
      public var clock:TextField;

      // the code's field shown selected when the game's clipboard refused it (WatchWindow.copied)
      public var codeField:TextField;

      // `reason`: why it cannot be watched (a dictionary key), null: Watch. `copied`: the copy
      // icon's last press on this card, null none ({ok}).
      public function MatchCard(look:RoomsLook, row:Object, w:Number, reason:String, copied:Object, watch:Function, copy:Function)
      {
         super();
         this._look = look;
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
         var right:Number = w - 10;
         if(reason == null)
         {
            var b:* = this.button(LocalizationData.getName("st_srp_replay_watch"),watch);
            b.x = int(right - b.width);
            b.y = int((H - 1 - b.height) / 2);
            right = b.x - 14;
         }
         else
         {
            var why:TextField = look.text(this,"",right - REASON_W,0,REASON_W,13,RoomsLook.GREY,"right");
            why.name = "srpWatchReason";
            why.wordWrap = true;
            why.multiline = true;
            why.text = LocalizationData.getName(reason);
            why.height = why.textHeight + 6;
            why.y = int((H - 1 - why.height) / 2);
            right = right - REASON_W - 14;
         }
         // line 1: the map, then its mode and teams or players
         var map:TextField = this.field(WatchWindow.mapName(row),TEXT_X,5,right - TEXT_X,16,RoomsLook.LIGHT,"srpWatchMap");
         var after:Number = TEXT_X + Math.min(map.width,map.textWidth + 4) + 8;
         this.field(this.modeLine(row),after,8,right - after,13,RoomsLook.DIM,"srpWatchMode");
         // line 2: the score or result
         this.secondLine(row,right);
         // line 3: age or watchers, the code and its copy icon, the friends
         this.thirdLine(row,right,copied,copy);
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

      private function thirdLine(row:Object, right:Number, copied:Object, copy:Function) : void
      {
         var x:Number = TEXT_X;
         if(row.kind == "live")
         {
            var note:String = int(row.spectators) > 0 ? LocalizationData.format("st_srp_live_spectators",row.spectators) : "";
            if(note != "")
            {
               x = this.next(this.field(note,x,45,right - x,12,RoomsLook.DIM,"srpWatchNote"));
            }
         }
         else
         {
            x = this.next(this.field(WatchWindow.ageText(int(row.minutes)),x,45,right - x,12,RoomsLook.DIM,"srpWatchAge"));
            var code:String = NotificationListScrollContent.matchNumberText(uint(row.match));
            x = this.next(this.field(code,x,45,right - x,12,RoomsLook.GREY,"srpWatchNumber")) - 4;
            x = this.copyIcon(x,46,copy) + 6;
            if(copied != null)
            {
               x = this.copiedNote(code,copied.ok,x,right);
            }
         }
         if(row.friends.length > 0)
         {
            var names:Array = [];
            for each(var friend:Object in row.friends)
            {
               names.push(friend.name);
            }
            this.field(LocalizationData.format("st_srp_watch_friends",names.join(", ")),x,45,Math.max(20,right - x),12,RoomsLook.YELLOW,"srpWatchFriends");
         }
      }

      // The x after a field and a gap.
      private function next(tf:TextField) : Number
      {
         return tf.x + Math.min(tf.width,tf.textWidth + 4) + 10;
      }

      // Two overlapping sheets, a press copies the code; its x end.
      private function copyIcon(x:Number, y:Number, copy:Function) : Number
      {
         var icon:Sprite = new Sprite();
         icon.name = "srpWatchCopy";
         icon.x = x;
         icon.y = y;
         icon.graphics.beginFill(0x000000,0.01);
         icon.graphics.drawRect(-3,-3,18,18);
         icon.graphics.endFill();
         icon.graphics.lineStyle(1,RoomsLook.GREY);
         icon.graphics.drawRect(0,3,8,9);
         icon.graphics.beginFill(0x131517,1);
         icon.graphics.drawRect(4,0,8,9);
         icon.graphics.endFill();
         icon.buttonMode = true;
         icon.mouseChildren = false;
         var press:Function = function(e:Event):void
         {
            copy();
         };
         // the game's press, or the window's route of one (WatchWindow.routeInList)
         icon.addEventListener(MouseEvent.CLICK,press);
         icon.addEventListener(ButtonEvent.CLICK,press);
         addChild(icon);
         this.presses.push(icon);
         return x + 12;
      }

      // After a copy: "Copied", or, when the game's clipboard took nothing, the code in a field
      // selected for Ctrl+C and the keys to press. Its x end.
      private function copiedNote(code:String, ok:Boolean, x:Number, right:Number) : Number
      {
         if(ok)
         {
            return this.next(this.field(LocalizationData.getName("st_srp_watch_copied"),x,45,right - x,12,GREEN,"srpWatchCopied"));
         }
         var tf:TextField = this._look.text(this,code,x,43,82,12,RoomsLook.GOLD);
         tf.name = "srpWatchCodeField";
         tf.height = 18;
         tf.background = true;
         tf.backgroundColor = 0x070A0D;
         tf.border = true;
         tf.borderColor = 0x3A4854;
         tf.type = TextFieldType.INPUT;
         tf.selectable = true;
         tf.mouseEnabled = true;
         tf.addEventListener(MouseEvent.MOUSE_DOWN,function(e:MouseEvent):void
         {
            FocusManager.setFocus(e.currentTarget as TextField);
         });
         this.codeField = tf;
         return this.next(this.field(LocalizationData.getName("st_srp_watch_copy_keys"),x + 90,45,right - x - 90,12,RoomsLook.GREY,"srpWatchCopyKeys"));
      }

      // A text cut with dots to its column.
      private function field(value:String, x:Number, y:Number, w:Number, size:Number, colour:uint, name:String) : TextField
      {
         var tf:TextField = this._look.text(this,"",x,y,Math.max(1,w),size,colour);
         tf.name = name;
         RoomsLook.fitName(tf,value,"");
         if(value == "")
         {
            tf.text = "";
         }
         return tf;
      }

      // The game's red framed button (the rooms window's main action), as wide as its label wants.
      private function button(label:String, handler:Function) : *
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
         b.addEventListener(ButtonEvent.CLICK,handler);
         this.presses.push(b);
         return b;
      }
   }
}
