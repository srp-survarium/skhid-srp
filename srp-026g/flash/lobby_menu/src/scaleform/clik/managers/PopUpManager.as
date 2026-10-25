package scaleform.clik.managers
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.geom.Point;
   import scaleform.gfx.FocusManager;
   
   public class PopUpManager
   {
      
      protected static var initialized:Boolean = false;
      
      protected static var _stage:Stage;
      
      protected static var _defaultPopupCanvas:MovieClip;

      // Where the popup canvas goes, kept on top of: the lobby's layers (LobbyMenu._layers, set
      // before any popup), else the stage.
      public static var layer:DisplayObjectContainer;

      protected static function get host() : DisplayObjectContainer
      {
         return layer ? layer : _stage;
      }
      
      protected static var _modalMc:Sprite;
      
      protected static var _modalBg:Sprite;
      
      public function PopUpManager()
      {
         super();
      }
      
      public static function init(stage:Stage) : void
      {
         if(initialized)
         {
            return;
         }
         PopUpManager._stage = stage;
         _defaultPopupCanvas = new MovieClip();
         _defaultPopupCanvas.addEventListener(Event.REMOVED,handleRemovePopup,false,0,true);
         host.addChild(_defaultPopupCanvas);
         initialized = true;
      }
      
      public static function show(mc:DisplayObject, x:Number = 0, y:Number = 0, scope:DisplayObjectContainer = null) : void
      {
         if(!_stage)
         {
            trace("PopUpManager has not been initialized. Automatic initialization has not occured or has failed; call PopUpManager.init() manually.");
            return;
         }
         if(mc.parent)
         {
            mc.parent.removeChild(mc);
         }
         handleStageAddedEvent(null);
         _defaultPopupCanvas.addChild(mc);
         if(!scope)
         {
            scope = _stage;
         }
         // The canvas is on the stage; it takes the scale the popup's owner is drawn
         // at (a movie that scales itself: 2 at 4K), so a dropdown's list is as big
         // as its box; never below 1, so at 1080p and below lists are as they were.
         // The canvas, not the popup: a component takes a scale as a size.
         var origin:Point = scope.localToGlobal(new Point(0,0));
         _defaultPopupCanvas.scaleX = Math.max(1,(scope.localToGlobal(new Point(100,0)).x - origin.x) / 100);
         _defaultPopupCanvas.scaleY = Math.max(1,(scope.localToGlobal(new Point(0,100)).y - origin.y) / 100);
         var p:Point = new Point(x,y);
         p = _defaultPopupCanvas.globalToLocal(scope.localToGlobal(p));
         mc.x = p.x;
         mc.y = p.y;
         host.setChildIndex(_defaultPopupCanvas,host.numChildren - 1);
         host.addEventListener(Event.ADDED,PopUpManager.handleStageAddedEvent,false,0,true);
      }
      
      public static function showModal(mc:Sprite, mcX:Number = 0, mcY:Number = 0, bg:Sprite = null, controllerIdx:uint = 0, newFocus:Sprite = null) : void
      {
         if(!_stage)
         {
            trace("PopUpManager has not been initialized. Automatic initialization has not occured or has failed; call PopUpManager.init() manually.");
            return;
         }
         if(_modalMc)
         {
            _defaultPopupCanvas.removeChild(_modalMc);
         }
         if(mc == null)
         {
            return;
         }
         if(bg == null)
         {
            bg = new Sprite();
            bg.graphics.lineStyle(0,16777215,0);
            bg.graphics.beginFill(16777215,0);
            bg.graphics.drawRect(0,0,_stage.stageWidth,_stage.stageHeight);
            bg.graphics.endFill();
         }
         _modalMc = mc;
         _modalBg = bg;
         _modalMc.x = mcX;
         _modalMc.y = mcY;
         _defaultPopupCanvas.addChild(_modalBg);
         _defaultPopupCanvas.addChild(_modalMc);
         FocusHandler.getInstance().setFocus(newFocus,controllerIdx,false);
         FocusManager.setModalClip(_modalMc,controllerIdx);
         _modalMc.addEventListener(Event.REMOVED_FROM_STAGE,handleRemoveModalMc,false,0,true);
         host.addEventListener(Event.ADDED,PopUpManager.handleStageAddedEvent,false,0,true);
      }
      
      protected static function handleStageAddedEvent(e:Event) : void
      {
         host.setChildIndex(_defaultPopupCanvas,host.numChildren - 1);
      }
      
      protected static function handleRemovePopup(e:Event) : void
      {
         removeAddedToStageListener();
      }
      
      protected static function handleRemoveModalMc(e:Event) : void
      {
         _modalBg.removeEventListener(Event.REMOVED_FROM_STAGE,handleRemoveModalMc,false);
         if(_modalBg)
         {
            _defaultPopupCanvas.removeChild(_modalBg);
         }
         _modalMc = null;
         _modalBg = null;
         removeAddedToStageListener();
      }
      
      protected static function removeAddedToStageListener() : void
      {
         if(_defaultPopupCanvas.numChildren == 0 && _modalMc == null)
         {
            host.removeEventListener(Event.ADDED,PopUpManager.handleStageAddedEvent,false);
         }
      }
   }
}

