package survarium.modes
{
   import flash.display.MovieClip;

   // FFA reuses the match timer's timeline frame, but has no opposing team counters.
   public class FfaProgress
   {
      private var _bar:MovieClip = null;

      // Keep the native callback compatible: score updates must not restore the team UI.
      public function tell(own:int, top:int, target:int) : void
      {
         this.draw();
      }

      public function show(bar:MovieClip) : void
      {
         _bar = bar;
         this.draw();
      }

      public function hide() : void
      {
         _bar = null;
      }

      public function draw() : void
      {
         if(this._bar == null)
         {
            return;
         }
         for each(var name:String in ["team_1_score", "team_2_score", "team1Progress", "team2Progress",
                                       "team1LastPlayers", "team2LastPlayers"])
         {
            if(this._bar[name] != null)
            {
               this._bar[name].visible = false;
            }
         }
      }
   }
}
