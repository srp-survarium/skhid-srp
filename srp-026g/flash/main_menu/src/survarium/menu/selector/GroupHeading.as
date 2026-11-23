package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.text.TextField;
   
   // A kind's heading over its actions (SrpKeysWindow.KINDS' `kind`), as the Settings tab's headings:
   // its name in the title's ink, a small mark of its colour before it, a rule to the row's end.
   public class GroupHeading extends SelectorRow
   {
      
      public static const HEIGHT:Number = 32;
      
      public function GroupHeading(kind:int, texts:SelectorTexts, rowWidth:Number, filters:Array)
      {
         super(rowWidth,HEIGHT);
         var colour:uint = uint(SrpKeysWindow.KINDS[kind].colour);
         var name:TextField = Paint.text(15,Paint.TITLE,filters);
         name.text = String(texts.groups[kind]);
         name.x = 26;
         name.y = HEIGHT - name.height - 2;
         addChild(name);
         var g:Graphics = graphics;
         var middle:Number = Math.round(name.y + name.height / 2);
         g.beginFill(colour);
         g.drawRect(12,middle - 4,8,8);
         g.endFill();
         g.beginFill(Paint.RULE);
         g.drawRect(name.x + name.width + 10,middle,rowWidth - 12 - (name.x + name.width + 10),1);
         g.endFill();
      }
   }
}
