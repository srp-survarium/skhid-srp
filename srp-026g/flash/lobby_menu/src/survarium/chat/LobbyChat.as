package survarium.chat
{
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.events.TextEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.gfx.MouseEventEx;
   import scaleform.gfx.TextFieldEx;
   import survarium.constants.ChatChannel;
   import survarium.constants.ClaimReason;
   import survarium.constants.CommentKind;
   import survarium.constants.FriendStatus;
   import survarium.controls.ButtonBarBottom;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.friends.FriendsMenu;
   import survarium.friends.SquadList;
   import survarium.rooms.RoomsText;

   public class LobbyChat extends Chat
   {
      
      protected const _def_tab_width:uint = 550;
      
      protected const _def_tab_height:uint = 510;
      
      protected const _start_tab_shiftX:uint = 40;
      
      protected const _start_tab_shiftY:uint = 70;
      
      protected var _min_tab_width:int = 350;
      
      protected var _min_tab_height:int = 210;
      
      protected var _max_tab_width:int = 900;
      
      protected var _max_tab_height:int = 570;
      
      protected const _def_font_size:uint = 15;
      
      protected const _def_font_size_big:uint = 19;
      
      protected const _fontScale:Number = 1.207;
      
      protected var _tapPageRenderer:ButtonBarBottom;
      
      protected var _friends:Array;
      
      protected var _lastIdx:int = 0;
      
      protected var _was_last:Boolean;
      
      protected var _lastName:String;
      
      protected var _changed:Boolean = false;
      
      protected var _last_index:int = -1;
      
      protected var _ignore_changed:Boolean = false;
      
      protected var _callCount:int = 0;
      
      protected var _selectedIndex:int = -1;
      
      protected var _tabs:Dictionary = new Dictionary();
      
      protected var _wasSelected:Boolean = false;

      // srp's chat of a room (the owner's of 2026-10-20: "each room has its
      // own chat"): ChatChannel.ROOM, no channel of the game's (its client
      // sends a line typed in a tab of it with id 0, and gives a received one
      // to the tabs that list it; social.md 9.8), and its tab ChatTab.ROOM,
      // far from the game's ids (ChatTab, then a private talk each). The rooms
      // window puts the tab up while in a room.

      // clearIngnore clears the ignore state on its 12th call
      private static const CLEAR_IGNORE_CALLS:int = 12;

      public function LobbyChat()
      {
         super();
         _activeTab = -1;
         _selectedIndex = -1;
      }
      
      public function scrollTabsToLastMessage() : void
      {
         if(_activeMode == -1)
         {
            return;
         }
         var modeTabs:Array = _data.tabs[_activeMode];
         if(!modeTabs)
         {
            return;
         }
         var tabCount:uint = modeTabs.length;
         var tab:LobbyChatTab;
         var i:uint = 0;
         while(i < tabCount)
         {
            tab = this._tabs[modeTabs[i].id];
            if(tab)
            {
               tab.scrollToBottom();
            }
            i++;
         }
      }
      
      public function ingnoreFoucOut() : void
      {
         trace("ingnoreFoucOut",_activeTab,_activeMode);
         if(_activeTab == -1)
         {
            return;
         }
         var tab:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(tab)
         {
            tab.ignoreFocus = true;
         }
      }
      
      public function set tapPageRenderer(value:ButtonBarBottom) : void
      {
         _tapPageRenderer = value;
         this._tapPageRenderer.addEventListener(ButtonEvent.CLICK,this.onTabIndexClick,false);
         this._tapPageRenderer.addEventListener(IndexEvent.INDEX_CHANGE,this.onTabIndexChange,false);
      }
      
      public function setMode(mode:uint) : void
      {
         clearTabs();
         _activeMode = mode;
         updateChatSize();
      }
      
      public function setPlayers(players:Array, refresh:Boolean = true) : void
      {
         _friends = players;
         if(_activeMode == -1 || !players)
         {
            return;
         }
         var modeTabs:Array = _data.tabs[_activeMode];
         if(!modeTabs)
         {
            return;
         }
         var tabCount:uint = modeTabs.length;
         var tab:LobbyChatTab;
         var i:uint = 0;
         while(i < tabCount)
         {
            tab = this._tabs[modeTabs[i].id];
            this.checkStatusIcon(tab);
            i++;
         }
      }
      
      protected function checkStatusIcon(tab:LobbyChatTab) : void
      {
         var tabData:Object;
         var playerName:String;
         var i:uint;
         var status:int;
         var isOnline:Boolean;
         if(tab && this._friends)
         {
            tab.statusIndicator.visible = false;
            tab.text_inpput.enabled = true;
            tabData = tab.getData();
            if(!tabData)
            {
               return;
            }
            playerName = tabData.name;
            i = 0;
            while(i < this._friends.length)
            {
               if(playerName == this._friends[i].name)
               {
                  status = (this._friends[i].status);
                  isOnline = status == FriendStatus.LOBBY || status == FriendStatus.IN_GAME || status == FriendStatus.AWAY;
                  tab.statusIndicator.gotoAndStop(isOnline ? "online" : "offline");
                  tab.statusIndicator.tipData = {name:FriendsMenu.getStatus(status)};
                  tab.statusIndicator.visible = true;
                  tab.text_inpput.enabled = isOnline;
                  return;
               }
               i++;
            }
         }
      }
      
      override public function focusTab(tabId:uint) : void
      {
         stackTrace("focusTab:");
         trace("selectActivereTabRenderer:: a",_activeTab,"focusTab:",tabId);
         super.focusTab(tabId);
         var count:int = (this._tapPageRenderer.dataProvider.length);
         var i:uint = 0;
         while(i < count)
         {
            if(this._tapPageRenderer.dataProvider[i].id == tabId)
            {
               _selectedIndex = i;
               break;
            }
            i++;
         }
         trace("selectActivereTabRenderer:: b",_activeTab,"focusTab::",this._selectedIndex);
         if(this._tapPageRenderer && this._tapPageRenderer.selectedIndex != this._selectedIndex)
         {
            trace("selectActivereTabRenderer:: st",_activeTab,"focusTab",this._selectedIndex);
            this._tapPageRenderer.selectedIndex = this._selectedIndex;
            if(this._tabs[tabId] && !stage.focus)
            {
               trace("ff:tab");
               stage.focus = this._tabs[tabId];
            }
            trace("selectActivereTabRenderer:: end",_activeTab,"focusTab",stage.focus);
         }
      }
      
      override public function submitText(event:Event = null) : void
      {
         if(_activeTab == -1 || !_data || !_data.tabs)
         {
            return;
         }
         var tab:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(tab)
         {
            tab.submitText();
         }
      }
      
      override public function clearChannel(channelId:uint) : void
      {
         var tab:LobbyChatTab;
         _messages[channelId] = "";
         if(this._tabs[channelId])
         {
            tab = this._tabs[channelId];
            tab.text_chat.textField.removeEventListener(MouseEvent.CLICK,this.onTxClick);
            tab.text_chat.textField.removeEventListener(TextEvent.LINK,this.clickLinkHandler);
            tab.text_chat.textField.removeEventListener(MouseEvent.DOUBLE_CLICK,this.clickLinkHandlerDouble);
            tab.removeEventListener(Event.CLOSE,this.hideTab);
            tab.clear();
            parent.removeChild(tab);
            trace("clearChannel",channelId,this._selectedIndex);
            if(this._selectedIndex > -1 && _data.tabs[_activeMode][this._selectedIndex].id == channelId)
            {
               this._tapPageRenderer.selectedIndex = -1;
               _activeTab = -1;
               _selectedIndex = -1;
            }
            delete this._tabs[channelId];
         }
      }
      
      protected function unfocusTab() : void
      {
         _activeTab = -1;
         _selectedIndex = -1;
         if(this._tapPageRenderer && this._tapPageRenderer.selectedIndex != this._selectedIndex)
         {
            trace("selectActivereTabRenderer::",_activeTab,"unfocusTab:",this._selectedIndex);
            this._tapPageRenderer.selectedIndex = this._selectedIndex;
         }
      }
      
      protected function onTabIndexClick(event:ButtonEvent) : void
      {
         if(!event.target.data)
         {
            return;
         }
         var tab:LobbyChatTab = this._tabs[event.target.data.id];
         var hasFocus:Boolean;
         trace("onTabIndex:ClickSt, changed:",this._changed,"_wasSelected:",this._wasSelected);
         if(this._changed)
         {
            _changed = false;
            trace("onTabIndex:Click_end");
            return;
         }
         if(this._tapPageRenderer.selectedIndex != -1 && this._tapPageRenderer.getButtonAt(this._tapPageRenderer.selectedIndex) != event.target as Button)
         {
            trace("onTabIndex:Click_end");
            return;
         }
         trace("onTabIndexClick",this._tapPageRenderer.selectedIndex);
         trace("setTabRendererToVisible::",!tab.visible,"cl");
         if(tab)
         {
            tab.visible = !tab.visible;
            if(!tab.visible)
            {
               stage.focus = null;
               this.unfocusTab();
            }
            else
            {
               parent.addChild(tab);
               hasFocus = stage.focus != null;
               if(!hasFocus)
               {
                  trace("ff:textField");
                  stage.focus = tab.text_inpput.textField;
               }
               tab.text_inpput.text = "";
            }
            this.clearIngnore(null,true);
         }
         trace("onTabIndex:Click_end");
      }
      
      protected function getTabIndex(tabId:int) : int
      {
         var count:int = (_data.tabs[_activeMode].length);
         var i:uint = 0;
         while(i < count)
         {
            if(_data.tabs[_activeMode][i].id == tabId)
            {
               return i;
            }
            i++;
         }
         return -1;
      }
      
      protected function onTabIndexChange(event:IndexEvent) : void
      {
         var tabIndex:int = event.index;
         var tabId:Number;
         var tab:LobbyChatTab;
         var hasFocus:Boolean;
         if(event.data)
         {
            trace("eventHasData",event.data.id);
            tabIndex = this.getTabIndex(event.data.id);
         }
         trace("onTabIndex:Change",tabIndex,"event_index",event.index,"==",this._last_index,stage.focus,"ignore",this._ignore_changed,"_wasSelected",this._wasSelected,"_changed",this._changed);
         if(event.index == -1 || this._wasSelected && this._ignore_changed && event.index == this._last_index)
         {
            _wasSelected = true;
            this._tapPageRenderer.addEventListener(Event.ENTER_FRAME,this.clearIngnore);
            return;
         }
         this._tapPageRenderer.removeEventListener(Event.ENTER_FRAME,this.clearIngnore);
         if(!this._ignore_changed || tabIndex != this._last_index)
         {
            trace("_changed = true","a");
            _changed = true;
            this._tapPageRenderer.addEventListener(Event.EXIT_FRAME,this.clearChanged);
         }
         else
         {
            _ignore_changed = false;
         }
         _activeTab = tabIndex;
         _selectedIndex = event.index;
         if(_data.tabs[_activeMode] && _data.tabs[_activeMode][tabIndex])
         {
            tabId = (_data.tabs[_activeMode][tabIndex].id);
         }
         else
         {
            return;
         }
         tab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         trace("setTabRendererToVisible::",true,stage.focus);
         if(tab)
         {
            if(!tab.visible)
            {
               tab.scrollToBottom();
            }
            tab.visible = true;
            parent.addChild(tab);
            if(stage.focus && (!tab.contains(stage.focus)) || !stage.focus)
            {
               hasFocus = stage.focus != null;
               if(!hasFocus)
               {
                  stage.focus = tab.text_inpput.textField;
               }
               else
               {
                  tab.focusTextInput();
               }
               this.clearIngnore(null,true);
            }
         }
         _last_index = tabIndex;

      }
      
      protected function handleFocusOut(event:FocusEvent) : void
      {
         trace("handleFocusOut:",event.currentTarget.name,"event:props",event.relatedObject,event.target,event.currentTarget);
         var tab:LobbyChatTab = event.currentTarget as LobbyChatTab;
         if(!hasEventListener(Event.RENDER))
         {
            trace("addEventListener::",stage.focus);
            addEventListener(Event.RENDER,this.onRenderFocus);
         }
         trace("handleFocusOut:_end");
      }
      
      protected function onRenderFocus(event:Event = null) : void
      {
         trace("onRenderFocus",event.target.name,stage.focus,this._changed,event.currentTarget,_activeTab);
         removeEventListener(event.type,this.onRenderFocus);
         if(_activeTab == -1)
         {
            return;
         }
         var tab:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(this._changed)
         {
            return;
         }
         if(!stage.focus || !tab.contains(stage.focus) && tab != stage.focus)
         {
            if(!stage.focus)
            {
               this.onClearSelectionFocus();
            }
            else
            {
               tab.addEventListener(Event.RENDER,this.onClearSelectionFocus,false,0,true);
            }
         }
         trace("onRenderFocus_end");
      }
      
      protected function onClearSelectionFocus(event:Event = null) : void
      {
         var tab:LobbyChatTab;
         trace("onClearSelectionFocus",event);
         if(event)
         {
            tab = event.currentTarget as LobbyChatTab;
            tab.removeEventListener(event.type,this.onClearSelectionFocus);
         }
         _ignore_changed = true;
         this.unfocusTab();
         trace("onClearSelectionFocus_end");
      }
      
      protected function clearIngnore(event:Event = null, force:Boolean = false) : void
      {
         trace("clearIngnore::",this._callCount);
         _callCount += 1;
         if(this._callCount == CLEAR_IGNORE_CALLS || force)
         {
            if(event)
            {
               this._tapPageRenderer.removeEventListener(event.type,this.clearIngnore);
            }
            _wasSelected = false;
            _callCount = 0;
            if(!force)
            {
               _changed = false;
               trace("_changed = false","clearIngnore");
            }
            _last_index = -1;
         }
         trace("clearIngnore::_end");
      }
      
      protected function clearChanged(event:Event = null) : void
      {
         this._tapPageRenderer.removeEventListener(event.type,this.clearChanged);
         trace("_changed = false","clearChanged");
         _changed = false;
      }
      
      override protected function get_styled_text(message:Object) : String
      {
         if(int(message.type) == ChatChannel.ROOM)
         {
            this.roomLine(message);
         }
         var nameStyle:String = String(message.color != undefined ? message.color : getColor(message.type));
         var clanPrefix:String;
         var playerName:String;
         var textStyle:String;
         var isAdmin:Boolean;
         var isVg:Boolean;
         trace("get_styled_text",nameStyle,message.text,message.color,message.type,_data.name,message.name);
         if(!message.name)
         {
            return styled(message.time,TIME) + " " + styled(message.text,nameStyle);
         }
         clanPrefix = message.abbr ? "[" + message.abbr + "] " : "";
         if(message.type == ChatChannel.SYSTEM)
         {
            return styled(message.time,TIME) + " " + styled(clanPrefix + linked(message.name + ": ","messager"),nameStyle) + " " + styledChar(message.text,nameStyle);
         }
         if(_data && _data.name == message.name)
         {
            trace("ISLOCAL");
            return styled(message.time,TIME) + " " + styled(clanPrefix + linked(message.name + ": ","name"),PLAYER) + " " + styledChar(message.text,PLAYER_TEXT);
         }
         playerName = message.name;
         textStyle = nameStyle;
         isAdmin = clanPrefix.indexOf("[admin]") == 0;
         if(isAdmin)
         {
            nameStyle = "GM";
            textStyle = "GMText";
         }
         else
         {
            isVg = clanPrefix.indexOf("[VG]") == 0;
            if(isVg)
            {
               nameStyle = "VG";
               textStyle = "VGText";
            }
         }
         if(nameStyle == "White")
         {
            nameStyle = "WhiteNickname";
         }
         return styled(message.time,TIME) + " " + styled(clanPrefix + linked(message.name + ": ","name"),nameStyle) + " " + styledChar(message.text,textStyle);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         updateChatSize();
         stage.addEventListener(Event.RESIZE,this.onStageResize,false,-1,false);
      }
      
      override protected function invalidateChatData() : void
      {
         this.updateTabs();
         invalidateData();
      }
      
      override protected function draw() : void
      {
         trace("LC::_changed = false","draw");
         _changed = false;
         if(isInvalid(InvalidationType.SIZE))
         {
            updateChatSize();
            size();
         }
         if(this._tapPageRenderer)
         {
            trace("selectActivereTabRenderer::",_activeTab,"draw",this._selectedIndex);
            this._tapPageRenderer.selectedIndex = this._selectedIndex;
         }
         if(isInvalid(InvalidationType.DATA))
         {
            if(this._tapPageRenderer && this._tapPageRenderer.selectedIndex != this._selectedIndex)
            {
               trace("selectActivereTabRendererOnDara");
               this._tapPageRenderer.selectedIndex = this._selectedIndex;
            }
         }
      }
      
      override protected function findAppropriateTab(message:Object, messageText:String) : void
      {
         var i:uint = 0;
         var j:uint;
         while(i < _data.tabs[_activeMode].length)
         {
            if(message.tab_id != undefined)
            {
               if(_data.tabs[_activeMode][i].id == message.tab_id)
               {
                  this._tabs[_data.tabs[_activeMode][i].id].addMessage(messageText);
                  break;
               }
            }
            else if(_data.tabs[_activeMode][i])
            {
               j = 0;
               while(j < _data.tabs[_activeMode][i].channels.length)
               {
                  if(_data.tabs[_activeMode][i].channels[j] == message.type)
                  {
                     this._tabs[_data.tabs[_activeMode][i].id].addMessage(messageText);
                     break;
                  }
                  j++;
               }
            }
            i++;
         }
      }
      
      protected function onStageResize(event:Event) : void
      {
         var fontSize:int = Math.ceil(this._def_font_size / MainScene.scaleFactor);
         var bigFontSize:int = Math.ceil(this._def_font_size_big / MainScene.scaleFactor);
         var i:uint = 0;
         while(i < _style.styleNames.length)
         {
            style = _style.getStyle(_style.styleNames[i]);
            style.fontSize = fontSize;
            _style.setStyle(_style.styleNames[i],style);
            i++;
         }
         var lineHeight:Number = (fontSize + 2) * this._fontScale;
         var tab:LobbyChatTab;
         var style:Object;
         var labelFormat:TextFormat;
         var minHeight:int;
         var maxHeight:int;
         for each(tab in this._tabs)
         {
            minHeight = int((this._min_tab_height - 82) / lineHeight) * lineHeight + 82;
            maxHeight = int((this._max_tab_width - 82) / lineHeight) * lineHeight + 82;
            tab.setSizeLimits(this._min_tab_width,minHeight,this._max_tab_width,maxHeight);
            trace("new_size",tab.height,MainScene.scaleFactor,fontSize);
            tab.height = int((tab.height - 82) / lineHeight) * lineHeight + 82;
            tab.x = MainScene.stageWidth - tab.width - this._start_tab_shiftX;
            tab.y = MainScene.stageHeight - tab.height - this._start_tab_shiftY;
            tab.prepareToResize();
            tab.text_chat.textField.text = "";
            tab.text_chat.invalidate();
            tab.text_inpput.textSize = fontSize;
            labelFormat = tab.labelTextField.textField.getTextFormat();
            labelFormat.size = bigFontSize;
            tab.labelTextField.textField.defaultTextFormat = labelFormat;
            tab.labelTextField.textField.setTextFormat(labelFormat);
         }
      }
      
      protected function updateTabs() : void
      {
         if(_activeMode == -1)
         {
            return;
         }
         var modeTabs:Array = _data.tabs[_activeMode];
         if(!modeTabs)
         {
            return;
         }
         var tabCount:uint = modeTabs.length;
         var tab:LobbyChatTab;
         var unhandledCount:uint = 0;
         var i:uint = 0;
         while(i < tabCount)
         {
            tab = this._tabs[modeTabs[i].id];
            if(tab && !tab.handled)
            {
               unhandledCount++;
            }
            i++;
         }
         trace("handleTabs::",unhandledCount);
         var cascadeStep:uint = 30;
         var tabClass:Class = getDefinitionByName("SrvChatTab") as Class;
         var j:uint = 0;
         var tabData:Object;
         var newTab:LobbyChatTab;
         var fontSize:int;
         var bigFontSize:int;
         var lineHeight:Number;
         var minHeight:int;
         var maxHeight:int;
         var labelFormat:TextFormat;
         while(j < tabCount)
         {
            tabData = modeTabs[j];
            tab = this._tabs[tabData.id];
            if(tabData && !tab)
            {
               newTab = new tabClass();
               fontSize = Math.ceil(this._def_font_size / MainScene.scaleFactor);
               bigFontSize = Math.ceil(this._def_font_size_big / MainScene.scaleFactor);
               lineHeight = (fontSize + 2) * this._fontScale;
               newTab.width = this._def_tab_width;
               newTab.height = int((this._def_tab_height - 82) / lineHeight) * lineHeight + 82;
               minHeight = int((this._min_tab_height - 82) / lineHeight) * lineHeight + 82;
               maxHeight = int((this._max_tab_width - 82) / lineHeight) * lineHeight + 82;
               newTab.setSizeLimits(this._min_tab_width,minHeight,this._max_tab_width,maxHeight);
               newTab.x = MainScene.stageWidth - newTab.width - 40 - unhandledCount * cascadeStep;
               newTab.y = MainScene.stageHeight - newTab.height - 70 - unhandledCount * cascadeStep;
               if(unhandledCount)
               {
                  unhandledCount--;
               }
               newTab.text_chat.textField.addEventListener(MouseEvent.CLICK,this.onTxClick);
               newTab.text_chat.textField.addEventListener(TextEvent.LINK,this.clickLinkHandler);
               newTab.text_chat.textField.addEventListener(MouseEvent.DOUBLE_CLICK,this.clickLinkHandlerDouble);
               newTab.text_chat.textField.text = "";
               newTab.text_chat.invalidate();
               newTab.text_inpput.textSize = fontSize;
               labelFormat = newTab.labelTextField.textField.getTextFormat();
               labelFormat.size = bigFontSize;
               newTab.labelTextField.textField.defaultTextFormat = labelFormat;
               newTab.labelTextField.textField.setTextFormat(labelFormat);
               newTab.addEventListener(Event.CLOSE,this.hideTab);
               newTab.addEventListener(Event.CHANNEL_MESSAGE,this.blinkTab);
               newTab.addEventListener(FocusEvent.FOCUS_OUT,this.handleFocusOut);
               TextFieldEx.setVerticalAlign(newTab.text_inpput.textField,TextFieldEx.VALIGN_CENTER);
               TextFieldEx.setVerticalAlign(newTab.labelTextField.textField,TextFieldEx.VALIGN_CENTER);
               newTab.owner = this;
               newTab.setData(tabData);
               newTab.name = "tab_" + tabData.name;
               newTab.send_function = send_function;
               newTab.setStyleSheet(_style);
               newTab.visible = false;
               this._tabs[tabData.id] = newTab;
               this.checkStatusIcon(newTab);
               trace("LC::addTabToParent",parent);
               parent.addChild(this._tabs[tabData.id]);
            }
            j++;
         }
      }
      
      protected function blinkTab(event:Event) : void
      {
         var tabData:Object = (event.target as LobbyChatTab).getData();
         var i:int;
         trace("blinkTab::",tabData ? tabData.id : "no_id");
         if(tabData)
         {
            i = 0;
            while(i < this._tapPageRenderer.dataProvider.length)
            {
               trace("IterateBottomData::",this._tapPageRenderer.dataProvider[i].id,tabData.id);
               if(this._tapPageRenderer.dataProvider[i] && this._tapPageRenderer.dataProvider[i].id == tabData.id)
               {
                  this._tapPageRenderer.dataProvider[i].blink = tabData.blink;
                  break;
               }
               i++;
            }
         }
         this._tapPageRenderer.dataProvider.invalidate();
      }
      
      protected function hideTab(event:Event) : void
      {
         trace("hideTab::",event.target);
         if(stage.focus)
         {
            stage.focus = null;
         }
         event.target.visible = false;
         this.unfocusTab();
      }
      
      protected function onClose(event:Event = null) : void
      {
         ExternalInterface.call("hide_chat_button");
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function onTxClick(event:MouseEventEx) : void
      {
         _lastIdx = event.buttonIdx;
      }
      
      protected function clickLinkHandlerDouble(event:MouseEvent) : void
      {
         trace("clickLinkHandlerDouble");
      }
      
      protected function clickLinkHandler(event:TextEvent) : void
      {
         trace("clickLinkHandler",event.text);
         var tab:LobbyChatTab = event.target.parent.parent as LobbyChatTab;
         if(_activeTab == -1 || _data.tabs[_activeMode][_activeTab].id != tab.getData().id)
         {
            this.focusTab(tab.getData().id);
            lineIndex = tab.text_chat.textField.getLineIndexAtPoint(tab.text_chat.textField.mouseX,tab.text_chat.textField.mouseY);
            if(lineIndex != -1)
            {
               lineText = tab.text_chat.textField.getLineText(lineIndex);
               trace("clickLinkHandler::text",lineText);
            }
         }
         var linkParts:Array = event.text.split(":");
         var lineIndex:int;
         var lineText:String;
         var isFriend:Boolean;
         var claimOptions:Array;
         var menuItems:Array;
         var i:int;
         if(linkParts[0] == "name" && _data.name != linkParts[1])
         {
            _lastName = linkParts[1];
            isFriend = false;
            if(this._friends)
            {
               i = 0;
               while(i < this._friends.length)
               {
                  if(this._friends[i].name == this._lastName)
                  {
                     isFriend = true;
                     break;
                  }
                  i++;
               }
            }
            claimOptions = [];
            if(this._lastIdx == MouseEventEx.RIGHT_BUTTON && this._lastName)
            {
               menuItems = [];
               if(!isFriend)
               {
                  menuItems.push({
                     name:LocalizationData.getName("add_to_friends"),
                     click_function:this.addToFriend
                  });
               }
               menuItems.push({
                  name:LocalizationData.getName("add_to_ignore"),
                  click_function:this.addToIgnore
               });
               if(_activeTab != -1 && _data.tabs[_activeMode][_activeTab].channels.length != 0)
               {
                  menuItems.push({
                     name:LocalizationData.getName("send_message"),
                     click_function:this.privateMessage
                  });
               }
               if(PlayerInfo.commander && !PlayerInfo.isSquadFull && !SquadList.isInSquad(this._lastName))
               {
                  menuItems.push({
                     name:LocalizationData.getName("invite_to_squad"),
                     click_function:this.addToSquad
                  });
               }
            }
            else if(this._lastIdx == MouseEventEx.LEFT_BUTTON && this._lastName && _activeTab != -1)
            {
               if(tab)
               {
                  tab.srartMessage(this._lastName + ", ");
               }
            }
            if(menuItems && menuItems.length)
            {
               claimOptions.push({
                  name:LocalizationData.getName("bot_or_cheater_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.BOT_OR_CHEATER
               });
               claimOptions.push({
                  name:LocalizationData.getName("swearing_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.SWEARING
               });
               claimOptions.push({
                  name:LocalizationData.getName("non_sport_behavior_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.NON_SPORT_BEHAVIOR
               });
               claimOptions.push({
                  name:LocalizationData.getName("account_trading_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.ACCOUNT_TRADING
               });
               menuItems.push({
                  name:LocalizationData.getName("claim_player"),
                  options:claimOptions
               });
               SrvContextMenu.show(menuItems,0,0,this);
            }
         }
      }
      
      protected function flagPlayer(item:Object) : void
      {
         trace("comment_about_user",item.nickname,item.index,item.subIndex);
         ExternalInterface.call("comment_about_user",item.nickname,item.index,item.subIndex);
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

      // A line of a room's chat: a player's in the general chat's colour
      // (the exe gives ChatChannel.ROOM none), the room's own (no sender: who
      // joined or left) in blue and in the game's language.
      protected function roomLine(message:Object) : void
      {
         if(!message.name)
         {
            message.text = RoomsText.notice(String(message.text));
            message.color = "Blue";
         }
         else if(message.color == undefined)
         {
            message.color = "White";
         }
      }
   }
}

