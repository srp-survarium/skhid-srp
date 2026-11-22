package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;

   // A thrown smoke grenade's marker (MarkerType.SMOKE, TOXIC_SMOKE) in the stock grenade marker's
   // shape (the icon's frame 1: the ring, the pointer under it, a white glyph inside), drawn in the
   // icon's own units: an upright canister with its fuse and ring, its cloud rising beside it, white;
   // the toxic smoke's cloud green.
   public class SmokeMark extends Shape
   {
      public static const WHITE:uint = 0xFFFFFF;

      // Toxic Smoke's cloud (the toxic-smoke pack's green smoke, lightened to read on the marker).
      public static const TOXIC:uint = 0x9BE15D;

      // The ring's centre and radius (the icon's sprite 31 at -21.5, -53: 43 px across, 1 px wide).
      public static const CENTRE_Y:Number = -31.5;
      public static const RADIUS:Number = 21.5;

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
         // the canister: body, fuse, the lever down its side, the pin's ring
         g.beginFill(WHITE,1);
         g.drawRoundRect(-13.5,-29.5,10,14.5,3,3);
         g.endFill();
         g.beginFill(WHITE,1);
         g.drawRect(-11.5,-32,6,2.5);
         g.endFill();
         g.beginFill(WHITE,1);
         g.moveTo(-5.5,-32);
         g.lineTo(-1.2,-31);
         g.lineTo(-1,-18.5);
         g.lineTo(-2.4,-18.5);
         g.lineTo(-2.6,-29.8);
         g.lineTo(-5.5,-29.5);
         g.endFill();
         g.lineStyle(1.3,WHITE,1);
         g.drawCircle(-13,-34.5,2);
         g.lineStyle();
         // the cloud rising from the fuse: a wisp, then three puffs on a flat base (one fill each:
         // overlapping parts of one fill would cut holes)
         var cloud:uint = toxic ? TOXIC : WHITE;
         for each(var puff:Array in [[-7.5,-35.6,1.5],[-3.5,-38,2],[3.5,-40,4],[8.5,-44,5.2],[13,-40,3.6]])
         {
            g.beginFill(cloud,1);
            g.drawCircle(puff[0],puff[1],puff[2]);
            g.endFill();
         }
         g.beginFill(cloud,1);
         g.drawRoundRect(3.5,-41.5,9.5,5.1,2,2);
         g.endFill();
      }
   }
}
