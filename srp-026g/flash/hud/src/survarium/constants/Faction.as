package survarium.constants
{
   // A faction id (factions_dict of gameplay/db_static_dictionaries; names from st_*_faction).
   public class Faction
   {
      
      // the movie's own: no single faction (all traders; no seller given)
      public static const ANY:int = -1;
      
      // no faction: the independent traders (non_faction_items)
      public static const NONE:int = 0;
      
      public static const SCAVENGERS:int = 1;
      
      public static const BLACK_MARKET:int = 2;
      
      public static const RENAISSANCE:int = 3;
      
      // "The Fringe Settlers" (st_border_faction)
      public static const BORDER:int = 4;
      
      // "Institute"
      public static const SCIENTISTS:int = 5;
      
      public static const MERCENARIES:int = 6;
      
      public function Faction()
      {
         super();
      }
   }
}
