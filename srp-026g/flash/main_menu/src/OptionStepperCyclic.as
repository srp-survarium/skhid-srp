package
{
   import survarium.controls.OptionStepperCyclic;
   
   public dynamic class OptionStepperCyclic extends survarium.controls.OptionStepperCyclic
   {
      
      public function OptionStepperCyclic()
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

