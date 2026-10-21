package
{
   import survarium.buttons.ButtonOver;
   
   public dynamic class OptionStepper_Arrow_bright_tops extends ButtonOver
   {
      
      public function OptionStepper_Arrow_bright_tops()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40);
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
      
      internal function frame40() : *
      {
         stop();
      }
   }
}

