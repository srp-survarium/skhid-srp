package
{
   import survarium.controls.SectionStatusIndicator;
   
   public dynamic class SrvSectionStatusIndicator extends SectionStatusIndicator
   {
      
      public function SrvSectionStatusIndicator()
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

