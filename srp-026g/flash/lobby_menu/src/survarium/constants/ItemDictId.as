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
      
      // The golden weapons (docs/notes/golden_weapons.md), and in the same
      // places their plain weapons: the server's table
      // (survarium::v026g::golden::GOLDEN_WEAPONS), which a test keeps the
      // same. Their descriptor has no rarity since the golden-weapons patch,
      // so the movie knows them by this list.
      public static const GOLDEN_WEAPONS:Array = [
         332, // kiparis_premium
         333, // uzi_premium
         334, // vityaz_premium
         335, // vss_premium
         336, // sr3m_premium
         337, // izh_27_lancaster_premium
         338, // ak_74n_premium
         2222, // wpn_mp7_silencer_special
         2226, // vepr_12_premium
         2228, // saiga_12_premium
         2231, // neostead
         2241, // akmn_premium
         2242, // akmn_premium_2
         2243, // ak_74u_premium
         2244, // vepr_premium
         2245, // ak_12_premium
         2246, // sks_premium
         2247, // sks_premium_2
         2248, // vsk_94_premium
         2249, // rem_700_premium
         2251, // izh_27_lancaster_premium_2
         2252 // uzi_premium_2
      ];
      
      public static const GOLDEN_PLAIN:Array = [
         88, // kiparis
         55, // uzi
         19, // vityaz
         308, // vss
         309, // sr3m
         108, // izh_27_lancaster
         306, // ak_74n
         2221, // mp7_silencer
         2225, // vepr_12
         2227, // saiga_12
         2230, // neostead_2
         80, // akmn
         80, // akmn
         13, // ak_74u
         216, // vepr
         363, // ak_12
         109, // sks
         109, // sks
         305, // vsk_94
         14, // rem_700
         108, // izh_27_lancaster
         55 // uzi
      ];
      
      public function ItemDictId()
      {
         super();
      }
   }
}
