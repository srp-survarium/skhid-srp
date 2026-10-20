package
{
   import survarium.buttons.ButtonBlink;
   
   public dynamic class SrvTutorButton extends ButtonBlink
   {
      
      public function SrvTutorButton()
      {
         super();
         addFrameScript(9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40,59,this.frame60);
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
      
      internal function frame60() : *
      {
         gotoAndPlay("blink");
      }
   }
}

