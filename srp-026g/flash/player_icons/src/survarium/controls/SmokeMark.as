package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;

   // A thrown smoke grenade's marker (MarkerType.SMOKE, TOXIC_SMOKE) in the stock grenade marker's
   // shape (the icon's frame 1: the ring, the pointer under it, a white sketch inside), drawn in the
   // icon's own units. The sketch is the RDG-M's, made from its HUD icon as the stock frag glyph is
   // the RGD-5's: the body solid, the fuse head and the lever's wire apart by a cut, tilted and sized
   // as the frag glyph (scripts/grenade-icons/smoke_glyph.py makes the polygons). The toxic smoke's
   // body is green inside its white edge.
   public class SmokeMark extends Shape
   {
      public static const WHITE:uint = 0xFFFFFF;

      // Toxic Smoke's green: the skill's own icon's (scripts/technical-skills/make_icons.py GREEN).
      public static const TOXIC:uint = 0x7BD11F;

      // The ring's centre and radius (the icon's sprite 31 at -21.5, -53: 43 px across, 1 px wide).
      public static const CENTRE_Y:Number = -31.5;
      public static const RADIUS:Number = 21.5;

      // smoke_glyph.py's polygons (x, y pairs)
      public static const BODY:Array = [-10.89,-26.95,-11.77,-24.95,-12.11,-22.2,-11.71,-20.23,-10.38,-18.16,-8.86,-17.11,-7.08,-16.67,-4.56,-17.01,-2.71,-17.96,-0.68,-19.98,6.66,-32.14,6.34,-32.46,4.74,-36.44,7.01,-40.35,5.46,-41.29,1.56,-40.02,0.72,-38.81,-1.71,-38.55,-4.96,-36.8];
      public static const BODY_INSIDE:Array = [-10.0,-26.49,-10.8,-24.68,-11.1,-22.24,-10.76,-20.61,-9.65,-18.87,-8.44,-18.04,-7.02,-17.68,-4.86,-17.97,-3.3,-18.78,-1.48,-20.6,5.43,-32.03,5.41,-32.09,3.81,-36.07,3.78,-36.17,3.76,-36.27,3.75,-36.37,3.74,-36.47,3.75,-36.57,3.77,-36.67,3.8,-36.76,3.83,-36.86,3.88,-36.95,5.65,-40.0,5.33,-40.19,2.19,-39.17,1.54,-38.24,1.48,-38.16,1.41,-38.09,1.34,-38.03,1.27,-37.97,1.19,-37.93,1.1,-37.89,1.01,-37.85,0.92,-37.83,0.83,-37.82,-1.41,-37.57,-4.25,-36.05];
      public static const HEAD:Array = [11.24,-45.7,11.24,-46.61,10.78,-47.23,10.04,-47.5,9.17,-47.24,6.59,-43.15,6.29,-42.07,8.35,-40.83];
      public static const LEVER:Array = [-0.67,-42.56,-7.64,-38.86,-8.06,-38.22,-7.91,-37.48,-7.36,-37.08,-6.69,-37.12,0.02,-40.68,4.98,-42.27,5.38,-42.69,5.49,-43.26,5.12,-43.93,4.38,-44.14];

      public function SmokeMark(toxic:Boolean)
      {
         super();
         var g:Graphics = graphics;
         // the ring: the stock disk (white 20 % in a clip of 30 %) and its 1 px edge (the clip's 30 %)
         g.beginFill(WHITE,0.06);
         g.drawCircle(0,CENTRE_Y,RADIUS - 1);
         g.endFill();
         g.beginFill(WHITE,0.3);
         g.drawCircle(0,CENTRE_Y,RADIUS);
         g.drawCircle(0,CENTRE_Y,RADIUS - 1);
         g.endFill();
         // the pointer (the stock shape 27): a triangle down, its top following the ring
         g.beginFill(WHITE,1);
         g.moveTo(0.3,-7.75);
         g.curveTo(-4.35,-7.75,-8.5,-9.7);
         g.lineTo(0.05,-0.25);
         g.lineTo(9.75,-10);
         g.curveTo(5.15,-7.75,0.3,-7.75);
         g.endFill();
         fill(g,BODY,WHITE);
         if(toxic)
         {
            fill(g,BODY_INSIDE,TOXIC);
         }
         fill(g,HEAD,WHITE);
         fill(g,LEVER,WHITE);
      }

      private static function fill(g:Graphics, points:Array, colour:uint) : void
      {
         g.beginFill(colour,1);
         g.moveTo(points[0],points[1]);
         for(var i:int = 2; i < points.length; i += 2)
         {
            g.lineTo(points[i],points[i + 1]);
         }
         g.endFill();
      }
   }
}
