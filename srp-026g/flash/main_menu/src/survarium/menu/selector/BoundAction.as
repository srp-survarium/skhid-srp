package survarium.menu.selector
{
   import survarium.constants.KeyBindGroup;

   // A row of the options' controls as the selector reads it once (root.set_keybindings' objects:
   // action_id, label, key, group_id, type): its action, label, key's text (null: none), its kind
   // (SrpKeysWindow.kindOf: KINDS' index, -1 none) and colour, and whether it is on the key being
   // edited. A row of type 0 (the chat's Send message and Send to) has a fixed key and is never offered.
   public class BoundAction
   {

      public var id:uint;

      public var label:String;

      public var key:String;

      public var kind:int;

      public var colour:uint;

      // Lobby keys and the match's never take each other's key (the binder's kinds, PR #2082): a key
      // holds one lobby action and one of the match.
      public var lobby:Boolean;

      // On the key the selector is open for (the request's `actions`).
      public var onKey:Boolean;

      public function BoundAction(id:uint, label:String, key:String, lobby:Boolean, onKey:Boolean)
      {
         super();
         this.id = id;
         this.label = label;
         this.key = key;
         this.kind = SrpKeysWindow.kindOf(id);
         this.colour = this.kind == -1 ? Paint.DIM : uint(SrpKeysWindow.KINDS[this.kind].colour);
         this.lobby = lobby;
         this.onKey = onKey;
      }

      // The rows the player may give a key, in the table's order, any other left out; those of
      // `onKey` (rows) marked as the key's.
      public static function parse(rows:Array, onKey:Array) : Array
      {
         var ids:Array = [];
         for each(var bound:Object in onKey)
         {
            if(bound != null)
            {
               ids.push(uint(bound.action_id));
            }
         }
         var actions:Array = [];
         for each(var row:Object in rows)
         {
            if(row == null || row.type == 0 || row.action_id == undefined)
            {
               continue;
            }
            var key:String = row.key == undefined || row.key == null || String(row.key) == "" ? null : String(row.key);
            var id:uint = uint(row.action_id);
            actions.push(new BoundAction(id,String(row.label),key,row.group_id == KeyBindGroup.LOBBY,ids.indexOf(id) != -1));
         }
         return actions;
      }

      // Whether this action, given the key, takes it from `other` (the game's conflict: the same kind).
      public function takesFrom(other:BoundAction) : Boolean
      {
         return other != this && other.lobby == this.lobby;
      }
   }
}
