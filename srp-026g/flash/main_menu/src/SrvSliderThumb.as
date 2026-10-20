package
{
   import scaleform.clik.controls.Button;
   
   public dynamic class SrvSliderThumb extends Button
   {
      
      public function SrvSliderThumb()
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

