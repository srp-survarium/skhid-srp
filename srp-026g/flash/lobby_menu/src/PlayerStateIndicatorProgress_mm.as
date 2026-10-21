package
{
   import survarium.controls.ProgressIndicator;
   
   public dynamic class PlayerStateIndicatorProgress_mm extends ProgressIndicator
   {
      
      public function PlayerStateIndicatorProgress_mm()
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

