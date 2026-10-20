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

   config namespace SOUND;
   SOUND const MESSAGE:int = 5;
   
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
      
      override public function set width(value:Number) : void
      {
         this.updateChatSize();
         invalidateSize();
      }
      
      override public function set height(value:Number) : void
      {
         invalidateSize();
      }
      
      public function focusTab(id:uint) : void
      {
         var i:uint;
         while(i < this._data.tabs[this._activeMode].length)
         {
            if(this._data.tabs[this._activeMode][i].id == id)
            {
               _activeTab = i;
               return;
            }
            i++;
         }
         _activeTab = -1;
      }
      
      public function addTab(data:Object) : void
      {
         if(data.tabs_group_id == undefined)
         {
            data.tabs_group_id = this._activeMode;
         }
         if(!this._data.tabs[data.tabs_group_id])
         {
            this._data.tabs[data.tabs_group_id] = new Array();
         }
         trace("add tab:",data.id,data.name,data.tabs_group_id,"|",this._data.tabs[data.tabs_group_id],"channel:",data.channel_to_send,data.channels);
         if(this.hasTab(data.id))
         {
            return;
         }
         this._messages[data.id] = "";
         this._data.tabs[data.tabs_group_id].push({
            id:data.id,
            channels:data.channels,
            name:data.name,
            icon:data.channel_to_send,
            closable:data.closable,
            channel_to_send:data.channel_to_send,
            index:data.id + 1,
            bool_save_history:data.save_history,
            tabs_group_id:data.tabs_group_id
         });
         this.invalidateChatData();
      }
      
      public function hasTab(id:uint) : Boolean
      {
         if(!this._data || !this._data.tabs)
         {
            return false;
         }
         var j:int = 0;
         var i:int;
         while(j < this._data.tabs.length)
         {
            i = 0;
            while(i < this._data.tabs[j].length)
            {
               if(this._data.tabs[j][i].id == id)
               {
                  return true;
               }
               i++;
            }
            j++;
         }
         return false;
      }
      
      public function removeTab(id:uint) : void
      {
         if(!this._data || !this._data.tabs)
         {
            return;
         }
         var j:int = 0;
         var i:int;
         while(j < this._data.tabs.length)
         {
            i = 0;
            while(i < this._data.tabs[j].length)
            {
               if(this._data.tabs[j][i].id == id)
               {
                  this.clearChannel(this._data.tabs[j][i].id);
                  (this._data.tabs[j] as Array).splice(i,1);
                  break;
               }
               i++;
            }
            j++;
         }
         this.invalidateChatData();
      }
      
      public function got_message(value:Object) : void
      {
         var _finalText:String;
         trace("chat_got_message::",value.type,value.text);
         var _date:Date = new Date();
         value.time = "" + Util.getZeroString(_date.getHours()) + ":" + Util.getZeroString(_date.getMinutes()) + ":" + Util.getZeroString(_date.getSeconds()) + " ";
         _finalText = this.get_styled_text(value);
         var slected:int = (this._data.tabs[this._activeMode][this._activeTab != -1 ? this._activeTab : 0].id);
         var k:int = 0;
         var j:int;
         var i:int;
         while(k < this._data.tabs.length)
         {
            j = 0;
            while(j < this._data.tabs[k].length)
            {
               if(value.tab_id != undefined)
               {
                  if(this._data.tabs[k][j].id == value.tab_id)
                  {
                     this._messages[this._data.tabs[k][j].id] += _finalText + this._nline;
                     if(this._activeMode != k)
                     {
                        this._data.tabs[k][j].updated = true;
                        ExternalInterface.call("sound_play",SOUND::MESSAGE);
                     }
                     else if(this._activeTab != j)
                     {
                        this._data.tabs[k][j].updated = true;
                        ExternalInterface.call("sound_play",SOUND::MESSAGE);
                        invalidateState();
                     }
                     break;
                  }
               }
               else
               {
                  i = 0;
                  while(i < this._data.tabs[k][j].channels.length)
                  {
                     if(this._data.tabs[k][j].channels[i] == value.type)
                     {
                        this._messages[this._data.tabs[k][j].id] += _finalText + this._nline;
                        if(this._activeTab != j)
                        {
                           this._data.tabs[k][j].updated = true;
                           invalidateState();
                        }
                        break;
                     }
                     i++;
                  }
               }
               j++;
            }
            k++;
         }
         this.findAppropriateTab(value,_finalText);
      }
      
      public function clearChannel(value:uint) : void
      {
         this._messages[value] = "";
      }
      
      public function activeTabId() : uint
      {
         return this._data.tabs[this._activeMode][this._activeTab].id;
      }
      
      public function setNewAdress(value:String) : void
      {
      }
      
      public function srartMessage(value:String) : void
      {
      }
      
      public function setLocalPlayer(value:String) : void
      {
         trace("CHAT::setLocalPlayer",value);
         this._data.name = value;
      }
      
      public function setData(value:Array) : void
      {
         for(var i:int = 0; i < value.length; i++)
         {
            value[i].index = i + 1;
         }
         this._data.channels_data = value;
         for(i = 0; i < this._data.channels_data.length; i++)
         {
            trace("colors is:",i,this._data.channels_data[i].id,this._data.channels_data[i].color);
         }
      }
      
      public function submitText(event:Event = null) : void
      {
         this.sendText();
      }
      
      public function set_focus_handler(bool:Boolean) : void
      {
      }
      
      protected function updateChatSize() : void
      {
      }
      
      protected function checkLenght(value:int) : void
      {
      }
      
      protected function get_styled_text(value:Object) : String
      {
         return value.text;
      }
      
      protected function addMessage(value:String) : void
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
         var _ts:Object = new Object();
         _ts.color = "#999999";
         var _tsNickname:Object = new Object();
         _ts.color = "#d3d3d3";
         var _ts_ingame:Object = new Object();
         _ts_ingame.color = "#F2F2F2";
         var _ts_player:Object = new Object();
         _ts_player.color = "#f5f33e";
         _ts_player.fontWeight = "bold";
         _ts_player.fontSize = 15;
         var _ts_player_squad:Object = new Object();
         _ts_player_squad.color = Colors.SQUAD_ST;
         _ts_player_squad.fontWeight = "bold";
         _ts_player_squad.fontSize = 15;
         var _ts_name:Object = new Object();
         _ts_name.color = "#d3d3d3";
         _ts_name.fontWeight = "bold";
         _ts_name.fontSize = 15;
         var _ts_name_enemy:Object = new Object();
         _ts_name_enemy.color = "#EC5662";
         _ts_name_enemy.fontWeight = "bold";
         _ts_name_enemy.fontSize = 15;
         var _ts_time:Object = new Object();
         _ts_time.color = "#A2A2A2";
         var _ad:Object = new Object();
         _ad.color = "#EC5662";
         _ad.fontStyle = "italic";
         var _cl:Object = new Object();
         _cl.color = "#079DD5";
         var _pr:Object = new Object();
         _pr.color = "#66FF66";
         var _ns:Object = new Object();
         _ns.color = "#FF0000";
         var _mh:Object = new Object();
         _mh.color = "#FFC000";
         var _tm:Object = new Object();
         _tm.color = "#079DD5";
         var _nsH:Object = new Object();
         _nsH.color = "#F2F1BC";
         _nsH.fontWeight = "bold";
         this._style.setStyle("WhiteInGame",_ts_ingame);
         this._style.setStyle("WhiteName",_ts_name);
         this._style.setStyle("WhiteNameEnemy",_ts_name_enemy);
         this._style.setStyle("WhiteTime",_ts_time);
         this._style.setStyle("WhiteLocal",_ts_player);
         this._style.setStyle("SquadName",_ts_player_squad);
         this._style.setStyle("White",_ts);
         this._style.setStyle("WhiteNickname",_tsNickname);
         this._style.setStyle("Pink",_ad);
         this._style.setStyle("Blue",_cl);
         this._style.setStyle("Green",_pr);
         this._style.setStyle("Red",_ad);
         this._style.setStyle("Yellow",_ts_player);
         var _vg:Object = new Object();
         _vg.color = "#EC7820";
         var _gm:Object = new Object();
         _gm.color = "#7075cc";
         var _vgDull:Object = new Object();
         _vgDull.color = "#E47327";
         var _gmDull:Object = new Object();
         _gmDull.color = "#7477a6";
         var _ts_player_text:Object = new Object();
         _ts_player_text.color = "#e9e8a1";
         this._style.setStyle("PlayerText",_ts_player_text);
         this._style.setStyle("GM",_gm);
         this._style.setStyle("VG",_vg);
         this._style.setStyle("GMText",_gmDull);
         this._style.setStyle("VGText",_vgDull);
         this.setStyleSheet();
      }
      
      protected function findAppropriateTab(value:Object, text:String) : void
      {
         var i:uint;
         if(this._activeTab != -1)
         {
            if(value.tab_id != undefined)
            {
               if(this._data.tabs[this._activeMode][this._activeTab].id == value.tab_id)
               {
                  this.addMessage(text);
               }
            }
            else
            {
               i = 0;
               while(i < this._data.tabs[this._activeMode][this._activeTab].channels.length)
               {
                  if(this._data.tabs[this._activeMode][this._activeTab].channels[i] == value.type)
                  {
                     this.addMessage(text);
                     break;
                  }
                  i++;
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
         var slected:int = (this._data.tabs[this._activeMode][this._activeTab].channel_to_send);
         var _received:*;
         trace("sendText::",slected);
         if(this.send_function is Function)
         {
            _received = this.send_function(this.text_inpput,slected);
            return _received;
         }
         return false;
      }
      
      protected function linked(value:String, subName:String) : String
      {
         return "<a href=\'event:" + subName + ":" + value + "\'>" + value + "</a>";
      }
      
      protected function clearTabs() : void
      {
         var i:int;
         if(this._data.tabs[this._activeMode])
         {
            i = 0;
            while(i < this._data.tabs[this._activeMode].length)
            {
               if(!this._data.tabs[this._activeMode][i].bool_save_history)
               {
                  this.clearChannel(this._data.tabs[this._activeMode][i].id);
               }
               i++;
            }
         }
      }
      
      protected function styledChar(value:String, style:String) : String
      {
         value = value.split("<").join("&lt;");
         return "<" + style + ">" + value + "</" + style + ">";
      }
      
      protected function styled(value:String, style:String) : String
      {
         return "<" + style + ">" + value + "</" + style + ">";
      }
      
      protected function getColor(value:int) : String
      {
         var i:int;
         if(this._data && this._data.channels_data)
         {
            i = 0;
            while(i < this._data.channels_data.length)
            {
               if(this._data.channels_data[i].id == value)
               {
                  return this._data.channels_data[i].color;
               }
               i++;
            }
         }
         return "";
      }
      
      protected function checkInputKeyForSubmit(event:KeyboardEvent) : void
      {
         trace("checkInputKeyForSubmit::",event.keyCode);
         if(event.keyCode == 150)
         {
            this.submitText();
         }
      }
   }
}

