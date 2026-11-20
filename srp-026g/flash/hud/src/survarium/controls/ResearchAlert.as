package survarium.controls
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.utils.getTimer;

   // One mutually exclusive overlay: no activity, contested swords, or an
   // enemy capture warning. Both movies use the same 54 x 53 artwork and pulse.
   public class ResearchAlert extends Sprite
   {
      public static const NONE:uint = 0;
      public static const CONTESTED:uint = 1;
      public static const ENEMY_CAPTURE:uint = 2;
      public static const PULSE_MS:uint = 2000;

      protected var _state:uint = NONE;
      protected var _started:uint = 0;
      protected var _swords:BitmapData = new ResearchSwords();
      protected var _warning:BitmapData = new ResearchWarning();
      protected var _bitmap:Bitmap;

      public function ResearchAlert()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         _bitmap = new Bitmap(_swords);
         _bitmap.smoothing = true;
         addChild(_bitmap);
         visible = false;
         addEventListener(Event.REMOVED_FROM_STAGE,onRemoved);
      }

      // `advancing` is the sole occupying team, not the team whose progress
      // remains. During neutralisation those teams are opposites.
      public static function forCapture(ownTeam:uint, advancing:uint, left:Number, contested:Boolean) : uint
      {
         if(contested)
         {
            return CONTESTED;
         }
         return ownTeam < 2 && advancing < 2 && advancing != ownTeam && left > 0 ? ENEMY_CAPTURE : NONE;
      }

      public function get state() : uint
      {
         return _state;
      }

      public function setState(value:uint) : void
      {
         if(value == _state)
         {
            return;
         }
         _state = value;
         visible = value != NONE;
         alpha = 1;
         if(value == NONE)
         {
            removeEventListener(Event.ENTER_FRAME,onFrame);
            return;
         }
         _bitmap.bitmapData = value == CONTESTED ? _swords : _warning;
         _started = uint(getTimer());
         addEventListener(Event.ENTER_FRAME,onFrame);
      }

      protected function onFrame(e:Event) : void
      {
         var elapsed:uint = uint(uint(getTimer()) - _started);
         alpha = Math.abs(1 - (elapsed % PULSE_MS) / PULSE_MS * 2);
      }

      protected function onRemoved(e:Event) : void
      {
         setState(NONE);
      }
   }
}
