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
      
      public static function init(stage:Stage) : void
      {
         if(_initialized)
         {
            return;
         }
         _initialized = true;
         DragManager._stage = stage;
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
      
      public static function handleStartDragEvent(e:DragEvent) : void
      {
         if(e.dragTarget == null || e.dragSprite == null)
         {
            return;
         }
         var refSprite:Sprite = e.dragSprite;
         _dragData = e.dragData;
         _dragTarget = e.dragData.dragIcon;
         _origDragSlot = e.dragTarget;
         var origin:Point = new Point(refSprite.x,refSprite.y);
         var dest:Point = refSprite.localToGlobal(origin);
         if(_dragCanvas.numChildren)
         {
            _dragCanvas.removeChildAt(0);
            trace("CANVAS::removedChild");
         }
         _dragCanvas.addChild(_dragTarget);
         var offsetX:int = 0;
         var offsetY:int = 0;
         if(_dragTarget)
         {
            offsetX = _dragTarget.width / 2;
            offsetY = _dragTarget.height / 2;
         }
         var dst_x:int = dest.x + offsetX - _stage.mouseX;
         var dst_y:int = dest.y + offsetY - _stage.mouseY;
         trace("DragManager::handleStartDragEvent::",dest.x,dest.y,dst_x,dst_y);
         _dragTarget.x = dest.x;
         _dragTarget.y = dest.y;
         var limit:int = 30;
         if(dst_x > limit)
         {
            _dragTarget.x -= dst_x - limit;
         }
         if(dst_y > limit)
         {
            _dragTarget.y -= dst_y - limit;
         }
         _inDrag = true;
         _stage.addEventListener(MouseEvent.MOUSE_UP,handleEndDragEvent,false,0,true);
         var dragMC:MovieClip = _dragTarget as MovieClip;
         dragMC.startDrag();
         dragMC.mouseEnabled = dragMC.mouseChildren = false;
         dragMC.trackAsMenu = true;
      }
      
      public static function handleEndDragEvent(e:MouseEvent) : void
      {
         _stage.removeEventListener(MouseEvent.MOUSE_UP,handleEndDragEvent,false);
         _inDrag = false;
         var isValidDrop:Boolean = false;
         var dropTarget:IDragSlot = findSpriteAncestorOf(_dragTarget.dropTarget) as IDragSlot;
         trace("DragManager::handleEndDragEvent:",dropTarget,dropTarget is IDragSlot,dropTarget != _origDragSlot,_origDragSlot);
         if(dropTarget != null && dropTarget is IDragSlot && dropTarget != _origDragSlot)
         {
            dropEvent = new DragEvent(DragEvent.DROP,_dragData,_origDragSlot,dropTarget,_dragTarget);
            isValidDrop = dropTarget.handleDropEvent(dropEvent);
         }
         _dragTarget.stopDrag();
         _dragTarget.mouseEnabled = _dragTarget.mouseChildren = true;
         (_dragTarget as MovieClip).trackAsMenu = false;
         _dragCanvas.removeChild(_dragTarget);
         var dragEndEvent:DragEvent = new DragEvent(DragEvent.DRAG_END,_dragData,_origDragSlot,dropTarget,_dragTarget);
         var dropEvent:DragEvent;
         if(e.target is Stage)
         {
            isValidDrop = true;
            _origDragSlot.handleDropEvent(dragEndEvent);
         }
         _origDragSlot.handleDragEndEvent(dragEndEvent,isValidDrop);
         _origDragSlot.dispatchEvent(dragEndEvent);
         _dragTarget = null;
         _origDragSlot = null;
      }
      
      protected static function handleStageAddedEvent(e:Event) : void
      {
      }
      
      protected static function findSpriteAncestorOf(obj:DisplayObject) : IDragSlot
      {
         while(obj && !(obj is IDragSlot))
         {
            obj = obj.parent;
         }
         return obj as IDragSlot;
      }
      
      protected static function cloneDisplayObjectAsSprite(original:DisplayObject) : DisplayObject
      {
         return original;
      }
   }
}

