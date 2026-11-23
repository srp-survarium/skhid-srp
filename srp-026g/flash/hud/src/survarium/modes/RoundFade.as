package survarium.modes
{
   import flash.display.Shape;

   // Artifact Hunt's round restart (0.46c2's show_fade_animation): the screen goes black over the
   // 1 s fade (phase 7), black at its end, when everyone is taken to his team's spawns, and comes
   // back over the first BACK ms of the round's countdown (phase 8). From the phase and its time
   // left only, so a late join or a resync in the middle of it shows it where it is. On the stage
   // below the hud's root (as DeathEyes), in screen pixels: the countdown's text stays over it, and
   // hiding the interface leaves it, as the screen effects.
   public class RoundFade extends Shape
   {
      public static const BACK:Number = 500;

      public function RoundFade()
      {
         super();
         visible = false;
      }

      // The screen's size in the stage's pixels.
      public function place(width:Number, height:Number) : void
      {
         graphics.clear();
         graphics.beginFill(0,1);
         graphics.drawRect(0,0,width,height);
         graphics.endFill();
      }

      // How black the screen is (0..1).
      public static function blackAt(s:ArtefactHuntState, now:int) : Number
      {
         if(s == null)
         {
            return 0;
         }
         if(s.phase == ArtefactHuntState.FADE)
         {
            return s.phaseWhole > 0 ? ease(s.phaseGoneAt(now) / s.phaseWhole) : 1;
         }
         if(s.phase == ArtefactHuntState.COUNTDOWN)
         {
            return 1 - ease(s.phaseGoneAt(now) / BACK);
         }
         return 0;
      }

      protected static function ease(t:Number) : Number
      {
         t = Math.max(0,Math.min(1,t));
         return t * t * (3 - 2 * t);
      }

      public function update(s:ArtefactHuntState, now:int) : void
      {
         var black:Number = blackAt(s,now);
         alpha = black;
         visible = black > 0;
      }
   }
}
