package
{
   import survarium.controls.CombatLogRenderer;
   
   public dynamic class SrvCombatLogRenderer extends CombatLogRenderer
   {
      
      public function SrvCombatLogRenderer()
      {
         super();
         addFrameScript(119,this.frame120);
      }
      
      internal function frame120() : *
      {
         stop();
      }
   }
}

