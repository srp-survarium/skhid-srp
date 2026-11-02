package survarium.constants
{
   // A profile (kit) slot: profile_slot_id of the dictionary's slots_restrictions; the paper doll's
   // _slotNames order (PaperDollBase).
   public class ProfileSlot
   {
      
      public static const HEAD:int = 0;
      
      public static const MASK:int = 1;
      
      public static const BODY:int = 2;
      
      public static const BACK:int = 3;
      
      public static const LEGS:int = 4;
      
      public static const ARMS:int = 5;
      
      public static const FOOT:int = 6;
      
      // the first weapon (categories 8, 13..17)
      public static const WEAPON_1:int = 7;
      
      public static const WEAPON_1_AMMO_1:int = 8;
      
      public static const WEAPON_1_AMMO_2:int = 9;
      
      // the second weapon
      public static const WEAPON_2:int = 10;
      
      public static const WEAPON_2_AMMO_1:int = 11;
      
      public static const WEAPON_2_AMMO_2:int = 12;
      
      // quick slots 13..18: medkits, traps, grenades
      public static const QUICK_1:int = 13;
      
      public static const QUICK_2:int = 14;
      
      public static const QUICK_3:int = 15;
      
      public static const QUICK_4:int = 16;
      
      public static const QUICK_5:int = 17;
      
      public static const QUICK_6:int = 18;
      
      // ammo slots 19..22 (slotWpn5..8)
      public static const AMMO_5:int = 19;
      
      public static const AMMO_6:int = 20;
      
      public static const AMMO_7:int = 21;
      
      public static const AMMO_8:int = 22;
      
      // no slot: an ammo item with no stock in the kit
      public static const NONE:int = 999;
      
      public function ProfileSlot()
      {
         super();
      }

      // A pocket: quick slots 13..18.
      public static function isPocket(slot:int) : Boolean
      {
         return slot >= QUICK_1 && slot <= QUICK_6;
      }
   }
}
