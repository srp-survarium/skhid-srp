package survarium.upgrade
{
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;

   // A slot's list beside its marker, 0.69d0's ModuleSelector: the slot's name, then a row a choice
   // (what the gun has there of its own or nothing, the carriers that go there, the inventory's
   // modules that go there), the one standing there marked. A click on a row is the choice: the list
   // dispatches Event.SELECT with `chosen` its entry; the view acts on it and closes the list.
   public class ModuleList extends Sprite
   {

      public static const WIDTH:Number = 250;

      protected static const ROW:Number = 34;

      protected static const HEAD:Number = 28;

      protected static const PAD:Number = 6;

      protected static const ICON_ROOM:Number = 28;

      protected static const LIGHT:uint = 0xE7F1EE;

      protected static const GREY:uint = 0xB4BEC4;

      protected static const BACK:uint = 0x0B1014;

      protected static const OPEN:uint = 0xF2C14E;

      // the entry clicked: {label, icon, standing, ...} as given
      public var chosen:Object;

      protected var _rows:Array = [];

      // each row's entry
      protected var _entries:Dictionary = new Dictionary();

      public function ModuleList()
      {
         super();
         visible = false;
      }

      protected static function label(size:int, colour:uint) : TextField
      {
         var field:TextField = new TextField();
         var format:TextFormat = new TextFormat("Calibri",size,colour,true);
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = format;
         return field;
      }

      // The list of a slot: its title and its entries ({label, icon: a display object or null,
      // standing: whether it stands there now}).
      public function setEntries(title:String, entries:Array) : void
      {
         var heading:TextField = label(14,LIGHT);
         var entry:Object = null;
         var row:Sprite = null;
         var i:int = 0;
         this.clear();
         heading.text = title;
         heading.x = PAD;
         heading.y = (HEAD - heading.height) / 2;
         addChild(heading);
         graphics.clear();
         graphics.lineStyle(1,LIGHT,0.65);
         graphics.beginFill(BACK,0.88);
         graphics.drawRect(0,0,WIDTH,HEAD + entries.length * ROW + PAD);
         graphics.endFill();
         graphics.lineStyle(1,LIGHT,0.25);
         graphics.moveTo(PAD,HEAD);
         graphics.lineTo(WIDTH - PAD,HEAD);
         while(i < entries.length)
         {
            entry = entries[i];
            row = this.makeRow(entry);
            row.y = HEAD + i * ROW;
            addChild(row);
            this._rows.push(row);
            i++;
         }
      }

      protected function makeRow(entry:Object) : Sprite
      {
         var row:Sprite = new Sprite();
         var face:Shape = new Shape();
         var text:TextField = label(13,Boolean(entry.standing) ? OPEN : GREY);
         var icon:DisplayObject = entry.icon as DisplayObject;
         var scale:Number = 1;
         var bounds:Rectangle = null;
         row.buttonMode = true;
         row.mouseChildren = false;
         face.name = "face";
         row.addChild(face);
         this.light(row,false);
         if(Boolean(icon))
         {
            icon.scaleX = icon.scaleY = 1;
            bounds = icon.getBounds(icon);
            scale = Math.min(1,ICON_ROOM / Math.max(1,bounds.width),ICON_ROOM / Math.max(1,bounds.height));
            icon.scaleX = icon.scaleY = scale;
            icon.x = PAD + ICON_ROOM / 2 - (bounds.x + bounds.width / 2) * scale;
            icon.y = ROW / 2 - (bounds.y + bounds.height / 2) * scale;
            row.addChild(icon);
         }
         text.text = String(entry.label);
         text.x = PAD * 2 + ICON_ROOM;
         text.y = (ROW - text.height) / 2;
         row.addChild(text);
         row.addEventListener(MouseEvent.MOUSE_OVER,this.onRowOver);
         row.addEventListener(MouseEvent.MOUSE_OUT,this.onRowOut);
         row.addEventListener(MouseEvent.CLICK,this.onRowClick);
         this._entries[row] = entry;
         return row;
      }

      protected function light(row:Sprite, over:Boolean) : void
      {
         var face:Shape = row.getChildByName("face") as Shape;
         face.graphics.clear();
         face.graphics.beginFill(over ? LIGHT : BACK,over ? 0.12 : 0.01);
         face.graphics.drawRect(1,1,WIDTH - 2,ROW - 2);
         face.graphics.endFill();
      }

      protected function onRowOver(event:MouseEvent) : void
      {
         this.light(event.currentTarget as Sprite,true);
      }

      protected function onRowOut(event:MouseEvent) : void
      {
         this.light(event.currentTarget as Sprite,false);
      }

      protected function onRowClick(event:MouseEvent) : void
      {
         this.chosen = this._entries[event.currentTarget];
         dispatchEvent(new Event(Event.SELECT));
      }

      // The list away: its rows gone.
      public function clear() : void
      {
         for each(var row:Sprite in this._rows)
         {
            row.removeEventListener(MouseEvent.MOUSE_OVER,this.onRowOver);
            row.removeEventListener(MouseEvent.MOUSE_OUT,this.onRowOut);
            row.removeEventListener(MouseEvent.CLICK,this.onRowClick);
         }
         this._rows = [];
         this._entries = new Dictionary();
         this.chosen = null;
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         graphics.clear();
      }
   }
}
