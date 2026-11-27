package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.geom.Point;

   // A mode's row under Team Deathmatch's bars (Research's units, Artifact Hunt's artefact): centred
   // under them on the live viewport (followIndicator), the countdowns placed under its visible
   // content (ResearchCountdownLayout, contentBottom).
   public class TopRow extends Sprite
   {
      public function TopRow()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
      }

      public function followIndicator(indicator:DisplayObject, centre:Number, top:Number) : void
      {
         if(stage == null || parent == null || indicator.parent == null)
         {
            return;
         }
         var middle:Point = indicator.parent.globalToLocal(new Point(stage.stageWidth / 2,0));
         indicator.x = middle.x - centre * indicator.scaleX;
         var anchor:Point = parent.globalToLocal(indicator.localToGlobal(new Point(centre,top)));
         x = Math.round(anchor.x);
         y = Math.round(anchor.y);
      }

      // The bottom of what shows, in the row's own space (DisplayObject.getBounds counts hidden
      // children too).
      public function get contentBottom() : Number
      {
         return 0;
      }
   }
}
