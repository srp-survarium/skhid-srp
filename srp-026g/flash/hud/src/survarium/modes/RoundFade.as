package survarium.modes
{
   import flash.display.Shape;

   // A round's restart, Artifact Hunt's and Freeze Tag's (0.46c2's show_fade_animation): the screen
   // goes black over the 1 s fade (phase 7), black at its end, when everyone is taken to his team's
   // spawns, and comes back over the first BACK ms of the round's countdown (phase 8). From the
   // phase and its time gone only, so a late join or a resync in the middle of it shows it where it
   // is. On the stage below the hud's root (as DeathEyes), in screen pixels: the countdown's text
   // stays over it, and hiding the interface leaves it, as the screen effects.
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

      // How black the screen is (0..1) in `phase` (ArtefactHuntState's ids), `gone` ms of its
      // `whole` in.
      public static function blackOf(phase:int, gone:Number, whole:Number) : Number
      {
         if(phase == ArtefactHuntState.FADE)
         {
            return whole > 0 ? ease(gone / whole) : 1;
         }
         if(phase == ArtefactHuntState.COUNTDOWN)
         {
            return 1 - ease(gone / BACK);
         }
         return 0;
      }

      public static function blackAt(s:ArtefactHuntState, now:int) : Number
      {
         return s == null ? 0 : blackOf(s.phase,s.phaseGoneAt(now),s.phaseWhole);
      }

      protected static function ease(t:Number) : Number
      {
         t = Math.max(0,Math.min(1,t));
         return t * t * (3 - 2 * t);
      }

      public function update(s:ArtefactHuntState, now:int) : void
      {
         this.show(blackAt(s,now));
      }

      public function show(black:Number) : void
      {
         alpha = black;
         visible = black > 0;
      }
   }
}
