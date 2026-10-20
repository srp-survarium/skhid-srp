package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class DamageCrossHair_132 extends MovieClip
   {
      
      public var part_1:MovieClip;
      
      public var part_2:MovieClip;
      
      public var part_3:MovieClip;
      
      public function DamageCrossHair_132()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

