package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.text.TextField;
   
   // "Unbind": the key being edited left with no action (only offered while it has one).
   public class UnbindRow extends SelectorRow
   {
      
      public static const HEIGHT:Number = 40;
      
      public var label:TextField;
      
      public function UnbindRow(editedKey:String, texts:SelectorTexts, rowWidth:Number)
      {
         super(rowWidth,HEIGHT);
         // a circle struck through
         var g:Graphics = marks.graphics;
         g.lineStyle(2,Paint.UNBIND,1,true);
         g.drawCircle(19,HEIGHT / 2,6.5);
         g.moveTo(19 - 4.6,HEIGHT / 2 + 4.6);
         g.lineTo(19 + 4.6,HEIGHT / 2 - 4.6);
         g.lineStyle();
         var cap:* = chip(editedKey,-1);
         this.label = Paint.text(17,Paint.mix(Paint.UNBIND,Paint.INK,0.3));
         Paint.fit(this.label,texts.unbind,cap.x - 14 - 34);
         this.label.x = 32;
         this.label.y = Math.round((HEIGHT - this.label.height) / 2);
         addChild(this.label);
      }
      
      override public function get choice() : Boolean
      {
         return true;
      }
      
      override protected function get accent() : uint
      {
         return Paint.UNBIND;
      }
   }
}
