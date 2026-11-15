package
{
   import flash.display.BitmapData;
   
   // The kill feed's weapon of a kill by a battery strike (exe patch 143): the battery's model and the gun-stock icon's burst (bitmaps/, which the build adds).
   public dynamic class CombatLogBatteryIcon extends BitmapData
   {
      
      public function CombatLogBatteryIcon(w:int = 53, h:int = 22)
      {
         super(w,h);
      }
   }
}
