package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.text.TextField;
   
   // A kind's heading over its actions (SrpKeysWindow.KINDS' `kind`): its name in its colour and a
   // rule to the row's end.
   public class GroupHeading extends SelectorRow
   {
      
      public static const HEIGHT:Number = 34;
      
      public function GroupHeading(kind:int, texts:SelectorTexts, rowWidth:Number)
      {
         super(rowWidth,HEIGHT);
         var colour:uint = uint(SrpKeysWindow.KINDS[kind].colour);
         var name:TextField = Paint.text(13,colour,1.6);
         name.text = String(texts.groups[kind]).toUpperCase();
         name.x = 12;
         name.y = HEIGHT - name.height - 3;
         addChild(name);
         var g:Graphics = graphics;
         var line:Number = Math.round(name.y + name.height / 2) + 0.5;
         g.lineStyle(1,Paint.mix(Paint.EDGE,colour,0.3));
         g.moveTo(name.x + name.width + 10,line);
         g.lineTo(rowWidth - 12,line);
      }
   }
}
