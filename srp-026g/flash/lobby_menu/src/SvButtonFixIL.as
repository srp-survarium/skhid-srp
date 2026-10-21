package
{
   import survarium.buttons.ButtonProgress;
   
   public dynamic class SvButtonFixIL extends ButtonProgress
   {
      
      public function SvButtonFixIL()
      {
         super();
         addFrameScript(3,this.frame4,7,this.frame8,11,this.frame12,21,this.frame22);
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

