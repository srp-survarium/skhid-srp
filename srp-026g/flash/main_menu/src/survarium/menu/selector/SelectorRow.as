package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;

   // A row of the selector's list, `rowWidth` wide and `rowHeight` tall: a choice (ActionRow,
   // UnbindRow) or a group's heading (GroupHeading). `highlight` draws it as the one the
   // keys and the mouse are on.
   public class SelectorRow extends Sprite
   {

      public var rowWidth:Number;

      public var rowHeight:Number;

      protected var back:Shape = new Shape();
      
      // the row's own marks (a dot, an icon), over its highlight
      protected var marks:Shape = new Shape();

      public function SelectorRow(rowWidth:Number, rowHeight:Number)
      {
         super();
         this.rowWidth = rowWidth;
         this.rowHeight = rowHeight;
         addChild(this.back);
         addChild(this.marks);
         mouseChildren = false;
         // the whole row takes the mouse, drawn or not (a fill of alpha 0 is not hit)
         graphics.beginFill(0,0.004);
         graphics.drawRect(0,0,rowWidth,rowHeight);
         graphics.endFill();
      }

      public function get choice() : Boolean
      {
         return false;
      }

      public function highlight(on:Boolean) : void
      {
         var g:Graphics = this.back.graphics;
         g.clear();
         if(!on)
         {
            return;
         }
         g.beginFill(Paint.mix(Paint.PANEL,this.accent,0.17));
         g.drawRoundRect(0,1,this.rowWidth,this.rowHeight - 2,12,12);
         g.endFill();
         g.beginFill(this.accent);
         g.drawRoundRect(0,7,3,this.rowHeight - 14,3,3);
         g.endFill();
      }

      protected function get accent() : uint
      {
         return Paint.INK;
      }

      // the key chip at the row's right end
      protected function chip(label:String, tint:int) : Sprite
      {
         var cap:Sprite = Paint.keycap(label,26,14,tint);
         cap.x = this.rowWidth - 12 - cap.width;
         cap.y = Math.round((this.rowHeight - cap.height) / 2);
         addChild(cap);
         return cap;
      }
   }
}
