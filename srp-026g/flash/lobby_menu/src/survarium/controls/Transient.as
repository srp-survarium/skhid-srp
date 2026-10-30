package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Matrix;
   import flash.geom.Point;

   // The lobby's transient windows (docs/notes/transient-windows.md): History, the match
   // settings under Play, the character panel's presets list, a dropdown's list. They behave as
   // an OS's or a browser's menus:
   // - one at a time: opening one closes the others, but for the one it is opened from (a
   //   dropdown inside the match settings keeps them open);
   // - a press anywhere outside closes it, and the press goes on to what it is on;
   // - Esc closes it, the top one first (LobbyMenu.close_active_window);
   // - any regular window opening closes them all (Modals.open), and so does a resize;
   // - while open it is drawn over every regular window: on the transient layer.
   // A transient enters as it opens (`open`) and leaves as it closes by its own path
   // (`closed`); closing it from here calls its `close`, which is what its own button does.
   public class Transient
   {

      // The transient layer (LobbyMenu's, over the regular windows; CLIK's popup canvas is in
      // it): an open transient is carried onto it, where it is drawn as at home, and goes back
      // home as it closes. One already on it (a dropdown's list) stays. Without a layer (a
      // harness's part), nothing moves.
      public static var layer:DisplayObjectContainer;

      // {view, opener, close, place}, the one opened last at the end; each is inside the one
      // before it (`opener` in the one before's `view`). `place`: null, or {holder, home, at}
      // for one carried onto the layer (carry).
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
            "place":carry(view)
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

      // The top transient (null: none).
      public static function top() : DisplayObject
      {
         return _open.length > 0 ? _open[_open.length - 1].view : null;
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

      // A press Scaleform gives the holder itself (scaleform-movie-pitfalls) is the transient's too
      // (onHolderPress hands it on).
      private static function holds(entry:Object, target:DisplayObject) : Boolean
      {
         if(within(entry.view,target) || entry.opener != null && within(entry.opener,target))
         {
            return true;
         }
         return entry.place != null && entry.place.holder === target;
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
               home(_open[i]);
               _open.splice(i,1);
            }
         }
         if(_open.length == 0)
         {
            listen(null);
         }
      }

      // `view` onto the layer, in a holder drawn as its parent is (the layer's own coordinates),
      // so it is where it was and as big, over every window. Its own coordinates are kept: its
      // owner's layout still places it. A resize closes it (the holder would be stale).
      private static function carry(view:DisplayObject) : Object
      {
         var parent:DisplayObjectContainer = view.parent;
         if(layer == null || parent == null || view === layer || layer.contains(view))
         {
            return null;
         }
         var place:Object = {
            "holder":new Sprite(),
            "home":parent,
            "at":parent.getChildIndex(view)
         };
         var drawn:Matrix = parent.transform.concatenatedMatrix.clone();
         var into:Matrix = layer.transform.concatenatedMatrix.clone();
         into.invert();
         drawn.concat(into);
         place.holder.transform.matrix = drawn;
         place.holder.addEventListener(MouseEvent.MOUSE_DOWN,onHolderPress,false,0,false);
         layer.addChild(place.holder);
         place.holder.addChild(view);
         return place;
      }

      // Back home, where it was among its parent's children.
      private static function home(entry:Object) : void
      {
         var place:Object = entry.place;
         entry.place = null;
         if(place == null)
         {
            return;
         }
         var view:DisplayObject = entry.view;
         var parent:DisplayObjectContainer = place.home;
         if(view.parent === place.holder)
         {
            parent.addChildAt(view,Math.min(int(place.at),parent.numChildren));
         }
         place.holder.removeEventListener(MouseEvent.MOUSE_DOWN,onHolderPress,false);
         if(place.holder.parent != null)
         {
            place.holder.parent.removeChild(place.holder);
         }
      }

      // A press Scaleform gives the holder itself, not what is under it (scaleform-movie-pitfalls:
      // a control straight in a sprite gives the sprite the press), goes on to the transient as a
      // press at the same point: its own press handling finds what is there (the presets list its
      // row). One on the transient or in it is its own already.
      private static function onHolderPress(e:MouseEvent) : void
      {
         var holder:DisplayObjectContainer = e.currentTarget as DisplayObjectContainer;
         if(e.target !== holder || holder.numChildren == 0)
         {
            return;
         }
         var view:DisplayObject = holder.getChildAt(0);
         var at:Point = view.globalToLocal(new Point(e.stageX,e.stageY));
         view.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,at.x,at.y,null,e.ctrlKey,e.altKey,e.shiftKey,e.buttonDown));
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
