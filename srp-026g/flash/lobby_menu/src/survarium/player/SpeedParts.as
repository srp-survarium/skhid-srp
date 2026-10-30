package survarium.player
{
   // The parts of movement-speed modifier 4 besides the weight, as fractions: the
   // kit's skills (the exe), the armour's bonuses, the boosters the kit carries
   // (a booster adds its modifiers at every spawn while it is in a pocket:
   // inventory fill 0x2FCB70 -> booster 0x33F960/0x33F990) and the weapon in
   // hand's own movement parameter (the kit's items), Bear Arms (the exe) leaving
   // a weapon's penalty out as the match does (0x2E2C5C..0x2E2C9C).
   public final class SpeedParts
   {
      private var _skills:Number;
      private var _bearArms:Boolean;
      private var _bonuses:Number;
      private var _boosters:Number;
      private var _equipment:Number;
      public function SpeedParts(skills:Number, bearArms:Boolean, bonuses:Number, boosters:Number, weapon:Number)
      {
         _skills = skills;
         _bearArms = bearArms;
         _bonuses = bonuses;
         _boosters = boosters;
         _equipment = bearArms && weapon < 0 ? 0 : weapon;
      }
      public function withItems(bonuses:Number, boosters:Number, weapon:Number) : SpeedParts
      {
         return new SpeedParts(_skills,_bearArms,finite(bonuses),finite(boosters),finite(weapon));
      }
      private static function finite(value:Number) : Number { return isFinite(value) ? value : 0; }
      public function get skills() : Number { return _skills; }
      public function get bonuses() : Number { return _bonuses; }
      public function get boosters() : Number { return _boosters; }
      public function get equipment() : Number { return _equipment; }
      // base x max(0, 1 + S): the weight's part with these (apply_modifier 0x2D5AE0).
      public function total(weight:Number) : Number { return Math.max(0,1 + weight + _skills + _bonuses + _boosters + _equipment); }
   }
}
