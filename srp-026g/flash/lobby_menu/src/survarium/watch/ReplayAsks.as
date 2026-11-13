package survarium.watch
{
   import flash.external.ExternalInterface;
   import flash.utils.getTimer;
   import survarium.controls.Message;
   import survarium.controls.NotificationListScrollContent;
   import survarium.data.LocalizationData;

   // The one ask-and-poll loop for a match's replay, ticked by the Watch window's second
   // (WatchWindow.onTimer). A watch or a save asks once, `3D 00 <player> <match>`, then polls
   // `3B 00 <start> 1` once a second until the lobby server's `91` row answers it, 30 polls at most,
   // or 180 s once it says `preparing`:
   // - a watch: `R1;<status>;<match 8 hex>;<n>`, polled at WATCH_START. `watching` means the game is
   //   in the match; WatchPrompt shows what happens meanwhile. One watch at a time.
   // - a save (research/local-replays-design.md 1.1): `R2;<status>;<match 8 hex>;<n>`, polled at
   //   SAVE_START. `ready` comes just after the `9E` with the hidden link `replay <id> <token>
   //   <sha256> <size>`: video_tutorial_link_clicked with no argument hands it to the exe (patch
   //   revision 14's replay_link), which starts updater_skhid.exe; the updater downloads the file,
   //   silently when it succeeds, a message box of its own when it fails. The lobby hears nothing
   //   of that: SAVED says only that the link went to the updater, and Save may be pressed again
   //   (a failed download retried). One save at a time.
   // The rows reach take through WatchWindow.take.
   public class ReplayAsks
   {
      // the lobby server's REPLAY_START, SAVE_ASK and SAVE_START
      public static const WATCH_START:uint = 2200000000;

      public static const SAVE_ASK:uint = 0xFFFFFF01;

      public static const SAVE_START:uint = 2210000000;

      private static const POLLS:int = 30;

      private static const PREPARE_MS:int = 180000;

      public static const SAVE:String = "save";

      public static const SAVING:String = "saving";

      public static const SAVED:String = "saved";

      // the texts of the answers that end an ask without a watch or a file
      private static const REFUSALS:Object = {
         "gone":"st_srp_replay_gone",
         "not_yours":"st_srp_replay_not_yours",
         "old_game":"st_srp_replay_old_game",
         "incompatible":"st_srp_replay_incompatible",
         "busy":"st_srp_replay_busy",
         "failed":"st_srp_replay_failed",
         "off":"st_srp_replay_off"
      };

      // what an answer says of its match's replay (NotificationListScrollContent.replayReason): it
      // cannot play (a reason), or it may (null); the other answers say nothing of it
      private static const REASONS:Object = {
         "watching":null,
         "preparing":null,
         "incompatible":NotificationListScrollContent.REASON_OLD,
         "gone":NotificationListScrollContent.REASON_GONE
      };

      private static const NO_ANSWER:String = "st_srp_replay_no_answer";

      private static const STILL_PREPARING:String = "st_srp_replay_still_preparing";

      // what the window is told (as of an answer) of a watch that ended here without one: no answer
      // in time, or the prompt closed
      private static const ENDED:String = "ended";

      // The window told (match 8 hex, status) of each answer about a watch.
      public static var told:Function;

      // the watch and the save under way ({hex, match, at, polls, preparing}), null none
      private static var _watch:Object = null;

      private static var _save:Object = null;

      // match 8 hex -> SAVED
      private static var _saved:Object = {};

      private static var _listeners:Array = [];

      public function ReplayAsks()
      {
         super();
      }

      public static function hex(match:uint) : String
      {
         var text:String = match.toString(16).toLowerCase();
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
         if(_save != null && _save.hex == h)
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

      // Asks to watch `match` with `ask` (a REPLAY_WATCH_* player, WatchRows.watchAsk); nothing
      // while another watch is being asked for (false).
      public static function watch(match:uint, ask:uint) : Boolean
      {
         if(_watch != null)
         {
            return false;
         }
         WatchPrompt.onClosed = promptClosed;
         _watch = begin(ask,match);
         WatchPrompt.asking();
         return true;
      }

      // Saves match `match`'s replay, again when SAVED (the updater's download may have failed);
      // nothing while a save is under way.
      public static function save(match:uint) : void
      {
         if(match == 0 || _save != null)
         {
            return;
         }
         _save = begin(SAVE_ASK,match);
         changed();
      }

      private static function begin(player:uint, match:uint) : Object
      {
         exe("victory_screen_request_player_data",player,match);
         return {
            "hex":hex(match),
            "match":match,
            "at":getTimer(),
            "polls":0,
            "preparing":false
         };
      }

      // The lobby server's rows: taken (true) when they are an `R1` or `R2` answer.
      public static function take(rows:Array) : Boolean
      {
         var name:String = null;
         var place:uint = 0;
         try
         {
            name = rows && rows.length > 0 && rows[0] ? String(rows[0].nickname) : "";
            place = name != "" ? uint(rows[0].place) : 0;
         }
         catch(e:Error)
         {
            return false;
         }
         // each answer's row is at its poll's start: a ranking row named so (a name may hold `;`)
         // is the ranking's
         var f:Array = name.split(";");
         if(f[0] == "R1" && place == WATCH_START)
         {
            watchAnswer(f);
            return true;
         }
         if(f[0] == "R2" && place == SAVE_START)
         {
            saveAnswer(f);
            return true;
         }
         return false;
      }

      private static function watchAnswer(f:Array) : void
      {
         var status:String = f.length > 1 ? String(f[1]) : "";
         var h:String = f.length > 2 ? String(f[2]).toLowerCase() : "";
         if(_watch == null || h != _watch.hex)
         {
            return;
         }
         tell(h,status);
         if(status == "watching")
         {
            // the game is in it already (the answer comes once it connected): no box over it
            _watch = null;
         }
         else if(status == "preparing")
         {
            _watch.preparing = true;
            WatchPrompt.preparing(int(_watch.at));
         }
         else if(status in REFUSALS)
         {
            _watch = null;
            WatchPrompt.show(REFUSALS[status] as String);
         }
      }

      private static function saveAnswer(f:Array) : void
      {
         var status:String = f.length > 1 ? String(f[1]) : "";
         if(_save == null || f.length != 4 || f[2] != _save.hex)
         {
            return;
         }
         if(status in REASONS)
         {
            noteOne(_save.hex,REASONS[status] as String);
         }
         if(status == "preparing")
         {
            _save.preparing = true;
         }
         else if(status == "ready")
         {
            _saved[_save.hex] = true;
            endSave(null);
            exe("video_tutorial_link_clicked");
         }
         else if(status in REFUSALS)
         {
            endSave(REFUSALS[status] as String);
         }
      }

      // The Watch window's second: each ask polls, or gives up.
      public static function second() : void
      {
         var now:int = getTimer();
         if(_watch != null)
         {
            if(pollAgain(_watch,WATCH_START,now))
            {
               if(_watch.preparing)
               {
                  WatchPrompt.preparing(int(_watch.at));
               }
            }
            else
            {
               var ended:Object = _watch;
               _watch = null;
               tell(ended.hex,ENDED);
               WatchPrompt.show(ended.preparing ? STILL_PREPARING : NO_ANSWER);
            }
         }
         if(_save != null && !pollAgain(_save,SAVE_START,now))
         {
            endSave(NO_ANSWER);
         }
      }

      private static function pollAgain(ask:Object, start:uint, now:int) : Boolean
      {
         ask.polls += 1;
         if(ask.preparing ? now - int(ask.at) >= PREPARE_MS : ask.polls > POLLS)
         {
            return false;
         }
         exe("request_players_range",start,1);
         return true;
      }

      // The prompt's Close: the watch is no longer asked for.
      private static function promptClosed() : void
      {
         if(_watch != null)
         {
            var ended:Object = _watch;
            _watch = null;
            tell(ended.hex,ENDED);
         }
      }

      // The save over: saved (no text), or why not, in the game's message box.
      private static function endSave(key:String) : void
      {
         _save = null;
         changed();
         if(key != null)
         {
            try
            {
               Message.showMessage("ok",LocalizationData.getName(key));
            }
            catch(e:Error)
            {
            }
         }
      }

      private static function tell(h:String, status:String) : void
      {
         if(status in REASONS)
         {
            noteOne(h,REASONS[status] as String);
         }
         if(told != null)
         {
            told(h,status);
         }
      }

      // What an answer or a list says of the replays of matches (8 hex -> a reason, null: it may be
      // watched): noted for the History's rows, and the listeners told when one changed (the results
      // screen drops its Watch replay and Save on a refusal: each press would count against the
      // server's lookups).
      public static function note(reasons:Object) : void
      {
         var changes:Boolean = false;
         for(var h:String in reasons)
         {
            var reason:String = reasons[h] as String;
            if(NotificationListScrollContent.replayReason(h) != reason)
            {
               NotificationListScrollContent.noteReplay(h,reason);
               changes = true;
            }
         }
         if(changes)
         {
            changed();
         }
      }

      private static function noteOne(h:String, reason:String) : void
      {
         var reasons:Object = {};
         reasons[h] = reason;
         note(reasons);
      }

      private static function exe(name:String, ... args) : void
      {
         try
         {
            ExternalInterface.call.apply(null,[name].concat(args));
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
