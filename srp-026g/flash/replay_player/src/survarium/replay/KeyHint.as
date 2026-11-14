package survarium.replay
{
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.constants.Colors;

   // srp's: a replay viewer's keys on a dark panel, each key a cap (a small box with a light border)
   // and its action beside it: [Ctrl] Cursor  [Space] Pause  [Shift]+[<][>] Moments  [M] Filter
   // [H] Hide  [Esc] Back. The arrows are drawn (the font may have no arrow glyphs). The actions and
   // the Ctrl and Space caps are the exe's dictionary texts; the groups wrap onto more rows rather than
   // leave `width`. It takes no mouse.
   public class KeyHint extends Sprite
   {
      public static const LEFT:String = "<";

      public static const RIGHT:String = ">";

      private static const PAD_X:Number = 12;

      private static const PAD_Y:Number = 8;

      private static const GROUP_GAP:Number = 18;

      private static const ROW_GAP:Number = 6;

      private static const CAP_H:Number = 20;

      private var _font:String;

      private var _panel:Shape = new Shape();

      // each group: a Sprite of its caps and its action
      public var groups:Array = [];

      private var _width:Number = 0;

      // `texts`: the actions Cursor, Pause, Moments, Filter, Hide, Back, then the caps Ctrl and Space.
      public function KeyHint(font:String, texts:Array)
      {
         super();
         name = "srpKeyHint";
         mouseEnabled = mouseChildren = false;
         this._font = font;
         addChild(this._panel);
         var keys:Array = [[texts[6]],[texts[7]],["Shift","+",LEFT,RIGHT],["M"],["H"],["Esc"]];
         for(var i:int = 0; i < keys.length; i++)
         {
            var group:Sprite = this.group(keys[i],texts[i]);
            group.name = "srpKeyGroup" + i;
            this.groups.push(group);
            addChild(group);
         }
      }

      // The groups in rows no wider than `width` (the panel's padding in it), on the panel.
      public function layout(width:Number) : void
      {
         if(width == this._width)
         {
            return;
         }
         this._width = width;
         var room:Number = width - 2 * PAD_X;
         var rows:Array = [[]];
         var used:Number = 0;
         for each(var group:Sprite in this.groups)
         {
            var row:Array = rows[rows.length - 1];
            var need:Number = (row.length > 0 ? GROUP_GAP : 0) + group.width;
            if(row.length > 0 && used + need > room)
            {
               rows.push([group]);
               used = group.width;
            }
            else
            {
               row.push(group);
               used += need;
            }
         }
         var widest:Number = 0;
         var y:Number = PAD_Y;
         for each(row in rows)
         {
            var x:Number = 0;
            for each(group in row)
            {
               group.x = x;
               group.y = y;
               x += group.width + GROUP_GAP;
            }
            widest = Math.max(widest,x - GROUP_GAP);
            y += CAP_H + ROW_GAP;
         }
         // each row centred on the widest
         for each(row in rows)
         {
            var last:Sprite = row[row.length - 1];
            var shift:Number = PAD_X + int((widest - (last.x + last.width)) / 2);
            for each(group in row)
            {
               group.x += shift;
            }
         }
         this._panel.graphics.clear();
         this._panel.graphics.beginFill(0x000000,0.6);
         this._panel.graphics.drawRoundRect(0,0,widest + 2 * PAD_X,y - ROW_GAP + PAD_Y,8,8);
         this._panel.graphics.endFill();
      }


      private function group(caps:Array, action:String) : Sprite
      {
         var group:Sprite = new Sprite();
         var x:Number = 0;
         for each(var cap:String in caps)
         {
            var part:Sprite = cap == "+" ? this.plus() : this.cap(cap);
            part.x = x;
            group.addChild(part);
            x += part.width + 3;
         }
         var label:TextField = this.field(action,13,Colors.LIGHT_GRAY,false);
         label.name = "action";
         label.x = x + 3;
         label.y = int((CAP_H - label.height) / 2);
         group.addChild(label);
         return group;
      }

      // A key cap: its text, or a drawn arrow for LEFT and RIGHT.
      private function cap(text:String) : Sprite
      {
         var box:Sprite = new Sprite();
         var inner:Number;
         if(text == LEFT || text == RIGHT)
         {
            inner = 10;
            var arrow:Shape = new Shape();
            var mid:Number = CAP_H / 2;
            arrow.graphics.beginFill(Colors.WHITE,1);
            if(text == LEFT)
            {
               arrow.graphics.moveTo(6 + 8,mid - 5);
               arrow.graphics.lineTo(6 + 8,mid + 5);
               arrow.graphics.lineTo(6 + 1,mid);
            }
            else
            {
               arrow.graphics.moveTo(6 + 2,mid - 5);
               arrow.graphics.lineTo(6 + 2,mid + 5);
               arrow.graphics.lineTo(6 + 9,mid);
            }
            arrow.graphics.endFill();
            box.addChild(arrow);
         }
         else
         {
            var label:TextField = this.field(text,12,Colors.WHITE,true);
            label.x = 6;
            label.y = int((CAP_H - label.height) / 2);
            inner = label.width;
            box.addChild(label);
         }
         box.graphics.lineStyle(1,0xB4B9BE,1);
         box.graphics.beginFill(0x1C2126,0.95);
         box.graphics.drawRoundRect(0,0,int(inner + 12),CAP_H,4,4);
         box.graphics.endFill();
         return box;
      }

      private function plus() : Sprite
      {
         var box:Sprite = new Sprite();
         var label:TextField = this.field("+",13,Colors.LIGHT_GRAY,true);
         label.y = int((CAP_H - label.height) / 2);
         box.addChild(label);
         return box;
      }

      private function field(text:String, size:int, colour:uint, bold:Boolean) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = new TextFormat(this._font,size,colour,bold);
         field.text = text;
         return field;
      }
   }
}
