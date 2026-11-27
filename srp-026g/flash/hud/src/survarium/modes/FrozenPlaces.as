package survarium.modes
{
   import flash.geom.ColorTransform;

   // Freeze Tag (mode 12, exe patch 160): the places frozen now, a bit each (HUD.freeze_tag_rows, sent
   // each frame; NONE outside the mode), and the ice blue a frozen player is shown in, the colour of his
   // name over his head (player_icons' HUDMarker.FROZEN_NAME_COLOR, the same value). The Tab list and
   // both maps ask the same one which places are frozen.
   public class FrozenPlaces
   {
      public static const COLOR:uint = 0x66C2FF;

      public static const NONE:FrozenPlaces = new FrozenPlaces(0);

      private var _mask:uint;

      public function FrozenPlaces(mask:uint)
      {
         super();
         this._mask = mask;
      }

      public function get mask() : uint
      {
         return this._mask;
      }

      public function has(place:int) : Boolean
      {
         return place >= 0 && place < 32 && (this._mask & (1 << place)) != 0;
      }

      // A shape in the ice blue, its alpha its own (as the squad's dots are green).
      public static function tint() : ColorTransform
      {
         var tint:ColorTransform = new ColorTransform();
         tint.color = COLOR;
         return tint;
      }
   }
}
