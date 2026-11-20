package survarium.modes
{
   // Shared lifecycle for mode introductions. The server owns per-account allowances;
   // this owner waits for the HUD, retries the request and owns the temporary chat reveal.
   // A later mode supplies its key/text and its own server eligibility rule.
   public class ModeIntroduction
   {
      public static const PREFIX:String = "#mode-intro:";
      public static const CHANNEL:uint = 10;
      public static const DISPLAY_MS:int = 20000;
      private var _state:Object = {kind:"inactive"};
      private var _show:Function;
      private var _reveal:Function;

      public function ModeIntroduction(show:Function, reveal:Function)
      {
         this._show = show;
         this._reveal = reveal;
      }

      public function begin(mode:String) : void
      {
         if(this._state.kind == "inactive")
            this._state = {kind:"waiting", mode:mode};
      }

      public function reset() : void
      {
         this._state = {kind:"inactive"};
         this._reveal(false);
      }

      public function update(now:int, ready:Boolean, send:Function) : void
      {
         if(this._state.kind == "showing" && int(now - this._state.until) >= 0)
         {
            this._state = {kind:"finished"};
            this._reveal(false);
         }
         if(!ready || send == null)
            return;
         if(this._state.kind == "waiting")
         {
            // New HUD activation: retries share this token, a reconnect gets another.
            var nonce:String = new Date().time.toString(16) + uint(Math.random() * uint.MAX_VALUE).toString(16);
            this._state = {kind:"requesting", request:PREFIX + this._state.mode + ":" + nonce,
                          mode:this._state.mode, next:now};
         }
         if(this._state.kind == "requesting" && int(now - this._state.next) >= 0)
         {
            this._state.next = now + 2000;
            // Same native callback as normal chat, without touching the player's input/focus.
            send({text:this._state.request},CHANNEL);
         }
      }

      // Also consumes the native local echo of requests. Only the reserved System sender's reply
      // to this activation may show anything; duplicate/old replies never restart the timer.
      public function receive(value:Object, now:int) : Boolean
      {
         if(value.type != CHANNEL || String(value.text).indexOf(PREFIX) != 0)
            return false;
         if(this._state.kind == "requesting" && value.name == "System")
         {
            if(value.text == this._state.request + ":show")
            {
               var mode:String = this._state.mode;
               this._state = {kind:"showing", until:now + DISPLAY_MS};
               this._show(mode);
               this._reveal(true);
            }
            else if(value.text == this._state.request + ":done")
               this._state = {kind:"finished"};
         }
         return true;
      }
   }
}
