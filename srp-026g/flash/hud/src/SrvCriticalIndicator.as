package
{
   import survarium.controls.CriticalIndicator;
   
   public dynamic class SrvCriticalIndicator extends CriticalIndicator
   {
      
      public function SrvCriticalIndicator()
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

