package survarium.buttons
{
   import com.greensock.TweenLite;
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.ImportantMessageType;
   
   public class ButtonNotificationLog extends ListItemRenderer
   {
      
      public var hideTimer:int = 1;
      
      public var btnClose:Button;
      
      public var background:MovieClip;
      
      public var closable:Boolean = false;
      
      public var tween:TweenLite;
      
      protected var _hideTimer:Timer = new Timer(1000,1);
      
      protected var _hideTimeout:int = -1;
      
      protected var _ignoreClick:Boolean = false;
      
      public function ButtonNotificationLog()
      {
         super();
         TweenLite.to(this,0.2,{
            alpha:1,
            delay:0.5,
            ease:Strong.easeOut
         });
      }
      
      override public function get height() : Number
      {
         return this.background ? this.background.height : super.height;
      }
      
      override public function set height(value:Number) : void
      {
         super.height = value;
      }
      
      public function clearTimer() : void
      {
         mouseChildren = false;
         mouseEnabled = false;
         this._hideTimer.stop();
         this._hideTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.onCloseEvent);
      }
      
      public function clear() : void
      {
         if(parent)
         {
            parent.removeChild(this);
         }
         this.btnClose.removeEventListener(ButtonEvent.CLICK,this.onCloseEvent);
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         trace("updatreTX::",this.background,textField,_data);
         if(_data)
         {
            if(_data.hideTimer != undefined)
            {
               hideTimer = _data.hideTimer;
            }
            textField.styleSheet = MainScene.textStyleSheet;
            textField.autoSize = TextFieldAutoSize.LEFT;
            textField.text = _data.label;
            this.background.height = textField.textHeight + 13;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnClose.visible = this.closable;
         if(this.btnClose.visible)
         {
            this.btnClose.addEventListener(ButtonEvent.CLICK,this.onCloseEvent);
         }
         preventAutosizing = true;
         mouseChildren = true;
         addEventListener(ButtonEvent.CLICK,this.onClicked,false,0,true);
      }
      
      private function onClicked(event:Event) : void
      {
         trace("onClicked::",event.target);
         if(_data && _data.type == ImportantMessageType.REPUTATION_LEVEL)
         {
            if(event.target == this)
            {
               if(this._ignoreClick == true)
               {
                  _ignoreClick = false;
               }
               else
               {
                  dispatchEvent(new Event("showMessage"));
               }
            }
            else
            {
               _ignoreClick = true;
            }
         }
      }
      
      private function onCloseEvent(event:Event = null) : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      public function startTimer() : void
      {
         this._hideTimer.start();
      }
      
      public function stopTimer() : void
      {
         this._hideTimer.stop();
      }
      
      override public function get data() : Object
      {
         return super.data;
      }
      
      override public function set data(value:Object) : void
      {
         super.data = value;
         if(_data)
         {
            if(!this._hideTimer.hasEventListener(TimerEvent.TIMER_COMPLETE))
            {
               this._hideTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.onCloseEvent);
               this._hideTimer.delay = _data.hideTimer;
            }
            visible = true;
         }
         else
         {
            visible = false;
         }
         invalidateData();
      }
   }
}

