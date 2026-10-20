package
{
   import survarium.buttons.ButtonClick;
   
   public dynamic class SrvBtnPremVS extends ButtonClick
   {
      
      public function SrvBtnPremVS()
      {
         super();
         addFrameScript(0,this.frame1,3,this.frame4,7,this.frame8,11,this.frame12,19,this.frame20);
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
      
      internal function frame20() : *
      {
         stop();
      }
   }
}

