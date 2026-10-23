package survarium.controls
{
   // The lobby's one Esc rule (docs/notes/esc.md): Esc closes the topmost modal only. Every modal
   // (a top-menu window, the results, a popup, the Upgrade window, the rooms window, a message,
   // the right-click menu...) enters this stack as it opens and leaves it as it closes;
   // LobbyMenu.close_active_window, the one Esc handler, closes the top.
   //
   // A layer is drawn over the ones below it whatever the order (messages over every window, the
   // right-click menu over messages); in a layer, the modal opened last is on top. A modal whose
   // `shown` says it is not up (hidden under the results, closed by a path that did not leave) is
   // passed over, not dropped.
   public class Modals
   {

      public static const LOBBY:int = 0;

      public static const MESSAGE:int = 1;

      public static const MENU:int = 2;

      // {owner, layer, close, shown}, the top last
      private static var _stack:Array = [];

      public function Modals()
      {
         super();
      }

      // Enters (again: to the top of its layer). `close` is what Esc does: what the modal's own
      // close (or Cancel) does, or nothing for one that keeps Esc out a while (a reroll being
      // paid). `shown`, true while it is up.
      public static function open(owner:Object, layer:int, close:Function, shown:Function) : void
      {
         leave(owner);
         var at:int = _stack.length;
         while(at > 0 && _stack[at - 1].layer > layer)
         {
            at--;
         }
         _stack.splice(at,0,{
            "owner":owner,
            "layer":layer,
            "close":close,
            "shown":shown
         });
      }

      public static function leave(owner:Object) : void
      {
         var i:int = _stack.length - 1;
         while(i >= 0)
         {
            if(_stack[i].owner === owner)
            {
               _stack.splice(i,1);
            }
            i--;
         }
      }

      // The modal that takes Esc now (null: none).
      public static function top() : Object
      {
         var entry:Object = topEntry();
         return entry != null ? entry.owner : null;
      }

      public static function any() : Boolean
      {
         return topEntry() != null;
      }

      // Esc: the top modal's close (true: there was one).
      public static function escape() : Boolean
      {
         var entry:Object = topEntry();
         if(entry == null)
         {
            return false;
         }
         try
         {
            entry.close();
         }
         catch(err:Error)
         {
            // a close that throws must not keep every later Esc on it
            leave(entry.owner);
         }
         return true;
      }

      private static function topEntry() : Object
      {
         var i:int = _stack.length - 1;
         while(i >= 0)
         {
            if(isShown(_stack[i]))
            {
               return _stack[i];
            }
            i--;
         }
         return null;
      }

      private static function isShown(entry:Object) : Boolean
      {
         try
         {
            return Boolean(entry.shown());
         }
         catch(err:Error)
         {
         }
         return false;
      }
   }
}
