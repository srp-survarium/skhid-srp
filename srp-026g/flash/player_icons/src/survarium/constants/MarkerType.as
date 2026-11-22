package survarium.constants
{
   // A world marker's type (root.set_player_icon_type, game_world_ui::set_hud_icon_type 0x23BEA0): the
   // frame of its icon the stock game shows, and the smoke grenades exe patch 194 tells apart. The
   // game's markers of thrown grenades are its own team's only (player::st_draw 0x24719C: the
   // thrower's team, within grenades_sensor_radius 7 m; docs/notes/grenade-icons.md).
   public class MarkerType
   {
      // A thrown grenade (initialize_match 0x235815, every grenade marker): the frag's glyph (RGD-5).
      public static const GRENADE:uint = 0;

      // A trap (initialize_match 0x23587E).
      public static const TRAP:uint = 1;

      // A thrown smoke grenade (RDG-M): the white sketch of the RDG-M (survarium.controls.SmokeMark).
      public static const SMOKE:uint = 3;

      // A smoke grenade thrown with Toxic Smoke (skill 441): the same sketch, its body green inside.
      public static const TOXIC_SMOKE:uint = 4;

      public function MarkerType()
      {
         super();
      }
   }
}
