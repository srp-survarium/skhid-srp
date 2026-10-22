package survarium.chat
{
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.events.ComponentEvent;
   
   public class GameChat extends Chat
   {
      
      protected static const NICKNAME_ENEMY:String = "WhiteNameEnemy";
      
      public var messageContainer:MessageLogList;
      
      public var textFieldName:TextField;
      
      public function GameChat()
      {
         super();
      }
      
      override public function set width(value:Number) : void
      {
         text_inpput.setSize(value - 140 - text_inpput.x,text_inpput.height);
         super.width = value;
      }
      
      override public function set height(value:Number) : void
      {
         text_inpput.y = value - 476;
         super.height = value;
      }
      
      override public function focusTab(id:uint) : void
      {
         super.focusTab(id);
         this.updateChatData();
      }
      
      override public function set_focus_handler(bool:Boolean) : void
      {
         if(bool)
         {
            text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.start_enter_text);
         }
         else
         {
            text_inpput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.start_enter_text);
         }
      }
      
      public function startEnterMessage() : void
      {
         if(_data.tabs[_activeMode] && _data.tabs[_activeMode][_activeTab])
         {
            this.textFieldName.text = _data.tabs[_activeMode][_activeTab].name + ":";
         }
         this.textFieldName.visible = true;
         text_inpput.x = this.textFieldName.width - 2;
         text_inpput.visible = true;
         stage.focus = text_inpput.textField;
      }
      
      public function stopEnterMessage() : void
      {
         this.textFieldName.visible = true;
         this.textFieldName.visible = false;
         stage.focus = null;
         text_inpput.visible = false;
      }
      
      public function nextTab() : void
      {
         ++_activeTab;
         if(_activeTab == _data.tabs[_activeMode].length)
         {
            _activeTab = 0;
         }
         if(this.textFieldName)
         {
            this.textFieldName.text = _data.tabs[_activeMode][_activeTab].name + ":";
            text_inpput.x = this.textFieldName.width - 2;
         }
      }
      
      public function clear() : void
      {
         this.messageContainer.clear(true);
         _data = new Object();
         _data.tabs = [];
         _activeTab = 0;
         _messages = [];
         _channels = [];
         _activeMode = 0;
      }
      
      public function prevTab() : void
      {
         --_activeTab;
         if(_activeTab == -1)
         {
            _activeTab = _data.tabs[_activeMode].length - 1;
         }
         if(this.textFieldName)
         {
            this.textFieldName.text = _data.tabs[_activeMode][_activeTab].name + ":";
            text_inpput.x = this.textFieldName.width - 2;
         }
      }
      
      override public function setNewAdress(value:String) : void
      {
         var _actual_txt:String = text_inpput.text;
         text_inpput.text = value;
         text_inpput.addEventListener(Event.RENDER,this.onCC);
      }
      
      override public function srartMessage(value:String) : void
      {
         text_inpput.text = value;
         text_inpput.addEventListener(Event.RENDER,this.onCC);
      }
      
      override public function submitText(event:Event = null) : void
      {
         if(text_inpput.text == "")
         {
            return;
         }
         var _actual_txt:String = text_inpput.text;
         var isValid:Boolean = sendText();
         trace("submitText::",isValid);
         if(isValid)
         {
            text_inpput.text = "";
            text_inpput.textField.text = "";
         }
      }
      
      protected function onNewState(e:Event = null) : void
      {
         text_inpput.removeEventListener(ComponentEvent.STATE_CHANGE,this.onNewState);
         this.moveCaret();
      }
      
      protected function updateChatData() : void
      {
         var slected:int;
         if(_data.tabs[_activeMode] && _data.tabs[_activeMode][_activeTab])
         {
            slected = (_data.tabs[_activeMode][_activeTab].id);
         }
         else
         {
            return;
         }
         if(this.messageContainer)
         {
            this.messageContainer.clear();
            if(_messages[slected])
            {
               this.messageContainer.addText(_messages[slected]);
            }
         }
         if(this.textFieldName)
         {
            this.textFieldName.text = _data.tabs[_activeMode][_activeTab].name + ":";
            text_inpput.x = this.textFieldName.width - 2;
         }
      }
      
      protected function stop_enter_text(e:FocusEvent = null) : void
      {
         if(e.relatedObject != null && e.relatedObject != text_inpput && !ignoreFocus || _isTyping && !ignoreFocus)
         {
            ExternalInterface.call("chat_enter_cancel");
            _isTyping = false;
         }
         ignoreFocus = false;
         this.messageContainer.resumeHideout();
      }
      
      protected function start_enter_text(e:FocusEvent = null) : void
      {
         var slected:int;
         if(e.relatedObject != null && !ignoreFocus)
         {
            ExternalInterface.call("chat_enter_start");
         }
         ignoreFocus = false;
         _isTyping = true;
         text_inpput.addEventListener(ComponentEvent.STATE_CHANGE,this.onNewState);
         trace("start_enter_text::");
         if(_data.tabs[_activeMode] && _data.tabs[_activeMode][_activeTab])
         {
            slected = (_data.tabs[_activeMode][_activeTab].id);
         }
         else
         {
            return;
         }
         if(this.messageContainer)
         {
            this.messageContainer.showLastMessages();
            this.messageContainer.stopHideout();
         }
      }
      
      protected function onCC(e:Event) : void
      {
         this.moveCaret();
         text_inpput.removeEventListener(Event.RENDER,this.onCC);
      }
      
      protected function moveCaret() : void
      {
         text_inpput.textField.setSelection(text_inpput.textField.text.length,text_inpput.textField.text.length);
      }
      
      override protected function setStyleSheet() : void
      {
         this.messageContainer.styleSheet = _style;
      }
      
      override protected function addMessage(value:String) : void
      {
         trace("addMessage",value);
         this.messageContainer.addMessage(value + _nline);
      }
      
      public function test_message(value:Object) : void
      {
         var _finalText:String = this.get_styled_text(value);
         trace("test_message::",_finalText);
         this.addMessage(_finalText);
      }
      
      protected function onLocal(value:String) : void
      {
         text_inpput.defaultText = value;
      }
      
      override protected function get_styled_text(value:Object) : String
      {
         var slyleStr:String = String(value.color != undefined ? value.color : getColor(value.type));
         var style:String;
         var isSquad:Boolean;
         var isAlly:Boolean;
         trace("get_styled_text",value.color,value.type,slyleStr,_data.name,value.name);
         if(!value.name)
         {
            return styled(value.text,slyleStr);
         }
         if(_data.name == value.name)
         {
            style = Chat.PLAYER;
         }
         else
         {
            isSquad = slyleStr == "Green";
            if(isSquad)
            {
               style = Chat.NICKNAME_SQUAD;
            }
            else
            {
               isAlly = slyleStr != "Red";
               style = isAlly ? Chat.NICKNAME : NICKNAME_ENEMY;
            }
         }
         return styled(value.name + ":",style) + " " + styledChar(value.text,"WhiteInGame");
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         text_inpput.addEventListener(KeyboardEvent.KEY_UP,checkInputKeyForSubmit,false,0,true);
         text_inpput.textField.addEventListener(FocusEvent.FOCUS_OUT,this.stop_enter_text,false,0,true);
         text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.start_enter_text,false,0,true);
      }
   }
}

