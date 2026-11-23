package survarium.player
{
   import flash.utils.getTimer;

   // A perk switch of the kit shown, one at a time (artefact-skills.md, "One switch at a time"):
   // the one owner of what the lobby draws of a kit while srp answers it (the owner, 2026-11-23:
   // "the operation is not atomic is how i see it"). srp answers a switch with the kit's account
   // (its pockets and items, the exe's `fillProfileItems`), then the presets list naming the
   // request. Drawing the one before the other shows a kit that is neither the old nor the new, so
   // while a switch is asked the kit's account is held, and its answer applies both at once.
   //
   // Its state: idle (`_asked` null), or a switch asked: its request, its kit, since when, and the
   // kit's account held (null: none yet). `step` is its one transition, from an event to the new
   // state and what the profile does now.
   public class PerkSwitch
   {

      // The events: a click asks (value {request, kit}); an account comes (the exe's fill, value
      // its data); a presets list comes (value the request it answers, -1 none); the time is up.
      public static const ASK:int = 0;

      public static const FILL:int = 1;

      public static const LIST:int = 2;

      public static const TIMEOUT:int = 3;

      // No answer in this long: what is held is drawn, srp's last word.
      public static const WAIT_MS:int = 3000;

      // Sends a request (the presets wire's offset: kit x 10 + perk x 2 + state).
      public var send:Function;

      private var _asked:Object = null;

      public function PerkSwitch(send:Function)
      {
         super();
         this.send = send;
      }

      // A switch is asked and not answered yet.
      public function get asked() : Boolean
      {
         return this._asked != null && getTimer() - this._asked.at < WAIT_MS;
      }

      // The transition: `event` with its `value`. What the profile does, now:
      // - `accepted` (ASK): the request was sent;
      // - `fills`: the accounts to draw, in order;
      // - `tell`: tell the paper doll srp's perk states (all of the kit, as last listed).
      // An expired switch ends first, as a TIMEOUT.
      public function step(event:int, value:*) : Object
      {
         var out:Object = {
            "accepted":false,
            "fills":[],
            "tell":false
         };
         if(this._asked != null && !this.asked)
         {
            this.end(out);
         }
         switch(event)
         {
            case ASK:
               if(this._asked == null)
               {
                  this._asked = {
                     "request":int(value.request),
                     "kit":uint(value.kit),
                     "at":getTimer(),
                     "fill":null
                  };
                  out.accepted = true;
                  this.send(int(value.request));
               }
               break;
            case FILL:
               if(this._asked != null && uint(value.profileId) == this._asked.kit)
               {
                  this._asked.fill = value;
               }
               else
               {
                  // Another kit shown meanwhile: the switch's kit is no longer drawn.
                  this._asked = null;
                  out.fills.push(value);
                  out.tell = true;
               }
               break;
            case LIST:
               if(this._asked == null)
               {
                  out.tell = true;
               }
               else if(int(value) == this._asked.request)
               {
                  this.end(out);
               }
               break;
            case TIMEOUT:
               if(this._asked != null)
               {
                  this.end(out);
               }
         }
         return out;
      }

      // The switch over: the account held drawn, and srp's states with it.
      private function end(out:Object) : void
      {
         if(this._asked.fill != null)
         {
            out.fills.push(this._asked.fill);
         }
         this._asked = null;
         out.tell = true;
      }
   }
}
