package survarium.modes
{
   import survarium.data.LocalizationData;

   // Freeze Tag: a short popup when a side goes down to its last one fighting, and when he is revealed
   // (the owner, 2026-11-26: "no need for it to be a timer, it should be a small popup for a few
   // seconds"). The exe tells each frame each side's reveal (HUD.freeze_tag_reveal, from the rule's
   // state and the match's time, the same in every game): whose and the ms left (0: revealed). A side
   // that starts counting gives one popup with its seconds left at that moment, rounded ("Last enemy
   // revealed in 15 s"; a late joiner's or a resynced game's real number); one that is revealed gives
   // "... revealed on the map". No ticking: the frames between tell nothing. The popup is the game's
   // own short message (HUD.set_parameterized_message), which hides itself after its time.
   public class RevealNotice
   {
      // Whose (the exe's numbers): the viewer himself, his last teammate, the enemy's last one.
      public static const VIEWER:uint = 1;
      public static const TEAMMATE:uint = 2;
      public static const ENEMY:uint = 3;

      // A side's phase.
      private static const NONE:uint = 0;
      private static const COUNTING:uint = 1;
      private static const REVEALED:uint = 2;

      // The popup's time on screen: down to one, then revealed (shorter).
      public static const COUNTING_MS:uint = 4000;
      public static const REVEALED_MS:uint = 3000;

      // Own side's phase, the other side's.
      private var _phases:Array = [NONE,NONE];

      // The popup's text and its time, or null for none this frame (each side's that changed phase).
      public var text:String = null;
      public var time:uint = 0;

      // Each side's whose (0 none) and ms left until he is revealed (0: he is). Sets text and time
      // when a side starts counting or is revealed.
      public function tell(own:uint, ownLeft:uint, enemy:uint, enemyLeft:uint) : void
      {
         var lines:Array = [];
         var revealedOnly:Boolean = true;
         var sides:Array = [[own,ownLeft],[enemy,enemyLeft]];
         for(var i:int = 0; i < 2; i++)
         {
            var who:uint = sides[i][0];
            var left:uint = sides[i][1];
            var phase:uint = who != VIEWER && who != TEAMMATE && who != ENEMY ? NONE : left > 0 ? COUNTING : REVEALED;
            if(phase != this._phases[i] && phase != NONE)
            {
               lines.push(line(who,left));
               revealedOnly = revealedOnly && phase == REVEALED;
            }
            this._phases[i] = phase;
         }
         text = lines.length ? lines.join("\n") : null;
         time = revealedOnly ? REVEALED_MS : COUNTING_MS;
      }

      public function reset() : void
      {
         this._phases = [NONE,NONE];
         text = null;
      }

      // A side's line: its seconds left, rounded (at least 1), or that he is revealed.
      private static function line(who:uint, left:uint) : String
      {
         if(left > 0)
         {
            return LocalizationData.format(who == VIEWER ? "st_srp_hud_freeze_reveal_self" :
               who == TEAMMATE ? "st_srp_hud_freeze_reveal_teammate" : "st_srp_hud_freeze_reveal_enemy",
               String(Math.max(1,Math.round(left / 1000))));
         }
         return LocalizationData.getName(who == VIEWER ? "st_srp_hud_freeze_revealed_self" :
            who == TEAMMATE ? "st_srp_hud_freeze_revealed_teammate" : "st_srp_hud_freeze_revealed_enemy");
      }
   }
}
