package
{
   import survarium.controls.DamageIndicator;
   
   public dynamic class SrvDamageIndicator extends DamageIndicator
   {
      
      public function SrvDamageIndicator()
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

