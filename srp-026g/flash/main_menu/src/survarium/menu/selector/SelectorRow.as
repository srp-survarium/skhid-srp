package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   
   // A row of the selector's list, `rowWidth` wide and `rowHeight` tall: an action (ActionRow) or a
   // kind's heading (GroupHeading). `highlight` draws it as the one the keys and the mouse are on: a
   // faint light band, the kind's colour as a thin edge.
   public class SelectorRow extends Sprite
   {
      
      public var rowWidth:Number;
      
      public var rowHeight:Number;
      
      protected var back:Shape = new Shape();
      
      public function SelectorRow(rowWidth:Number, rowHeight:Number)
      {
         super();
         this.rowWidth = rowWidth;
         this.rowHeight = rowHeight;
         addChild(this.back);
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
         g.beginFill(0xFFFFFF,0.07);
         g.drawRect(0,0,this.rowWidth,this.rowHeight);
         g.endFill();
         g.beginFill(0xFFFFFF,0.12);
         g.drawRect(0,0,this.rowWidth,1);
         g.drawRect(0,this.rowHeight - 1,this.rowWidth,1);
         g.endFill();
         g.beginFill(this.accent);
         g.drawRect(0,0,2,this.rowHeight);
         g.endFill();
      }
      
      protected function get accent() : uint
      {
         return Paint.LABEL;
      }
   }
}
