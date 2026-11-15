package
{
   import flash.display.BitmapData;
   
   // The kill feed's weapon of a kill by a battery strike (exe patch 143): the hud's battery rows' battery (bitmaps/, which the build adds).
   public dynamic class CombatLogBatteryIcon extends BitmapData
   {
      
      public function CombatLogBatteryIcon(w:int = 25, h:int = 19)
      {
         super(w,h);
      }
   }
}
