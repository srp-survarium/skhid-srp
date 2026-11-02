package
{
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.text.StyleSheet;
   import flash.utils.Timer;
   import scaleform.clik.layout.Layout;
   import survarium.controls.Modals;
   
   public class MainScene
   {
      
      public static var activeMatchMaking:Boolean;
      
      public static var isShift:Boolean;
      
      // whether a Ctrl key is down (LobbyMenu.onKeyDownStage; the item lists' Ctrl+F)
      public static var isCtrl:Boolean;
      
      // whether an Alt key is down (LobbyMenu.onKeyDownStage; a weapon card's stats in a match)
      public static var isAlt:Boolean;
      
      public static var showDebugInfo:Boolean;
      
      public static var stage:DisplayObjectContainer;
      
      private static var _activeWindow:MovieClip;

      private static var _activePop:MovieClip;

      public static var activeComponent:MovieClip;

      // The top menu's window open (or the results): a modal (Modals), Esc its closeWindow.
      public static function get activeWindow() : MovieClip
      {
         return _activeWindow;
      }

      public static function set activeWindow(value:MovieClip) : void
      {
         _activeWindow = enter(_activeWindow,value,"closeWindow");
      }

      // A popup over the windows (the attachments window, the ammo box): a modal, Esc its closePopup.
      public static function get activePop() : MovieClip
      {
         return _activePop;
      }

      public static function set activePop(value:MovieClip) : void
      {
         _activePop = enter(_activePop,value,"closePopup");
      }

      private static function enter(old:MovieClip, value:MovieClip, closeName:String) : MovieClip
      {
         if(old != null && old != value)
         {
            Modals.leave(old);
         }
         if(value != null)
         {
            Modals.open(value,Modals.LOBBY,function():void
            {
               if(value[closeName] is Function)
               {
                  value[closeName]();
               }
            },function():Boolean
            {
               return value.visible && value.stage != null;
            });
         }
         return value;
      }
      
      public static var layout:Layout;
      
      public static var relocateItem:Function;
      
      public static var scaleFactor:Number;
      
      // The scale of what the lobby puts on the stage itself, outside its scaled
      // layout (tooltips, dragged icons, the context menu, the sync clip): the
      // lobby's, never below 1 (LobbyMenu.onStageResize).
      public static var overlayScale:Number = 1;
      
      public static var textStyleSheet:StyleSheet;
      
      public static var buyFunction:Function;
      
      protected static var _timer:Timer = new Timer(1000);
      
      public function MainScene()
      {
         super();
      }
      
      public static function getTimerInstance() : Timer
      {
         return _timer;
      }
      
      public static function get stageWidth() : Number
      {
         if(!stage)
         {
            return 0;
         }
         return stage.stage.stageWidth / stage.scaleX;
      }
      
      public static function get stageHeight() : Number
      {
         if(!stage)
         {
            return 0;
         }
         return stage.stage.stageHeight / stage.scaleY;
      }
   }
}

