package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.MouseEvent;

   // The lobby's transient windows (docs/notes/transient-windows.md): History, the match
   // settings under Play, a dropdown's list. They behave as an OS's or a browser's menus:
   // - one at a time: opening one closes the others, but for the one it is opened from (a
   //   dropdown inside the match settings keeps them open);
   // - a press anywhere outside closes it, and the press goes on to what it is on;
   // - Esc closes it, the top one first (LobbyMenu.close_active_window);
   // - any regular window opening closes them all (Modals.open), and so does a resize;
   // - while open it is drawn over every regular window.
   // A transient enters as it opens (`open`) and leaves as it closes by its own path
   // (`closed`); closing it from here calls its `close`, which is what its own button does.
   public class Transient
   {

      // The lobby's root, whose children are the regular windows: a transient's child of the
      // root goes to its top while the transient is open, and back under the window it was
      // under when it closes. Without a root (a harness's part), nothing moves.
      public static var root:DisplayObjectContainer;

      // {view, opener, close, place}, the one opened last at the end; each is inside the one
      // before it (`opener` in the one before's `view`). `place`: null, or {top, under} for a
      // root's child raised while it is open (raise).
      private static var _open:Array = [];

      private static var _stage:Stage;

      public function Transient()
      {
         super();
      }

      // `view` opens. `opener`, the button that opens and closes it: a press on it is its own.
      // `close`, what its own close does (it calls `closed`).
      public static function open(view:DisplayObject, opener:DisplayObject, close:Function) : void
      {
         leave(view);
         closeOutside(opener != null ? opener : view);
         var entry:Object = {
            "view":view,
            "opener":opener,
            "close":close,
            "place":raise(view)
         };
         _open.push(entry);
         if(view.stage != null)
         {
            listen(view.stage);
         }
      }

      // `view` closed by its own path (its button, an item chosen): what was opened from it closes.
      public static function closed(view:DisplayObject) : void
      {
         for(var i:int = 0; i < _open.length; i++)
         {
            if(_open[i].view === view)
            {
               while(_open.length > i + 1)
               {
                  shut(_open[_open.length - 1]);
               }
               leave(view);
               return;
            }
         }
      }

      // A regular window opens (Modals.open): every transient closes.
      public static function closeAll() : void
      {
         while(_open.length > 0)
         {
            shut(_open[_open.length - 1]);
         }
      }

      // Esc: the top transient closes (true: there was one).
      public static function escape() : Boolean
      {
         if(_open.length == 0)
         {
            return false;
         }
         shut(_open[_open.length - 1]);
         return true;
      }

      public static function any() : Boolean
      {
         return _open.length > 0;
      }

      // Whether `view` is open now.
      public static function isOpen(view:DisplayObject) : Boolean
      {
         for each(var entry:Object in _open)
         {
            if(entry.view === view)
            {
               return true;
            }
         }
         return false;
      }

      // The transients `target` is not in close, the top first: a press outside them all closes
      // them all, a press in one closes only those opened from it.
      private static function closeOutside(target:DisplayObject) : void
      {
         while(_open.length > 0 && !holds(_open[_open.length - 1],target))
         {
            shut(_open[_open.length - 1]);
         }
      }

      private static function holds(entry:Object, target:DisplayObject) : Boolean
      {
         return within(entry.view,target) || entry.opener != null && within(entry.opener,target);
      }

      private static function within(box:DisplayObject, target:DisplayObject) : Boolean
      {
         if(target == null)
         {
            return false;
         }
         if(box === target)
         {
            return true;
         }
         var container:DisplayObjectContainer = box as DisplayObjectContainer;
         return container != null && container.contains(target);
      }

      // It leaves first, so its close finds it gone (its `closed` does nothing) and a close that
      // throws leaves it closed here all the same.
      private static function shut(entry:Object) : void
      {
         leave(entry.view);
         try
         {
            entry.close();
         }
         catch(err:Error)
         {
         }
      }

      private static function leave(view:DisplayObject) : void
      {
         for(var i:int = _open.length - 1; i >= 0; i--)
         {
            if(_open[i].view === view)
            {
               lower(_open[i]);
               _open.splice(i,1);
            }
         }
         if(_open.length == 0)
         {
            listen(null);
         }
      }

      // The root's child holding `view` goes to the top of the root (a dropdown's list, on the
      // popup canvas over every window, stays where it is).
      private static function raise(view:DisplayObject) : Object
      {
         if(root == null)
         {
            return null;
         }
         var top:DisplayObject = view;
         while(top != null && top.parent != root)
         {
            top = top.parent;
         }
         if(top == null)
         {
            return null;
         }
         var at:int = root.getChildIndex(top);
         if(at >= root.numChildren - 1)
         {
            return null;
         }
         var place:Object = {
            "top":top,
            "under":root.getChildAt(at + 1)
         };
         root.setChildIndex(top,root.numChildren - 1);
         return place;
      }

      // Back under the window it was under, if both are still there and it is above it.
      private static function lower(entry:Object) : void
      {
         var place:Object = entry.place;
         entry.place = null;
         if(place == null)
         {
            return;
         }
         var top:DisplayObject = place.top as DisplayObject;
         var under:DisplayObject = place.under as DisplayObject;
         if(top.parent != root || under.parent != root)
         {
            return;
         }
         if(root.getChildIndex(top) > root.getChildIndex(under))
         {
            root.setChildIndex(top,root.getChildIndex(under));
         }
      }

      // The stage's presses (capture: before what is pressed acts) and resizes while any is open.
      private static function listen(stage:Stage) : void
      {
         if(stage === _stage)
         {
            return;
         }
         if(_stage != null)
         {
            _stage.removeEventListener(MouseEvent.MOUSE_DOWN,onStageDown,true);
            _stage.removeEventListener(Event.RESIZE,onStageResize);
         }
         _stage = stage;
         if(_stage != null)
         {
            _stage.addEventListener(MouseEvent.MOUSE_DOWN,onStageDown,true,0,false);
            _stage.addEventListener(Event.RESIZE,onStageResize,false,0,false);
         }
      }

      private static function onStageDown(e:MouseEvent) : void
      {
         closeOutside(e.target as DisplayObject);
      }

      private static function onStageResize(e:Event) : void
      {
         closeAll();
      }
   }
}
