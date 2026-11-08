package survarium.watch
{
   import flash.display.Bitmap;
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.MatchPhase;
   import survarium.controls.NotificationListScrollContent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsText;

   // One match of the Watch window's lists (WatchWindow), a row of the rooms list's look: the map's
   // render and name, the mode and what else the row tells under it, then columns of its state
   // (live: its phase and clock; else its number and age), its score or the player's result, the
   // friends in it; at the right the replay's chip and the buttons. The row (WatchRows' object,
   // `kind` live, recent or played) and what its buttons do come from the window.
   public class MatchCard extends Sprite
   {
      public static const H:int = 52;

      private static const THUMB_W:int = 72;

      private static const THUMB_H:int = 40;

      // the columns' x and widths, the friends' column to the chip
      private static const C1:int = 90;

      private static const C1_W:int = 290;

      private static const C2:int = 390;

      private static const C2_W:int = 150;

      private static const C3:int = 550;

      private static const C3_W:int = 150;

      private static const C4:int = 710;

      private static const GREEN:uint = 0x8FD18F;

      // a result's word by its number (0 defeat, 1 victory, 2 draw)
      private static const RESULT_KEYS:Array = ["defeat","victory","st_srp_player_matches_draw"];

      private var _look:RoomsLook;

      // the card's buttons and what a press on each does, for the window's routes
      public var presses:Array = [];

      // the clock's field of a live match being played, ticked by the window
      public var clock:TextField;

      // `chip`: the replay chip's dictionary key, null none. `actions`: [label, name, enabled,
      // handler] for each button, the rightmost first.
      public function MatchCard(look:RoomsLook, row:Object, w:Number, chip:String, actions:Array)
      {
         super();
         this._look = look;
         graphics.beginFill(0x000000,0.01);
         graphics.drawRect(0,0,w,H - 1);
         graphics.endFill();
         graphics.beginFill(0x1E2830,1);
         graphics.drawRect(0,H - 1,w,1);
         graphics.endFill();
         var thumb:Bitmap = look.mapRender(int(row.map),1,THUMB_W,THUMB_H);
         thumb.x = 8;
         thumb.y = int((H - 1 - THUMB_H) / 2);
         addChild(thumb);
         var right:Number = w - 8;
         for each(var action:Array in actions)
         {
            var b:* = this.button(action[0],action[1],action[3]);
            b.x = int(right - b.width);
            b.y = int((H - 1 - b.height) / 2);
            b.enabled = action[2];
            right = b.x - 8;
         }
         if(chip != null)
         {
            var box:Sprite = this.chip(chip);
            box.x = int(right - box.width);
            box.y = int((H - 1 - box.height) / 2);
            right = box.x - 10;
         }
         var map:TextField = this.field(RoomsText.levelName(int(row.map)),C1,7,C1_W,16,RoomsLook.LIGHT,"srpWatchMap");
         this.field(this.underMap(row),C1,28,C1_W,12,RoomsLook.DIM,"srpWatchMode");
         if(row.kind == "live")
         {
            var playing:Boolean = int(row.phase) == MatchPhase.PLAYING;
            this.field(playing ? LocalizationData.getName("st_srp_watch_live_word") : WatchWindow.phaseWord(row),C2,7,C2_W,playing ? 15 : 13,
               playing ? RoomsLook.GOLD : RoomsLook.GREY,"srpWatchState");
            this.clock = this.field("",C2,28,C2_W,12,RoomsLook.LIGHT,"srpWatchTime");
            this.field(String(row.score),C3,12,C3_W,17,RoomsLook.LIGHT,"srpWatchScore");
         }
         else
         {
            this.field(NotificationListScrollContent.matchNumberText(uint(row.match)),C2,8,C2_W,14,RoomsLook.GREY,"srpWatchNumber");
            this.field(WatchWindow.ageText(int(row.minutes)),C2,28,C2_W,12,RoomsLook.DIM,"srpWatchAge");
            if(row.kind == "recent")
            {
               this.field(String(row.score),C3,12,C3_W,17,RoomsLook.LIGHT,"srpWatchScore");
            }
            else
            {
               this.field(LocalizationData.getName(RESULT_KEYS[int(row.result)]),C3,7,C3_W,15,
                  int(row.result) == 1 ? GREEN : int(row.result) == 2 ? RoomsLook.LIGHT : RoomsLook.GREY,"srpWatchResult");
               this.field(LocalizationData.format("st_srp_player_matches_kd",row.kills,row.deaths),C3,28,C3_W,12,RoomsLook.DIM,"srpWatchKd");
            }
         }
         var room:Number = Math.max(20,right - C4);
         if(row.friends.length > 0)
         {
            var names:Array = [];
            for each(var friend:Object in row.friends)
            {
               names.push(friend.name);
            }
            this.field(LocalizationData.format("st_srp_watch_friends",names.join(", ")),C4,7,room,13,RoomsLook.YELLOW,"srpWatchFriends");
         }
         if(row.kind == "live")
         {
            this.field(WatchWindow.liveNote(row),C4,row.friends.length > 0 ? 28 : 16,room,12,RoomsLook.DIM,"srpWatchNote");
         }
      }

      // The line under the map: the mode, and the teams of a live match or the players and length
      // of a recent one.
      private function underMap(row:Object) : String
      {
         var parts:Array = [RoomsText.modeName(int(row.mode))];
         if(row.kind == "live")
         {
            parts.push(row.a + " " + LocalizationData.getName("st_srp_matches_versus") + " " + row.b);
         }
         else if(row.kind == "recent")
         {
            parts.push(LocalizationData.format("st_srp_watch_players",row.players),RoomsText.clock(int(row.length)));
         }
         return parts.filter(function(part:String, i:int, all:Array):Boolean
         {
            return part != null && part != "";
         }).join("  ·  ");
      }

      // A text cut with dots to its column.
      private function field(value:String, x:Number, y:Number, w:Number, size:Number, colour:uint, name:String) : TextField
      {
         var tf:TextField = this._look.text(this,"",x,y,w,size,colour);
         tf.name = name;
         RoomsLook.fitName(tf,value,"");
         if(value == "")
         {
            tf.text = "";
         }
         return tf;
      }

      // The replay's chip, as the History rows': its word in a rounded box of its colour.
      private function chip(key:String) : Sprite
      {
         var colour:uint = NotificationListScrollContent.chipColour(key);
         var box:Sprite = new Sprite();
         box.name = "srpWatchChip";
         box.mouseEnabled = false;
         box.mouseChildren = false;
         var tf:TextField = this._look.text(box,LocalizationData.getName(key),7,2,200,12,colour);
         tf.width = int(tf.textWidth + 6);
         box.graphics.lineStyle(1,colour,0.9);
         box.graphics.beginFill(colour,0.15);
         box.graphics.drawRoundRect(0,0,int(tf.width + 14),24,8,8);
         box.graphics.endFill();
         addChild(box);
         return box;
      }

      // The game's framed button (the rooms window's), as wide as its label wants.
      private function button(label:String, name:String, handler:Function) : *
      {
         var b:* = new (getDefinitionByName("SrvButtonSkinned") as Class)();
         b.name = name;
         b.label = label;
         addChild(b);
         var w:Number = Math.max(110,WatchWindow.measure(this._look,label,14) + 36);
         try
         {
            b.setSize(w,30);
         }
         catch(e:Error)
         {
            b.width = w;
            b.height = 30;
         }
         b.addEventListener(ButtonEvent.CLICK,handler);
         this.presses.push(b);
         return b;
      }
   }
}
