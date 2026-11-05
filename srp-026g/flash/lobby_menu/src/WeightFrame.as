package
{
   import flash.display.BitmapData;

   // The weight's frame: the original's own bitmap (character 2857, 177 x 47,
   // which the profile's shape 2858 draws under playerWeight; bitmaps/, which the
   // build adds), for the speed's box beside it.
   public dynamic class WeightFrame extends BitmapData
   {

      public function WeightFrame(w:int = 177, h:int = 47)
      {
         super(w,h);
      }
   }
}
