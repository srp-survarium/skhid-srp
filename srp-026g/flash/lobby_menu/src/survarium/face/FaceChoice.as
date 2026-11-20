package survarium.face
{
   import flash.events.Event;
   import flash.events.EventDispatcher;
   import flash.external.ExternalInterface;

   // The account's face (face-choice.md) as srp says it and as the player picks it: the one owner
   // of that state, which the Face button and the faces' window show (Event.CHANGE at each change).
   //
   // srp says it two ways. The exe (patch 123) gives the account's face and the faces' count from
   // srp's account (0x87, root.srp_faces), which comes at the sign-in and again at each change of
   // the account (a choice too). A choice is srp's wire (face-choice.md, Wire): the ranking request at FACE_CHOOSE +
   // face, answered by one row at FACES_FIRST, `F1;<done>;<face>;<count>` (done 1 chosen, 2
   // refused). A pick is shown at once; one choice is asked at a time and the last pick wins: the
   // answer to one sends the next when the player went on meanwhile. The lobby's character shows
   // the face when srp's account comes (the exe), so the movie draws nothing of the face itself.
   public class FaceChoice extends EventDispatcher
   {

      private static var _shared:FaceChoice;

      private static const FACES_FIRST:int = 2097000000;

      private static const FACES_LAST:int = FACES_FIRST + 99;

      private static const FACE_CHOOSE:int = FACES_FIRST + 10;

      // FACE_CHOOSE + face, inside the range.
      private static const FACES_MAX:int = FACES_LAST - FACE_CHOOSE + 1;

      // What srp's word says was done: only the state (the account), a choice made, refused.
      private static const STATED:int = 0;

      private static const CHOSEN:int = 1;

      private static const REFUSED:int = 2;

      // The faces of the game data (0: srp said nothing yet), the account's
      // face as srp last said it, and the face shown (the player's last pick
      // while it is not srp's yet).
      private var _count:int = 0;

      private var _face:int = 0;

      private var _shown:int = 0;

      // The face of the choice asked and not answered (-1: none).
      private var _asked:int = -1;

      public function FaceChoice()
      {
         super();
      }

      // The lobby's one: made by the first who needs it (the exe's word, the Face button).
      public static function shared() : FaceChoice
      {
         if(_shared == null)
         {
            _shared = new FaceChoice();
         }
         return _shared;
      }

      // LobbyMenu.srp_faces: srp's account, as the exe read it.
      public static function told(face:uint, count:uint) : void
      {
         shared().said(STATED,face,count);
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

      // `F1;<done>;<face>;<count>`: srp's answer to a choice.
      private function answer(row:String) : void
      {
         var head:Array = row.split(";");
         if(head.length < 4 || head[0] != "F1")
         {
            return;
         }
         this.said(int(head[1]),int(head[2]),int(head[3]));
      }

      // srp's word on the account's face: what was done, the face, the faces' count.
      private function said(done:int, face:int, count:int) : void
      {
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
         try
         {
            ExternalInterface.call("request_players_range",FACE_CHOOSE + this._shown,1);
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
