package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.filters.GlowFilter;

   // Artifact Hunt (mode 13): the carry mark of the artefact's carrier over his head, in place of the
   // battery's box (HUDMarker.container): the artefact's orb in its light, as the hud draws it
   // (the hud movie's survarium.modes.ArtefactHuntLook.drawOrb, GLOW).
   public class ArtefactMark extends Shape
   {
      public static const GLOW:uint = 0xF4E6A2;

      public function ArtefactMark(radius:Number)
      {
         super();
         var g:Graphics = graphics;
         g.beginFill(0,0.45);
         g.drawCircle(0,0,radius * 1.25);
         g.endFill();
         g.lineStyle(Math.max(1,radius * 0.16),GLOW,0.85);
         g.drawCircle(0,0,radius);
         g.lineStyle();
         g.beginFill(GLOW,1);
         g.drawCircle(0,0,radius * 0.56);
         g.endFill();
         g.beginFill(0xFFFFFF,0.9);
         g.drawCircle(-radius * 0.18,-radius * 0.18,radius * 0.2);
         g.endFill();
         filters = [new GlowFilter(GLOW,0.9,6,6,2)];
         visible = false;
      }
   }
}
