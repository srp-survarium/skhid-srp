package
{
   import survarium.buttons.ButtonClickPlay;
   
   public dynamic class SrvTopPlayBtn extends ButtonClickPlay
   {
      
      public function SrvTopPlayBtn()
      {
         super();
         addFrameScript(9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40);
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

