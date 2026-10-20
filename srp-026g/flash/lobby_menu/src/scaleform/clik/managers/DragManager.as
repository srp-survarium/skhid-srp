package scaleform.clik.managers
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.interfaces.IDragSlot;
   
   public class DragManager
   {
      
      protected static var _stage:Stage;
      
      protected static var _dragCanvas:Sprite;
      
      protected static var _initialized:Boolean = false;
      
      protected static var _inDrag:Boolean = false;
      
      protected static var _dragData:Object;
      
      protected static var _dragTarget:Sprite;
      
      protected static var _origDragSlot:IDragSlot;
      
      public function DragManager()
      {
         super();
      }
      
      public static function init(param1:Stage) : void
      {
         if(_initialized)
         {
            return;
         }
         _initialized = true;
         DragManager._stage = param1;
         _dragCanvas = new Sprite();
         _dragCanvas.mouseEnabled = _dragCanvas.mouseChildren = false;
         _stage.addChild(_dragCanvas);
         _stage.addEventListener(DragEvent.DRAG_START,DragManager.handleStartDragEvent,false,0,true);
      }
      
      public static function getDragCanvas() : Sprite
      {
         return _dragCanvas;
      }
      
      public static function inDrag() : Boolean
      {
         return _inDrag;
      }
      
      public static function handleStartDragEvent(param1:DragEvent) : void
      {
         if(param1.dragTarget == null || param1.dragSprite == null)
         {
            return;
         }
         var _loc2_:Sprite = param1.dragSprite;
         _dragData = param1.dragData;
         _dragTarget = param1.dragData.dragIcon;
         _origDragSlot = param1.dragTarget;
         var _loc3_:Point = new Point(_loc2_.x,_loc2_.y);
         var _loc4_:Point = _loc2_.localToGlobal(_loc3_);
         if(_dragCanvas.numChildren)
         {
            _dragCanvas.removeChildAt(0);
            trace("CANVAS::removedChild");
         }
         _dragCanvas.addChild(_dragTarget);
         var _loc5_:int = 0;
         var _loc6_:int = 0;
         if(_dragTarget)
         {
            _loc5_ = _dragTarget.width / 2;
            _loc6_ = _dragTarget.height / 2;
         }
         var _loc7_:int = _loc4_.x + _loc5_ - _stage.mouseX;
         var _loc8_:int = _loc4_.y + _loc6_ - _stage.mouseY;
         trace("DragManager::handleStartDragEvent::",_loc4_.x,_loc4_.y,_loc7_,_loc8_);
         _dragTarget.x = _loc4_.x;
         _dragTarget.y = _loc4_.y;
         var _loc9_:int = 30;
         if(_loc7_ > _loc9_)
         {
            _dragTarget.x -= _loc7_ - _loc9_;
         }
         if(_loc8_ > _loc9_)
         {
            _dragTarget.y -= _loc8_ - _loc9_;
         }
         _inDrag = true;
         _stage.addEventListener(MouseEvent.MOUSE_UP,handleEndDragEvent,false,0,true);
         var _loc10_:MovieClip = _dragTarget as MovieClip;
         _loc10_.startDrag();
         _loc10_.mouseEnabled = _loc10_.mouseChildren = false;
         _loc10_.trackAsMenu = true;
      }
      
      public static function handleEndDragEvent(param1:MouseEvent) : void
      {
         _stage.removeEventListener(MouseEvent.MOUSE_UP,handleEndDragEvent,false);
         _inDrag = false;
         var _loc2_:Boolean = false;
         var _loc3_:IDragSlot = findSpriteAncestorOf(_dragTarget.dropTarget) as IDragSlot;
         trace("DragManager::handleEndDragEvent:",_loc3_,_loc3_ is IDragSlot,_loc3_ != _origDragSlot,_origDragSlot);
         if(_loc3_ != null && _loc3_ is IDragSlot && _loc3_ != _origDragSlot)
         {
            _loc5_ = new DragEvent(DragEvent.DROP,_dragData,_origDragSlot,_loc3_,_dragTarget);
            _loc2_ = _loc3_.handleDropEvent(_loc5_);
         }
         _dragTarget.stopDrag();
         _dragTarget.mouseEnabled = _dragTarget.mouseChildren = true;
         (_dragTarget as MovieClip).trackAsMenu = false;
         _dragCanvas.removeChild(_dragTarget);
         var _loc4_:DragEvent = new DragEvent(DragEvent.DRAG_END,_dragData,_origDragSlot,_loc3_,_dragTarget);
         var _loc5_:DragEvent;
         if(param1.target is Stage)
         {
            _loc2_ = true;
            _origDragSlot.handleDropEvent(_loc4_);
         }
         _origDragSlot.handleDragEndEvent(_loc4_,_loc2_);
         _origDragSlot.dispatchEvent(_loc4_);
         _dragTarget = null;
         _origDragSlot = null;
      }
      
      protected static function handleStageAddedEvent(param1:Event) : void
      {
      }
      
      protected static function findSpriteAncestorOf(param1:DisplayObject) : IDragSlot
      {
         while(param1 && !(param1 is IDragSlot))
         {
            param1 = param1.parent;
         }
         return param1 as IDragSlot;
      }
      
      protected static function cloneDisplayObjectAsSprite(param1:DisplayObject) : DisplayObject
      {
         return param1;
      }
   }
}

