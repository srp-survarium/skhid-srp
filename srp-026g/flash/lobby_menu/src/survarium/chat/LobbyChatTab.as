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
   import survarium.constants.Cursor;
   import survarium.constants.KeyCode;
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
      
      public function set owner(value:LobbyChat) : void
      {
         _owner = value;
      }
      
      public function get handled() : Boolean
      {
         return this._hasMoved;
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set width(value:Number) : void
      {
         this.text_inpput.setSize(value - 8 - this.text_inpput.x,this.text_inpput.height);
         this.back.width = value;
         this.btn_close.x = value - this.btn_close.width - 5;
         this.btn_hide.x = (this.btn_close.visible ? this.btn_close.x : value) - this.btn_hide.width - 5;
         this.text_chat.setSize(value - 13,this.text_chat.height);
         this.dragger1.x = this.dragger3.x = value - 20;
         this.enterArrow.x = value - 40;
         this.labelTextField.x = 51;
         invalidateSize();
      }
      
      override public function get width() : Number
      {
         return this.back.width;
      }
      
      override public function set height(value:Number) : void
      {
         if(value < this._minHeight)
         {
            value = this._minHeight;
         }
         this.back.height = value;
         this.text_inpput.y = value - 38;
         this.text_chat.setSize(this.text_chat.width,value - 105);
         this.dragger2.y = this.dragger3.y = value - 20;
         this.enterArrow.y = value - 28;
         invalidateSize();
      }
      
      override public function get height() : Number
      {
         return this.back.height;
      }
      
      public function setData(tabData:Object) : void
      {
         _data = tabData;
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
      
      public function srartMessage(messageText:String) : void
      {
         trace("srartMessage::",name,messageText);
         this.text_inpput.text = messageText;
         this.text_inpput.addEventListener(Event.RENDER,this.onRender);
      }
      
      public function setSizeLimits(newMinWidth:int, newMinHeight:int, newMaxWidth:int, newMaxHeight:int) : void
      {
         _minWidth = newMinWidth;
         _minHeight = newMinHeight;
         _maxWidth = newMaxWidth;
         _maxHeight = newMaxHeight;
      }
      
      public function submitText(event:Event = null) : void
      {
         if(this.text_inpput.text == "")
         {
            return;
         }
         var isSent:Boolean = this.sendText();
         if(isSent)
         {
            this.text_inpput.text = "";
            this.text_inpput.textField.text = "";
         }
      }
      
      public function setStyleSheet(styleSheet:StyleSheet) : void
      {
         this.text_chat.textField.styleSheet = styleSheet;
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
      
      private function onMouseIN(event:MouseEvent) : void
      {
         this.addMoveListeners();
         this.onMouseMove(event);
      }
      
      private function onMouseLeave(event:Event) : void
      {
         this.removeMoveListeners();
         if(event.target != this.back)
         {
            ExternalInterface.call("set_mouse_cursor",Cursor.ARROW);
         }
      }
      
      private function onMouseMove(event:MouseEvent) : void
      {
         if(mouseY < 50 && (event.target === this.back || event.target === this.dragger0 || event.target === this.dragger1))
         {
            ExternalInterface.call("set_mouse_cursor",Cursor.MOVE);
         }
         else if(event.target === this.dragger2)
         {
            ExternalInterface.call("set_mouse_cursor",Cursor.RESIZE_NESW);
         }
         else if(event.target === this.dragger3)
         {
            ExternalInterface.call("set_mouse_cursor",Cursor.RESIZE_NWSE);
         }
         else if(!(event.target is TextField) || event.target is TextField && (event.target as TextField).selectable == false)
         {
            ExternalInterface.call("set_mouse_cursor",Cursor.ARROW);
         }
      }
      
      private function startResize(event:Event) : void
      {
         _dragPoint = new Point(mouseX,mouseY);
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.chResize);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.stopResize);
      }
      
      private function startMoveTop(event:Event) : void
      {
         if(mouseY < 50)
         {
            this.startMove(event);
         }
      }
      
      private function startMove(event:Event) : void
      {
         _dragPoint = new Point(mouseX,mouseY);
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.chMove);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.stopMove);
      }
      
      private function stopMove(event:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.chMove);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.stopMove);
      }
      
      private function stopResize(event:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.chResize);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.stopResize);
         ExternalInterface.call("set_mouse_cursor",Cursor.ARROW);
      }
      
      private function chMove(event:MouseEvent) : void
      {
         var dx:int = this._dragPoint.x - mouseX;
         var dy:int = this._dragPoint.y - mouseY;
         if(y - dy < 0)
         {
            dy = y;
         }
         if(y - dy + this.height / this._myScaleFactor > MainScene.stageHeight)
         {
            dy = -(MainScene.stageHeight - (y + this.height / this._myScaleFactor));
         }
         if(x - dx < 0)
         {
            dx = x;
         }
         if(x - dx + this.width / this._myScaleFactor > MainScene.stageWidth)
         {
            dx = -(MainScene.stageWidth - (x + this.width / this._myScaleFactor));
         }
         x = x - dx;
         y = y - dy;
         _hasMoved = true;
      }
      
      private function chResize(event:MouseEvent) : void
      {
         var dw:Number;
         if(this._dragPoint.x < 80)
         {
            dw = Math.min(Math.max(this._dragPoint.x - mouseX,this._minWidth - this.width),this._maxWidth - this.width);
            oldX = x;
            x = x - dw / this._myScaleFactor;
            width += oldX - x;
         }
         else
         {
            dw = Math.min(Math.max(mouseX - this._dragPoint.x,this._minWidth - this.width),this._maxWidth - this.width);
            this._dragPoint.x += dw;
            width += dw;
         }
         var dh:Number = Math.min(Math.max(mouseY - this._dragPoint.y,this._minHeight - this.back.height),this._maxHeight - this.back.height);
         var oldX:Number;
         this._dragPoint.y += dh;
         height = this.back.height + dh;
      }
      
      protected function checkLenght(maxLines:int) : void
      {
         var chatField:TextField = this.text_chat.textField;
         var cutPos:int;
         var i:int;
         trace("currentLength::",chatField.numLines);
         if(chatField.numLines > maxLines)
         {
            cutPos = -1;
            i = 0;
            while(i < chatField.numLines - maxLines)
            {
               cutPos = chatField.htmlText.indexOf("\n",cutPos + 1);
               i++;
            }
            this.text_chat.htmlText = chatField.htmlText.substring(cutPos + 1,chatField.htmlText.length);
            this.text_chat.validateNow();
         }
      }
      
      protected function addMessage(messageText:String) : void
      {
         _pos = this.text_chat.position;
         _was_last = this.text_chat.position == this.text_chat.textField.maxScrollV;
         this.text_chat.appendHtml(messageText + this._nline);
         this.text_chat.validateNow();
         this.text_chat.addEventListener(Event.RENDER,this.checkChat);
         if(!visible)
         {
            if(this._data)
            {
               this._data.blink = true;
            }
            trace("CHANNEL_MESSAGE::",name,messageText,visible,parent);
            dispatchEvent(new Event(Event.CHANNEL_MESSAGE));
         }
      }
      
      protected function onClose(event:Event = null) : void
      {
         if(stage)
         {
            stage.focus = null;
         }
         ExternalInterface.call("chat_tab_close",this._data.id);
      }
      
      protected function onHide(event:Event = null) : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function checkChat(event:Event) : void
      {
         trace("checkChat::",event);
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
         var inputFormat:TextFormat = this.text_inpput.textField.getTextFormat();
         this.text_inpput.textField.setTextFormat(inputFormat);
         this.text_inpput.textField.defaultTextFormat = inputFormat;
         this.text_inpput.defaultTextFormat = inputFormat;
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
      
      protected function handleFocusIn(event:FocusEvent) : void
      {
         trace("LCT::focusIn",this._data.id,this._data.name,"event:props",event.relatedObject,event.target,event.currentTarget);
         event.stopImmediatePropagation();
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
      
      protected function onLocal(value:String) : void
      {
         this.text_inpput.defaultText = value;
      }
      
      protected function moveCaret() : void
      {
         this.text_inpput.textField.setSelection(this.text_inpput.textField.text.length,this.text_inpput.textField.text.length);
      }
      
      protected function onRenderFocus(event:Event) : void
      {
         stage.focus = this.text_inpput.textField;
         this.text_inpput.text = "";
         this.text_inpput.removeEventListener(Event.RENDER,this.onRenderFocus);
      }
      
      protected function onRender(event:Event) : void
      {
         stage.focus = this.text_inpput;
         this.moveCaret();
         this.text_inpput.removeEventListener(Event.RENDER,this.onRender);
      }
      
      protected function stop_enter_text(event:FocusEvent = null) : void
      {
         trace("stop_enter text::",event);
         if(event.relatedObject != null && event.relatedObject != this.text_inpput && !this.ignoreFocus || this._isTyping && !this.ignoreFocus)
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
      
      protected function start_enter_text(event:FocusEvent = null) : void
      {
         trace("start_enter text",event.relatedObject,this.ignoreFocus);
         if(event.relatedObject != null && !this.ignoreFocus)
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
      
      protected function stop_enter_text_area(event:FocusEvent = null) : void
      {
         trace("stop_enter text area",event.relatedObject,this.ignoreFocus,this._isTyping);
         if(event.relatedObject != null && event.relatedObject != this.text_chat && !this.ignoreFocus || this._isTyping && !this.ignoreFocus)
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
      
      protected function start_enter_text_area(event:FocusEvent = null) : void
      {
         trace("start_enter text area",event.relatedObject,this.ignoreFocus);
         if(event.relatedObject != null && !this.ignoreFocus)
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
         var channel:int = (this._data.channel_to_send);
         var isValid:*;
         if(this._owner.send_function is Function)
         {
            isValid = this._owner.send_function(this.text_inpput,channel);
            trace("LCT::isValid",isValid);
            return isValid;
         }
         return false;
      }
      
      protected function onNewState(event:Event = null) : void
      {
         this.text_inpput.removeEventListener(ComponentEvent.STATE_CHANGE,this.onNewState);
         this.moveCaret();
      }
      
      protected function checkInputKeyForSubmit(event:KeyboardEvent) : void
      {
         trace("checkInputKeyForSubmit",event.keyCode);
         if(event.keyCode == KeyCode.CHAT_SUBMIT)
         {
            this.submitText();
         }
      }
   }
}

