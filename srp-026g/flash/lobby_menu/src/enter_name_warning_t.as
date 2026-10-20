package
{
   import survarium.buttons.ButtonWarning;
   
   public dynamic class enter_name_warning_t extends ButtonWarning
   {
      
      public function enter_name_warning_t()
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

