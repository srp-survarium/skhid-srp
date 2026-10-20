package survarium.chat
{
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.StyleSheet;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.layout.LayoutData;
   import survarium.Util;
   import survarium.constants.Colors;
   
   public class Chat extends UIComponent
   {
      
      protected static const TIME:String = "WhiteTime";
      
      protected static const PLAYER:String = "WhiteLocal";
      
      protected static const PLAYER_TEXT:String = "PlayerText";
      
      protected static const NICKNAME:String = "WhiteName";
      
      protected static const NICKNAME_SQUAD:String = "SquadName";
      
      protected static const PRIVATE_CHANNELS:Array = [4,1,3];
      
      public var ignoreFocus:Boolean = false;
      
      public var send_function:Function;
      
      public var text_inpput:TextInput;
      
      protected var _messages:Array = [];
      
      protected var _channels:Array = [];
      
      protected var _activeMode:uint = 0;
      
      protected var _activeTab:int = 0;
      
      protected var _style:StyleSheet = new StyleSheet();
      
      protected var _data:Object = new Object();
      
      protected var _isTyping:Boolean = false;
      
      protected var _nline:String = "\n";
      
      public function Chat()
      {
         super();
         if(!this._data.tabs)
         {
            this._data.tabs = new Array();
         }
      }
      
      override public function get width() : Number
      {
         return super.width;
      }
      
      override public function get height() : Number
      {
         return super.height;
      }
      
      override public function set width(param1:Number) : void
      {
         this.updateChatSize();
         invalidateSize();
      }
      
      override public function set height(param1:Number) : void
      {
         invalidateSize();
      }
      
      public function focusTab(param1:uint) : void
      {
         var _loc2_:uint;
         while(_loc2_ < this._data.tabs[this._activeMode].length)
         {
            if(this._data.tabs[this._activeMode][_loc2_].id == param1)
            {
               _activeTab = _loc2_;
               return;
            }
            _loc2_++;
         }
         _activeTab = -1;
      }
      
      public function addTab(param1:Object) : void
      {
         if(param1.tabs_group_id == undefined)
         {
            param1.tabs_group_id = this._activeMode;
         }
         if(!this._data.tabs[param1.tabs_group_id])
         {
            this._data.tabs[param1.tabs_group_id] = new Array();
         }
         trace("add tab:",param1.id,param1.name,param1.tabs_group_id,"|",this._data.tabs[param1.tabs_group_id],"channel:",param1.channel_to_send,param1.channels);
         if(this.hasTab(param1.id))
         {
            return;
         }
         this._messages[param1.id] = "";
         this._data.tabs[param1.tabs_group_id].push({
            id:param1.id,
            channels:param1.channels,
            name:param1.name,
            icon:param1.channel_to_send,
            closable:param1.closable,
            channel_to_send:param1.channel_to_send,
            index:param1.id + 1,
            bool_save_history:param1.save_history,
            tabs_group_id:param1.tabs_group_id
         });
         this.invalidateChatData();
      }
      
      public function hasTab(param1:uint) : Boolean
      {
         if(!this._data || !this._data.tabs)
         {
            return false;
         }
         var _loc2_:int = 0;
         var _loc3_:int;
         while(_loc2_ < this._data.tabs.length)
         {
            _loc3_ = 0;
            while(_loc3_ < this._data.tabs[_loc2_].length)
            {
               if(this._data.tabs[_loc2_][_loc3_].id == param1)
               {
                  return true;
               }
               _loc3_++;
            }
            _loc2_++;
         }
         return false;
      }
      
      public function removeTab(param1:uint) : void
      {
         if(!this._data || !this._data.tabs)
         {
            return;
         }
         var _loc2_:int = 0;
         var _loc3_:int;
         while(_loc2_ < this._data.tabs.length)
         {
            _loc3_ = 0;
            while(_loc3_ < this._data.tabs[_loc2_].length)
            {
               if(this._data.tabs[_loc2_][_loc3_].id == param1)
               {
                  this.clearChannel(this._data.tabs[_loc2_][_loc3_].id);
                  (this._data.tabs[_loc2_] as Array).splice(_loc3_,1);
                  break;
               }
               _loc3_++;
            }
            _loc2_++;
         }
         this.invalidateChatData();
      }
      
      public function got_message(param1:Object) : void
      {
         var _loc2_:String;
         trace("chat_got_message::",param1.type,param1.text);
         var _loc3_:Date = new Date();
         param1.time = "" + Util.getZeroString(_loc3_.getHours()) + ":" + Util.getZeroString(_loc3_.getMinutes()) + ":" + Util.getZeroString(_loc3_.getSeconds()) + " ";
         _loc2_ = this.get_styled_text(param1);
         var _loc4_:int = (this._data.tabs[this._activeMode][this._activeTab != -1 ? this._activeTab : 0].id);
         var _loc5_:int = 0;
         var _loc6_:int;
         var _loc7_:int;
         while(_loc5_ < this._data.tabs.length)
         {
            _loc6_ = 0;
            while(_loc6_ < this._data.tabs[_loc5_].length)
            {
               if(param1.tab_id != undefined)
               {
                  if(this._data.tabs[_loc5_][_loc6_].id == param1.tab_id)
                  {
                     this._messages[this._data.tabs[_loc5_][_loc6_].id] += _loc2_ + this._nline;
                     if(this._activeMode != _loc5_)
                     {
                        this._data.tabs[_loc5_][_loc6_].updated = true;
                        ExternalInterface.call("sound_play",5);
                     }
                     else if(this._activeTab != _loc6_)
                     {
                        this._data.tabs[_loc5_][_loc6_].updated = true;
                        ExternalInterface.call("sound_play",5);
                        invalidateState();
                     }
                     break;
                  }
               }
               else
               {
                  _loc7_ = 0;
                  while(_loc7_ < this._data.tabs[_loc5_][_loc6_].channels.length)
                  {
                     if(this._data.tabs[_loc5_][_loc6_].channels[_loc7_] == param1.type)
                     {
                        this._messages[this._data.tabs[_loc5_][_loc6_].id] += _loc2_ + this._nline;
                        if(this._activeTab != _loc6_)
                        {
                           this._data.tabs[_loc5_][_loc6_].updated = true;
                           invalidateState();
                        }
                        break;
                     }
                     _loc7_++;
                  }
               }
               _loc6_++;
            }
            _loc5_++;
         }
         this.findAppropriateTab(param1,_loc2_);
      }
      
      public function clearChannel(param1:uint) : void
      {
         this._messages[param1] = "";
      }
      
      public function activeTabId() : uint
      {
         return this._data.tabs[this._activeMode][this._activeTab].id;
      }
      
      public function setNewAdress(param1:String) : void
      {
      }
      
      public function srartMessage(param1:String) : void
      {
      }
      
      public function setLocalPlayer(param1:String) : void
      {
         trace("CHAT::setLocalPlayer",param1);
         this._data.name = param1;
      }
      
      public function setData(param1:Array) : void
      {
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            param1[_loc2_].index = _loc2_ + 1;
            _loc2_++;
         }
         this._data.channels_data = param1;
         _loc2_ = 0;
         while(_loc2_ < this._data.channels_data.length)
         {
            trace("colors is:",_loc2_,this._data.channels_data[_loc2_].id,this._data.channels_data[_loc2_].color);
            _loc2_++;
         }
      }
      
      public function submitText(param1:Event = null) : void
      {
         this.sendText();
      }
      
      public function set_focus_handler(param1:Boolean) : void
      {
      }
      
      protected function updateChatSize() : void
      {
      }
      
      protected function checkLenght(param1:int) : void
      {
      }
      
      protected function get_styled_text(param1:Object) : String
      {
         return param1.text;
      }
      
      protected function addMessage(param1:String) : void
      {
      }
      
      protected function setStyleSheet() : void
      {
      }
      
      protected function invalidateChatData() : void
      {
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!layoutData)
         {
            layoutData = new LayoutData();
         }
         var _loc1_:Object = new Object();
         _loc1_.color = "#999999";
         var _loc2_:Object = new Object();
         _loc1_.color = "#d3d3d3";
         var _loc3_:Object = new Object();
         _loc3_.color = "#F2F2F2";
         var _loc4_:Object = new Object();
         _loc4_.color = "#f5f33e";
         _loc4_.fontWeight = "bold";
         _loc4_.fontSize = 15;
         var _loc5_:Object = new Object();
         _loc5_.color = Colors.SQUAD_ST;
         _loc5_.fontWeight = "bold";
         _loc5_.fontSize = 15;
         var _loc6_:Object = new Object();
         _loc6_.color = "#d3d3d3";
         _loc6_.fontWeight = "bold";
         _loc6_.fontSize = 15;
         var _loc7_:Object = new Object();
         _loc7_.color = "#EC5662";
         _loc7_.fontWeight = "bold";
         _loc7_.fontSize = 15;
         var _loc8_:Object = new Object();
         _loc8_.color = "#A2A2A2";
         var _loc9_:Object = new Object();
         _loc9_.color = "#EC5662";
         _loc9_.fontStyle = "italic";
         var _loc10_:Object = new Object();
         _loc10_.color = "#079DD5";
         var _loc11_:Object = new Object();
         _loc11_.color = "#66FF66";
         var _loc12_:Object = new Object();
         _loc12_.color = "#FF0000";
         var _loc13_:Object = new Object();
         _loc13_.color = "#FFC000";
         var _loc14_:Object = new Object();
         _loc14_.color = "#079DD5";
         var _loc15_:Object = new Object();
         _loc15_.color = "#F2F1BC";
         _loc15_.fontWeight = "bold";
         this._style.setStyle("WhiteInGame",_loc3_);
         this._style.setStyle("WhiteName",_loc6_);
         this._style.setStyle("WhiteNameEnemy",_loc7_);
         this._style.setStyle("WhiteTime",_loc8_);
         this._style.setStyle("WhiteLocal",_loc4_);
         this._style.setStyle("SquadName",_loc5_);
         this._style.setStyle("White",_loc1_);
         this._style.setStyle("WhiteNickname",_loc2_);
         this._style.setStyle("Pink",_loc9_);
         this._style.setStyle("Blue",_loc10_);
         this._style.setStyle("Green",_loc11_);
         this._style.setStyle("Red",_loc9_);
         this._style.setStyle("Yellow",_loc4_);
         var _loc16_:Object = new Object();
         _loc16_.color = "#EC7820";
         var _loc17_:Object = new Object();
         _loc17_.color = "#7075cc";
         var _loc18_:Object = new Object();
         _loc18_.color = "#E47327";
         var _loc19_:Object = new Object();
         _loc19_.color = "#7477a6";
         var _loc20_:Object = new Object();
         _loc20_.color = "#e9e8a1";
         this._style.setStyle("PlayerText",_loc20_);
         this._style.setStyle("GM",_loc17_);
         this._style.setStyle("VG",_loc16_);
         this._style.setStyle("GMText",_loc19_);
         this._style.setStyle("VGText",_loc18_);
         this.setStyleSheet();
      }
      
      protected function findAppropriateTab(param1:Object, param2:String) : void
      {
         var _loc3_:uint;
         if(this._activeTab != -1)
         {
            if(param1.tab_id != undefined)
            {
               if(this._data.tabs[this._activeMode][this._activeTab].id == param1.tab_id)
               {
                  this.addMessage(param2);
               }
            }
            else
            {
               _loc3_ = 0;
               while(_loc3_ < this._data.tabs[this._activeMode][this._activeTab].channels.length)
               {
                  if(this._data.tabs[this._activeMode][this._activeTab].channels[_loc3_] == param1.type)
                  {
                     this.addMessage(param2);
                     break;
                  }
                  _loc3_++;
               }
            }
         }
      }
      
      protected function size() : void
      {
      }
      
      protected function goFakeMessage() : *
      {
         return true;
      }
      
      protected function sendText() : Boolean
      {
         var _loc1_:int = (this._data.tabs[this._activeMode][this._activeTab].channel_to_send);
         var _loc2_:*;
         trace("sendText::",_loc1_);
         if(this.send_function is Function)
         {
            _loc2_ = this.send_function(this.text_inpput,_loc1_);
            return _loc2_;
         }
         return false;
      }
      
      protected function linked(param1:String, param2:String) : String
      {
         return "<a href=\'event:" + param2 + ":" + param1 + "\'>" + param1 + "</a>";
      }
      
      protected function clearTabs() : void
      {
         var _loc1_:int;
         if(this._data.tabs[this._activeMode])
         {
            _loc1_ = 0;
            while(_loc1_ < this._data.tabs[this._activeMode].length)
            {
               if(!this._data.tabs[this._activeMode][_loc1_].bool_save_history)
               {
                  this.clearChannel(this._data.tabs[this._activeMode][_loc1_].id);
               }
               _loc1_++;
            }
         }
      }
      
      protected function styledChar(param1:String, param2:String) : String
      {
         param1 = param1.split("<").join("&lt;");
         return "<" + param2 + ">" + param1 + "</" + param2 + ">";
      }
      
      protected function styled(param1:String, param2:String) : String
      {
         return "<" + param2 + ">" + param1 + "</" + param2 + ">";
      }
      
      protected function getColor(param1:int) : String
      {
         var _loc2_:int;
         if(this._data && this._data.channels_data)
         {
            _loc2_ = 0;
            while(_loc2_ < this._data.channels_data.length)
            {
               if(this._data.channels_data[_loc2_].id == param1)
               {
                  return this._data.channels_data[_loc2_].color;
               }
               _loc2_++;
            }
         }
         return "";
      }
      
      protected function checkInputKeyForSubmit(param1:KeyboardEvent) : void
      {
         trace("checkInputKeyForSubmit::",param1.keyCode);
         if(param1.keyCode == 150)
         {
            this.submitText();
         }
      }
   }
}

