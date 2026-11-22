package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;

   // A thrown smoke grenade's marker (MarkerType.SMOKE, TOXIC_SMOKE) in the stock grenade marker's
   // shape (the icon's frame 1: the ring, the pointer under it, a white sketch inside), drawn in the
   // icon's own units. The sketch is the RDG-M's, made from its HUD icon as the stock frag glyph is
   // the RGD-5's: the body solid down to its bottom cap, the fuse head and the lever's wire apart by a
   // cut, tilted and sized as the frag glyph (scripts/grenade-icons/smoke_glyph.py makes the
   // polygons). The toxic smoke's body is green inside its white edge.
   public class SmokeMark extends Shape
   {
      public static const WHITE:uint = 0xFFFFFF;

      // Toxic Smoke's green: halfway from the skill's own icon's (scripts/technical-skills/make_icons.py
      // GREEN, 0x7BD11F: too bright beside the white marks) to the toxic cloud's tint in the game (the
      // toxic-smoke pack's emissive, 0x668561: too dull at this size).
      public static const TOXIC:uint = 0x71AB40;

      // The ring's centre and radius (the icon's sprite 31 at -21.5, -53: 43 px across, 1 px wide).
      public static const CENTRE_Y:Number = -31.5;
      public static const RADIUS:Number = 21.5;

      // smoke_glyph.py's polygons (x, y pairs)
      public static const BODY:Array = [-12.39,-25.96,-11.23,-25.05,-11.8,-22.64,-11.5,-20.41,-10.39,-18.51,-9.1,-17.43,-7.04,-16.7,-5.36,-16.72,-3.25,-17.5,-1.39,-19.13,-0.02,-18.53,1.25,-20.64,0.66,-21.04,0.61,-21.6,6.95,-32.15,6.62,-32.47,5.03,-36.46,7.3,-40.36,5.75,-41.29,1.85,-40.02,1.01,-38.82,-1.41,-38.56,-4.67,-36.81,-9.99,-27.96,-10.5,-27.75,-11.13,-28.08];
      public static const HEAD:Array = [11.52,-45.7,11.53,-46.61,11.06,-47.24,10.19,-47.5,9.46,-47.24,6.88,-43.16,6.58,-42.07,8.63,-40.84];
      public static const LEVER:Array = [-0.37,-42.57,-7.34,-38.88,-7.76,-38.24,-7.6,-37.49,-7.06,-37.09,-6.39,-37.13,0.31,-40.69,5.27,-42.28,5.67,-42.69,5.78,-43.26,5.41,-43.93,4.67,-44.15];
      public static const BODY_INSIDE:Array = [-11.08,-26.2,-10.61,-25.83,-10.54,-25.77,-10.48,-25.7,-10.42,-25.63,-10.37,-25.55,-10.32,-25.46,-10.29,-25.38,-10.26,-25.29,-10.24,-25.19,-10.23,-25.1,-10.23,-25.0,-10.24,-24.91,-10.26,-24.82,-10.78,-22.59,-10.53,-20.74,-9.62,-19.16,-8.59,-18.31,-6.87,-17.7,-5.54,-17.72,-3.77,-18.37,-2.05,-19.88,-1.97,-19.94,-1.89,-19.99,-1.81,-20.03,-1.72,-20.07,-1.63,-20.1,-1.54,-20.12,-1.44,-20.13,-1.35,-20.13,-1.26,-20.12,-1.16,-20.1,-1.07,-20.08,-0.98,-20.04,-0.43,-19.8,-0.08,-20.37,-0.12,-20.42,-0.18,-20.49,-0.23,-20.58,-0.27,-20.67,-0.3,-20.76,-0.32,-20.85,-0.34,-20.95,-0.39,-21.5,-0.39,-21.61,-0.39,-21.71,-0.37,-21.82,-0.34,-21.92,-0.3,-22.02,-0.25,-22.11,5.72,-32.05,5.69,-32.1,4.1,-36.09,4.07,-36.18,4.05,-36.28,4.04,-36.38,4.03,-36.48,4.04,-36.58,4.06,-36.68,4.09,-36.78,4.12,-36.87,4.17,-36.96,5.94,-40.01,5.62,-40.2,2.48,-39.18,1.83,-38.25,1.77,-38.17,1.71,-38.1,1.64,-38.04,1.56,-37.98,1.48,-37.94,1.39,-37.9,1.31,-37.86,1.21,-37.84,1.12,-37.83,-1.11,-37.58,-3.95,-36.06,-9.13,-27.45,-9.19,-27.36,-9.26,-27.28,-9.33,-27.21,-9.42,-27.14,-9.51,-27.09,-9.61,-27.04,-10.12,-26.83,-10.21,-26.8,-10.3,-26.77,-10.4,-26.76,-10.5,-26.75,-10.59,-26.76,-10.69,-26.77,-10.74,-26.78];

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
