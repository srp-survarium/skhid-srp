package survarium.player
{
   // The kit the exe shows, given in two calls (`on_profile_changed` 0x222330, from a status of srp
   // and from a relocation it accepted): its items (`fill_player_profile_items`, the kit named),
   // then its counts (`update_profile_slots_count`, no kit named). A kit is drawn only whole: one
   // half waits for the other, in either order, so no draw ever lays one kit's items on another
   // kit's counts (revision 72 did: the paper doll took the new kit's extra pockets for missing
   // slots or containers and sent their items back, and srp took them off the kit).
   //
   // Its state: nothing waiting (null), or one half waiting ({half, value}). `step` is its one
   // transition: an event and its value to the new state and the kit made whole, if one is.
   public class KitShown
   {

      // The halves: the items (value the exe's fill; one that carries `item_slots` and
      // `artifact_slots` is whole by itself), the counts (value {pockets, containers}).
      public static const ITEMS:int = 0;

      public static const COUNTS:int = 1;

      private var _waiting:Object = null;

      public function KitShown()
      {
         super();
      }

      // The kit made whole by `event` (the fill, its counts set in its `item_slots` and
      // `artifact_slots`), or null while its other half has not come. A half that comes again
      // before the other replaces the one waiting.
      public function step(event:int, value:Object) : Object
      {
         if(event == ITEMS && value.item_slots != undefined)
         {
            this._waiting = null;
            return value;
         }
         if(this._waiting == null || this._waiting.half == event)
         {
            this._waiting = {
               "half":event,
               "value":value
            };
            return null;
         }
         var items:Object = event == ITEMS ? value : this._waiting.value;
         var counts:Object = event == COUNTS ? value : this._waiting.value;
         this._waiting = null;
         items.item_slots = counts.pockets;
         items.artifact_slots = counts.containers;
         return items;
      }
   }
}
