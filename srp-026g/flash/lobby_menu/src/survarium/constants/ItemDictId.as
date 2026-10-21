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
      
      public function ItemDictId()
      {
         super();
      }
   }
}
