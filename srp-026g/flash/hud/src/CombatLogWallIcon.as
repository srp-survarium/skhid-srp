package
{
   import flash.display.BitmapData;
   
   // The kill feed's mark of a kill through a wall: 0.69d0's icon_penetration.png (bitmaps/, which the build adds).
   public dynamic class CombatLogWallIcon extends BitmapData
   {
      
      public function CombatLogWallIcon(w:int = 40, h:int = 30)
      {
         super(w,h);
      }
   }
}

