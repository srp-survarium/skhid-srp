package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.EventDispatcher;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import flash.utils.Timer;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.controls.ScrollingListSmoothDynam;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataRenderer;
   import scaleform.gfx.MouseEventEx;
   import survarium.Util;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.LocalizationData;
   import survarium.constants.Faction;
   import survarium.constants.QuestStatus;
   import survarium.constants.Reward;
   
   public class QuestWindow extends UIWindow
   {
      
      protected static var _initTime:int = 0;
      
      protected static var _curTime:int = 0;
      
      protected static var _minuteTimer:Timer = new Timer(1000);
      
      protected static const _timerFactor:int = 1000 * 60;
      
      public static const GOTO_SHOP:String = "goto_shop";
      
      public var label:TextField;
      
      public var label_reputation:TextField;
      
      public var questsContent:MovieClip;
      
      public var background:Sprite;
      
      public var gradientSprite:Sprite;
      
      public var dummy:MovieClip;
      
      public var buttonsBack:MovieClip;
      
      public var btnAccept:Button;
      
      public var btnShop:Button;
      
      public var btnJoin:Button;
      
      public var btnInfo:Button;
      
      public var all_factions:Button;
      
      public var factionDescription:MovieClip;
      
      public var noFactionSelected:MovieClip;
      
      protected var _inventory:InventoryList;
      
      protected var _quests:Array = [];
      
      protected var _proposalQuests:Array = [];
      
      protected var _reputationsList:ButtonBar;
      
      protected var _questsList:ScrollingListSmoothDynam;
      
      protected var _factionUpdated:int = 0;
      
      public var factionBackground:MovieClip;
      
      protected var _activeFactionData:Object = null;
      
      protected var _activeFaction:uint = 0;
      
      protected var _selectedFaction:ReputationRenderer = null;
      
      protected var _hideDescriptionOnQuests:Boolean = false;
      
      protected var _lastSelected:int = -1;
      
      protected var _lastSelectedEnabled:Boolean = false;
      
      protected var _external_renderer:IDataRenderer;
      
      protected var _factions:Array;
      
      public function QuestWindow()
      {
         super();
         trace("QuestWindowConstruct::",this.questsContent.selectedTabGlow,this.questsContent.activeTabGlow);
         _initTime = getTimer() / _timerFactor;
         _curTime = 0;
         _minuteTimer.delay = _timerFactor;
         _minuteTimer.start();
         _minuteTimer.addEventListener(TimerEvent.TIMER,this.timerStep,false,0,true);
         this.label.mouseEnabled = false;
         LocalizationData.addTextField(this.label,"quests_label");
         this.questsContent.selectedTabGlow.visible = false;
         this.questsContent.activeTabGlow.visible = false;
         this.questsContent.selectedTabGlow.mouseEnabled = false;
         this.questsContent.selectedTabGlow.width = 215;
         this.questsContent.activeTabGlow.mouseEnabled = false;
         _lastSelected = 0;
         trace("btns::",this.btnShop,this.btnJoin,this.btnInfo);
         if(this.btnShop && this.btnJoin)
         {
            this.btnShop.addEventListener(ButtonEvent.PRESS,this.gotoShop);
            this.btnJoin.addEventListener(ButtonEvent.PRESS,this.joinFactionSelected);
            this.btnInfo.addEventListener(ButtonEvent.PRESS,this.showFactionInfoByButton2);
            this.btnShop.x += 70;
            this.btnJoin.x += 70;
            this.btnInfo.x = this.btnJoin.x;
            LocalizationData.addButton(this.btnShop,"shop_label");
            LocalizationData.addButton(this.btnJoin,"faction_join");
            this.btnInfo.label = LocalizationData.getName("faction_info");
         }
         this.noFactionSelected.visible = true;
         LocalizationData.addCallback(this.onLocal,"quests_label");
         LocalizationData.addTextField(this.noFactionSelected.textField,"choose_faction_description");
         trace("btns::",this.noFactionSelected.button1,this.noFactionSelected.button2,this.questsContent.scrollList);
         this.noFactionSelected.addEventListener(ButtonEvent.CLICK,this.showFactionInfoByButton);
         this.noFactionSelected.button1.addEventListener(MouseEvent.ROLL_OVER,this.showFactionInfoHover);
         this.noFactionSelected.button2.addEventListener(MouseEvent.ROLL_OVER,this.showFactionInfoHover);
         this.noFactionSelected.button1.addEventListener(MouseEvent.ROLL_OUT,this.showFactionInfoHover);
         this.noFactionSelected.button2.addEventListener(MouseEvent.ROLL_OUT,this.showFactionInfoHover);
         _questsList = this.questsContent.scrollList;
         this._questsList.rowStep = 5;
         _reputationsList = this.questsContent.scrollListReputations;
         this._reputationsList.x += 3;
         this._reputationsList.addEventListener(IndexEvent.INDEX_CHANGE,this.onReputationIndexChange);
         this._reputationsList.addEventListener(ButtonEvent.CLICK,this.onReputationIndexChange2);
         this.questsContent.addEventListener(MouseEvent.CLICK,this.onReputationItemClick);
         this.factionDescription.visible = false;
         var _textField:TextField = this.factionDescription.textField;
         var _textFieldLabel:TextField = this.factionDescription.factionLabel;
         var _back:MovieClip = this.factionDescription.factionDescriptionBack;
         var _backLabel:MovieClip = this.factionDescription.factionLabelBack;
         _textField.autoSize = TextFieldAutoSize.LEFT;
         _textField.mouseEnabled = _back.mouseEnabled = _backLabel.mouseEnabled = _textFieldLabel.mouseEnabled = false;
      }
      
      public static function getTimerInstance() : Timer
      {
         return _minuteTimer;
      }
      
      public static function getTime() : int
      {
         return _curTime;
      }
      
      public function getFactionProgress(value:int) : Object
      {
         var l:uint;
         var i:uint;
         var progressData:Object;
         if(this._factions)
         {
            l = this._factions.length;
            for(i = 0; i < l; i++)
            {
               progressData = this._factions[i];
               if(progressData)
               {
                  if(progressData.faction_id == value)
                  {
                     return progressData;
                  }
               }
            }
         }
         return {};
      }
      
      public function setControlsEnabled(value:Boolean = true) : void
      {
         this.btnShop.enabled = value;
         btnClose.enabled = value;
      }
      
      override public function get height() : Number
      {
         return 902;
      }
      
      override public function get width() : Number
      {
         return 580;
      }
      
      private function showFactionInfoHover(e:MouseEvent) : void
      {
         var isHover:Boolean = e.type == MouseEvent.ROLL_OVER;
         var target:MovieClip = e.currentTarget as MovieClip;
         var factionIndex:int = target.name == "button1" ? Faction.SCAVENGERS : Faction.BLACK_MARKET;
         trace("factionIndex::",target.name,e.currentTarget);
         var frame:int = isHover ? (factionIndex + 4) : factionIndex;
         var glowTarget:MovieClip = factionIndex == Faction.SCAVENGERS ? this.noFactionSelected.factionLabelBack : this.noFactionSelected.factionLabelBack2;
         trace("showFactionInfoHover::",glowTarget,target,factionIndex,frame);
      }
      
      private function onLocal(value:String = "") : void
      {
         var faction1Label:String = "«" + GangShop.getSellerName(Faction.SCAVENGERS) + "»";
         var faction1Label2:String = "«" + GangShop.getSellerName(Faction.BLACK_MARKET) + "»";
         this.noFactionSelected.factionLabel.text = faction1Label;
         this.noFactionSelected.factionLabel2.text = faction1Label2;
         this.noFactionSelected.factionLabelBack2.gotoAndStop(2);
      }
      
      private function showFactionInfoByButton2(e:ButtonEvent) : void
      {
         if(this.factionDescription.visible)
         {
            this.hideFactionInfo();
         }
         else
         {
            this.showFactionInfo();
         }
      }
      
      private function showFactionInfoByButton(e:ButtonEvent) : void
      {
         this._reputationsList.selectedIndex = e.target.name == "button1" ? 0 : 1;
         this._reputationsList.validateNow();
         this.showFactionInfo();
      }
      
      public function getSelectedFaction() : int
      {
         return this._lastSelected;
      }
      
      private function gotoShop(e:ButtonEvent) : void
      {
         trace("gotoShopWithSeller::",this._lastSelected);
         dispatchEvent(new Event(GOTO_SHOP,true));
      }
      
      override public function get visible() : Boolean
      {
         return super.visible;
      }
      
      override public function set visible(value:Boolean) : void
      {
         trace("QW::visible",this.noFactionSelected,this.buttonsBack,this.factionDescription);
         if(!this._activeFaction)
         {
            this.noFactionSelected.visible = true;
            this.buttonsBack.visible = false;
            this.btnInfo.visible = this.btnJoin.visible = this.btnShop.visible = false;
            this.factionDescription.visible = false;
         }
         else
         {
            this.noFactionSelected.visible = false;
            this.buttonsBack.visible = true;
            this.updateButtons();
         }
         super.visible = value;
      }
      
      public function updateButtons() : void
      {
         this.btnShop.visible = true;
         trace("btnJoin.enabled",this._lastSelectedEnabled);
         var btnJoinEnabled:Boolean = this._lastSelected + 1 != this._activeFaction || !this._activeFaction;
         this.btnJoin.enabled = this._lastSelectedEnabled && btnJoinEnabled;
         this.btnJoin.visible = btnJoinEnabled;
         this.btnInfo.visible = !btnJoinEnabled;
         this.getInfoLabel();
      }
      
      protected function getInfoLabel() : void
      {
         if(!this.factionDescription.visible)
         {
            this.btnInfo.label = LocalizationData.getName("faction_info");
         }
         else
         {
            this.btnInfo.label = LocalizationData.getName("quests");
         }
      }
      
      protected function updateFactionInfo(factionId:uint) : void
      {
         trace("updateFactionInfo::",factionId);
         _factionUpdated = factionId;
         var _textField:TextField = this.factionDescription.textField;
         var _textFieldLabel:TextField = this.factionDescription.factionLabel;
         var _back:MovieClip = this.factionDescription.factionDescriptionBack;
         var _backLabel:MovieClip = this.factionDescription.factionLabelBack;
         var textFormat:TextFormat = _textField.getTextFormat();
         textFormat.align = TextFormatAlign.CENTER;
         textFormat.leading = -1;
         _textField.defaultTextFormat = textFormat;
         _back.gotoAndStop(factionId);
         _backLabel.gotoAndStop(factionId);
         _textFieldLabel.text = "«" + GangShop.getSellerName(factionId) + "»";
         _textField.text = LocalizationData.getName("faction_" + factionId.toString() + "_description");
         _back.y = 759 - _back.height;
      }
      
      public function setActiveFaction(value:uint) : void
      {
         var l:uint;
         var i:uint;
         var progressData:Object;
         if(value != 0)
         {
            this.noFactionSelected.visible = false;
         }
         _activeFaction = value;
         if(this.factionDescription.visible)
         {
         }
         this.btnJoin.enabled = this._lastSelectedEnabled && (this._lastSelected + 1 != this._activeFaction || !this._activeFaction);
         trace("setActiveFaction::",this._activeFaction,this._reputationsList.selectedIndex);
         if(this._factions)
         {
            l = this._factions.length;
            for(i = 0; i < l; i++)
            {
               progressData = this._factions[i];
               if(progressData)
               {
                  if(progressData.faction_id == value)
                  {
                     progressData.selected = true;
                     _activeFactionData = progressData;
                  }
                  else
                  {
                     progressData.selected = false;
                  }
               }
            }
         }
         if(value != 0)
         {
            this._reputationsList.selectedIndex = value - 1;
            this.questsContent.activeTabGlow.visible = false;
            this.questsContent.activeTabGlow.x = this._reputationsList.selectedIndex * this._reputationsList.buttonWidth - 100;
         }
         else
         {
            this.questsContent.activeTabGlow.visible = false;
         }
         this._reputationsList.dataProvider.invalidate();
         this.updateActiveFactionData();
      }
      
      private function updateActiveFactionData() : void
      {
         var statusPanel:MovieClip = parent["statusPanel"];
         if(statusPanel)
         {
            statusPanel.setFactionProgress(this._activeFactionData);
         }
      }
      
      private function onReputationItemClick(e:MouseEventEx) : void
      {
         return;
         var target:Object = e.target;
         var isSelectable:Boolean;
         var isJoinable:Boolean;
         var quests:Array;
      }
      
      protected function deselect(e:Event) : void
      {
         SrvContextMenu.getInstance().removeEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
      }
      
      protected function showFactionInfo2() : void
      {
         if(this._selectedFaction)
         {
            this._reputationsList.selectedIndex = this._selectedFaction.data.faction_id - 1;
            this._reputationsList.validateNow();
            this.showFactionInfo();
         }
      }
      
      protected function showFactionInfo() : void
      {
         trace("showFactionInfo::",this._lastSelected);
         this.updateFactionInfo(this._lastSelected + 1);
         this.factionDescription.visible = true;
         this.btnJoin.enabled = this._lastSelectedEnabled && (this._lastSelected + 1 != this._activeFaction || !this._activeFaction);
         this._questsList.visible = false;
         this.noFactionSelected.visible = false;
         this.getInfoLabel();
      }
      
      protected function hideFactionInfo() : void
      {
         this.factionDescription.visible = false;
         this._questsList.visible = true;
         if(!this._activeFaction)
         {
            this.noFactionSelected.visible = true;
            this.buttonsBack.visible = false;
            this.btnInfo.visible = this.btnJoin.visible = this.btnShop.visible = false;
         }
         this.getInfoLabel();
      }
      
      protected function joinFactionSelected(e:Event = null) : void
      {
         var repRenderer:Object;
         trace("joinFaction::",this._lastSelected);
         if(this._lastSelected != -1)
         {
            repRenderer = this._reputationsList.getButtonAt(this._lastSelected) as MovieClip;
            if(repRenderer && repRenderer.data)
            {
               _hideDescriptionOnQuests = true;
               ExternalInterface.call("change_quest_faction",uint(repRenderer.data.faction_id));
            }
         }
      }
      
      protected function joinFaction(e:Event = null) : void
      {
         trace("joinFaction::",this._selectedFaction);
         if(this._selectedFaction)
         {
            ExternalInterface.call("change_quest_faction",uint(this._selectedFaction.data.faction_id));
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this._questsList.addEventListener(Event.CONNECT,this.showProposalQuests);
      }
      
      private function showProposalQuests(e:Event) : void
      {
         var itemRednerer:ListItemRenderer = e.target as ListItemRenderer;
         if(!itemRednerer)
         {
            return;
         }
         var itemData:Object = itemRednerer.data;
         if(!itemData)
         {
            return;
         }
         var validProposals:Array = new Array();
         for(var i:int = 0; i < this._proposalQuests.length; i++)
         {
            if(itemData.faction_id == this._proposalQuests[i].faction_id)
            {
               validProposals.push(this._proposalQuests[i]);
            }
         }
         if(validProposals.length == 0)
         {
            return;
         }
         var messageData:Object = {
            proposals:validProposals,
            callerId:itemData.id
         };
         Message.showMessage("quest_selector",messageData,true);
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         var newY:int = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
         super.y = newY;
         if(this._external_renderer)
         {
            (this._external_renderer as DisplayObject).y = newY + 48;
         }
         parent["squadList"].y = newY + 48;
      }
      
      override public function closeWindow(e:Event = null) : void
      {
         trace("STOP_QUEST");
         super.closeWindow();
         if(!this._activeFaction)
         {
            this._reputationsList.selectedIndex = -1;
         }
      }
      
      public function onActivate() : void
      {
         trace("START_QUEST");
         _minuteTimer.start();
         this.timerStep();
         _minuteTimer.dispatchEvent(new TimerEvent(TimerEvent.TIMER));
      }
      
      protected function timerStep(e:Event = null) : void
      {
         _curTime = getTimer() / _timerFactor - _initTime;
      }
      
      protected function resetFilter(e:ButtonEvent) : void
      {
      }
      
      protected function checkReskin(e:Event = null) : void
      {
      }
      
      protected function onReputationIndexChange2(e:Event) : void
      {
         trace("onReputationIndexChange2",e.target,e.currentTarget);
         if(this._activeFaction == e.target.data.faction_id)
         {
            this.hideFactionInfo();
         }
      }
      
      protected function onReputationIndexChange(e:IndexEvent) : void
      {
         var repRenderer:*;
         if(this._lastSelected != -1)
         {
            repRenderer = this._reputationsList.getButtonAt(this._lastSelected) as MovieClip;
         }
         if(this._reputationsList.selectedIndex != -1)
         {
            this.questsContent.selectedTabGlow.visible = true;
            this.questsContent.selectedTabGlow.gotoAndStop(1);
            this.questsContent.selectedTabGlow.x = this._reputationsList.buttonWidth * this._reputationsList.selectedIndex - 34;
         }
         else
         {
            this.questsContent.selectedTabGlow.visible = false;
            this.questsContent.selectedTabGlow.x = 0;
         }
         if(e.index != -1)
         {
            _lastSelected = e.index;
            _lastSelectedEnabled = e.data ? e.data.faction_enabled : true;
         }
         trace("onReputationIndexChange",this._lastSelected,this._lastSelectedEnabled,repRenderer,e.data,"_activeFaction",this._activeFaction);
         if(e.data && (e.data.faction_id != this._activeFaction || this._activeFaction == Faction.ANY && !e.target.enabled))
         {
            this.showFactionInfo();
         }
         else
         {
            this.hideFactionInfo();
         }
         trace("btnJoin.enabled",this._lastSelectedEnabled);
         this.btnJoin.enabled = this._lastSelectedEnabled && (this._lastSelected + 1 != this._activeFaction || !this._activeFaction);
         trace("quest_faction_selected",e.index,this.btnJoin.enabled);
         ExternalInterface.call("quest_faction_selected");
         this.btnInfo.enabled = true;
         this.buttonsBack.visible = true;
         this.updateButtons();
         this.filterQuests();
         if(this._questsList.scrollBar)
         {
            this._questsList.scrollBar.position = 0;
            this._questsList.scrollPosition = 0;
         }
         else
         {
            this._questsList.scrollPosition = 0;
         }
      }
      
      protected function filterQuests() : void
      {
         this._quests.sortOn("faction_id",Array.NUMERIC);
         var index:int = this._lastSelected + 1;
         var filterList:Array = [];
         for(var i:uint = 0; i < this._quests.length; i++)
         {
            if(this._quests[i].faction_id == this._activeFaction)
            {
               filterList.push(this._quests[i]);
            }
         }
         this._questsList.dataProvider = new DataProvider(filterList);
      }
      
      protected function filterExternal(e:ListEvent) : void
      {
         var list:ButtonBar;
         if(e.itemData)
         {
            list = this._reputationsList;
            list.selectedIndex = e.itemData.faction_id - 1;
            dispatchEvent(new Event(Event.CONNECT,true));
            this.checkReskin();
         }
      }
      
      public function register_inventory(value:InventoryList) : void
      {
         _inventory = value;
      }
      
      public function addQuest(value:Object) : void
      {
         var item_name:String = "";
         this.fillRewardItems(value);
         var icons:IconContainer = IconContainer.getInstance("factionItemIcons");
         var factionIcon:Icon = icons.getIcon(value.faction_id);
         value.faction_icon = factionIcon;
         value.start_time = getTime();
         trace("QW::addQuestRewards:",value.reward,"|",value.tasks,factionIcon,value.faction_id);
         trace("questAddedAtTimer::",_initTime - _curTime,"|",getTime());
         Util.logObject(value);
         if(value.proposal == false || value.proposal == undefined && value.status == QuestStatus.OFFERED)
         {
            this._quests.push(value);
         }
         else
         {
            this._proposalQuests.push(value);
         }
         if(this._hideDescriptionOnQuests)
         {
            _hideDescriptionOnQuests = false;
            if(this._lastSelected + 1 == this._activeFaction)
            {
               this.hideFactionInfo();
            }
         }
         if(this._external_renderer)
         {
            this._external_renderer.data = this._quests;
         }
         this.filterQuests();
         this.updateButtons();
      }
      
      public function clearQuests() : void
      {
         this._quests.length = 0;
         this._proposalQuests.length = 0;
         this._questsList.dataProvider = new DataProvider(this._quests);
         if(this._external_renderer)
         {
            this._external_renderer.data = this._quests;
         }
      }
      
      public function removeQuest(quest_id:uint) : void
      {
         for(var i:uint = 0; i < this._quests.length; i++)
         {
            if(this._quests[i].id == quest_id)
            {
               this._quests.splice(i,1);
               break;
            }
         }
         if(this._external_renderer)
         {
            this._external_renderer.data = this._quests;
         }
         this.filterQuests();
      }
      
      public function updateQuest(value:Object) : void
      {
         this.fillRewardItems(value);
         for(var i:uint = 0; i < this._quests.length; i++)
         {
            if(this._quests[i].id == value.id)
            {
               this._quests[i] = value;
               value.start_time = getTime();
               break;
            }
         }
         this.filterQuests();
      }
      
      public function register_external_renderer(value:IDataRenderer) : void
      {
         _external_renderer = value;
         (this._external_renderer as EventDispatcher).addEventListener(ListEvent.ITEM_CLICK,this.filterExternal,true);
      }
      
      public function setFactionProgress(value:Array) : void
      {
         trace("setFactionProgress::",value.length,value);
         if(value)
         {
            value.length = Faction.TRADING_FACTIONS;
         }
         _factions = value;
         this._reputationsList.dataProvider = new DataProvider(this._factions);
         this._reputationsList.validateNow();
         this.resetFilter(null);
         if(this._activeFaction != Faction.ANY)
         {
            this.setActiveFaction(this._activeFaction);
         }
      }
      
      public function getFactionButton(value:uint) : MovieClip
      {
         trace("getFactionButton",value,this._reputationsList.getButtonAt(value) as MovieClip);
         return this._reputationsList.getButtonAt(value) as MovieClip;
      }
      
      public function fillRewardItems(value:Object) : void
      {
         var i:uint;
         var item_descr:Object;
         if(value.reward)
         {
            for(i = 0; i < value.reward.length; i++)
            {
               if(value.reward[i].type == Reward.ITEM)
               {
                  item_descr = this._inventory.getItemData(value.reward[i].param);
                  value.reward[i].item_name = item_descr.name;
               }
            }
         }
      }
      
      protected function onTabIndexChange(e:IndexEvent) : void
      {
         this.questsContent.visible = e.index == 0;
      }
      
      protected function onAddQuestClick(e:Event) : void
      {
         var newQuest:Object = {
            text:this.dummy.textField.text,
            quest_label:this.dummy.labelTextField.text,
            experience:100,
            reward:this.dummy.rewardTextField.text,
            tasks:[{
               text:this.dummy.taskTextField.text,
               hint:this.dummy.hintTextField.text
            }],
            info:this.dummy.infoTextField.text,
            time_limit:this.dummy.timeTextField.text,
            status:QuestStatus.STATUS_1
         };
         this._quests.push(newQuest);
         this._questsList.dataProvider = new DataProvider(this._quests);
      }
   }
}

