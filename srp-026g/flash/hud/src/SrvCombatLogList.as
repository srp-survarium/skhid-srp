package
{
   import survarium.controls.CombatLogList;
   
   public dynamic class SrvCombatLogList extends CombatLogList
   {
      
      public function SrvCombatLogList()
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

