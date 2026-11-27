package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Shape;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import survarium.modes.ArtefactHuntLook;

   // Artifact Hunt: the artefact's light round a player's dot on the minimaps (the carrier glows in
   // the world too): a ring in its colour over the dot, centred on what the dot draws.
   public class CarryRing extends Shape
   {
      public static const RADIUS:Number = 8;

      public function CarryRing()
      {
         super();
         graphics.lineStyle(2,ArtefactHuntLook.GLOW,1);
         graphics.drawCircle(0,0,RADIUS);
         graphics.lineStyle();
         filters = [ArtefactHuntLook.glow()];
      }

      // The ring on `dot` (on) or none; one at most.
      public static function mark(dot:DisplayObjectContainer, on:Boolean) : void
      {
         var ring:CarryRing = null;
         for(var i:int = 0; i < dot.numChildren; i++)
         {
            if(dot.getChildAt(i) is CarryRing)
            {
               ring = CarryRing(dot.getChildAt(i));
            }
         }
         if(!on)
         {
            if(ring)
            {
               dot.removeChild(ring);
            }
            return;
         }
         if(ring)
         {
            return;
         }
         // what the dot draws, without its name (MinimapObjectPlayer.showName)
         var drawn:Rectangle = new Rectangle();
         var child:DisplayObject;
         for(i = 0; i < dot.numChildren; i++)
         {
            child = dot.getChildAt(i);
            if(!(child is TextField) && child.visible)
            {
               drawn = drawn.union(child.getBounds(dot));
            }
         }
         ring = new CarryRing();
         ring.x = drawn.width > 0 ? drawn.x + drawn.width / 2 : 0;
         ring.y = drawn.height > 0 ? drawn.y + drawn.height / 2 : 0;
         dot.addChild(ring);
      }

      public static function marked(dot:DisplayObjectContainer) : Boolean
      {
         for(var i:int = 0; i < dot.numChildren; i++)
         {
            if(dot.getChildAt(i) is CarryRing)
            {
               return true;
            }
         }
         return false;
      }
   }
}
