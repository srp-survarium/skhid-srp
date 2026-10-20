package survarium.buttons
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextFieldAutoSize;
   import flash.utils.clearInterval;
   import flash.utils.setInterval;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.events.ComponentEvent;
   
   public class BottomTab extends ButtonBlink
   {
      
      public var icon:Sprite;
      
      public var new_icon:Sprite;
      
      public var background:Sprite;
      
      public var overlay:Sprite;
      
      protected var flashInterval:int = -1;
      
      protected var _actualWidth:int = 82;
      
      protected var _maxSize:uint = 146;
      
      public function BottomTab()
      {
         super();
         addEventListener(Event.RENDER,this.step);
      }
      
      override public function set width(value:Number) : void
      {
         if(this.background)
         {
            this.updateBackgroundWidth(value);
         }
         else
         {
            super.width = value;
         }
      }
      
      override public function get width() : Number
      {
         if(this.background)
         {
            return this.background.width;
         }
         return super.width;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         invalidate(InvalidationType.DATA);
      }
      
      protected function fnc_flash() : void
      {
      }
      
      override protected function configUI() : void
      {
         textField.mouseEnabled = false;
         super.configUI();
      }
      
      override protected function updateText() : void
      {
         preventAutosizing = true;
         mouseChildren = false;
         textField.mouseEnabled = false;
         this.background.mouseEnabled = false;
         this.overlay.visible = false;
         if(_data)
         {
            textField.text = _data.name;
         }
         width = this._actualWidth;
         if(_data && (selected || !_data.blink) && this.flashInterval != -1)
         {
            clearInterval(this.flashInterval);
            flashInterval = -1;
            _data.blink = false;
            textField.text = _data.name;
            blink = false;
         }
         if(isInvalid(InvalidationType.SIZE) || isInvalid(InvalidationType.DATA))
         {
         }
      }
      
      override protected function draw() : void
      {
         if(_data)
         {
            if(_data.blink && this.flashInterval == -1)
            {
               invalidate(InvalidationType.STATE);
               flashInterval = setInterval(this.fnc_flash,200);
               blink = true;
            }
         }
         if(isInvalid(InvalidationType.STATE))
         {
            if(_newFrame)
            {
               gotoAndPlay(_newFrame);
               _newFrame = null;
            }
            if(_newFocusIndicatorFrame)
            {
               focusIndicator.gotoAndPlay(_newFocusIndicatorFrame);
               _newFocusIndicatorFrame = null;
            }
            updateAfterStateChange();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
            invalidate(InvalidationType.SIZE);
         }
         if(isInvalid(InvalidationType.DATA) || isInvalid(InvalidationType.STATE))
         {
            this.updateText();
            if(autoSize != TextFieldAutoSize.NONE)
            {
               invalidateSize();
            }
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            if(!preventAutosizing)
            {
               alignForAutoSize();
               setActualSize(_width,_height);
            }
            if(!constraintsDisabled)
            {
               constraints.update(this.width,_height);
            }
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            if(parent && parent.parent is ButtonBar)
            {
               (parent.parent as ButtonBar).invalidateSize();
            }
         }
      }
      
      protected function close(e:Event) : void
      {
         ExternalInterface.call("chat_tab_close",_data.id);
      }
      
      override protected function setState(state:String) : void
      {
         if(_data && _data is Object)
         {
            _state = state;
         }
         var prefixes:Vector.<String> = getStatePrefixes();
         var states:Array = _stateMap[state];
         if(states == null || states.length == 0)
         {
            return;
         }
         var l:uint = prefixes.length;
         for(var i:uint = 0; i < l; i++)
         {
            var prefix:String;
            var sl:uint;
            var j:uint;
            var thisLabel:String;
            prefix = prefixes[i];
            sl = states.length;
            for(j = 0; j < sl; j++)
            {
               thisLabel = prefix + states[j];
               if(_labelHash[thisLabel])
               {
                  _newFrame = thisLabel;
                  invalidateState();
                  return;
               }
            }
         }
      }
      
      protected function stepClose(e:Event) : void
      {
         var newSize:int = Math.max(this.width - 20,80);
         _actualWidth = newSize;
         if(this.background)
         {
            this.updateBackgroundWidth(newSize);
         }
         invalidateSize();
         if(this.width <= 80)
         {
            removeEventListener(e.type,this.stepClose);
            ExternalInterface.call("chat_tab_close",_data.id);
            return;
         }
      }
      
      protected function updateBackgroundWidth(value:Number) : void
      {
         this.overlay.width = value - 6;
         this.background.width = value;
      }
      
      protected function step(e:Event) : void
      {
         var newSize:int = Math.min(this.width + 25,this._maxSize);
         _actualWidth = newSize;
         if(this.background)
         {
            this.updateBackgroundWidth(newSize);
         }
         invalidateSize();
         if(this.width >= this._maxSize)
         {
            removeEventListener(e.type,this.step);
            return;
         }
      }
   }
}

