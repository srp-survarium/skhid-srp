package
{
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.text.StyleSheet;
   import flash.utils.Timer;
   import scaleform.clik.layout.Layout;
   
   public class MainScene
   {
      
      public static var activeMatchMaking:Boolean;
      
      public static var isShift:Boolean;
      
      public static var showDebugInfo:Boolean;
      
      public static var stage:DisplayObjectContainer;
      
      public static var activeWindow:MovieClip;
      
      public static var activePop:MovieClip;
      
      public static var activeComponent:MovieClip;
      
      public static var layout:Layout;
      
      public static var relocateItem:Function;
      
      public static var scaleFactor:Number;
      
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

