package survarium.constants
{
   // Items the movie names by dict_id (items_dict of gameplay/db_static_dictionaries).
   public class ItemDictId
   {
      
      // ItemFactory's fallback when an item has no dict_id
      public static const AMMO_9X18_MAKAROV:int = 70;
      
      // generic_money
      public static const GENERIC_MONEY:int = 214;
      
      // the game's scopes (docs/notes/weapon_scopes.md)
      public static const SCOPE_LEUPOLD:int = 69;
      
      public static const SCOPE_PSO_1:int = 304;
      
      public static const SCOPE_GIPERON:int = 362;
      
      // srp's collimators
      public static const SCOPE_EOTECH_552:int = 2206;
      
      public static const SCOPE_WALTHER_COMP2:int = 2207;
      
      // srp's shop scopes: 2101..2199 (docs/notes/weapon_scopes.md)
      public static const SHOP_SCOPE_FIRST:int = 2101;
      
      public static const SHOP_SCOPE_LAST:int = 2199;
      
      // The premium armour (docs/notes/premium_armour.md), in the same places its plain pieces and
      // the premium bonuses srp gives each as its own first modifications: items.tsv's copies, which
      // a test keeps the same (survarium::v026g::golden::tests). Golden items: their descriptor has
      // no rarity since the premium-armour patch, so the movie knows them by this list.
      public static const PREMIUM_ARMOUR:Array = [
         345, // scav_torso_4_premium
         346, // scav_legs_4_premium
         352, // bm_torso_4_premium
         354 // bm_legs_4_premium
      ];
      
      public static const PREMIUM_ARMOUR_PLAIN:Array = [
         325, // scav_torso_4
         326, // scav_legs_4
         351, // bm_torso_4
         353 // bm_legs_4
      ];
      
      public static const PREMIUM_ARMOUR_BONUSES:Array = [
         [1240,1250], // scav_torso_4_premium
         [1240,1250], // scav_legs_4_premium
         [1240,1260], // bm_torso_4_premium
         [1240,1260] // bm_legs_4_premium
      ];
      
      public function ItemDictId()
      {
         super();
      }
   }
}
