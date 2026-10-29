package survarium.player
{
   // What a native-card tooltip of the weight row explains: its heading and rows
   // ({icon, label}), drawn by WeightTipRenderer.
   public interface IEffectRows
   {
      function get title() : String;
      function rows() : Array;
   }
}
