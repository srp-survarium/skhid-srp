package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.controls.Label;
   
   public class HUDMarker extends MovieClip
   {
      
      protected static var _labelClass:Class;
      
      protected static var _labelShift:int = 5;
      
      public var textField:TextField;
      
      public var icon:MovieClip;
      
      public var icon_box:MovieClip;
      
      public var baseScale:Number;
      
      public var team:uint;
      
      public var kind:uint;
      
      protected var _tm:Timer = new Timer(500,1);
      
      protected var _visibleTarget:Boolean = false;
      
      protected var _labelRenderer:Label;

      // The interface's level shows the players' names (HUDIconLayer.set_interface_level).
      protected var _nameShown:Boolean = true;
      
      public function HUDMarker()
      {
         super();
         if(this.icon_box)
         {
            this.icon_box.visible = false;
         }
         if(this.icon && this.icon.glow)
         {
            this.icon.glow.visible = false;
         }
      }
      
      public static function setLabelRendererClass(value:Class) : void
      {
         _labelClass = value;
      }
      
      public function set type(value:uint) : void
      {
         this.icon.gotoAndStop(value + 1);
      }
      
      public function set label(value:String) : void
      {
         if(value)
         {
            if(!this._labelRenderer)
            {
               _labelRenderer = new _labelClass();
            }
            this._labelRenderer.autoSize = TextFieldAutoSize.LEFT;
            this._labelRenderer.text = value;
            addChild(this._labelRenderer);
            this._labelRenderer.validateNow();
            this._labelRenderer.x = this.icon.x + this.icon.width + _labelShift;
            this._labelRenderer.y = this.textField.y - this._labelRenderer.height - _labelShift;
         }
         else if(this._labelRenderer)
         {
            removeChild(this._labelRenderer);
            _labelRenderer = null;
         }
      }
      
      public function set glow(value:Boolean) : void
      {
         trace("set_glow::",value);
         if(!this.icon || !this.icon.glow)
         {
            return;
         }
         this.icon.glow.visible = value;
         this.icon.glow.gotoAndPlay(2);
      }
      
      public function set container(value:Boolean) : void
      {
         if(this.icon_box)
         {
            this.icon_box.visible = value;
         }
      }
      
      public function set delay(value:uint) : void
      {
         this._tm.delay = value;
      }
      
      public function set nameShown(value:Boolean) : void
      {
         _nameShown = value;
         this.textField.visible = value && this._visibleTarget;
      }

      public function showName() : *
      {
         if(this._visibleTarget == true)
         {
            return;
         }
         _visibleTarget = true;
         if(!this.textField.visible)
         {
            this._tm.reset();
            this._tm.start();
            this._tm.addEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         }
         else
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
      }
      
      public function hideName() : *
      {
         if(this._visibleTarget == false)
         {
            return;
         }
         _visibleTarget = false;
         if(this.textField.visible)
         {
            this._tm.reset();
            this._tm.start();
            this._tm.addEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
         else
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         }
      }
      
      public function resetTimer() : void
      {
         if(this._tm.hasEventListener(TimerEvent.TIMER_COMPLETE))
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
      }
      
      protected function handleTimerShow(e:Event) : void
      {
         this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         this.textField.visible = this._nameShown;
      }
      
      protected function handleTimerHide(e:Event) : void
      {
         this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         this.textField.visible = false;
      }
   }
}

