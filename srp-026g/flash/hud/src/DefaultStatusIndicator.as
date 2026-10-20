package
{
   import scaleform.clik.controls.StatusIndicator;
   
   public dynamic class DefaultStatusIndicator extends StatusIndicator
   {
      
      public function DefaultStatusIndicator()
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

