package survarium.constants
{
   // An item's category (item_category of gameplay/db_static_dictionaries items_dict). The names are the
   // game's own: gameplay/item_categories (item_category_booby_traps, item_category_grenades) and the
   // lobby's shop filters (FiltersWindow: filter_helmet, filter_assault_rifle, ...). 23 has no items.
   public class ItemCategory
   {
      
      // filter_helmet; item configs of type arm_hlmt
      public static const HELMETS:int = 1;
      
      // filter_mask; arm_mask
      public static const MASKS:int = 2;
      
      // filter_body_armor; arm_tors
      public static const BODY_ARMOR:int = 3;
      
      // filter_rig ("Gear"); arm_back and arm_oxy
      public static const RIGS:int = 4;
      
      // filter_pants; arm_legs
      public static const PANTS:int = 5;
      
      // filter_gloves; arm_hand
      public static const GLOVES:int = 6;
      
      // filter_boots; arm_boot
      public static const BOOTS:int = 7;
      
      // filter_assault_rifle
      public static const ASSAULT_RIFLES:int = 8;
      
      // filter_ammo_assault_rifle
      public static const ASSAULT_RIFLE_AMMO:int = 9;
      
      // filter_medkit ("Medicines"): drugs and boosters
      public static const MEDKITS:int = 10;
      
      // item_category_booby_traps; filter_mine
      public static const BOOBY_TRAPS:int = 11;
      
      // artefact_lifebone, artefact_spring, artefact_onyx
      public static const ARTEFACTS:int = 12;
      
      // filter_sniper_rifle
      public static const SNIPER_RIFLES:int = 13;
      
      // filter_shotgun
      public static const SHOTGUNS:int = 14;
      
      // filter_submachine_gun
      public static const SUBMACHINE_GUNS:int = 15;
      
      // filter_machine_gun
      public static const MACHINE_GUNS:int = 16;
      
      // filter_pistol
      public static const PISTOLS:int = 17;
      
      // filter_ammo_sniper_rifle
      public static const SNIPER_RIFLE_AMMO:int = 18;
      
      // filter_ammo_shotgun
      public static const SHOTGUN_AMMO:int = 19;
      
      // filter_ammo_pistol
      public static const PISTOL_AMMO:int = 20;
      
      // filter_ammo_machine_gun
      public static const MACHINE_GUN_AMMO:int = 21;
      
      // filter_scope
      public static const SCOPES:int = 22;
      
      // item_category_grenades; filter_grenade
      public static const GRENADES:int = 24;
      
      // filter_camo
      public static const CAMOUFLAGES:int = 25;
      
      // filter_decal
      public static const DECALS:int = 26;
      
      // filter_vouchers
      public static const VOUCHERS:int = 27;
      
      // the item generic_money (the lobby's "regular" money)
      public static const GENERIC_MONEY:int = 28;
      
      // the item premium_money (gold)
      public static const PREMIUM_MONEY:int = 29;
      
      // not a category of the dictionary: the lobby gives its shop bundles this one
      public static const BUNDLE:int = 100;
      
      public function ItemCategory()
      {
         super();
      }
   }
}
