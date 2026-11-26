package survarium.upgrade
{
   // A second sight's place on its weapon: the movie's one reading and making of the value its kit
   // record holds (the exe gives it as kit_side; the window sends it as
   // srp_set_upgrade_value(host, upgrade, value), the exe's 46 05). The servers' is
   // weapon_kit::Side (of_value, value: srp-026g/crates/weapon-kit), which refuses any other value.
   // Today the value is the rail's side alone, LEFT 0 or RIGHT 1; a place along the rail would be
   // more of the value, made (value) and read (sideOf) here only.
   public class SightPlace
   {

      public static const LEFT:int = 0;

      public static const RIGHT:int = 1;

      // the sides in the window's order (its toggle, left to right)
      public static const SIDES:Array = [LEFT,RIGHT];

      // each side's text (the toggle's buttons)
      protected static const LABELS:Array = ["st_srp_side_left","st_srp_side_right"];

      public function SightPlace()
      {
         super();
      }

      // The record value of a second sight on `side`.
      public static function value(side:int) : uint
      {
         return uint(side);
      }

      // The side a record value names (the exe gives only a side's, kit_side).
      public static function sideOf(value:uint) : int
      {
         return value == uint(RIGHT) ? RIGHT : LEFT;
      }

      // The side's bit in SecondSights' sides mask (1 left, 2 right).
      public static function bit(side:int) : int
      {
         return 1 << side;
      }

      public static function labelKey(side:int) : String
      {
         return LABELS[side];
      }
   }
}
