package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.clearTimeout;
   import flash.utils.setTimeout;
   import scaleform.gfx.MouseEventEx;
   import survarium.data.LocalizationData;

   // A player's menu on the Tab list (docs/notes/tab-cursor-friends.md): a right-click on a row while the
   // list's cursor is shown (Ctrl, exe patch 86) asks the exe what the row offers
   // (srp_player_menu(place), answered root.srp_player_menu -> offered), and shows those entries at the
   // mouse. An entry is {key: its text's dictionary key, method: the exe's call with the row's place}; the
   // exe answers each (root.srp_friend_request -> requested) and the menu shows the answer's line until
   // NOTE_MS pass. The list hidden (Tab let go) or a press elsewhere closes it.
   public class PlayerRowMenu extends Sprite
   {
      // The menu's states: closed; asking the exe for a row's entries; open; an entry sent, awaiting its
      // answer; showing the answer's line.
      public static const CLOSED:String = "closed";

      public static const ASKING:String = "asking";

      public static const OPEN:String = "open";

      public static const SENDING:String = "sending";

      public static const TOLD:String = "told";

      public static const ADD_FRIEND:Object = {
         key:"st_srp_hud_add_friend",
         method:"srp_add_friend"
      };

      private static const NOTE_MS:uint = 2000;

      private static const ROW:Number = 24;

      private static const PAD:Number = 8;

      private static const MIN_WIDTH:Number = 140;

      // the exe's calls (ExternalInterface.call; a harness puts its own)
      public var call:Function = ExternalInterface.call;

      private var _list:PlayerList;

      private var _state:String = CLOSED;

      // the row the menu is for (its place, `id`), in every state but CLOSED
      private var _row:Object;

      private var _noteTimeout:uint;

      private var _format:TextFormat;

      private var _embed:Boolean;

      public function PlayerRowMenu(list:PlayerList)
      {
         super();
         this._list = list;
         // the team header's embedded font: it shows the dictionaries' texts of every language
         var source:TextField = list.team1 ? list.team1 : new DefaultLabel3().textField;
         this._embed = source.embedFonts;
         this._format = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         this._format.size = 15;
         this._format.color = 0xFFFFFF;
         this._format.align = "left";
         // on the way down (capture): the row renderer is the click's target
         list.team1list.addEventListener(MouseEvent.CLICK,this.onRowClick,true);
         list.team2list.addEventListener(MouseEvent.CLICK,this.onRowClick,true);
         visible = false;
      }

      public function get state() : String
      {
         return this._state;
      }

      // A click on a row: the right button opens the row's menu (not the player's own row).
      private function onRowClick(e:MouseEvent) : void
      {
         var button:MouseEventEx = e as MouseEventEx;
         if(button == null || button.buttonIdx != MouseEventEx.RIGHT_BUTTON)
         {
            return;
         }
         var row:Object = rowAt(e.target as DisplayObject);
         if(row == null || row.isLocal || row.id == undefined)
         {
            return;
         }
         this.ask(row,parent ? parent.mouseX : 0,parent ? parent.mouseY : 0);
      }

      private static function rowAt(target:DisplayObject) : Object
      {
         while(target != null && !(target is survarium.controls.PlayerListRenderer))
         {
            target = target.parent;
         }
         return target ? survarium.controls.PlayerListRenderer(target).data : null;
      }

      // Asks the exe what `row` offers; the menu opens at (x, y) of its parent when it answers.
      public function ask(row:Object, atX:Number, atY:Number) : void
      {
         this.close();
         this._row = row;
         this._state = ASKING;
         x = atX;
         y = atY;
         this.call("srp_player_menu",String(row.id));
      }

      // The exe's answer to srp_player_menu: what the row at `place` offers.
      public function offered(place:uint, addFriend:Boolean) : void
      {
         if(this._state != ASKING || uint(this._row.id) != place)
         {
            return;
         }
         var entries:Array = [];
         if(addFriend)
         {
            entries.push(ADD_FRIEND);
         }
         if(entries.length == 0)
         {
            this.close();
            return;
         }
         this.draw(entries);
         this._state = OPEN;
         this.listen(true);
      }

      // The exe's answer to srp_add_friend: the request sent or not.
      public function requested(place:uint, sent:Boolean) : void
      {
         if(this._state != SENDING || uint(this._row.id) != place)
         {
            return;
         }
         this.draw([{key:sent ? "st_srp_hud_friend_sent" : "st_srp_hud_friend_not_sent"}]);
         this._state = TOLD;
         this._noteTimeout = setTimeout(this.close,NOTE_MS);
      }

      public function close() : void
      {
         clearTimeout(this._noteTimeout);
         this.listen(false);
         this._state = CLOSED;
         this._row = null;
         visible = false;
         this.clear();
      }

      private function clear() : void
      {
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         graphics.clear();
      }

      private function choose(entry:Object) : void
      {
         if(this._state != OPEN)
         {
            return;
         }
         this._state = SENDING;
         this.call(entry.method,String(this._row.id));
      }

      // The entries (an entry without a method is a line of text), one row each.
      private function draw(entries:Array) : void
      {
         this.clear();
         var width:Number = MIN_WIDTH;
         var fields:Array = [];
         for each(var entry:Object in entries)
         {
            var field:TextField = new TextField();
            field.defaultTextFormat = this._format;
            field.embedFonts = this._embed;
            field.selectable = false;
            field.mouseEnabled = false;
            field.autoSize = TextFieldAutoSize.LEFT;
            field.text = LocalizationData.getName(entry.key);
            width = Math.max(width,field.textWidth + 2 * PAD + 4);
            fields.push(field);
         }
         graphics.lineStyle(1,0x5A5A5A);
         graphics.beginFill(0x141414,0.92);
         graphics.drawRect(0,0,width,ROW * entries.length + 4);
         graphics.endFill();
         for(var i:int = 0; i < entries.length; i++)
         {
            var item:Sprite = new Sprite();
            item.y = 2 + ROW * i;
            this.paint(item,width,false);
            fields[i].x = PAD;
            fields[i].y = (ROW - fields[i].height) / 2;
            item.addChild(fields[i]);
            if(entries[i].method)
            {
               this.makeChoice(item,width,entries[i]);
            }
            else
            {
               item.mouseEnabled = false;
            }
            addChild(item);
         }
         visible = true;
      }

      private function makeChoice(item:Sprite, width:Number, entry:Object) : void
      {
         item.buttonMode = true;
         item.addEventListener(MouseEvent.MOUSE_OVER,function(e:MouseEvent):void
         {
            paint(item,width,true);
         });
         item.addEventListener(MouseEvent.MOUSE_OUT,function(e:MouseEvent):void
         {
            paint(item,width,false);
         });
         item.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
         {
            var button:MouseEventEx = e as MouseEventEx;
            if(button == null || button.buttonIdx == MouseEventEx.LEFT_BUTTON)
            {
               choose(entry);
            }
         });
      }

      // A row's hit area (alpha 0.01: alpha 0 takes no click), lit under the mouse.
      private function paint(item:Sprite, width:Number, lit:Boolean) : void
      {
         item.graphics.clear();
         item.graphics.beginFill(lit ? 0x3C3C3C : 0x141414,lit ? 1 : 0.01);
         item.graphics.drawRect(1,0,width - 2,ROW);
         item.graphics.endFill();
      }

      // While open, a press anywhere else closes the menu.
      private function listen(on:Boolean) : void
      {
         if(stage == null)
         {
            return;
         }
         if(on)
         {
            stage.addEventListener(MouseEvent.MOUSE_DOWN,this.onStageDown);
         }
         else
         {
            stage.removeEventListener(MouseEvent.MOUSE_DOWN,this.onStageDown);
         }
      }

      private function onStageDown(e:MouseEvent) : void
      {
         var target:DisplayObject = e.target as DisplayObject;
         if(this._state == OPEN && !(target != null && contains(target)))
         {
            this.close();
         }
      }
   }
}
