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
      public static const BODY:Array = [-12.14,-25.65,-11.48,-25.0,-12.05,-22.64,-11.76,-20.43,-10.65,-18.51,-9.35,-17.43,-7.29,-16.7,-5.59,-16.72,-3.51,-17.5,-1.69,-19.11,-0.79,-18.83,0.48,-20.95,0.37,-21.64,6.69,-32.15,6.37,-32.47,4.78,-36.46,7.04,-40.36,5.49,-41.29,1.6,-40.02,0.76,-38.82,-1.66,-38.56,-4.92,-36.81,-10.22,-28.0,-10.87,-27.78];
      public static const HEAD:Array = [11.27,-45.7,11.27,-46.61,10.81,-47.24,9.94,-47.5,9.2,-47.24,6.63,-43.16,6.33,-42.07,8.38,-40.84];
      public static const LEVER:Array = [-0.63,-42.57,-7.6,-38.88,-8.02,-38.24,-7.86,-37.49,-7.31,-37.09,-6.64,-37.13,0.06,-40.69,5.01,-42.28,5.42,-42.69,5.52,-43.26,5.15,-43.93,4.42,-44.15];
      public static const BODY_INSIDE:Array = [-10.88,-25.81,-10.77,-25.7,-10.7,-25.63,-10.64,-25.54,-10.59,-25.45,-10.55,-25.36,-10.52,-25.26,-10.5,-25.17,-10.48,-25.06,-10.48,-24.96,-10.49,-24.86,-10.51,-24.76,-11.03,-22.58,-10.8,-20.76,-9.87,-19.16,-8.85,-18.31,-7.13,-17.7,-5.78,-17.72,-4.03,-18.37,-2.35,-19.86,-2.27,-19.92,-2.19,-19.98,-2.1,-20.02,-2.0,-20.06,-1.9,-20.09,-1.8,-20.1,-1.7,-20.11,-1.6,-20.11,-1.5,-20.09,-1.4,-20.07,-1.24,-20.02,-0.56,-21.15,-0.62,-21.48,-0.63,-21.58,-0.63,-21.68,-0.62,-21.78,-0.6,-21.87,-0.57,-21.97,-0.53,-22.06,-0.49,-22.15,5.46,-32.05,5.44,-32.1,3.85,-36.09,3.82,-36.18,3.79,-36.28,3.78,-36.38,3.78,-36.48,3.79,-36.58,3.8,-36.68,3.83,-36.78,3.87,-36.87,3.91,-36.96,5.68,-40.01,5.37,-40.2,2.23,-39.18,1.58,-38.25,1.52,-38.17,1.45,-38.1,1.38,-38.04,1.31,-37.98,1.22,-37.94,1.14,-37.9,1.05,-37.86,0.96,-37.84,0.86,-37.83,-1.36,-37.58,-4.21,-36.06,-9.36,-27.49,-9.42,-27.4,-9.48,-27.33,-9.55,-27.26,-9.63,-27.19,-9.71,-27.14,-9.8,-27.09,-9.89,-27.06,-10.2,-26.95];

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
