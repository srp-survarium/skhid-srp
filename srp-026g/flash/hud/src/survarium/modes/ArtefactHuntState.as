package survarium.modes
{
   // Artifact Hunt (mode 13, docs/notes/artefact_hunt.md "HUD calls"): the whole state of one
   // root.artefact_hunt_update, parsed once (parse: null for a call outside the contract's domains).
   // Every view draws from the last one and the time since it came: a late join, a resync and a
   // rollback are just the next call.
   public class ArtefactHuntState
   {
      // The phase (#4): 0.46c2's state ids.
      public static const BEFORE_START:int = 0;
      public static const WAITING:int = 2;
      public static const SPAWNED:int = 3;
      public static const CARRIED:int = 4;
      public static const DROPPED:int = 5;
      public static const BROUGHT:int = 6;
      public static const FADE:int = 7;
      public static const COUNTDOWN:int = 8;
      public static const FINISHED:int = 10;

      protected static const PHASES:Array = [BEFORE_START,WAITING,SPAWNED,CARRIED,DROPPED,BROUGHT,FADE,COUNTDOWN,FINISHED];

      // What follows the 3 s after a delivery (#7).
      public static const AFTER_NONE:int = 0;
      public static const AFTER_END:int = 1;
      public static const AFTER_ROUND:int = 2;

      // Sudden death (#10).
      public static const SUDDEN_OFF:int = 0;
      public static const SUDDEN_COMING:int = 1;
      public static const SUDDEN_ON:int = 2;

      // Where the artefact is (#15).
      public static const NOWHERE:int = 0;
      public static const LYING:int = 1;
      public static const HELD:int = 2;

      // #0 for a viewer without a team.
      public static const NO_TEAM:int = 3;

      public static const VALUES:int = 22;

      public static const PLACES:int = 20;

      // #0..#3
      public var ownTeam:int;
      public var scores:Array;
      public var goal:int;
      // #4..#7
      public var phase:int;
      public var phaseLeft:Number;
      public var phaseWhole:Number;
      public var after:int;
      // #8..#14
      public var clockLeft:Number;
      public var clockWhole:Number;
      public var sudden:int;
      public var ownAlive:int;
      public var ownTotal:int;
      public var enemyAlive:int;
      public var enemyTotal:int;
      // #15..#20
      public var artefact:int;
      public var x:Number;
      public var y:Number;
      // -1 none
      public var carrier:int;
      public var carrierTeam:int;
      public var viewerCarries:Boolean;
      // #21.. {team, x, y}
      public var bases:Array;

      // getTimer() when the call came: the ms values count down from it.
      public var received:int;

      public function ArtefactHuntState()
      {
         super();
      }

      // null with the reason traced for a call the contract does not allow; a viewer's team 3 and a
      // place 255 / team 2 "none" are the contract's own values.
      public static function parse(values:Array, now:int) : ArtefactHuntState
      {
         var why:String = refusal(values);
         if(why != null)
         {
            trace("artefact_hunt_update refused: " + why + " (" + values + ")");
            return null;
         }
         var s:ArtefactHuntState = new ArtefactHuntState();
         s.received = now;
         s.ownTeam = int(values[0]);
         s.scores = [int(values[1]),int(values[2])];
         s.goal = int(values[3]);
         s.phase = int(values[4]);
         s.phaseLeft = Number(values[5]);
         s.phaseWhole = Number(values[6]);
         s.after = int(values[7]);
         s.clockLeft = Number(values[8]);
         s.clockWhole = Number(values[9]);
         s.sudden = int(values[10]);
         s.ownAlive = int(values[11]);
         s.ownTotal = int(values[12]);
         s.enemyAlive = int(values[13]);
         s.enemyTotal = int(values[14]);
         s.artefact = int(values[15]);
         s.x = Number(values[16]);
         s.y = Number(values[17]);
         s.carrier = int(values[18]) == 255 ? -1 : int(values[18]);
         s.carrierTeam = int(values[19]) == 2 ? -1 : int(values[19]);
         s.viewerCarries = int(values[20]) != 0;
         s.bases = [];
         for(var i:int = 0; i < int(values[21]); i++)
         {
            s.bases.push({team:int(values[VALUES + 3 * i]),x:Number(values[VALUES + 3 * i + 1]),y:Number(values[VALUES + 3 * i + 2])});
         }
         return s;
      }

      protected static function refusal(v:Array) : String
      {
         if(v.length < VALUES)
         {
            return v.length + " values";
         }
         for(var i:int = 0; i < v.length; i++)
         {
            // every value is a whole count, ms or id but the places (x, -z: #16, #17, a base's)
            var place:Boolean = i == 16 || i == 17 || i >= VALUES && (i - VALUES) % 3 != 0;
            if(!(v[i] is Number) || !isFinite(v[i]) || !place && (v[i] < 0 || v[i] != Math.floor(v[i])))
            {
               return "#" + i + " not a number of the contract";
            }
         }
         if(!(v[0] == 0 || v[0] == 1 || v[0] == NO_TEAM))
         {
            return "own team";
         }
         if(v[3] < 1 || v[1] > v[3] || v[2] > v[3])
         {
            return "scores against the goal";
         }
         if(PHASES.indexOf(int(v[4])) < 0)
         {
            return "phase";
         }
         if(v[7] > AFTER_ROUND || (v[7] != AFTER_NONE) != (v[4] == BROUGHT))
         {
            return "what follows the delivery";
         }
         if(v[10] > SUDDEN_ON || v[10] == SUDDEN_COMING && v[4] != BROUGHT)
         {
            return "sudden death";
         }
         if(v[15] > HELD)
         {
            return "artefact";
         }
         var carried:Boolean = v[15] == HELD;
         if(carried != (v[18] < PLACES) || carried != (v[19] < 2) || !carried && (v[18] != 255 || v[19] != 2))
         {
            return "carrier";
         }
         if(v[20] > 1 || v[20] == 1 && !carried)
         {
            return "the viewer's carry";
         }
         if(v[21] > 2 || v.length < VALUES + 3 * v[21])
         {
            return "bases";
         }
         for(i = 0; i < v[21]; i++)
         {
            if(v[VALUES + 3 * i] > 1)
            {
               return "base " + i + "'s team";
            }
         }
         return null;
      }

      // The team shown as "own": a viewer without a team sees team 0 as his (the contract's counts).
      public function get viewTeam() : int
      {
         return this.ownTeam == NO_TEAM ? 0 : this.ownTeam;
      }

      public function isOwn(team:int) : Boolean
      {
         return team == this.viewTeam;
      }

      public function get ownScore() : int
      {
         return this.scores[this.viewTeam];
      }

      public function get enemyScore() : int
      {
         return this.scores[1 - this.viewTeam];
      }

      // ms left of the phase at `now` (getTimer()), from the call's own.
      public function phaseLeftAt(now:int) : Number
      {
         return Math.max(0,this.phaseLeft - (now - this.received));
      }

      // ms of the phase gone at `now`.
      public function phaseGoneAt(now:int) : Number
      {
         return Math.max(0,this.phaseWhole - this.phaseLeftAt(now));
      }

      // The 3 s before a new round (0.46c2's show_round_restart).
      public function get restarting() : Boolean
      {
         return this.phase == BROUGHT && this.after == AFTER_ROUND;
      }

      // The round's 5 s at the spawns, frozen.
      public function get frozen() : Boolean
      {
         return this.phase == COUNTDOWN;
      }

      public function get suddenDeath() : Boolean
      {
         return this.sudden == SUDDEN_ON;
      }
   }
}
