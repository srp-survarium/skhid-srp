package survarium.player
{
   // What the kit's items add, summed by the profile from their modifications:
   // movement speed (prop 105) and carry limit (prop 106, kg), the armour's
   // (HEAD..FOOT) as bonuses and the pockets' (QUICK_1..6) as boosters, and the
   // first weapon's own movement parameter.
   public final class KitItems
   {
      public var speedBonuses:Number = 0;
      public var speedBoosters:Number = 0;
      public var weapon:Number = 0;
      public var limitBonuses:Number = 0;
      public var limitBoosters:Number = 0;
   }
}
