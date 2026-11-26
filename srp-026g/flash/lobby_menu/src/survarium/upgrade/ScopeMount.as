package survarium.upgrade
{
   import flash.utils.Dictionary;
   import survarium.data.ScopeMounts;

   // Which of its ways onto its weapon a scope takes: the movie's one reading and making of the value
   // its kit record holds (the exe gives it as kit_mount; the window sends it as
   // srp_set_upgrade_value(host, scope, value), the exe's 46 05). The servers' is
   // weapon_kit::Mount (of_value, value: srp-026g/crates/weapon-kit), which refuses any other value; 0
   // is the way the scope took before a player could choose. The ways of a (weapon, optic) and their
   // names are ScopeMounts' (docs/notes/scope-mounts.md).
   public class ScopeMount
   {

      // the ways a kit can name (two bits)
      public static const WAYS:int = 4;

      // ScopeMounts by weapon dict id: {optic dict id: its ways' text keys}
      protected static var _ways:Dictionary;

      public function ScopeMount()
      {
         super();
      }

      // The record value of a scope on its `way`th way.
      public static function value(way:int) : uint
      {
         return uint(way);
      }

      // The way a record value names (the exe gives only a way's, kit_mount).
      public static function wayOf(value:uint) : int
      {
         return value < uint(WAYS) ? int(value) : 0;
      }

      protected static function waysOf(weaponDictId:int) : Object
      {
         var row:Object = null;
         if(!_ways)
         {
            _ways = new Dictionary();
            for each(row in ScopeMounts.fits)
            {
               if(!_ways[row.weapon])
               {
                  _ways[row.weapon] = {};
               }
               _ways[row.weapon][row.optic] = String(row.ways).split(",");
            }
         }
         return _ways[weaponDictId];
      }

      // The text keys of the ways an optic has onto the weapon, in the ways' order; null for one way.
      public static function ways(weaponDictId:int, opticDictId:int) : Array
      {
         var optics:Object = waysOf(weaponDictId);
         return Boolean(optics) && opticDictId in optics ? optics[opticDictId] as Array : null;
      }

      // Whether some optic has more than one way onto the weapon: its scope's slot keeps room for the
      // buttons.
      public static function takesWays(weaponDictId:int) : Boolean
      {
         return Boolean(waysOf(weaponDictId));
      }
   }
}
