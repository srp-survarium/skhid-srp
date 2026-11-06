package survarium.player
{
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.utils.ConstrainedElement;

   // The greyed Play's and Join's tip (presets.md, the reasons), and the
   // item search's (item-search.md, InventoryList.makeSearch): the game's
   // tooltip (SrvProfileTipRendererStyled, as Autobuy's).
   // - tipData.name: its text a line a reason, with the left margin as wide
   //   as the right (the owner's of 2026-10-23: "a bit more padding from the
   //   left"). The stock renderer pads the right by 5 more than its margins
   //   and takes one line only.
   // - tipData.rows ([left, right] each): two columns, the right one's lines
   //   all starting at one x, the rows evenly spaced, ROW_PAD round them on
   //   every side (the owner's of 2026-11-06).
   // - anchor: the tip just above it, their left edges together, where the
   //   pointer is aside; null: at the pointer, as the game's tips.
   public class ReasonsTip extends ToolTip
   {

      public var anchor:DisplayObject;

      // the rows' padding on every side, the gap between the columns and
      // between the rows, and the tip's gap above the anchor
      private static const ROW_PAD:int = 8;

      private static const COLUMN_GAP:int = 16;

      private static const ROW_GAP:int = 3;

      private static const ANCHOR_GAP:int = 4;

      public function ReasonsTip()
      {
         super();
      }

      override protected function showTip(e:Event = null) : void
      {
         super.showTip(e);
         try
         {
            if(tipData != null && tipData.rows is Array)
            {
               this.fitRows(_tip_content,tipData.rows as Array);
            }
            else
            {
               this.fit(_tip_content);
            }
            if(this.anchor != null && _tip_content != null && _tip_content.parent != null)
            {
               var at:Rectangle = this.anchor.getBounds(_tip_content.parent);
               _tip_content.x = Math.round(at.left);
               _tip_content.y = Math.max(4,Math.round(at.top - _tip_content.height - ANCHOR_GAP));
            }
         }
         catch(err:Error)
         {
         }
      }

      // The renderer's margins (its Constraints element, the stock layout's
      // own setting): the left one set so the text's gap on the left is the
      // one on the right (the renderer adds 5 on the right, the field's
      // gutter is 2 a side); every line shown; its size as the renderer
      // works it out.
      private function fit(renderer:Object) : void
      {
         if(renderer == null || renderer.textField == null || renderer.constraints == null)
         {
            return;
         }
         var field:TextField = renderer.textField as TextField;
         var element:ConstrainedElement = renderer.constraints.getElement("textField");
         if(field == null || element == null)
         {
            return;
         }
         element.left = Math.max(element.left,element.right + 1);
         field.multiline = true;
         field.wordWrap = false;
         field.text = String(tipData != null ? tipData.name : "");
         var w:Number = Math.ceil(field.textWidth + element.left + element.right + 5);
         var h:Number = Math.ceil(field.textHeight + element.top + element.bottom + 5);
         renderer.width = w;
         renderer.height = h;
         renderer.constraints.update(w,h);
      }

      // Two columns in the renderer, each cell a text field of the renderer's
      // own font: the left ones as wide as the widest, the right ones from
      // COLUMN_GAP after it. The renderer scales its art to its size and keeps
      // its field unscaled (Constraints: COUNTER_SCALE): the cells' holder is
      // scaled as that field, so the cells are unscaled too.
      private function fitRows(renderer:Object, rows:Array) : void
      {
         if(renderer == null || renderer.textField == null || renderer.constraints == null)
         {
            return;
         }
         var field:TextField = renderer.textField as TextField;
         field.text = "M";
         var format:TextFormat = field.getTextFormat(0,1);
         field.text = "";
         field.visible = false;
         var cells:Sprite = new Sprite();
         var left:Array = [];
         var right:Array = [];
         var leftWidth:Number = 0;
         var rightWidth:Number = 0;
         var line:Number = 0;
         for each(var row:Array in rows)
         {
            left.push(this.cell(field,format,String(row[0])));
            right.push(this.cell(field,format,String(row[1])));
            leftWidth = Math.max(leftWidth,left[left.length - 1].textWidth);
            rightWidth = Math.max(rightWidth,right[right.length - 1].textWidth);
            line = Math.max(line,left[left.length - 1].textHeight,right[right.length - 1].textHeight);
         }
         // a field's text starts 2 in from its edge (its gutter)
         for(var i:int = 0; i < rows.length; i++)
         {
            var y:Number = ROW_PAD + i * (line + ROW_GAP) - 2;
            left[i].x = ROW_PAD - 2;
            left[i].y = y;
            right[i].x = ROW_PAD + leftWidth + COLUMN_GAP - 2;
            right[i].y = y;
            cells.addChild(left[i]);
            cells.addChild(right[i]);
         }
         var w:Number = Math.ceil(ROW_PAD * 2 + leftWidth + COLUMN_GAP + rightWidth);
         var h:Number = Math.ceil(ROW_PAD * 2 + rows.length * line + (rows.length - 1) * ROW_GAP);
         renderer.width = w;
         renderer.height = h;
         renderer.constraints.update(w,h);
         cells.scaleX = field.scaleX;
         cells.scaleY = field.scaleY;
         renderer.addChild(cells);
         // the rows centred in the art as it is drawn (its bounds need not start at the
         // renderer's origin): the same padding on every side
         var place:DisplayObject = (renderer.parent != null ? renderer.parent : renderer) as DisplayObject;
         var art:Rectangle = renderer.getBounds(place);
         var text:Rectangle = cells.getBounds(place);
         text.inflate(-2,-2);
         var from:Point = renderer.globalToLocal(place.localToGlobal(new Point(0,0)));
         var to:Point = renderer.globalToLocal(place.localToGlobal(new Point((art.left + art.right - text.left - text.right) / 2,(art.top + art.bottom - text.top - text.bottom) / 2)));
         cells.x += to.x - from.x;
         cells.y += to.y - from.y;
      }

      private function cell(field:TextField, format:TextFormat, text:String) : TextField
      {
         var cell:TextField = new TextField();
         cell.embedFonts = field.embedFonts;
         cell.antiAliasType = field.antiAliasType;
         cell.defaultTextFormat = format;
         cell.selectable = false;
         cell.mouseEnabled = false;
         cell.multiline = false;
         cell.wordWrap = false;
         cell.autoSize = TextFieldAutoSize.LEFT;
         cell.text = text;
         return cell;
      }
   }
}
