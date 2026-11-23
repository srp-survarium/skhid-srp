package survarium.player
{
   // The kit the exe shows, given in two calls, always in this order (`on_profile_changed`
   // 0x222330, from a status of srp and from a relocation it accepted): its items
   // (`fill_player_profile_items`, the kit named), then its counts (`update_profile_slots_count`,
   // no kit named). A kit is drawn only whole, so no draw lays one kit's items on another kit's
   // counts (revision 72 did: the paper doll took the new kit's extra pockets for missing slots or
   // containers and sent their items back, and srp took them off the kit).
   //
   // Its state: the kit drawn last (null: none yet) and the items waiting for their counts (null:
   // none). Items always start a new kit; counts complete the kit waiting, or else are a change of
   // the kit drawn (its backpack put on or taken off). Counts never wait, so a half that comes
   // alone can not pair with another kit's. `step` is its one transition.
   public class KitShown
   {

      // The calls: the items (value the exe's fill; one that carries `item_slots` and
      // `artifact_slots` is whole by itself), the counts (value {pockets, containers}).
      public static const ITEMS:int = 0;

      public static const COUNTS:int = 1;

      private var _drawn:Object = null;

      private var _waiting:Object = null;

      public function KitShown()
      {
         super();
      }

      // The kit to draw after `event` (a fill with its counts in `item_slots` and
      // `artifact_slots`), or null: the items wait for their counts, or counts came before any kit.
      public function step(event:int, value:Object) : Object
      {
         if(event == ITEMS)
         {
            if(value.item_slots == undefined)
            {
               this._waiting = value;
               return null;
            }
            this._waiting = null;
            this._drawn = value;
            return value;
         }
         var items:Object = this._waiting != null ? this._waiting : this._drawn;
         this._waiting = null;
         if(items == null)
         {
            return null;
         }
         this._drawn = {
            "profileId":items.profileId,
            "items":items.items,
            "item_slots":value.pockets,
            "artifact_slots":value.containers
         };
         return this._drawn;
      }
   }
}
