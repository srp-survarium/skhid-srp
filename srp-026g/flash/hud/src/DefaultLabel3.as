package
{
   import scaleform.clik.controls.Label;
   
   public dynamic class DefaultLabel3 extends Label
   {
      
      public function DefaultLabel3()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20);
      }
      
      internal function frame1() : *
      {
         textField.defaultTextFormat = textField.getTextFormat();
      }
      
      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
   }
}

