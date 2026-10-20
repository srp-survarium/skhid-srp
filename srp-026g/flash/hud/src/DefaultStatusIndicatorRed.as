package
{
   import scaleform.clik.controls.StatusIndicator;
   
   public dynamic class DefaultStatusIndicatorRed extends StatusIndicator
   {
      
      public function DefaultStatusIndicatorRed()
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

