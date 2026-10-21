package survarium.clan
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.controls.ScrollingListSmooth;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.controls.TextArea;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.ui.InputDetails;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.MouseEventEx;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.friends.FriendsMenu;
   
   public class ManageClanWindow extends UIComponent
   {
      
      public static const CLAN_OVERVIEW_STATE:uint = 3;
      
      public static const CLAN_MANAGE_STATE:uint = 4;
      
      public var textFieldClan:TextField;
      
      public var textDailyMessage:TextField;
      
      public var dailyMessage:TextArea;
      
      public var textFieldRank:TextField;
      
      public var textFieldLabel:TextField;
      
      public var textFieldLevel:TextField;
      
      public var textFieldName:TextField;
      
      public var textFieldStatus:TextField;
      
      public var textFieldClanLevel:TextField;
      
      public var textFieldClanLevelLabel:TextField;
      
      public var mainTabBar:ButtonBar;
      
      public var clanPlayersList:ScrollingListSmooth;
      
      public var btnRankUp:Button;
      
      public var btnRankDown:Button;
      
      public var btnKickMember:Button;
      
      public var btnInviteMember:Button;
      
      public var btnLeaveClan:Button;
      
      public var btnCancel:Button;
      
      public var btnChangeDailyMessage:Button;
      
      public var clanManage:MovieClip;
      
      public var progressIndicator:StatusIndicator;
      
      public var clanProgressHintArea:Button;
      
      public var clanSettings:MovieClip;
      
      private var _playersList:Array;
      
      private var _memberRestrictions:Object = "";
      
      private var _clanRanks:Object = "";
      
      private var _state:uint = 0;
      
      private var _dailyMessageText:String = "";
      
      private var _freeRankSlotsAvailable:Boolean = false;
      
      private var _ranks:Array = null;
      
      private var renderFrames:uint = 0;
      
      private var _selectedData:Object;
      
      private var _selectedRankId:int;
      
      public function ManageClanWindow()
      {
         super();
         this.setState(CLAN_OVERVIEW_STATE);
      }
      
      public function setClanProgress(minimum:uint, maximum:uint, current:uint, players:uint, level:uint) : void
      {
         this.progressIndicator.minimum = minimum;
         this.progressIndicator.maximum = maximum;
         this.progressIndicator.value = current;
         this.clanProgressHintArea.tooltipRendererName = "SrvProfileTipRenderer";
         this.clanProgressHintArea.tipData = {name:current + " / " + maximum};
         this.textFieldClanLevel.text = level.toString();
         var playersL:uint = (this.clanManage.clanPlayersList.dataProvider) ? (this.clanManage.clanPlayersList.dataProvider.length) : 0;
         this.clanManage.textFieldPlayers.text = LocalizationData.getName("clan_players_list_availabe_prefix") + String(playersL) + "/" + String(players) + " " + LocalizationData.getName("clan_players_list_availabe_postfix");
      }
      
      public function setState(value:uint) : void
      {
         _state = value;
         var isAdmin:Boolean = this._state == CLAN_MANAGE_STATE;
         this.checkRanksEnabled(this.clanSettings.ranksList.selectedIndex);
         this.clanManage.btnLeaveClan.label = isAdmin ? LocalizationData.getName("btn_disband_clan_label") : LocalizationData.getName("btn_leave_clan_label");
         if((this.mainTabBar.getButtonAt(1)))
         {
            this.mainTabBar.getButtonAt(1).enabled = isAdmin;
         }
      }
      
      private function checkRanksEnabled(index:int = 0) : void
      {
         var isAdmin:Boolean = this._state == CLAN_MANAGE_STATE;
         this.clanSettings.checkBoxInviteRight.enabled = this.clanSettings.checkBoxKickRight.enabled = this.clanSettings.checkBoxChangeRankRight.enabled = this.clanSettings.checkBoxDailyMessageRight.enabled = this.clanSettings.checkBoxBattleCommanderRight.enabled = isAdmin && index > 0;
      }
      
      public function setPlayerData(value:Array) : void
      {
         var lastId:int = -1;
         var data:DataProvider;
         var i:int;
         if(this.clanManage.clanPlayersList.selectedIndex != -1 && (this.clanManage.clanPlayersList.dataProvider))
         {
            lastId = (this.clanManage.clanPlayersList.dataProvider[this.clanManage.clanPlayersList.selectedIndex].id);
         }
         this.clanManage.clanPlayersList.dataProvider = new DataProvider(value);
         if(this.clanManage.clanPlayersList.selectedIndex == -1)
         {
            this.clanManage.clanPlayersList.selectedIndex = 0;
         }
         else if(lastId != -1)
         {
            data = this.clanManage.clanPlayersList.dataProvider;
            for(i = 0; i < data.length; i++)
            {
               if(data[i].id == lastId)
               {
                  this.clanManage.clanPlayersList.selectedIndex = i;
                  return;
               }
            }
            _selectedData = value[this.clanManage.clanPlayersList.selectedIndex];
         }
      }
      
      public function setDailyMessage(value:String) : void
      {
         _dailyMessageText = value;
         this.dailyMessage.text = this._dailyMessageText;
      }
      
      public function setRanksData(value:Array, freeRankSlotsAvailable:Boolean) : void
      {
         for(var i:uint = 0; i < value.length; i++)
         {
            var rankData:Object;
            rankData = value[i];
            if((rankData.default))
            {
               rankData.name = LocalizationData.getName("clan_rank_" + rankData.name);
            }
         }
         _freeRankSlotsAvailable = freeRankSlotsAvailable;
         _ranks = value;
         this.fillRanksList(value);
      }
      
      protected function fillRanksList(value:Array) : void
      {
         var list:ScrollingList = this.clanSettings.ranksList;
         list.dataProvider = new DataProvider(value);
         list.height = value.length * (38 + 5);
         list.validateNow();
         if(list.selectedIndex == -1)
         {
            list.selectedIndex = 0;
         }
         trace("RANKlist::",value.length,list.height,list.visible);
         this.checkAddRankEnabled();
         this.clanSettings.btnAddRank.y = list.y + value.length * (38 + 5) + 10;
      }
      
      private function checkAddRankEnabled() : void
      {
         var isAdmin:Boolean = this._state == CLAN_MANAGE_STATE;
         this.clanSettings.btnAddRank.enabled = isAdmin && this._freeRankSlotsAvailable;
      }
      
      public function setManagePermissions(value:Object) : void
      {
         var rankData:Object = value;
         this.clanManage.btnRankUp.enabled = rankData.change_rank_permission;
         this.clanManage.btnRankDown.enabled = rankData.change_rank_permission;
         this.clanManage.btnKickMember.enabled = rankData.kick_permission;
         this.clanManage.btnInviteMember.enabled = rankData.invite_permission;
         this.btnChangeDailyMessage.visible = rankData.daily_message_permission;
         this.dailyMessage.editable = rankData.daily_message_permission;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.clanSettings.visible = false;
         LocalizationData.addCallback(this.onLocalization,"clan_manage_label");
         this.clanSettings.rankName.enabled = false;
         this.clanSettings.ranksList.addEventListener(ListEvent.INDEX_CHANGE,this.onChangeRankSettings);
         this.clanSettings.btnAddRank.addEventListener(ButtonEvent.CLICK,this.onCreateRank);
         this.clanSettings.btnDeleteRank.addEventListener(ButtonEvent.CLICK,this.onDeleteRank);
         this.clanSettings.btnDeleteRank.enabled = false;
         this.clanSettings.checkBoxInviteRight.addEventListener(ButtonEvent.CLICK,this.checkBoxInviteRightChange);
         this.clanSettings.checkBoxKickRight.addEventListener(ButtonEvent.CLICK,this.checkBoxKickRightChange);
         this.clanSettings.checkBoxChangeRankRight.addEventListener(ButtonEvent.CLICK,this.checkBoxChangeRankRightChange);
         this.clanSettings.checkBoxDailyMessageRight.addEventListener(ButtonEvent.CLICK,this.checkBoxDailyMessageRightChange);
         this.clanSettings.checkBoxBattleCommanderRight.addEventListener(ButtonEvent.CLICK,this.checkBoxBattleCommanderRightChange);
         this.clanManage.searchComponent.visible = false;
         this.clanManage.searchComponent.setDropDown(false);
         this.clanManage.btnRankUp.addEventListener(ButtonEvent.CLICK,this.rankUpMember);
         this.clanManage.btnRankDown.addEventListener(ButtonEvent.CLICK,this.rankDownMember);
         this.clanManage.btnLeaveClan.addEventListener(ButtonEvent.CLICK,this.leaveClan);
         this.clanManage.btnKickMember.addEventListener(ButtonEvent.CLICK,this.kickMember);
         this.clanManage.btnInviteMember.addEventListener(ButtonEvent.CLICK,this.inviteMember);
         this.clanManage.clanPlayersList.addEventListener(ListEvent.INDEX_CHANGE,this.onNewIndex);
         this.mainTabBar.addEventListener(IndexEvent.INDEX_CHANGE,this.onMainTabBarIndexChange);
         this.mainTabBar.selectedIndex = 0;
         this.clanManage.searchComponent.playersList.addEventListener(ListEvent.ITEM_CLICK,this.invitePlayerByName);
         this.clanManage.searchComponent.addEventListener(FocusEvent.FOCUS_OUT,this.onTextInputFocusOut);
         this.dailyMessage.addEventListener(InputEvent.INPUT,this.handleInput);
         this.dailyMessage.addEventListener(FocusEvent.FOCUS_OUT,this.onMessFocusOut);
         this.btnChangeDailyMessage.addEventListener(ButtonEvent.CLICK,this.editDailyMessage);
      }
      
      private function onMouseItemClick(e:MouseEventEx) : void
      {
         var activeItem:*;
         if(e.buttonIdx == 0)
         {
            return;
         }
         if((e.target) && e.target.parent is SrvClanListRenderer)
         {
            activeItem = e.target.parent;
         }
         else if((e.target) && e.target.parent.parent is SrvClanListRenderer)
         {
            activeItem = e.target.parent.parent;
         }
         else
         {
            activeItem = e.target;
         }
         trace("onMouseItemClick:::",e.target,e.currentTarget,activeItem);
         if(!activeItem || activeItem.enabled == false || !activeItem.data)
         {
            return;
         }
         var options:Array = new Array();
         var itemData:Object = activeItem.data;
         if((options.length))
         {
            SrvContextMenu.show(options,0,0,this);
         }
      }
      
      private function checkBoxInviteRightChange(e:ButtonEvent) : void
      {
         ExternalInterface.call("update_invite_permission",this._selectedRankId,this.clanSettings.checkBoxInviteRight.selected);
      }
      
      private function checkBoxKickRightChange(e:ButtonEvent) : void
      {
         ExternalInterface.call("update_kick_permission",this._selectedRankId,this.clanSettings.checkBoxKickRight.selected);
      }
      
      private function checkBoxChangeRankRightChange(e:ButtonEvent) : void
      {
         ExternalInterface.call("update_change_rank_permission",this._selectedRankId,this.clanSettings.checkBoxChangeRankRight.selected);
      }
      
      private function checkBoxDailyMessageRightChange(e:ButtonEvent) : void
      {
         ExternalInterface.call("update_daily_message_permission",this._selectedRankId,this.clanSettings.checkBoxDailyMessageRight.selected);
      }
      
      private function checkBoxBattleCommanderRightChange(e:ButtonEvent) : void
      {
         ExternalInterface.call("update_battle_commander_permission",this._selectedRankId,this.clanSettings.checkBoxBattleCommanderRight.selected);
      }
      
      private function onTextInputFocusOut(e:FocusEvent) : void
      {
         trace("onTextInputFocusOut",FocusManager.getFocus(),stage.focus,(stage.focus) ? this.clanManage.searchComponent.contains(stage.focus) : null);
         stage.addEventListener(Event.RENDER,this.onRender);
      }
      
      private function onRender(e:Event) : void
      {
         ++this.renderFrames;
         if(this.renderFrames == 12 || (stage.focus))
         {
            renderFrames = 0;
            stage.removeEventListener(Event.RENDER,this.onRender);
            trace("onRender",this.renderFrames,FocusManager.getFocus(),stage.focus,(stage.focus) ? this.clanManage.searchComponent.contains(stage.focus) : null);
            if((SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
            {
               return;
            }
            if((stage.focus) && (this.clanManage.searchComponent.contains(stage.focus)))
            {
               return;
            }
            this.clanManage.searchComponent.cleanUp();
            this.clanManage.searchComponent.visible = false;
            this.clanManage.searchComponent.x = 0;
         }
      }
      
      private function invitePlayerByName(e:ListEvent) : void
      {
         trace("changeInputName::init");
         if(!e || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var itemData:* = e.itemData;
         trace("changeInputName::",itemData);
         if((itemData))
         {
            this.clanManage.searchComponent.cleanUp();
            this.clanManage.searchComponent.visible = false;
            this.clanManage.searchComponent.x = 0;
            stage.focus = this;
            trace("invite_player_to_clan",itemData.name);
            ExternalInterface.call("invite_player_to_clan",itemData.name);
         }
      }
      
      private function onMessFocusOut(e:Event) : void
      {
         trace("onMessFocusOut",stage.focus);
         if(stage.focus == this.dailyMessage || stage.focus == this.dailyMessage.textField)
         {
            return;
         }
         this.dailyMessage.text = this._dailyMessageText;
         this.btnChangeDailyMessage.visible = true;
      }
      
      private function editDailyMessage(e:Event) : void
      {
         trace("editDailyMessage");
         stage.focus = this.dailyMessage;
         this.dailyMessage.textField.setSelection(0,this.dailyMessage.text.length);
         this.btnChangeDailyMessage.visible = false;
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var index:uint = details.controllerIndex;
         if(!details.shiftKey)
         {
            trace("HandleInputDM::",details.navEquivalent,NavigationCode.ENTER,details.code);
            switch(details.code)
            {
               case Keyboard.ENTER:
                  if(details.value == InputValue.KEY_DOWN)
                  {
                     if(this.dailyMessage.editable && (stage.focus == this.dailyMessage || stage.focus == this.dailyMessage.textField))
                     {
                        trace("send_daily_message::",this.dailyMessage.text + "||");
                        ExternalInterface.call("update_daily_message",this.dailyMessage.text);
                        _dailyMessageText = this.dailyMessage.text;
                        event.handled = true;
                        stage.focus = this;
                     }
                  }
                  break;
            }
         }
      }
      
      private function onMainTabBarIndexChange(e:IndexEvent) : void
      {
         if(this.clanSettings.visible && this.mainTabBar.selectedIndex == 0)
         {
            ExternalInterface.call("clan_members_tab_opened");
         }
         this.clanManage.visible = this.mainTabBar.selectedIndex == 0;
         this.clanSettings.visible = this.mainTabBar.selectedIndex == 1;
      }
      
      private function onNewIndex(e:ListEvent) : void
      {
         _selectedData = e.itemData;
      }
      
      private function rankUpMember(e:ButtonEvent) : void
      {
         ExternalInterface.call("rank_up_member",this._selectedData.id);
      }
      
      private function rankDownMember(e:ButtonEvent) : void
      {
         ExternalInterface.call("rank_down_member",this._selectedData.id);
      }
      
      private function leaveClan(e:ButtonEvent) : void
      {
         var call:String = this._state == CLAN_OVERVIEW_STATE ? "leave_clan" : ExternalInterface.call("disband_clan");
         trace("onLeaveClan::",call);
         ExternalInterface.call(call);
      }
      
      private function kickMember(e:ButtonEvent) : void
      {
         ExternalInterface.call("kick_member",this._selectedData.id,this._selectedData.name);
      }
      
      private function inviteMember(e:ButtonEvent) : void
      {
         this.clanManage.searchComponent.setFriendsList(FriendsMenu.getFriendsList());
         this.clanManage.searchComponent.visible = true;
         stage.focus = this.clanManage.searchComponent.textInput;
         this.clanManage.searchComponent.x = -103;
         this.clanManage.searchComponent.y = 542;
      }
      
      private function onDeleteRank(e:ButtonEvent) : void
      {
         trace("onDeleteRank");
         var list:ScrollingList = this.clanSettings.ranksList;
         if(list.selectedIndex == -1)
         {
            return;
         }
         ExternalInterface.call("clan_remove_rank",uint(list.selectedIndex));
      }
      
      private function onCreateRank(e:ButtonEvent) : void
      {
         trace("onCreateRank");
         var rankName:TextInput = this.clanSettings.rankName;
         rankName.enabled = true;
         rankName.validateNow();
         stage.focus = rankName;
         rankName.text = LocalizationData.getName("enter_rank_name");
         rankName.validateNow();
         this.onRankNameChange();
         rankName.addEventListener(Event.CHANGE,this.onRankNameChange);
         rankName.addEventListener(FocusEvent.FOCUS_OUT,this.addNewRank);
         rankName.addEventListener(KeyboardEvent.KEY_DOWN,this.onKeyboardDow);
         (rankName.textField as TextField).setSelection(0,this.clanSettings.rankName.text.length);
      }
      
      private function onRankNameChange(e:Event = null) : void
      {
         var rankName:TextInput = this.clanSettings.rankName;
         var newArray:Array = this._ranks.concat();
         newArray.push({name:rankName.text});
         this.fillRanksList(newArray);
         var list:ScrollingList = this.clanSettings.ranksList;
         list.selectedIndex = newArray.length - 1;
         this.clanSettings.btnAddRank.enabled = false;
      }
      
      private function onKeyboardDow(e:KeyboardEvent) : void
      {
         if(e.keyCode == Keyboard.ENTER)
         {
            this.addNewRank();
         }
      }
      
      private function addNewRank(e:Event = null) : void
      {
         this.clanSettings.rankName.removeEventListener(Event.CHANGE,this.onRankNameChange);
         this.clanSettings.rankName.removeEventListener(FocusEvent.FOCUS_OUT,this.addNewRank);
         this.clanSettings.rankName.removeEventListener(KeyboardEvent.KEY_DOWN,this.onKeyboardDow);
         stage.focus = this;
         var newRankName:String = this.clanSettings.rankName.text;
         this.clanSettings.rankName.enabled = false;
         this.checkAddRankEnabled();
         var failedRankName:Boolean = !newRankName || newRankName == LocalizationData.getName("enter_rank_name");
         if(failedRankName)
         {
            this.fillRanksList(this._ranks);
            return;
         }
         ExternalInterface.call("clan_create_rank",newRankName);
      }
      
      private function onChangeRankSettings(e:ListEvent) : void
      {
         trace("onChangeRankSettings::",e.index);
         var rankData:Object = e.itemData;
         this.clanSettings.checkBoxInviteRight.selected = rankData.invite_permission;
         this.clanSettings.checkBoxKickRight.selected = rankData.kick_permission;
         this.clanSettings.checkBoxChangeRankRight.selected = rankData.change_rank_permission;
         this.clanSettings.checkBoxDailyMessageRight.selected = rankData.daily_message_permission;
         this.clanSettings.checkBoxBattleCommanderRight.selected = rankData.battle_commander_permission;
         this.checkRanksEnabled(e.index);
         this.clanSettings.rankName.text = rankData.name;
         _selectedRankId = rankData.id;
         this.clanSettings.btnDeleteRank.enabled = rankData.default == false;
      }
      
      private function onLocalization(value:String = null) : void
      {
         this.clanManage.btnRankUp.label = LocalizationData.getName("btn_rank_up_clan_memeber_label");
         this.clanManage.btnRankDown.label = LocalizationData.getName("btn_rank_down_clan_memeber_label");
         this.clanManage.btnKickMember.label = LocalizationData.getName("btn_kick_clan_memeber_label");
         this.clanManage.btnInviteMember.label = LocalizationData.getName("btn_invite_clan_memeber_label");
         this.textFieldClanLevelLabel.text = LocalizationData.getName("clan_level_label");
         this.textDailyMessage.text = LocalizationData.getName("daily_message_label");
         this.clanManage.textFieldRank.text = LocalizationData.getName("clan_players_rank_label");
         this.clanManage.textFieldLevel.text = LocalizationData.getName("clan_players_level_label");
         this.clanManage.textFieldName.text = LocalizationData.getName("clan_players_name_label");
         this.clanManage.textFieldStatus.text = LocalizationData.getName("clan_players_status_label");
         this.clanSettings.btnAddRank.label = LocalizationData.getName("btn_add_rank_label");
         this.clanSettings.btnDeleteRank.label = LocalizationData.getName("btn_remove_rank_label");
         this.clanSettings.checkBoxInviteRight.label = LocalizationData.getName("checkbox_clan_invite_permission");
         this.clanSettings.checkBoxKickRight.label = LocalizationData.getName("checkbox_clan_member_kick_permission");
         this.clanSettings.checkBoxChangeRankRight.label = LocalizationData.getName("checkbox_clan_change_rank_permission");
         this.clanSettings.checkBoxDailyMessageRight.label = LocalizationData.getName("checkbox_clan_daily_message_permission");
         this.clanSettings.checkBoxBattleCommanderRight.label = LocalizationData.getName("checkbox_battle_commander_permission");
         trace("MCW::mainTabBar",this.mainTabBar.dataProvider,this.mainTabBar.visible,this.mainTabBar.enabled);
         this.mainTabBar.width += 2;
         this.mainTabBar.dataProvider = new DataProvider([LocalizationData.getName("clan_control_tab"),LocalizationData.getName("clan_options_tab")]);
         this.mainTabBar.selectedIndex = 0;
         this.mainTabBar.validateNow();
      }
   }
}

