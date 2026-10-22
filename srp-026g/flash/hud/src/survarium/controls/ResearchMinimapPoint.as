package survarium.controls
{
   import flash.display.Graphics;

   // Research (mode 5): a research unit on the minimap, its letter in the
   // colour of its owner, the capture's ring round it (ResearchUnits' look).
   public class ResearchMinimapPoint extends MinimapObjectBasix
   {

      protected static const RADIUS:Number = 7;

      public function ResearchMinimapPoint()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
      }

      public function setState(cpId:uint, owner:uint, capturing:uint, progress:Number, ownTeam:uint) : void
      {
         var g:Graphics = graphics;
         g.clear();
         g.beginFill(0,0.5);
         g.drawCircle(0,0,RADIUS + 2);
         g.endFill();
         g.beginFill(ResearchUnits.colourOf(owner,ownTeam),0.95);
         g.drawCircle(0,0,RADIUS);
         g.endFill();
         if(capturing != ResearchUnits.NO_TEAM && progress > 0 && !(owner == capturing && progress >= 100))
         {
            ResearchUnits.drawRing(g,RADIUS,2,progress / 100,ResearchUnits.colourOf(capturing,ownTeam));
         }
         ResearchUnits.drawLetter(g,cpId,RADIUS * 1.1);
      }
   }
}
