package
{
   import survarium.controls.mc_text_warning;
   
   public dynamic class SrvTextWarning extends mc_text_warning
   {
      
      public function SrvTextWarning()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         contextText.defaultTextFormat = contextText.getTextFormat();
      }
   }
}

