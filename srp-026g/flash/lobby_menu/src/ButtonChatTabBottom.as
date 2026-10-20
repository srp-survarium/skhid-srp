package
{
   import survarium.buttons.BottomTab;
   
   public dynamic class ButtonChatTabBottom extends BottomTab
   {
      
      public function ButtonChatTabBottom()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40,40,this.frame41,49,this.frame50,59,this.frame60,69,this.frame70,79,this.frame80,89,this.frame90,101,this.frame102);
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
      
      internal function frame41() : *
      {
      }
      
      internal function frame50() : *
      {
         stop();
      }
      
      internal function frame60() : *
      {
         stop();
      }
      
      internal function frame70() : *
      {
         stop();
      }
      
      internal function frame80() : *
      {
         stop();
      }
      
      internal function frame90() : *
      {
      }
      
      internal function frame102() : *
      {
         gotoAndPlay("blink");
      }
   }
}

