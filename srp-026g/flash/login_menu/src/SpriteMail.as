package
{
   import flash.display.Graphics;
   import flash.display.MovieClip;

   // The icon of the account field: srp signs players in by nickname, so a person (head and
   // shoulders) in place of the stock movie's mail bitmap, drawn as plainly as the mail was:
   // a white 1 px outline at half opacity, centred where the mail's 18 x 13 px were.
   public dynamic class SpriteMail extends MovieClip
   {

      public function SpriteMail()
      {
         super();
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         // Each outline is a fill with a hole (two paths in one fill), not a line: Scaleform
         // draws a stray line from the pen to a circle drawn after lineStyle.
         var g:Graphics = graphics;
         g.beginFill(16777215,0.5);
         g.drawCircle(9,3,4);
         g.drawCircle(9,3,3);
         g.moveTo(2,14);
         g.curveTo(2,9,9,9);
         g.curveTo(16,9,16,14);
         g.lineTo(2,14);
         g.moveTo(3,13);
         g.curveTo(3,10,9,10);
         g.curveTo(15,10,15,13);
         g.lineTo(3,13);
         g.endFill();
      }
   }
}
