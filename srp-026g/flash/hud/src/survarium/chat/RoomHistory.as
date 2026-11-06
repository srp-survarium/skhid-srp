package survarium.chat
{
   // The room's last lines at a match's start (the server's room_chat.rs): asked once the match chat
   // is up, with a token its retries share; the server sends the lines once a match, then
   // "<request>:done", which ends the asking. Lines are taken only while asking, so a late or repeated
   // answer never shows them twice.
   public class RoomHistory
   {
      public static const PREFIX:String = "#room-chat:";
      public static const CHANNEL:uint = 10;
      private static const RETRY_MS:int = 2000;
      private static const TRIES:int = 3;
      private var _state:Object = {kind:"waiting"};
      private var _show:Function;

      public function RoomHistory(show:Function)
      {
         this._show = show;
      }

      // A new match: asked again once its chat is up.
      public function reset() : void
      {
         this._state = {kind:"waiting"};
      }

      public function update(now:int, ready:Boolean, send:Function) : void
      {
         if(!ready || send == null)
            return;
         if(this._state.kind == "waiting")
         {
            var nonce:String = new Date().time.toString(16) + uint(Math.random() * uint.MAX_VALUE).toString(16);
            this._state = {kind:"asking", request:PREFIX + nonce, next:now, tries:0};
         }
         if(this._state.kind == "asking" && this._state.tries < TRIES && int(now - this._state.next) >= 0)
         {
            this._state.next = now + RETRY_MS;
            this._state.tries++;
            send({text:this._state.request},CHANNEL);
         }
      }

      // Every line of the room's channel in a match: the request's own local echo and the server's
      // answers are consumed, a room line shown while asking, any other dropped.
      public function receive(value:Object) : Boolean
      {
         if(value.type != CHANNEL)
            return false;
         if(this._state.kind != "asking" || value.text == this._state.request)
            return true;
         // The server's answers (this ask's, or another hud request's).
         if(value.name == "System" && String(value.text).charAt(0) == "#")
         {
            if(value.text == this._state.request + ":done")
               this._state = {kind:"done"};
            return true;
         }
         this._show(value);
         return true;
      }
   }
}
