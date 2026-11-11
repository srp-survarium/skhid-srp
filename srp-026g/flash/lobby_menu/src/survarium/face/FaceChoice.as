package survarium.face
{
   import flash.events.Event;
   import flash.events.EventDispatcher;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.utils.Timer;
   import flash.utils.getTimer;

   // The account's face (face-choice.md) as srp says it and as the player picks it: the one owner
   // of that state, which the Face button and the faces' window show (Event.CHANGE at each change).
   //
   // srp's wire (face-choice.md, Wire): the ranking request at FACES_FIRST (the state) and
   // FACE_CHOOSE + face (a choice), each answered by one row at FACES_FIRST,
   // `F1;<done>;<face>;<count>` (done: 0 the state, 1 chosen, 2 refused; face the account's, from
   // 0; count the faces). A pick is shown at once; one choice is asked at a time and the last pick
   // wins: the answer to one sends the next when the player went on meanwhile. The lobby's
   // character shows the face when srp's account (87) comes (the exe), so the movie draws nothing
   // of the face itself.
   public class FaceChoice extends EventDispatcher
   {

      private static var _shared:FaceChoice;

      private static const FACES_FIRST:int = 2097000000;

      private static const FACES_LAST:int = FACES_FIRST + 99;

      private static const FACE_CHOOSE:int = FACES_FIRST + 10;

      // FACE_CHOOSE + face, inside the range.
      private static const FACES_MAX:int = FACES_LAST - FACE_CHOOSE + 1;

      // What a row's header says was done.
      private static const CHOSEN:int = 1;

      private static const REFUSED:int = 2;

      // A choice unanswered this long (ms) is taken as lost: the face srp
      // last said is shown, and the state asked.
      private static const ANSWER_MS:int = 5000;

      // The state asked every RETRY_MS until it comes: the asks sent while
      // the movie loads, before the game takes them, are lost (as the
      // presets', LocalPlayerProfile).
      private static const RETRY_MS:int = 2000;

      // The faces of the game data (0: srp said nothing yet), the account's
      // face as srp last said it, and the face shown (the player's last pick
      // while it is not srp's yet).
      private var _count:int = 0;

      private var _face:int = 0;

      private var _shown:int = 0;

      // The face of the choice asked and not answered (-1: none), and when.
      private var _asked:int = -1;

      private var _askedAt:int = 0;

      private var _retry:Timer;

      public function FaceChoice()
      {
         super();
         this._retry = new Timer(RETRY_MS,0);
         this._retry.addEventListener(TimerEvent.TIMER,this.onRetry);
         this._retry.start();
         this.send(FACES_FIRST);
      }

      // The lobby's one: made by the first who asks (the player panel's Face button, made with the
      // lobby), which asks srp the state.
      public static function shared() : FaceChoice
      {
         if(_shared == null)
         {
            _shared = new FaceChoice();
         }
         return _shared;
      }

      // PlayerRanking.fillPlayers: rows of the faces' range are an answer
      // (true: taken).
      public static function take(rows:Array) : Boolean
      {
         if(rows == null || rows.length == 0 || rows[0] == null)
         {
            return false;
         }
         var place:Number = Number(rows[0].place);
         if(place < FACES_FIRST || place > FACES_LAST)
         {
            return false;
         }
         shared().answer(String(rows[0].nickname));
         return true;
      }

      // RoomsWindow.playEnabled (by LocalPlayerProfile.lobbyLive): the game
      // takes asks now; the state asked if it has not come.
      public static function lobbyLive() : void
      {
         if(shared()._count == 0)
         {
            shared().send(FACES_FIRST);
         }
      }

      // The faces of the game data: 0 until srp says, 1 when there is nothing to choose.
      public function get count() : int
      {
         return this._count;
      }

      // The face shown as the account's: the last pick, or srp's.
      public function get shown() : int
      {
         return this._shown;
      }

      // The player picks a face of the list: shown at once, asked of srp.
      public function pick(face:int) : void
      {
         if(face < 0 || face >= this._count || face == this._shown)
         {
            return;
         }
         this._shown = face;
         this.changed();
         this.askNext();
      }

      // `F1;<done>;<face>;<count>`: srp's word on the account's face.
      private function answer(row:String) : void
      {
         var head:Array = row.split(";");
         if(head.length < 4 || head[0] != "F1")
         {
            return;
         }
         var done:int = int(head[1]);
         var face:int = int(head[2]);
         var count:int = int(head[3]);
         if(count < 1 || face < 0 || face >= count)
         {
            return;
         }
         // The player went on from srp's last word: that pick stays shown.
         var picked:Boolean = this._count > 0 && this._shown != this._face;
         var asked:int = this._asked;
         if(done == CHOSEN || done == REFUSED)
         {
            this._asked = -1;
         }
         this._count = Math.min(count,FACES_MAX);
         this._face = face;
         if(!picked || this._shown >= this._count || done == REFUSED && this._shown == asked)
         {
            this._shown = face;
         }
         this.changed();
         this.askNext();
      }

      // The face shown asked for, unless srp has it or a choice is asked.
      private function askNext() : void
      {
         if(this._asked >= 0 || this._shown == this._face)
         {
            return;
         }
         this._asked = this._shown;
         this._askedAt = getTimer();
         this.send(FACE_CHOOSE + this._shown);
      }

      private function onRetry(e:TimerEvent) : void
      {
         if(this._count == 0)
         {
            this.send(FACES_FIRST);
         }
         else if(this._asked >= 0 && getTimer() - this._askedAt > ANSWER_MS)
         {
            this._asked = -1;
            this._shown = this._face;
            this.changed();
            this.send(FACES_FIRST);
         }
      }

      private function send(start:int) : void
      {
         try
         {
            ExternalInterface.call("request_players_range",start,1);
         }
         catch(e:Error)
         {
         }
      }

      private function changed() : void
      {
         dispatchEvent(new Event(Event.CHANGE));
      }
   }
}
