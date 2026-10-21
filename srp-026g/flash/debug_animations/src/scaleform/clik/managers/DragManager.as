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
         _dragCanvas.addChild(_dragTarget);
         _dragTarget.x = dest.x;
         _dragTarget.y = dest.y;
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
         while((obj) && !(obj is IDragSlot))
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

