package survarium.chat
{
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.StyleSheet;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.layout.LayoutData;
   import survarium.Util;
   import survarium.constants.ChatChannel;
   import survarium.constants.Colors;
   import survarium.constants.KeyCode;
   import survarium.constants.UiSound;
   
   public class Chat extends UIComponent
   {
      
      protected static const TIME:String = "WhiteTime";
      
      protected static const PLAYER:String = "WhiteLocal";
      
      protected static const PLAYER_TEXT:String = "PlayerText";
      
      protected static const NICKNAME:String = "WhiteName";
      
      protected static const NICKNAME_SQUAD:String = "SquadName";
      
      protected static const PRIVATE_CHANNELS:Array = [ChatChannel.PRIVATE,ChatChannel.GENERAL,ChatChannel.CLAN];
      
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
                        ExternalInterface.call("sound_play",UiSound.MESSAGE);
                     }
                     else if(this._activeTab != j)
                     {
                        this._data.tabs[k][j].updated = true;
                        ExternalInterface.call("sound_play",UiSound.MESSAGE);
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
      
      public function setData(channels:Array) : void
      {
         var i:int = 0;
         while(i < channels.length)
         {
            channels[i].index = i + 1;
            i++;
         }
         this._data.channels_data = channels;
         i = 0;
         while(i < this._data.channels_data.length)
         {
            trace("colors is:",i,this._data.channels_data[i].id,this._data.channels_data[i].color);
            i++;
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
      
      // The senders the chat's name links stand for: a link carries a number, never the
      // name, so a name's characters cannot leave the attribute or come back changed.
      // The game's escape and encodeURIComponent do not round-trip every name (escape
      // keeps 16 bits of a character, 0x571700; encodeURIComponent writes U+0080..U+00FF as
      // one %XX, 0x571996, the others as their UTF-8 bytes, 0x571934, which no decoding,
      // 0x571A10, gives both back). The LINKED_NAMES senders who spoke last, each its number
      // while kept (a tab keeps 180 lines); a link to one let go reads as no name, never
      // another one.
      private static const LINKED_NAMES:int = 2048;
      private var _linkIds:Dictionary = new Dictionary();
      private var _linkNames:Dictionary = new Dictionary();
      // The kept numbers, the one whose sender spoke longest ago first.
      private var _linkOrder:Array = [];
      private var _nextLinkId:int = 0;

      // A sender's name as the link "<name>: " ("event:<subName>:<its number>"), which
      // LobbyChat.clickLinkHandler reads back with linkedName.
      protected function linked(name:String, subName:String) : String
      {
         // Keyed "@<name>": a name such as "constructor" never finds what every object has.
         var id:* = this._linkIds["@" + name];
         if(id === undefined)
         {
            id = this._nextLinkId++;
            this._linkIds["@" + name] = id;
            this._linkNames[id] = name;
            if(this._linkOrder.length >= LINKED_NAMES)
            {
               var gone:int = this._linkOrder.shift();
               delete this._linkIds["@" + this._linkNames[gone]];
               delete this._linkNames[gone];
            }
         }
         else
         {
            this._linkOrder.splice(this._linkOrder.indexOf(id),1);
         }
         this._linkOrder.push(id);
         return "<a href=\'event:" + subName + ":" + id + "\'>" + Util.escapeHtml(name) + ": </a>";
      }

      // The name a link's number stands for, null for one the chat has let go.
      protected function linkedName(id:String) : String
      {
         var name:* = /^[0-9]+$/.test(id) ? this._linkNames[int(id)] : undefined;
         return name === undefined ? null : String(name);
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
         return "<" + style + ">" + Util.escapeHtml(value) + "</" + style + ">";
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
         if(event.keyCode == KeyCode.CHAT_SUBMIT)
         {
            this.submitText();
         }
      }
   }
}

