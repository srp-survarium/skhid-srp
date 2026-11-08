package survarium.watch
{
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.utils.Timer;
   import flash.utils.getTimer;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;

   // Save replay (research/local-replays-design.md 1.1): a match's replay saved as a file in the
   // game's `replays` folder. The Watch window's cards (MatchCard's save icon) and the results
   // screen (VictoryScreen's Save replay) save through here; one save at a time.
   // - The ask: `3D 00 <ASK> <match>` (victory_screen_request_player_data), then a poll
   //   `3B 00 <START> 1` (request_players_range) once a second.
   // - The lobby server's answer: one `91` row `R2;<status>;<match 8 hex>;<n>` (PlayerRanking's rows,
   //   taken by WatchWindow.take): `preparing` (n: the place in line), `ready` (n: the size), or why
   //   not (gone, not_yours, old_game, incompatible, busy, failed, off).
   // - `ready` comes just after the `9E` with the hidden link `replay <id> <token> <sha256> <size>`:
   //   video_tutorial_link_clicked with no argument hands it to the exe (patch revision 14's
   //   replay_link), which starts updater_skhid.exe; the updater downloads the file and says where it
   //   went (and what went wrong, if anything did) in a message box of its own.
   // A match is SAVED from then on for this session. The Open half's folder listing will say which
   // files are really there (markSaved).
   public class ReplaySave
   {
      // the lobby server's SAVE_ASK and SAVE_START
      public static const ASK:uint = 0xFFFFFF01;

      public static const START:uint = 2210000000;

      // polls with no answer, and how long `preparing` is waited for
      public static const POLLS:int = 30;

      public static const PREPARE_MS:int = 180000;

      public static const SAVE:String = "save";

      public static const SAVING:String = "saving";

      public static const SAVED:String = "saved";

      // the texts of the answers that end a save without a file
      private static const REFUSALS:Object = {
         "gone":"st_srp_replay_gone",
         "not_yours":"st_srp_replay_not_yours",
         "old_game":"st_srp_replay_old_game",
         "incompatible":"st_srp_replay_incompatible",
         "busy":"st_srp_replay_busy",
         "failed":"st_srp_replay_failed",
         "off":"st_srp_replay_off"
      };

      private static const NO_ANSWER:String = "st_srp_replay_no_answer";

      // The exe's callbacks (a harness may stand in for them, Ruffle has no exe).
      public static var call:Function = function(name:String, args:Array):void
      {
         if(args.length == 0)
         {
            ExternalInterface.call(name);
         }
         else
         {
            ExternalInterface.call(name,args[0],args[1]);
         }
      };

      // A text for the player: the game's message box with OK.
      public static var tell:Function = function(key:String):void
      {
         Message.showMessage("ok",LocalizationData.getName(key));
      };

      // match 8 hex -> SAVED
      private static var _saved:Object = {};

      // the save under way ({hex, match, at, polls, preparing}), null none
      private static var _pending:Object = null;

      private static var _timer:Timer;

      private static var _listeners:Array = [];

      public function ReplaySave()
      {
         super();
      }

      public static function hex(match:uint) : String
      {
         var text:String = match.toString(16);
         while(text.length < 8)
         {
            text = "0" + text;
         }
         return text;
      }

      // SAVE, SAVING or SAVED.
      public static function stateOf(match:uint) : String
      {
         var h:String = hex(match);
         if(_pending != null && _pending.hex == h)
         {
            return SAVING;
         }
         return _saved[h] === true ? SAVED : SAVE;
      }

      // Told (no argument) of every change of a match's state.
      public static function listen(listener:Function) : void
      {
         if(_listeners.indexOf(listener) < 0)
         {
            _listeners.push(listener);
         }
      }

      // The seam for the Open half: a file of match `hex` is in the replays folder.
      public static function markSaved(hex:String) : void
      {
         _saved[hex] = true;
         changed();
      }

      // Saves match `match`'s replay; nothing while another save is under way or it is saved.
      public static function save(match:uint) : void
      {
         if(match == 0 || _pending != null || stateOf(match) != SAVE)
         {
            return;
         }
         _pending = {
            "hex":hex(match),
            "match":match,
            "at":getTimer(),
            "polls":0,
            "preparing":false
         };
         exe("victory_screen_request_player_data",[ASK,match]);
         if(_timer == null)
         {
            _timer = new Timer(1000,0);
            _timer.addEventListener(TimerEvent.TIMER,onTimer);
         }
         _timer.reset();
         _timer.start();
         changed();
      }

      // The lobby server's rows: taken (true) when they are an `R2` answer.
      public static function take(rows:Array) : Boolean
      {
         var name:String = null;
         try
         {
            name = rows && rows.length > 0 && rows[0] ? String(rows[0].nickname) : "";
         }
         catch(e:Error)
         {
            return false;
         }
         if(name.indexOf("R2;") != 0)
         {
            return false;
         }
         var f:Array = name.split(";");
         if(_pending == null || f.length != 4 || f[2] != _pending.hex)
         {
            return true;
         }
         var status:String = f[1];
         if(status == "preparing")
         {
            _pending.preparing = true;
         }
         else if(status == "ready")
         {
            _saved[_pending.hex] = true;
            end(null);
            exe("video_tutorial_link_clicked",[]);
         }
         else if(status in REFUSALS)
         {
            end(REFUSALS[status] as String);
         }
         return true;
      }

      private static function onTimer(e:TimerEvent) : void
      {
         poll();
      }

      // The timer's tick: the next poll, or the end of a save with no answer.
      public static function poll() : void
      {
         if(_pending == null)
         {
            if(_timer != null)
            {
               _timer.stop();
            }
            return;
         }
         _pending.polls += 1;
         var waited:int = getTimer() - int(_pending.at);
         if(_pending.preparing ? waited >= PREPARE_MS : _pending.polls > POLLS)
         {
            end(NO_ANSWER);
            return;
         }
         exe("request_players_range",[START,1]);
      }

      // The save over: saved (no text), or why not.
      private static function end(key:String) : void
      {
         _pending = null;
         if(_timer != null)
         {
            _timer.stop();
         }
         changed();
         if(key != null)
         {
            try
            {
               tell(key);
            }
            catch(e:Error)
            {
            }
         }
      }

      private static function exe(name:String, args:Array) : void
      {
         try
         {
            call(name,args);
         }
         catch(e:Error)
         {
         }
      }

      private static function changed() : void
      {
         for each(var listener:Function in _listeners.concat())
         {
            try
            {
               listener();
            }
            catch(e:Error)
            {
            }
         }
      }
   }
}
