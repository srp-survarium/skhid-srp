package survarium.player
{
   // The parts of movement-speed modifier 4 besides the weight, as fractions: the
   // kit's skills (the exe), the armour's bonuses and the weapon in hand's own
   // movement parameter (the kit's items), Bear Arms (the exe) leaving a weapon's
   // penalty out as the match does (0x2E2C5C..0x2E2C9C).
   public final class SpeedParts
   {
      private var _skills:Number;
      private var _bearArms:Boolean;
      private var _bonuses:Number;
      private var _equipment:Number;
      public function SpeedParts(skills:Number, bearArms:Boolean, bonuses:Number, weapon:Number)
      {
         _skills = skills;
         _bearArms = bearArms;
         _bonuses = bonuses;
         _equipment = bearArms && weapon < 0 ? 0 : weapon;
      }
      public function withItems(bonuses:Number, weapon:Number) : SpeedParts
      {
         return new SpeedParts(_skills,_bearArms,isFinite(bonuses) ? bonuses : 0,isFinite(weapon) ? weapon : 0);
      }
      public function get skills() : Number { return _skills; }
      public function get bonuses() : Number { return _bonuses; }
      public function get equipment() : Number { return _equipment; }
      // base x max(0, 1 + S): the weight's part with these (apply_modifier 0x2D5AE0).
      public function total(weight:Number) : Number { return Math.max(0,1 + weight + _skills + _bonuses + _equipment); }
   }
}
