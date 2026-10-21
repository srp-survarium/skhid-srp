package
{
   import survarium.buttons.ButtonWarning;
   
   public dynamic class enter_name_warning extends ButtonWarning
   {
      
      public function enter_name_warning()
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

