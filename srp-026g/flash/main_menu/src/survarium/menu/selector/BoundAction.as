package survarium.menu.selector
{
   import survarium.constants.ActionGroup;
   import survarium.constants.KeyBindGroup;

   // A row of the options' controls as the selector reads it once (root.set_keybindings' objects:
   // action_id, label, key, group_id, type): its action, label, key (null: none) and group. A row
   // of type 0 (the chat's Send message and Send to) has a fixed key and is never offered.
   public class BoundAction
   {

      public var id:uint;

      public var label:String;

      public var key:String;

      public var group:ActionGroup;

      // Lobby keys and the match's never take each other's key (the binder's kinds, PR #2082): a key
      // holds one lobby action and one of the match.
      public var lobby:Boolean;

      public function BoundAction(id:uint, label:String, key:String, lobby:Boolean)
      {
         super();
         this.id = id;
         this.label = label;
         this.key = key;
         this.group = ActionGroup.of(id);
         this.lobby = lobby;
      }

      // The rows the player may give a key, in the table's order; any other row left out.
      public static function parse(rows:Array) : Array
      {
         var actions:Array = [];
         for each(var row:Object in rows)
         {
            if(row == null || row.type == 0 || row.action_id == undefined)
            {
               continue;
            }
            var key:String = row.key == undefined || row.key == null || String(row.key) == "" ? null : String(row.key);
            actions.push(new BoundAction(uint(row.action_id),String(row.label),key,row.group_id == KeyBindGroup.LOBBY));
         }
         return actions;
      }

      // Whether this action, given `key`, takes it from `other` (the game's conflict: the same kind).
      public function takesFrom(other:BoundAction) : Boolean
      {
         return other != this && other.lobby == this.lobby;
      }
   }
}
