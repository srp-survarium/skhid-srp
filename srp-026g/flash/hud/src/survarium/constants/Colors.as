package survarium.constants
{
   public class Colors
   {
      
      public static const GREEN:uint = 11790999;
      
      public static const RED:uint = 13582914;
      
      public static const YELLOW:uint = 15852869;
      
      public static const ORANGE:uint = 16763904;
      
      public static const WHITE:uint = 16777215;
      
      public static const SILVER:uint = 13162969;
      
      public static const GRAY:uint = 5921629;
      
      public static const SOFT_GRAY:uint = 8356742;
      
      public static const LIGHT_GRAY:uint = 13421772;
      
      public static const PRICE_REG:uint = 11252921;
      
      public static const EQUIPPED:uint = 7971167;
      
      public static const RANK_1:uint = 6131619;
      
      public static const RANK_2:uint = 8301155;
      
      public static const RANK_3:uint = 15709443;
      
      public static const GOLD:uint = 15317048;
      
      public static const REGULAR:uint = 10137262;
      
      public static const HIGHLIGHT:uint = 16704178;
      
      public static const NOT_READY:uint = 3619127;
      
      public static const READY:uint = 7959153;
      
      // The hud's own blue: its text style "blue" (HUD.init, #079DD5).
      public static const HUD_BLUE:uint = 0x079DD5;

      // A speed boost's text: Oxygen Rush's countdown (the owner, 2026-11-23: "should become blue",
      // "since both are for speed"). HUD_BLUE mixed 60 % with white, as light and as grey as the
      // hud's green (GREEN, its style "current": saturation 0.35, value 0.92), drawn with the
      // text's own black shadow and a glow of HUD_BLUE (SPEED_BOOST_GLOW). The owner, 2026-11-26:
      // Spring's pure blue (0x0066FF) "is a bit too on the nose", not in the hud's style.
      public static const SPEED_BOOST:uint = 0x9CD8EE;

      public static const SPEED_BOOST_GLOW:uint = HUD_BLUE;

      public static const GREEN_DIFF:uint = 8890390;
      
      public static const RED_DIFF:uint = 9834254;
      
      public static const PLAYER:uint = YELLOW;
      
      public static const SQUAD:uint = GREEN;
      
      public static const ENEMY:uint = RED;
      
      public static const OTHER_PLAYER:uint = WHITE;
      
      public static const GREEN_ST:String = "#b3ea97";
      
      public static const RED_ST:String = "#f4242";
      
      public static const YELLOW_ST:String = "#F1E545";
      
      public static const WHITE_ST:String = "#ffffff";
      
      public static const PLAYER_ST:String = YELLOW_ST;
      
      public static const SQUAD_ST:String = GREEN_ST;
      
      public static const ENEMY_ST:String = RED_ST;
      
      public static const OTHER_PLAYER_ST:String = WHITE_ST;
      
      public function Colors()
      {
         super();
      }
   }
}

