package
{
   import survarium.buttons.ModificatorListRenderer;
   
   public dynamic class SrvModifierLabeledRendererMess extends ModificatorListRenderer
   {
      
      public function SrvModifierLabeledRendererMess()
      {
         super();
         addFrameScript(0,this.frame1,3,this.frame4,7,this.frame8,11,this.frame12,21,this.frame22);
      }
      
      internal function frame1() : *
      {
      }
      
      internal function frame4() : *
      {
         stop();
      }
      
      internal function frame8() : *
      {
         stop();
      }
      
      internal function frame12() : *
      {
         stop();
      }
      
      internal function frame22() : *
      {
         stop();
      }
   }
}

