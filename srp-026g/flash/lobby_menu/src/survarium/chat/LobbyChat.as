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
   import survarium.controls.ButtonBarBottom;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.friends.FriendsMenu;
   import survarium.friends.SquadList;
   
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
         var _loc1_:Array = _data.tabs[_activeMode];
         if(!_loc1_)
         {
            return;
         }
         var _loc2_:uint = _loc1_.length;
         var _loc3_:LobbyChatTab;
         var _loc4_:uint = 0;
         while(_loc4_ < _loc2_)
         {
            _loc3_ = this._tabs[_loc1_[_loc4_].id];
            if(_loc3_)
            {
               _loc3_.scrollToBottom();
            }
            _loc4_++;
         }
      }
      
      public function ingnoreFoucOut() : void
      {
         trace("ingnoreFoucOut",_activeTab,_activeMode);
         if(_activeTab == -1)
         {
            return;
         }
         var _loc1_:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(_loc1_)
         {
            _loc1_.ignoreFocus = true;
         }
      }
      
      public function set tapPageRenderer(param1:ButtonBarBottom) : void
      {
         _tapPageRenderer = param1;
         this._tapPageRenderer.addEventListener(ButtonEvent.CLICK,this.onTabIndexClick,false);
         this._tapPageRenderer.addEventListener(IndexEvent.INDEX_CHANGE,this.onTabIndexChange,false);
      }
      
      public function setMode(param1:uint) : void
      {
         clearTabs();
         _activeMode = param1;
         updateChatSize();
      }
      
      public function setPlayers(param1:Array, param2:Boolean = true) : void
      {
         _friends = param1;
         if(_activeMode == -1 || !param1)
         {
            return;
         }
         var _loc3_:Array = _data.tabs[_activeMode];
         if(!_loc3_)
         {
            return;
         }
         var _loc4_:uint = _loc3_.length;
         var _loc5_:LobbyChatTab;
         var _loc6_:uint = 0;
         while(_loc6_ < _loc4_)
         {
            _loc5_ = this._tabs[_loc3_[_loc6_].id];
            this.checkStatusIcon(_loc5_);
            _loc6_++;
         }
      }
      
      protected function checkStatusIcon(param1:LobbyChatTab) : void
      {
         var _loc2_:Object;
         var _loc3_:String;
         var _loc4_:uint;
         var _loc5_:int;
         var _loc6_:Boolean;
         if(param1 && this._friends)
         {
            param1.statusIndicator.visible = false;
            param1.text_inpput.enabled = true;
            _loc2_ = param1.getData();
            if(!_loc2_)
            {
               return;
            }
            _loc3_ = _loc2_.name;
            _loc4_ = 0;
            while(_loc4_ < this._friends.length)
            {
               if(_loc3_ == this._friends[_loc4_].name)
               {
                  _loc5_ = (this._friends[_loc4_].status);
                  _loc6_ = _loc5_ == 0 || _loc5_ == 1 || _loc5_ == 3;
                  param1.statusIndicator.gotoAndStop(_loc6_ ? "online" : "offline");
                  param1.statusIndicator.tipData = {name:FriendsMenu.getStatus(_loc5_)};
                  param1.statusIndicator.visible = true;
                  param1.text_inpput.enabled = _loc6_;
                  return;
               }
               _loc4_++;
            }
         }
      }
      
      override public function focusTab(param1:uint) : void
      {
         stackTrace("focusTab:");
         trace("selectActivereTabRenderer:: a",_activeTab,"focusTab:",param1);
         super.focusTab(param1);
         var _loc2_:int = (this._tapPageRenderer.dataProvider.length);
         var _loc3_:uint = 0;
         while(_loc3_ < _loc2_)
         {
            if(this._tapPageRenderer.dataProvider[_loc3_].id == param1)
            {
               _selectedIndex = _loc3_;
               break;
            }
            _loc3_++;
         }
         trace("selectActivereTabRenderer:: b",_activeTab,"focusTab::",this._selectedIndex);
         if(this._tapPageRenderer && this._tapPageRenderer.selectedIndex != this._selectedIndex)
         {
            trace("selectActivereTabRenderer:: st",_activeTab,"focusTab",this._selectedIndex);
            this._tapPageRenderer.selectedIndex = this._selectedIndex;
            if(this._tabs[param1] && !stage.focus)
            {
               trace("ff:tab");
               stage.focus = this._tabs[param1];
            }
            trace("selectActivereTabRenderer:: end",_activeTab,"focusTab",stage.focus);
         }
      }
      
      override public function submitText(param1:Event = null) : void
      {
         if(_activeTab == -1 || !_data || !_data.tabs)
         {
            return;
         }
         var _loc2_:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(_loc2_)
         {
            _loc2_.submitText();
         }
      }
      
      override public function clearChannel(param1:uint) : void
      {
         var _loc2_:LobbyChatTab;
         _messages[param1] = "";
         if(this._tabs[param1])
         {
            _loc2_ = this._tabs[param1];
            _loc2_.text_chat.textField.removeEventListener(MouseEvent.CLICK,this.onTxClick);
            _loc2_.text_chat.textField.removeEventListener(TextEvent.LINK,this.clickLinkHandler);
            _loc2_.text_chat.textField.removeEventListener(MouseEvent.DOUBLE_CLICK,this.clickLinkHandlerDouble);
            _loc2_.removeEventListener(Event.CLOSE,this.hideTab);
            _loc2_.clear();
            parent.removeChild(_loc2_);
            trace("clearChannel",param1,this._selectedIndex);
            if(this._selectedIndex > -1 && _data.tabs[_activeMode][this._selectedIndex].id == param1)
            {
               this._tapPageRenderer.selectedIndex = -1;
               _activeTab = -1;
               _selectedIndex = -1;
            }
            delete this._tabs[param1];
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
      
      protected function onTabIndexClick(param1:ButtonEvent) : void
      {
         if(!param1.target.data)
         {
            return;
         }
         var _loc2_:LobbyChatTab = this._tabs[param1.target.data.id];
         var _loc3_:Boolean;
         trace("onTabIndex:ClickSt, changed:",this._changed,"_wasSelected:",this._wasSelected);
         if(this._changed)
         {
            _changed = false;
            trace("onTabIndex:Click_end");
            return;
         }
         if(this._tapPageRenderer.selectedIndex != -1 && this._tapPageRenderer.getButtonAt(this._tapPageRenderer.selectedIndex) != param1.target as Button)
         {
            trace("onTabIndex:Click_end");
            return;
         }
         trace("onTabIndexClick",this._tapPageRenderer.selectedIndex);
         trace("setTabRendererToVisible::",!_loc2_.visible,"cl");
         if(_loc2_)
         {
            _loc2_.visible = !_loc2_.visible;
            if(!_loc2_.visible)
            {
               stage.focus = null;
               this.unfocusTab();
            }
            else
            {
               parent.addChild(_loc2_);
               _loc3_ = stage.focus != null;
               if(!_loc3_)
               {
                  trace("ff:textField");
                  stage.focus = _loc2_.text_inpput.textField;
               }
               _loc2_.text_inpput.text = "";
            }
            this.clearIngnore(null,true);
         }
         trace("onTabIndex:Click_end");
      }
      
      protected function getTabIndex(param1:int) : int
      {
         var _loc2_:int = (_data.tabs[_activeMode].length);
         var _loc3_:uint = 0;
         while(_loc3_ < _loc2_)
         {
            if(_data.tabs[_activeMode][_loc3_].id == param1)
            {
               return _loc3_;
            }
            _loc3_++;
         }
         return -1;
      }
      
      protected function onTabIndexChange(param1:IndexEvent) : void
      {
         var _loc2_:int = param1.index;
         var _loc3_:Number;
         var _loc4_:LobbyChatTab;
         var _loc5_:Boolean;
         if(param1.data)
         {
            trace("eventHasData",param1.data.id);
            _loc2_ = this.getTabIndex(param1.data.id);
         }
         trace("onTabIndex:Change",_loc2_,"event_index",param1.index,"==",this._last_index,stage.focus,"ignore",this._ignore_changed,"_wasSelected",this._wasSelected,"_changed",this._changed);
         if(param1.index == -1 || this._wasSelected && this._ignore_changed && param1.index == this._last_index)
         {
            _wasSelected = true;
            this._tapPageRenderer.addEventListener(Event.ENTER_FRAME,this.clearIngnore);
            return;
         }
         this._tapPageRenderer.removeEventListener(Event.ENTER_FRAME,this.clearIngnore);
         if(!this._ignore_changed || _loc2_ != this._last_index)
         {
            trace("_changed = true","a");
            _changed = true;
            this._tapPageRenderer.addEventListener(Event.EXIT_FRAME,this.clearChanged);
         }
         else
         {
            _ignore_changed = false;
         }
         _activeTab = _loc2_;
         _selectedIndex = param1.index;
         if(_data.tabs[_activeMode] && _data.tabs[_activeMode][_loc2_])
         {
            _loc3_ = (_data.tabs[_activeMode][_loc2_].id);
         }
         else
         {
            return;
         }
         _loc4_ = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         trace("setTabRendererToVisible::",true,stage.focus);
         if(_loc4_)
         {
            if(!_loc4_.visible)
            {
               _loc4_.scrollToBottom();
            }
            _loc4_.visible = true;
            parent.addChild(_loc4_);
            if(stage.focus && (!_loc4_.contains(stage.focus)) || !stage.focus)
            {
               _loc5_ = stage.focus != null;
               if(!_loc5_)
               {
                  stage.focus = _loc4_.text_inpput.textField;
               }
               else
               {
                  _loc4_.focusTextInput();
               }
               this.clearIngnore(null,true);
            }
         }
         _last_index = _loc2_;

      }
      
      protected function handleFocusOut(param1:FocusEvent) : void
      {
         trace("handleFocusOut:",param1.currentTarget.name,"event:props",param1.relatedObject,param1.target,param1.currentTarget);
         var _loc2_:LobbyChatTab = param1.currentTarget as LobbyChatTab;
         if(!hasEventListener(Event.RENDER))
         {
            trace("addEventListener::",stage.focus);
            addEventListener(Event.RENDER,this.onRenderFocus);
         }
         trace("handleFocusOut:_end");
      }
      
      protected function onRenderFocus(param1:Event = null) : void
      {
         trace("onRenderFocus",param1.target.name,stage.focus,this._changed,param1.currentTarget,_activeTab);
         removeEventListener(param1.type,this.onRenderFocus);
         if(_activeTab == -1)
         {
            return;
         }
         var _loc2_:LobbyChatTab = this._tabs[_data.tabs[_activeMode][_activeTab].id];
         if(this._changed)
         {
            return;
         }
         if(!stage.focus || !_loc2_.contains(stage.focus) && _loc2_ != stage.focus)
         {
            if(!stage.focus)
            {
               this.onClearSelectionFocus();
            }
            else
            {
               _loc2_.addEventListener(Event.RENDER,this.onClearSelectionFocus,false,0,true);
            }
         }
         trace("onRenderFocus_end");
      }
      
      protected function onClearSelectionFocus(param1:Event = null) : void
      {
         var _loc2_:LobbyChatTab;
         trace("onClearSelectionFocus",param1);
         if(param1)
         {
            _loc2_ = param1.currentTarget as LobbyChatTab;
            _loc2_.removeEventListener(param1.type,this.onClearSelectionFocus);
         }
         _ignore_changed = true;
         this.unfocusTab();
         trace("onClearSelectionFocus_end");
      }
      
      protected function clearIngnore(param1:Event = null, param2:Boolean = false) : void
      {
         trace("clearIngnore::",this._callCount);
         _callCount += 1;
         if(this._callCount == 12 || param2)
         {
            if(param1)
            {
               this._tapPageRenderer.removeEventListener(param1.type,this.clearIngnore);
            }
            _wasSelected = false;
            _callCount = 0;
            if(!param2)
            {
               _changed = false;
               trace("_changed = false","clearIngnore");
            }
            _last_index = -1;
         }
         trace("clearIngnore::_end");
      }
      
      protected function clearChanged(param1:Event = null) : void
      {
         this._tapPageRenderer.removeEventListener(param1.type,this.clearChanged);
         trace("_changed = false","clearChanged");
         _changed = false;
      }
      
      override protected function get_styled_text(param1:Object) : String
      {
         var _loc2_:String = String(param1.color != undefined ? param1.color : getColor(param1.type));
         var _loc3_:String;
         var _loc4_:String;
         var _loc5_:String;
         var _loc6_:Boolean;
         var _loc7_:Boolean;
         trace("get_styled_text",_loc2_,param1.text,param1.color,param1.type,_data.name,param1.name);
         if(!param1.name)
         {
            return styled(param1.time,TIME) + " " + styled(param1.text,_loc2_);
         }
         _loc3_ = param1.abbr ? "[" + param1.abbr + "] " : "";
         if(param1.type == 2)
         {
            return styled(param1.time,TIME) + " " + styled(_loc3_ + linked(param1.name + ": ","messager"),_loc2_) + " " + styledChar(param1.text,_loc2_);
         }
         if(_data && _data.name == param1.name)
         {
            trace("ISLOCAL");
            return styled(param1.time,TIME) + " " + styled(_loc3_ + linked(param1.name + ": ","name"),PLAYER) + " " + styledChar(param1.text,PLAYER_TEXT);
         }
         _loc4_ = param1.name;
         _loc5_ = _loc2_;
         _loc6_ = _loc3_.indexOf("[admin]") == 0;
         if(_loc6_)
         {
            _loc2_ = "GM";
            _loc5_ = "GMText";
         }
         else
         {
            _loc7_ = _loc3_.indexOf("[VG]") == 0;
            if(_loc7_)
            {
               _loc2_ = "VG";
               _loc5_ = "VGText";
            }
         }
         if(_loc2_ == "White")
         {
            _loc2_ = "WhiteNickname";
         }
         return styled(param1.time,TIME) + " " + styled(_loc3_ + linked(param1.name + ": ","name"),_loc2_) + " " + styledChar(param1.text,_loc5_);
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
      
      override protected function findAppropriateTab(param1:Object, param2:String) : void
      {
         var _loc3_:uint = 0;
         var _loc4_:uint;
         while(_loc3_ < _data.tabs[_activeMode].length)
         {
            if(param1.tab_id != undefined)
            {
               if(_data.tabs[_activeMode][_loc3_].id == param1.tab_id)
               {
                  this._tabs[_data.tabs[_activeMode][_loc3_].id].addMessage(param2);
                  break;
               }
            }
            else if(_data.tabs[_activeMode][_loc3_])
            {
               _loc4_ = 0;
               while(_loc4_ < _data.tabs[_activeMode][_loc3_].channels.length)
               {
                  if(_data.tabs[_activeMode][_loc3_].channels[_loc4_] == param1.type)
                  {
                     this._tabs[_data.tabs[_activeMode][_loc3_].id].addMessage(param2);
                     break;
                  }
                  _loc4_++;
               }
            }
            _loc3_++;
         }
      }
      
      protected function onStageResize(param1:Event) : void
      {
         var _loc2_:int = Math.ceil(this._def_font_size / MainScene.scaleFactor);
         var _loc3_:int = Math.ceil(this._def_font_size_big / MainScene.scaleFactor);
         var _loc4_:uint = 0;
         while(_loc4_ < _style.styleNames.length)
         {
            _loc7_ = _style.getStyle(_style.styleNames[_loc4_]);
            _loc7_.fontSize = _loc2_;
            _style.setStyle(_style.styleNames[_loc4_],_loc7_);
            _loc4_++;
         }
         var _loc5_:Number = (_loc2_ + 2) * this._fontScale;
         var _loc6_:LobbyChatTab;
         var _loc7_:Object;
         var _loc8_:TextFormat;
         var _loc9_:int;
         var _loc10_:int;
         for each(_loc6_ in this._tabs)
         {
            _loc9_ = int((this._min_tab_height - 82) / _loc5_) * _loc5_ + 82;
            _loc10_ = int((this._max_tab_width - 82) / _loc5_) * _loc5_ + 82;
            _loc6_.setSizeLimits(this._min_tab_width,_loc9_,this._max_tab_width,_loc10_);
            trace("new_size",_loc6_.height,MainScene.scaleFactor,_loc2_);
            _loc6_.height = int((_loc6_.height - 82) / _loc5_) * _loc5_ + 82;
            _loc6_.x = MainScene.stageWidth - _loc6_.width - this._start_tab_shiftX;
            _loc6_.y = MainScene.stageHeight - _loc6_.height - this._start_tab_shiftY;
            _loc6_.prepareToResize();
            _loc6_.text_chat.textField.text = "";
            _loc6_.text_chat.invalidate();
            _loc6_.text_inpput.textSize = _loc2_;
            _loc8_ = _loc6_.labelTextField.textField.getTextFormat();
            _loc8_.size = _loc3_;
            _loc6_.labelTextField.textField.defaultTextFormat = _loc8_;
            _loc6_.labelTextField.textField.setTextFormat(_loc8_);
         }
      }
      
      protected function updateTabs() : void
      {
         if(_activeMode == -1)
         {
            return;
         }
         var _loc1_:Array = _data.tabs[_activeMode];
         if(!_loc1_)
         {
            return;
         }
         var _loc2_:uint = _loc1_.length;
         var _loc3_:LobbyChatTab;
         var _loc4_:uint = 0;
         var _loc5_:uint = 0;
         while(_loc5_ < _loc2_)
         {
            _loc3_ = this._tabs[_loc1_[_loc5_].id];
            if(_loc3_ && !_loc3_.handled)
            {
               _loc4_++;
            }
            _loc5_++;
         }
         trace("handleTabs::",_loc4_);
         var _loc6_:uint = 30;
         var _loc7_:Class = getDefinitionByName("SrvChatTab") as Class;
         var _loc8_:uint = 0;
         var _loc9_:Object;
         var _loc10_:LobbyChatTab;
         var _loc11_:int;
         var _loc12_:int;
         var _loc13_:Number;
         var _loc14_:int;
         var _loc15_:int;
         var _loc16_:TextFormat;
         while(_loc8_ < _loc2_)
         {
            _loc9_ = _loc1_[_loc8_];
            _loc3_ = this._tabs[_loc9_.id];
            if(_loc9_ && !_loc3_)
            {
               _loc10_ = new _loc7_();
               _loc11_ = Math.ceil(this._def_font_size / MainScene.scaleFactor);
               _loc12_ = Math.ceil(this._def_font_size_big / MainScene.scaleFactor);
               _loc13_ = (_loc11_ + 2) * this._fontScale;
               _loc10_.width = this._def_tab_width;
               _loc10_.height = int((this._def_tab_height - 82) / _loc13_) * _loc13_ + 82;
               _loc14_ = int((this._min_tab_height - 82) / _loc13_) * _loc13_ + 82;
               _loc15_ = int((this._max_tab_width - 82) / _loc13_) * _loc13_ + 82;
               _loc10_.setSizeLimits(this._min_tab_width,_loc14_,this._max_tab_width,_loc15_);
               _loc10_.x = MainScene.stageWidth - _loc10_.width - 40 - _loc4_ * _loc6_;
               _loc10_.y = MainScene.stageHeight - _loc10_.height - 70 - _loc4_ * _loc6_;
               if(_loc4_)
               {
                  _loc4_--;
               }
               _loc10_.text_chat.textField.addEventListener(MouseEvent.CLICK,this.onTxClick);
               _loc10_.text_chat.textField.addEventListener(TextEvent.LINK,this.clickLinkHandler);
               _loc10_.text_chat.textField.addEventListener(MouseEvent.DOUBLE_CLICK,this.clickLinkHandlerDouble);
               _loc10_.text_chat.textField.text = "";
               _loc10_.text_chat.invalidate();
               _loc10_.text_inpput.textSize = _loc11_;
               _loc16_ = _loc10_.labelTextField.textField.getTextFormat();
               _loc16_.size = _loc12_;
               _loc10_.labelTextField.textField.defaultTextFormat = _loc16_;
               _loc10_.labelTextField.textField.setTextFormat(_loc16_);
               _loc10_.addEventListener(Event.CLOSE,this.hideTab);
               _loc10_.addEventListener(Event.CHANNEL_MESSAGE,this.blinkTab);
               _loc10_.addEventListener(FocusEvent.FOCUS_OUT,this.handleFocusOut);
               TextFieldEx.setVerticalAlign(_loc10_.text_inpput.textField,TextFieldEx.VALIGN_CENTER);
               TextFieldEx.setVerticalAlign(_loc10_.labelTextField.textField,TextFieldEx.VALIGN_CENTER);
               _loc10_.owner = this;
               _loc10_.setData(_loc9_);
               _loc10_.name = "tab_" + _loc9_.name;
               _loc10_.send_function = send_function;
               _loc10_.setStyleSheet(_style);
               _loc10_.visible = false;
               this._tabs[_loc9_.id] = _loc10_;
               this.checkStatusIcon(_loc10_);
               trace("LC::addTabToParent",parent);
               parent.addChild(this._tabs[_loc9_.id]);
            }
            _loc8_++;
         }
      }
      
      protected function blinkTab(param1:Event) : void
      {
         var _loc2_:Object = (param1.target as LobbyChatTab).getData();
         var _loc3_:int;
         trace("blinkTab::",_loc2_ ? _loc2_.id : "no_id");
         if(_loc2_)
         {
            _loc3_ = 0;
            while(_loc3_ < this._tapPageRenderer.dataProvider.length)
            {
               trace("IterateBottomData::",this._tapPageRenderer.dataProvider[_loc3_].id,_loc2_.id);
               if(this._tapPageRenderer.dataProvider[_loc3_] && this._tapPageRenderer.dataProvider[_loc3_].id == _loc2_.id)
               {
                  this._tapPageRenderer.dataProvider[_loc3_].blink = _loc2_.blink;
                  break;
               }
               _loc3_++;
            }
         }
         this._tapPageRenderer.dataProvider.invalidate();
      }
      
      protected function hideTab(param1:Event) : void
      {
         trace("hideTab::",param1.target);
         if(stage.focus)
         {
            stage.focus = null;
         }
         param1.target.visible = false;
         this.unfocusTab();
      }
      
      protected function onClose(param1:Event = null) : void
      {
         ExternalInterface.call("hide_chat_button");
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function onTxClick(param1:MouseEventEx) : void
      {
         _lastIdx = param1.buttonIdx;
      }
      
      protected function clickLinkHandlerDouble(param1:MouseEvent) : void
      {
         trace("clickLinkHandlerDouble");
      }
      
      protected function clickLinkHandler(param1:TextEvent) : void
      {
         trace("clickLinkHandler",param1.text);
         var _loc2_:LobbyChatTab = param1.target.parent.parent as LobbyChatTab;
         if(_activeTab == -1 || _data.tabs[_activeMode][_activeTab].id != _loc2_.getData().id)
         {
            this.focusTab(_loc2_.getData().id);
            _loc4_ = _loc2_.text_chat.textField.getLineIndexAtPoint(_loc2_.text_chat.textField.mouseX,_loc2_.text_chat.textField.mouseY);
            if(_loc4_ != -1)
            {
               _loc5_ = _loc2_.text_chat.textField.getLineText(_loc4_);
               trace("clickLinkHandler::text",_loc5_);
            }
         }
         var _loc3_:Array = param1.text.split(":");
         var _loc4_:int;
         var _loc5_:String;
         var _loc6_:Boolean;
         var _loc7_:Array;
         var _loc8_:Array;
         var _loc9_:int;
         if(_loc3_[0] == "name" && _data.name != _loc3_[1])
         {
            _lastName = _loc3_[1];
            _loc6_ = false;
            if(this._friends)
            {
               _loc9_ = 0;
               while(_loc9_ < this._friends.length)
               {
                  if(this._friends[_loc9_].name == this._lastName)
                  {
                     _loc6_ = true;
                     break;
                  }
                  _loc9_++;
               }
            }
            _loc7_ = [];
            if(this._lastIdx == 1 && this._lastName)
            {
               _loc8_ = [];
               if(!_loc6_)
               {
                  _loc8_.push({
                     name:LocalizationData.getName("add_to_friends"),
                     click_function:this.addToFriend
                  });
               }
               _loc8_.push({
                  name:LocalizationData.getName("add_to_ignore"),
                  click_function:this.addToIgnore
               });
               if(_activeTab != -1 && _data.tabs[_activeMode][_activeTab].channels.length != 0)
               {
                  _loc8_.push({
                     name:LocalizationData.getName("send_message"),
                     click_function:this.privateMessage
                  });
               }
               if(PlayerInfo.commander && !PlayerInfo.isSquadFull && !SquadList.isInSquad(this._lastName))
               {
                  _loc8_.push({
                     name:LocalizationData.getName("invite_to_squad"),
                     click_function:this.addToSquad
                  });
               }
            }
            else if(this._lastIdx == 0 && this._lastName && _activeTab != -1)
            {
               if(_loc2_)
               {
                  _loc2_.srartMessage(this._lastName + ", ");
               }
            }
            if(_loc8_ && _loc8_.length)
            {
               _loc7_.push({
                  name:LocalizationData.getName("bot_or_cheater_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:1
               });
               _loc7_.push({
                  name:LocalizationData.getName("swearing_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:2
               });
               _loc7_.push({
                  name:LocalizationData.getName("non_sport_behavior_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:3
               });
               _loc7_.push({
                  name:LocalizationData.getName("account_trading_claim"),
                  nickname:this._lastName,
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:4
               });
               _loc8_.push({
                  name:LocalizationData.getName("claim_player"),
                  options:_loc7_
               });
               SrvContextMenu.show(_loc8_);
            }
         }
      }
      
      protected function flagPlayer(param1:Object) : void
      {
         trace("comment_about_user",param1.nickname,param1.index,param1.subIndex);
         ExternalInterface.call("comment_about_user",param1.nickname,param1.index,param1.subIndex);
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
   }
}

