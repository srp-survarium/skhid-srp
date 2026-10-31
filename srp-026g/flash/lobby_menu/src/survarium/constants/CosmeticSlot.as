package survarium.constants
{
   // A slot of the upgrade window (an upgrade record's param; relocate_upgrade_item's third argument).
   public class CosmeticSlot
   {
      
      public static const DECAL_1:int = 0;
      
      public static const DECAL_2:int = 1;
      
      // on a weapon: the camouflage
      public static const DECAL_3:int = 2;
      
      // on a weapon: the scope
      public static const CAMOUFLAGE:int = 3;

      // a weapon's muzzle device and grip (the weapon kit's logical slots, docs/notes/weapon-kit.md)
      public static const MUZZLE:int = 4;

      public static const GRIP:int = 5;
      
      public function CosmeticSlot()
      {
         super();
      }
   }
}
