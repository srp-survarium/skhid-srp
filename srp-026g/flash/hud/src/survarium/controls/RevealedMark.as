package survarium.controls
{
   import flash.display.DisplayObjectContainer;
   import flash.display.Shape;
   import flash.filters.GlowFilter;
   import survarium.modes.RevealedPlaces;

   // Freeze Tag: the mark of a revealed enemy's dot (a team's last one fighting) on a map: a red dot
   // with a dark edge, the size of a teammate's arrow. An enemy's icon (PlayerRed) has no picture of
   // its own in 0.26g's hud (its twelve frames are empty, in the stock movie too), so without it the
   // map shows nothing where he is.
   public class RevealedMark extends Shape
   {
      public static const RADIUS:Number = 6;

      public function RevealedMark()
      {
         super();
         graphics.lineStyle(1.5,0x1A0000,1);
         graphics.beginFill(RevealedPlaces.COLOR,1);
         graphics.drawCircle(0,0,RADIUS);
         graphics.endFill();
         filters = [new GlowFilter(0,0.8,4,4,2)];
      }

      // The mark on `dot` (on) or none; one at most, under its name.
      public static function mark(dot:DisplayObjectContainer, on:Boolean) : void
      {
         var mark:RevealedMark = null;
         for(var i:int = 0; i < dot.numChildren; i++)
         {
            if(dot.getChildAt(i) is RevealedMark)
            {
               mark = RevealedMark(dot.getChildAt(i));
            }
         }
         if(!on)
         {
            if(mark)
            {
               dot.removeChild(mark);
            }
            return;
         }
         if(!mark)
         {
            dot.addChildAt(new RevealedMark(),0);
         }
      }

      public static function marked(dot:DisplayObjectContainer) : Boolean
      {
         for(var i:int = 0; i < dot.numChildren; i++)
         {
            if(dot.getChildAt(i) is RevealedMark)
            {
               return true;
            }
         }
         return false;
      }
   }
}
