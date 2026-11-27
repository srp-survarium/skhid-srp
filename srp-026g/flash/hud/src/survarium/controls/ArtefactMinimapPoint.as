package survarium.controls
{
   import flash.display.Graphics;
   import survarium.modes.ArtefactHuntLook;

   // Artifact Hunt (mode 13): the artefact lying in the world on the minimaps, the orb of the row
   // under the bars (ArtefactHuntRow) in its light; at the corner minimap's edge while out of it.
   public class ArtefactMinimapPoint extends MinimapObjectBasix
   {
      protected static const RADIUS:Number = 7;

      public function ArtefactMinimapPoint()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         var g:Graphics = graphics;
         g.beginFill(0,0.5);
         g.drawCircle(0,0,RADIUS + 2);
         g.endFill();
         ArtefactHuntLook.drawOrb(g,RADIUS,true);
         filters = [ArtefactHuntLook.glow(1.5)];
      }
   }
}
