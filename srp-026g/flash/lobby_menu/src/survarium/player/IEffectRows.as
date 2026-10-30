package survarium.player
{
   // What a native-card tooltip of the weight row explains: its heading and rows
   // ({icon, label}), drawn by WeightTipRenderer.
   public interface IEffectRows
   {
      function get title() : String;
      // The part of the title to colour (TextFormat, as the heading takes no style
      // sheet) and its style's name in the rows' style sheet; null for none.
      function get titleValue() : String;
      function get titleStyle() : String;
      function rows() : Array;
   }
}
