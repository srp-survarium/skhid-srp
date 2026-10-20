package
{
   import survarium.controls.NumericStepperExchange;
   
   public dynamic class OptionStepperBtnExchangeMess extends NumericStepperExchange
   {
      
      public function OptionStepperBtnExchangeMess()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20,29,this.frame30);
      }
      
      internal function frame1() : *
      {
         preventAutosizing = true;
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

