package scaleform.gfx
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.InteractiveObject;
   import flash.display.Sprite;
   import flash.geom.Rectangle;

   // A model of Scaleform GFx's focus manager for the Ruffle harness. GFx implements this class
   // natively; the movies carry an empty stub (scaleform/gfx/FocusManager.as), which Ruffle would
   // run. check.sh builds a copy of the login movie with this one in its place; the main menu,
   // loaded into a child domain of it, gets it too (a parent domain's class wins). What it models: one focus and one modal clip (controller 0); a
   // focus is also Flash's stage.focus, so CLIK's FocusHandler sees it; "tab" finds the next
   // tab-enabled, visible object inside the modal clip (the whole stage without one), in GFx's
   // order (top to bottom, then left to right), looping.
   public final class FocusManager
   {
      public static const isModel:Boolean = true;

      private static var focus:InteractiveObject = null;

      private static var modal:Sprite = null;

      public function FocusManager()
      {
         super();
      }

      public static function set alwaysEnableArrowKeys(enable:Boolean) : void
      {
      }

      public static function get alwaysEnableArrowKeys() : Boolean
      {
         return false;
      }

      public static function set disableFocusKeys(disable:Boolean) : void
      {
      }

      public static function get disableFocusKeys() : Boolean
      {
         return false;
      }

      public static function moveFocus(keyToSimulate:String, startFromMovie:InteractiveObject = null, includeFocusEnabledChars:Boolean = false, controllerIdx:uint = 0) : InteractiveObject
      {
         var next:InteractiveObject = findFocus(keyToSimulate,null,true,startFromMovie || focus);
         if(next != null)
         {
            setFocus(next);
         }
         return next;
      }

      public static function findFocus(keyToSimulate:String, parentMovie:DisplayObjectContainer = null, loop:Boolean = false, startFromMovie:InteractiveObject = null, includeFocusEnabledChars:Boolean = false, controllerIdx:uint = 0) : InteractiveObject
      {
         var scope:DisplayObjectContainer = parentMovie || modal;
         if(scope == null && startFromMovie != null)
         {
            scope = startFromMovie.stage;
         }
         if(scope == null || scope.stage == null)
         {
            return null;
         }
         var found:Array = [];
         collect(scope,found);
         found.sortOn(["y","x"],Array.NUMERIC);
         if(found.length == 0)
         {
            return null;
         }
         var back:Boolean = keyToSimulate == "shifttab" || keyToSimulate == "up" || keyToSimulate == "left";
         var at:int = -1;
         for(var i:int = 0; i < found.length; i++)
         {
            if(found[i].object == startFromMovie)
            {
               at = i;
            }
         }
         var next:int = at == -1 ? (back ? found.length - 1 : 0) : at + (back ? -1 : 1);
         if(next < 0 || next >= found.length)
         {
            if(!loop)
            {
               return null;
            }
            next = (next + found.length) % found.length;
         }
         return found[next].object;
      }

      private static function collect(container:DisplayObjectContainer, found:Array) : void
      {
         for(var i:int = 0; i < container.numChildren; i++)
         {
            var child:DisplayObject = container.getChildAt(i);
            if(child == null || child.stage == null || !child.visible)
            {
               continue;
            }
            var object:InteractiveObject = child as InteractiveObject;
            if(object != null && object.tabEnabled)
            {
               var bounds:Rectangle = object.getBounds(object.stage);
               found.push({object:object,x:bounds.x,y:bounds.y});
            }
            var inner:DisplayObjectContainer = child as DisplayObjectContainer;
            if(inner != null && inner.tabChildren)
            {
               collect(inner,found);
            }
         }
      }

      public static function setFocus(obj:InteractiveObject, controllerIdx:uint = 0) : void
      {
         focus = obj;
         if(obj != null && obj.stage != null)
         {
            obj.stage.focus = obj;
         }
      }

      public static function getFocus(controllerIdx:uint = 0) : InteractiveObject
      {
         return focus;
      }

      public static function get numFocusGroups() : uint
      {
         return 1;
      }

      public static function setFocusGroupMask(obj:InteractiveObject, mask:uint) : void
      {
      }

      public static function getFocusGroupMask(obj:InteractiveObject) : uint
      {
         return 1;
      }

      public static function setControllerFocusGroup(controllerIdx:uint, focusGroupIdx:uint) : Boolean
      {
         return false;
      }

      public static function getControllerFocusGroup(controllerIdx:uint) : uint
      {
         return 0;
      }

      public static function getControllerMaskByFocusGroup(focusGroupIdx:uint) : uint
      {
         return 1;
      }

      public static function getModalClip(controllerIdx:uint = 0) : Sprite
      {
         return modal;
      }

      public static function setModalClip(mc:Sprite, controllerIdx:uint = 0) : void
      {
         modal = mc;
      }
   }
}
