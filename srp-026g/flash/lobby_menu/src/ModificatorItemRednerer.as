package
{
   import survarium.buttons.ModificatorButton;
   
   public dynamic class ModificatorItemRednerer extends ModificatorButton
   {
      
      public function ModificatorItemRednerer()
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

