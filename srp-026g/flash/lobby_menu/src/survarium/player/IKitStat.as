package survarium.player
{
   // A number of the kit with a box of its own in the weight's row (KitStatIndicator): the
   // box's word, its value and the value's style (its name in the lobby's style sheet). The
   // box's tooltip is its rows (IEffectRows).
   public interface IKitStat extends IEffectRows
   {
      function get label() : String;
      function get value() : String;
      function get valueStyle() : String;
   }
}
