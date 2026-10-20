package
{
   import survarium.controls.NumericStepperSrv;
   
   public dynamic class OptionStepperSrvBig extends NumericStepperSrv
   {
      
      public function OptionStepperSrvBig()
      {
         super();
         addFrameScript(9,this.frame10,19,this.frame20,29,this.frame30);
      }
      
      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         stop();
      }
   }
}

