package survarium.chat
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Point;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.TextArea;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import survarium.controls.FixedTextDots;
   import survarium.data.LocalizationData;
   
   public class LobbyChatTab extends UIComponent
   {
      
      public var send_function:Function;
      
      public var text_inpput:TextInput;
      
      public var ignoreFocus:Boolean = false;
      
      public var back:MovieClip;
      
      public var text_chat:TextArea;
      
      public var labelTextField:FixedTextDots;
      
      public var btn_close:Button;
      
      public var btn_hide:Button;
      
      public var dragger0:Sprite;
      
      public var dragger1:Sprite;
      
      public var dragger2:Sprite;
      
      public var dragger3:Sprite;
      
      public var enterArrow:Sprite;
      
      public var statusIndicator:ToolTip;
      
      protected var _minWidth:int = 350;
      
      protected var _minHeight:int = 210;
      
      protected var _maxWidth:int = 650;
      
      protected var _maxHeight:int = 569;
      
      protected var _myScaleFactor:Number = 1;
      
      protected var _hasMoved:Boolean = false;
      
      protected var _isTyping:Boolean = false;
      
      protected var _nline:String = "\n";
      
      protected var _maxLines:int = 180;
      
      protected var _lastIdx:int = 0;
      
      protected var _pos:int = 1;
      
      protected var _was_last:Boolean;
      
      protected var _allowed_names:Dictionary = new Dictionary();
      
      protected var _lastName:String;
      
      protected var _dragPoint:Point = new Point();
      
      protected var _data:Object;
      
      protected var _owner:LobbyChat;
      
      protected var _started:Boolean = false;
      
      public function LobbyChatTab()
      {
         super();
         trace("LobbyChatTab::",this.back,this.statusIndicator);
         this.statusIndicator.visible = false;
         this.statusIndicator.tooltipRendererName = "SrvProfileTipRenderer";
         this.statusIndicator.gotoAndStop(1);
      }
      
      public function set owner(param1:LobbyChat) : void
      {
         _owner = param1;
      }
      
      public function get handled() : Boolean
      {
         return this._hasMoved;
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set width(param1:Number) : void
      {
         this.text_inpput.setSize(param1 - 8 - this.text_inpput.x,this.text_inpput.height);
         this.back.width = param1;
         this.btn_close.x = param1 - this.btn_close.width - 5;
         this.btn_hide.x = (this.btn_close.visible ? this.btn_close.x : param1) - this.btn_hide.width - 5;
         this.text_chat.setSize(param1 - 13,this.text_chat.height);
         this.dragger1.x = this.dragger3.x = param1 - 20;
         this.enterArrow.x = param1 - 40;
         this.labelTextField.x = 51;
         invalidateSize();
      }
      
      override public function get width() : Number
      {
         return this.back.width;
      }
      
      override public function set height(param1:Number) : void
      {
         if(param1 < this._minHeight)
         {
            param1 = this._minHeight;
         }
         this.back.height = param1;
         this.text_inpput.y = param1 - 38;
         this.text_chat.setSize(this.text_chat.width,param1 - 105);
         this.dragger2.y = this.dragger3.y = param1 - 20;
         this.enterArrow.y = param1 - 28;
         invalidateSize();
      }
      
      override public function get height() : Number
      {
         return this.back.height;
      }
      
      public function setData(param1:Object) : void
      {
         _data = param1;
         invalidateData();
      }
      
      public function getData() : Object
      {
         return this._data;
      }
      
      public function focusTextInput() : void
      {
         trace("focusTextInput::",name);
         this.text_inpput.addEventListener(Event.RENDER,this.onRenderFocus);
      }
      
      public function scrollToBottom() : void
      {
         this.text_chat.position = this.text_chat.textField.maxScrollV;
      }
      
      public function srartMessage(param1:String) : void
      {
         trace("srartMessage::",name,param1);
         this.text_inpput.text = param1;
         this.text_inpput.addEventListener(Event.RENDER,this.onRender);
      }
      
      public function setSizeLimits(param1:int, param2:int, param3:int, param4:int) : void
      {
         _minWidth = param1;
         _minHeight = param2;
         _maxWidth = param3;
         _maxHeight = param4;
      }
      
      public function submitText(param1:Event = null) : void
      {
         if(this.text_inpput.text == "")
         {
            return;
         }
         var _loc2_:Boolean = this.sendText();
         if(_loc2_)
         {
            this.text_inpput.text = "";
            this.text_inpput.textField.text = "";
         }
      }
      
      public function setStyleSheet(param1:StyleSheet) : void
      {
         this.text_chat.textField.styleSheet = param1;
      }
      
      public function prepareToResize() : void
      {
         _pos = this.text_chat.position;
         _was_last = this.text_chat.position == this.text_chat.textField.maxScrollV;
         this.text_chat.addEventListener(Event.RENDER,this.checkChat);
      }
      
      public function clear() : void
      {
         _data = null;
         this.btn_close.removeEventListener(ButtonEvent.CLICK,this.onClose);
         this.btn_hide.removeEventListener(ButtonEvent.CLICK,this.onHide);
         this.text_inpput.removeEventListener(KeyboardEvent.KEY_UP,this.checkInputKeyForSubmit);
         this.text_inpput.textField.removeEventListener(FocusEvent.FOCUS_OUT,this.stop_enter_text);
         this.text_inpput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.start_enter_text);
         this.text_chat.textField.removeEventListener(FocusEvent.FOCUS_OUT,this.stop_enter_text_area);
         this.text_chat.textField.removeEventListener(FocusEvent.FOCUS_IN,this.start_enter_text_area);
         this.dragger2.removeEventListener(MouseEvent.MOUSE_DOWN,this.startResize);
         this.dragger3.removeEventListener(MouseEvent.MOUSE_DOWN,this.startResize);
         removeEventListener(MouseEvent.ROLL_OUT,this.onMouseLeave);
         removeEventListener(MouseEvent.ROLL_OVER,this.onMouseIN);
         removeEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
         removeEventListener(FocusEvent.FOCUS_IN,this.handleFocusIn);
         this.back.removeEventListener(MouseEvent.MOUSE_DOWN,this.startMoveTop);
         this.dragger0.removeEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
         this.dragger1.removeEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
      }
      
      private function addMoveListeners() : void
      {
         if(!hasEventListener(MouseEvent.MOUSE_MOVE))
         {
            addEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
         }
      }
      
      private function removeMoveListeners() : void
      {
         removeEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
      }
      
      private function onMouseIN(param1:MouseEvent) : void
      {
         this.addMoveListeners();
         this.onMouseMove(param1);
      }
      
      private function onMouseLeave(param1:Event) : void
      {
         this.removeMoveListeners();
         if(param1.target != this.back)
         {
            ExternalInterface.call("set_mouse_cursor",1);
         }
      }
      
      private function onMouseMove(param1:MouseEvent) : void
      {
         if(mouseY < 50 && (param1.target === this.back || param1.target === this.dragger0 || param1.target === this.dragger1))
         {
            ExternalInterface.call("set_mouse_cursor",7);
         }
         else if(param1.target === this.dragger2)
         {
            ExternalInterface.call("set_mouse_cursor",8);
         }
         else if(param1.target === this.dragger3)
         {
            ExternalInterface.call("set_mouse_cursor",9);
         }
         else if(!(param1.target is TextField) || param1.target is TextField && (param1.target as TextField).selectable == false)
         {
            ExternalInterface.call("set_mouse_cursor",1);
         }
      }
      
      private function startResize(param1:Event) : void
      {
         _dragPoint = new Point(mouseX,mouseY);
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.chResize);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.stopResize);
      }
      
      private function startMoveTop(param1:Event) : void
      {
         if(mouseY < 50)
         {
            this.startMove(param1);
         }
      }
      
      private function startMove(param1:Event) : void
      {
         _dragPoint = new Point(mouseX,mouseY);
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.chMove);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.stopMove);
      }
      
      private function stopMove(param1:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.chMove);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.stopMove);
      }
      
      private function stopResize(param1:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.chResize);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.stopResize);
         ExternalInterface.call("set_mouse_cursor",1);
      }
      
      private function chMove(param1:MouseEvent) : void
      {
         var _loc2_:int = this._dragPoint.x - mouseX;
         var _loc3_:int = this._dragPoint.y - mouseY;
         if(y - _loc3_ < 0)
         {
            _loc3_ = y;
         }
         if(y - _loc3_ + this.height / this._myScaleFactor > MainScene.stageHeight)
         {
            _loc3_ = -(MainScene.stageHeight - (y + this.height / this._myScaleFactor));
         }
         if(x - _loc2_ < 0)
         {
            _loc2_ = x;
         }
         if(x - _loc2_ + this.width / this._myScaleFactor > MainScene.stageWidth)
         {
            _loc2_ = -(MainScene.stageWidth - (x + this.width / this._myScaleFactor));
         }
         x = x - _loc2_;
         y = y - _loc3_;
         _hasMoved = true;
      }
      
      private function chResize(param1:MouseEvent) : void
      {
         var _loc2_:Number;
         if(this._dragPoint.x < 80)
         {
            _loc2_ = Math.min(Math.max(this._dragPoint.x - mouseX,this._minWidth - this.width),this._maxWidth - this.width);
            _loc4_ = x;
            x = x - _loc2_ / this._myScaleFactor;
            width += _loc4_ - x;
         }
         else
         {
            _loc2_ = Math.min(Math.max(mouseX - this._dragPoint.x,this._minWidth - this.width),this._maxWidth - this.width);
            this._dragPoint.x += _loc2_;
            width += _loc2_;
         }
         var _loc3_:Number = Math.min(Math.max(mouseY - this._dragPoint.y,this._minHeight - this.back.height),this._maxHeight - this.back.height);
         var _loc4_:Number;
         this._dragPoint.y += _loc3_;
         height = this.back.height + _loc3_;
      }
      
      protected function checkLenght(param1:int) : void
      {
         var _loc2_:TextField = this.text_chat.textField;
         var _loc3_:int;
         var _loc4_:int;
         trace("currentLength::",_loc2_.numLines);
         if(_loc2_.numLines > param1)
         {
            _loc3_ = -1;
            _loc4_ = 0;
            while(_loc4_ < _loc2_.numLines - param1)
            {
               _loc3_ = _loc2_.htmlText.indexOf("\n",_loc3_ + 1);
               _loc4_++;
            }
            this.text_chat.htmlText = _loc2_.htmlText.substring(_loc3_ + 1,_loc2_.htmlText.length);
            this.text_chat.validateNow();
         }
      }
      
      protected function addMessage(param1:String) : void
      {
         _pos = this.text_chat.position;
         _was_last = this.text_chat.position == this.text_chat.textField.maxScrollV;
         this.text_chat.appendHtml(param1 + this._nline);
         this.text_chat.validateNow();
         this.text_chat.addEventListener(Event.RENDER,this.checkChat);
         if(!visible)
         {
            if(this._data)
            {
               this._data.blink = true;
            }
            trace("CHANNEL_MESSAGE::",name,param1,visible,parent);
            dispatchEvent(new Event(Event.CHANNEL_MESSAGE));
         }
      }
      
      protected function onClose(param1:Event = null) : void
      {
         if(stage)
         {
            stage.focus = null;
         }
         ExternalInterface.call("chat_tab_close",this._data.id);
      }
      
      protected function onHide(param1:Event = null) : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function checkChat(param1:Event) : void
      {
         trace("checkChat::",param1);
         this.text_chat.removeEventListener(Event.RENDER,this.checkChat);
         this.checkLenght(this._maxLines);
         if(!this._was_last)
         {
            this.text_chat.position = this._pos;
         }
         else
         {
            this.text_chat.position = this.text_chat.textField.maxScrollV;
         }
      }
      
      protected function addToSquad() : void
      {
         ExternalInterface.call("invite_to_squad",this._lastName);
      }
      
      protected function addToFriend() : void
      {
         ExternalInterface.call("add_friend_name",this._lastName);
      }
      
      protected function addToIgnore() : void
      {
         ExternalInterface.call("add_ignore_name",this._lastName);
      }
      
      protected function privateMessage() : void
      {
         ExternalInterface.call("start_friend_message",this._lastName);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         focusable = true;
         this.back.focusTarget = this;
         LocalizationData.addCallback(this.onLocal,"enter_chat_message");
         layoutData = null;
         if(this.labelTextField)
         {
            this.labelTextField.mouseEnabled = false;
         }
         if(this.labelTextField)
         {
            this.labelTextField.mouseChildren = false;
         }
         this.back.addEventListener(MouseEvent.MOUSE_DOWN,this.startMoveTop);
         this.dragger0.addEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
         this.dragger1.addEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
         this.btn_close.addEventListener(ButtonEvent.CLICK,this.onClose);
         this.btn_hide.addEventListener(ButtonEvent.CLICK,this.onHide);
         var _loc1_:TextFormat = this.text_inpput.textField.getTextFormat();
         this.text_inpput.textField.setTextFormat(_loc1_);
         this.text_inpput.textField.defaultTextFormat = _loc1_;
         this.text_inpput.defaultTextFormat = _loc1_;
         this.text_inpput.addEventListener(KeyboardEvent.KEY_UP,this.checkInputKeyForSubmit,false,0,true);
         this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_OUT,this.stop_enter_text,false,0,true);
         this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.start_enter_text,false,0,true);
         this.text_chat.textField.addEventListener(FocusEvent.FOCUS_OUT,this.stop_enter_text_area,false,0,true);
         this.text_chat.textField.addEventListener(FocusEvent.FOCUS_IN,this.start_enter_text_area,false,0,true);
         this.back.addEventListener(MouseEvent.MOUSE_DOWN,this.startMoveTop);
         this.dragger0.addEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
         this.dragger1.addEventListener(MouseEvent.MOUSE_DOWN,this.startMove);
         this.dragger2.addEventListener(MouseEvent.MOUSE_DOWN,this.startResize);
         this.dragger3.addEventListener(MouseEvent.MOUSE_DOWN,this.startResize);
         addEventListener(MouseEvent.ROLL_OUT,this.onMouseLeave);
         addEventListener(MouseEvent.ROLL_OVER,this.onMouseIN);
         addEventListener(FocusEvent.FOCUS_IN,this.handleFocusIn);
         this.btn_hide.visible = true;
         this.btn_hide.enabled = true;
         this.dragger0.scaleX = this.dragger1.scaleX = this.dragger0.scaleY = this.dragger1.scaleY = 0.5;
         this.dragger1.y = -6;
         this.enterArrow.addEventListener(MouseEvent.CLICK,this.submitText);
         this.enterArrow.alpha = 0.3;
         this.text_chat.selectable = true;
      }
      
      protected function handleFocusIn(param1:FocusEvent) : void
      {
         trace("LCT::focusIn",this._data.id,this._data.name,"event:props",param1.relatedObject,param1.target,param1.currentTarget);
         param1.stopImmediatePropagation();
         this._owner.focusTab(this._data.id);
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SIZE))
         {
            this.text_chat.invalidate(InvalidationType.SIZE);
            this.text_chat.invalidate(InvalidationType.STATE);
         }
         if(isInvalid(InvalidationType.DATA))
         {
            if(this.labelTextField)
            {
               this.labelTextField.text = this._data.name;
               this.labelTextField.validateNow();
            }
            this.labelTextField.x = this.statusIndicator.visible ? 51 : 34;
            this.btn_close.enabled = this._data.closable;
            this.btn_close.visible = this._data.closable;
            this.btn_hide.x = (this.btn_close.visible ? this.btn_close.x : this.width) - this.btn_hide.width - 5;
         }
      }
      
      protected function onLocal(param1:String) : void
      {
         this.text_inpput.defaultText = param1;
      }
      
      protected function moveCaret() : void
      {
         this.text_inpput.textField.setSelection(this.text_inpput.textField.text.length,this.text_inpput.textField.text.length);
      }
      
      protected function onRenderFocus(param1:Event) : void
      {
         stage.focus = this.text_inpput.textField;
         this.text_inpput.text = "";
         this.text_inpput.removeEventListener(Event.RENDER,this.onRenderFocus);
      }
      
      protected function onRender(param1:Event) : void
      {
         stage.focus = this.text_inpput;
         this.moveCaret();
         this.text_inpput.removeEventListener(Event.RENDER,this.onRender);
      }
      
      protected function stop_enter_text(param1:FocusEvent = null) : void
      {
         trace("stop_enter text::",param1);
         if(param1.relatedObject != null && param1.relatedObject != this.text_inpput && !this.ignoreFocus || this._isTyping && !this.ignoreFocus)
         {
            if(!this._started)
            {
               trace("isCanceledAlready");
            }
            else
            {
               trace("chat_enter_cancel");
               _started = false;
               ExternalInterface.call("chat_enter_cancel");
               this.enterArrow.alpha = 0.3;
            }
            _isTyping = false;
         }
         ignoreFocus = false;
      }
      
      protected function start_enter_text(param1:FocusEvent = null) : void
      {
         trace("start_enter text",param1.relatedObject,this.ignoreFocus);
         if(param1.relatedObject != null && !this.ignoreFocus)
         {
            if(this._started)
            {
               trace("isStartedAlready");
            }
            else
            {
               trace("chat_enter_start");
               _started = true;
               ExternalInterface.call("chat_enter_start");
               this.enterArrow.alpha = 1;
            }
         }
         ignoreFocus = false;
         _isTyping = true;
         this.text_inpput.addEventListener(ComponentEvent.STATE_CHANGE,this.onNewState);
      }
      
      protected function stop_enter_text_area(param1:FocusEvent = null) : void
      {
         trace("stop_enter text area",param1.relatedObject,this.ignoreFocus,this._isTyping);
         if(param1.relatedObject != null && param1.relatedObject != this.text_chat && !this.ignoreFocus || this._isTyping && !this.ignoreFocus)
         {
            if(!this._started)
            {
               trace("isCanceledAlready");
            }
            else
            {
               trace("chat_enter_cancel");
               _started = false;
               ExternalInterface.call("chat_enter_cancel");
            }
            _isTyping = false;
         }
         ignoreFocus = false;
      }
      
      protected function start_enter_text_area(param1:FocusEvent = null) : void
      {
         trace("start_enter text area",param1.relatedObject,this.ignoreFocus);
         if(param1.relatedObject != null && !this.ignoreFocus)
         {
            if(this._started)
            {
               trace("isStartedAlready");
            }
            else
            {
               trace("chat_enter_start");
               _started = true;
               ExternalInterface.call("chat_enter_start");
            }
         }
         ignoreFocus = false;
         _isTyping = true;
      }
      
      protected function sendText() : Boolean
      {
         var _loc1_:int = (this._data.channel_to_send);
         var _loc2_:*;
         if(this._owner.send_function is Function)
         {
            _loc2_ = this._owner.send_function(this.text_inpput,_loc1_);
            trace("LCT::isValid",_loc2_);
            return _loc2_;
         }
         return false;
      }
      
      protected function onNewState(param1:Event = null) : void
      {
         this.text_inpput.removeEventListener(ComponentEvent.STATE_CHANGE,this.onNewState);
         this.moveCaret();
      }
      
      protected function checkInputKeyForSubmit(param1:KeyboardEvent) : void
      {
         trace("checkInputKeyForSubmit",param1.keyCode);
         if(param1.keyCode == 150)
         {
            this.submitText();
         }
      }
   }
}

