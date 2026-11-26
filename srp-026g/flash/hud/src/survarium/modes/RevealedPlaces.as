package survarium.modes
{
   // Freeze Tag (mode 12): the places revealed now, a bit each: a team's last one fighting, his
   // teammates frozen, two at least (the rule's FreezeTag::last_standing; HUD.freeze_tag_revealed,
   // sent each frame by exe patch 160's freeze_tag::on_hud; NONE outside the mode). The exe lists such
   // an enemy among the maps' players (minimap.enemy_spotted); this says which listed enemy the map
   // marks, since an enemy's own icon (PlayerRed) draws nothing.
   public class RevealedPlaces
   {
      // The game's enemy red (a name over an enemy's head, render_name 0x1F300F; PlayerRed's name).
      public static const COLOR:uint = 0xFF0000;

      public static const NONE:RevealedPlaces = new RevealedPlaces(0);

      private var _mask:uint;

      public function RevealedPlaces(mask:uint)
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
   }
}
