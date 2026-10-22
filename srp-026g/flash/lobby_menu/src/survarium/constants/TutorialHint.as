package survarium.constants
{
   // The lobby's tutorial hint (show_tutorial); the tutorial is gone from srp, the code stays.
   public class TutorialHint
   {
      
      public static const NOT_SHOWN:int = -1;
      
      // hides the hint
      public static const NONE:int = 0;
      
      public static const OPEN_INVENTORY:int = 1;
      
      public static const EQUIP_ITEM:int = 2;
      
      public static const SHOW_AMMO:int = 3;
      
      public function TutorialHint()
      {
         super();
      }
   }
}
