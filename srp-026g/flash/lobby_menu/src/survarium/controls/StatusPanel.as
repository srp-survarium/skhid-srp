package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.getTimer;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.MouseEventEx;
   import survarium.Util;
   import survarium.buttons.ButtonPremium;
   import survarium.data.IconContainer;
   import survarium.data.LocalizationData;
   import survarium.gsearch.LobbyGameSearch;
   import survarium.gsearch.MatchMakingSettings;
   import survarium.messages.MessageBase;
   import survarium.messages.MessageEnterName;
   import survarium.messages.MessageMoneyExchange;
   import survarium.rooms.RoomsLook;
   import survarium.player.LocalPlayerProfile;
   import survarium.rooms.RoomsWindow;
   import survarium.constants.ClanState;
   import survarium.constants.Faction;
   import survarium.constants.VoucherType;
   
   public class StatusPanel extends UIComponent
   {
      
      private static var _iconContainer:IconContainer;
      
      public static const MATCH_SETTINGS:String = "matchSettings";
      
      public static const SHOW_DAILY_MESSAGE:String = "showDailyMessage";

      // dispatched when draw has laid the panel out (its exit and settings buttons placed): the
      // lobby draws those buttons above every modal where the panel draws them (LobbyMenu._corner)
      public static const LAID_OUT:String = "laidOut";
      
      private var _show_query:Boolean = true;
      
      private var _time:int;
      
      private var _margin:uint = 3;
      
      // the player level shown as the top-rank icon in place of its number
      private static const TOP_RANK_LEVEL:int = 100;
      
      // meaning not traced: set_day_progress hides the daily bonus text at this step
      private static const DAILY_PROGRESS_LAST:int = 3;
      
      private var _anno_message:String = "";
      
      public var panelFaction:MovieClip;
      
      public var panel_players_block:MovieClip;
      
      public var panel_player:MovieClip;
      
      public var panel_time_block:MovieClip;
      
      public var panel_info_blog:MovieClip;
      
      public var panel_ping_block:MovieClip;
      
      public var panel_message:MovieClip;
      
      public var panel_games_block:MovieClip;
      
      public var profiles_container:MovieClip;
      
      public var lobbyGameSearch:LobbyGameSearch;
      
      public var btn_settings:Button;
      
      public var dropDownButton:Button;
      
      public var btn_off:Button;
      
      public var playBtn:Button;
      
      public var survivalNotesBtn:MovieClip;
      
      public var btn_add_premium_money:Button;
      
      public var btn_add_regular_money:Button;
      
      public var btnChangeName:Button;
      
      public var play_container:MovieClip;
      
      public var changeNameFailedDelay:uint = 6000;
      
      public var changeNameDelay:uint = 1000;
      
      public var background:MovieClip;
      
      public var factionHint:ToolTip;
      
      public var icon_generic:MovieClip;
      
      public var icon_premium:MovieClip;
      
      public var txGeneric:TextField;
      
      public var txPremium:TextField;
      
      public var btn_premium:ButtonPremium;
      
      public var centerContainerTop:MovieClip;
      
      public var right_corner:MovieClip;
      
      public var left_corner:MovieClip;
      
      public var mid_back:MovieClip;
      
      public var newItemsIcon:MovieClip;
      
      public var videoTutorial:MovieClip;
      
      public var labelInfo:MovieClip;
      
      public var labelInfoAdmin:MovieClip;
      
      public var changeNameCostPremium:int;
      
      public var postRewardData:Object;
      
      protected var _topMenu:MovieClip;
      
      protected var _premiumOptions:Array = [];
      
      protected var _premiumOptionsDiscount:Array = [];
      
      protected var _changeNicknameMessage:MessageEnterName;
      
      public var matchSettings:*;
      
      protected var _activeFactionName:String = null;
      
      protected var _clanAbb:String = "";
      
      protected var _clanState:uint = ClanState.RESERVE;
      
      private var _notesLabels:Array = ["prepare_to_battle_notes","items_notes"];
      
      private var _notesIndex:uint = 0;
      
      private var _vtMessage:MessageBase;
      
      protected var _factionData:Object;
      
      protected var firstScale:int = 0;

      // The game's own label of Play while it is the rooms' Leave, and ours.
      private var _roomsPlayLabel:String;

      private var _roomsOurLabel:String;
      
      public function StatusPanel()
      {
         super();
         trace("StatusPanel",this.panel_player,this.right_corner);
         this.btn_premium = this.panel_player.btn_premium;
         this.right_corner = this.panelFaction;
         this.icon_generic = this.panel_player.icon_generic;
         this.btn_add_regular_money = this.panel_player.btn_add_regular_money;
         this.btn_add_premium_money = this.panel_player.btn_add_premium_money;
         this.txGeneric = this.panel_player.btn_add_regular_money.content.txMoney;
         this.txPremium = this.panel_player.btn_add_premium_money.content.txMoney;
         this.videoTutorial = this.left_corner.videoTutorial;
         this.icon_premium = this.panel_player.btn_add_premium_money.content.icon;
         this.txPremium.mouseEnabled = this.btn_add_premium_money.mouseEnabled = false;
         this.txGeneric.mouseEnabled = this.btn_add_regular_money.mouseEnabled = false;
         this.playBtn = this.play_container.playBtn;
         this.factionHint = this.panelFaction.factionHint;
         this.newItemsIcon.visible = false;
         this.btnChangeName = this.panel_player.btnChangeName;
         trace("StatusPanel11",this.playBtn,this.newItemsIcon,this.btnChangeName,this.matchSettings,this.labelInfo,this.left_corner.notesTutorial);
         this.btnChangeName.addEventListener(ComponentEvent.STATE_CHANGE,this.updateIconPosition);
         this.btnChangeName.preventAutosizing = true;
         this.btnChangeName.constraintsDisabled = true;
         this.survivalNotesBtn = this.left_corner.notesTutorial;
         this.panel_player.btnChangeName.addEventListener(ButtonEvent.CLICK,this.showChangeNameMessage);
         this.right_corner.btnShowDailyMessage.addEventListener(ButtonEvent.CLICK,this.showDailyMessage);
         trace("StatusPanel2",this.play_container.dropDownButton,this.panel_player.txLevel,this.matchSettings);
         this.dropDownButton = this.play_container.dropDownButton;
         this.matchSettings.playContainer = this.play_container;
         this.dropDownButton.addEventListener(ButtonEvent.CLICK,this.showMatchSettings);
         this.panel_player.txLevel.text = "";
         this.matchSettings.visible = false;
      }
      
      private function showMatchSettings(e:ButtonEvent) : void
      {
         this.openMatchSettings(!this.matchSettings.visible);
      }

      // The match settings under Play (the arrow beside the mode) are a transient window
      // (Transient): on the transient layer while open, over every window (the bar stays where it
      // is); another window opening, a press outside them or Esc closes them as the arrow does.
      // Their own dropdowns open from them.
      private function openMatchSettings(value:Boolean) : void
      {
         this.matchSettings.visible = value;
         if(value)
         {
            Transient.open(this.matchSettings,this.dropDownButton,function():void
            {
               openMatchSettings(false);
            });
         }
         else
         {
            Transient.closed(this.matchSettings);
         }
         dispatchEvent(new Event(MATCH_SETTINGS));
      }
      
      private function showDailyMessage(e:ButtonEvent) : void
      {
         dispatchEvent(new Event(SHOW_DAILY_MESSAGE));
      }
      
      public function set_info_label(value:String, text2:String = "") : void
      {
         this.labelInfo.visible = value != "";
         this.labelInfo.textField.text = value;
         this.labelInfo.textFieldTimer.text = text2;
         this.sortAdminMessages();
      }
      
      private function sortAdminMessages() : void
      {
         this.labelInfo.y = (this.left_corner.notesTutorial.visible) || (this.left_corner.videoTutorial.visible) ? 164 : 108;
         this.labelInfoAdmin.y = this.labelInfo.visible ? this.labelInfo.y + 60 : this.labelInfo.y;
      }
      
      public function set_admin_label(valueLabel:String, text:String = "") : void
      {
         this.labelInfoAdmin.visible = valueLabel != "";
         this.labelInfoAdmin.textFieldLabel.text = valueLabel;
         this.labelInfoAdmin.textField.autoSize = TextFieldAutoSize.LEFT;
         this.labelInfoAdmin.textField.text = text;
         this.labelInfoAdmin.background.height = this.labelInfoAdmin.textField.y + this.labelInfoAdmin.textField.textHeight + 22;
         this.sortAdminMessages();
      }
      
      public function set show_query(value:Boolean) : void
      {
         _show_query = value;
         this.panel_info_blog.tx_query.visible = value;
         this.panel_info_blog.query_icon.visible = value;
      }
      
      public function set active_faction_name(value:String) : void
      {
         trace("active_faction_name",value);
         _activeFactionName = value;
         this.updateFactionData(this._factionData);
      }
      
      public function set active_faction(value:int) : void
      {
         this.panelFaction.btnActiveFaction.icon.mouseEnabled = false;
         if(value != Faction.NONE)
         {
            this.panelFaction.btnActiveFaction.icon.gotoAndStop(value + 1);
         }
         else
         {
            this.panelFaction.btnActiveFaction.icon.gotoAndStop(1);
         }
         this.panelFaction.btnActiveFaction.icon.visible = true;
      }
      
      public function set query(value:uint) : void
      {
         if(value > 0)
         {
            this.panel_info_blog.tx_query.text = value.toString();
            if(this._show_query && this.panel_info_blog.tx_query.visible == false)
            {
               this.panel_info_blog.tx_query.visible = true;
               this.panel_info_blog.query_icon.visible = true;
            }
         }
         else
         {
            this.panel_info_blog.tx_query.visible = false;
            this.panel_info_blog.query_icon.visible = false;
         }
      }
      
      public function set players(value:uint) : void
      {
         this.panel_players_block.tx_online.text = Util.getNumberString(value);
         invalidate(InvalidationType.RENDERERS);
         this.reflowPlayers();
      }
      
      public function set games(value:uint) : void
      {
         this.panel_games_block.tx_online.text = Util.getNumberString(value);
         this.reflowPlayers();
      }
      
      public function set ping(value:uint) : void
      {
         this.panel_ping_block.textField.text = value.toString() + LocalizationData.getName("ping_postfix");
      }
      
      public function set player_level(value:uint) : void
      {
         if(value != TOP_RANK_LEVEL)
         {
            this.panel_player.top_rank_icon.visible = false;
            this.panel_player.txLevel.text = value.toString();
         }
         else
         {
            this.panel_player.top_rank_icon.visible = true;
            this.panel_player.txLevel.text = "";
         }
      }
      
      public function set progress_hint(value:String) : void
      {
         trace("progressHint::",value);
         this.panel_player.hintButton.tipData = {name:value};
      }
      
      public function set player_progress(value:uint) : void
      {
         this.panel_player.progress_indicator.value = value;
      }
      
      public function handleNicknameValidation(result:Boolean, name:String = null, message:String = null) : void
      {
         if((this._changeNicknameMessage))
         {
            this._changeNicknameMessage.handleNicknameValidation(result,name,message);
         }
      }
      
      public function setNewItems(value:uint) : void
      {
         this.newItemsIcon.visible = value != 0;
         this.newItemsIcon.textField.text = value.toString();
         this.newItemsIcon.x = this._topMenu.x + ((this._topMenu.tabPage as ButtonBar).buttonWidth + (this._topMenu.tabPage as ButtonBar).spacing) * ((this._topMenu as TopMenu).SHOP_INDEX + 1) - 45;
         this.newItemsIcon.y = this._topMenu.y + 20;
         addChild(this.newItemsIcon);
      }
      
      public function setPremiumStore(value:Array) : void
      {
         _premiumOptions = value;
      }
      
      public function setPremiumStoreDiscount(value:Array) : void
      {
         _premiumOptionsDiscount = value;
      }
      
      public function getPremiumStore() : Array
      {
         return this._premiumOptions;
      }
      
      public function setTopMenu(value:MovieClip) : void
      {
         _topMenu = value;
         this._topMenu.questButton = this.panelFaction.btnActiveFaction;
      }
      
      public function setPlayerName(value:String) : void
      {
         this.panel_player.textFieldName.text = (this._clanAbb) && (this._clanState == ClanState.OVERVIEW || this._clanState == ClanState.MANAGE) ? "[" + this._clanAbb + "] " + value : value;
         this.panel_player.textFieldName.mouseEnabled = false;
         this.panel_player.textFieldName.mouseChildren = false;
         this.updateIconPosition();
      }
      
      public function setPlayerClanAbb(value:String) : void
      {
         _clanAbb = value;
         this.setPlayerName(PlayerInfo.nickname);
      }
      
      public function setPlayerClanState(value:uint) : void
      {
         _clanState = value;
         this.setPlayerName(PlayerInfo.nickname);
      }
      
      public function setStatusMessage(value:String, firstTime:Boolean) : void
      {
         _anno_message = value;
         var tx:TextField = this.panel_message.textField;
         tx.text = value;
         tx.text = tx.getLineText(0);
         var charIndex:uint = tx.length;
         while(tx.textWidth > tx.width && (charIndex))
         {
            tx.getCharBoundaries(charIndex);
            tx.text = tx.text.substr(0,charIndex) + "...";
            charIndex--;
         }
         if(firstTime)
         {
            this.onClickMessage();
         }
      }
      
      public function setMoneyData(generic:uint, premium:uint) : void
      {
         this.panel_player.hintButton.alpha = 0;
         trace("setMoneyData",this.btn_add_regular_money);
         this.txGeneric.text = Util.getMoneyString(generic);
         this.txPremium.text = Util.getMoneyString(premium);
         invalidateData();
      }
      
      public function show_match_making(bool:Boolean) : void
      {
         // The game's search panel is not shown to a room's member (or right
         // after Play), whom the server sends to its room's match through the
         // game's queue state (survarium.rooms).
         if(bool && RoomsWindow.me != null && RoomsWindow.me.hidesSearch())
         {
            return;
         }
         if(bool)
         {
            this.lobbyGameSearch.reset_sync();
            this.lobbyGameSearch.init();
            this.reflowPlayers(true);
         }
         else
         {
            this.lobbyGameSearch.close();
            this.reflowPlayers(true);
         }
      }
      
      public function setProfileCost(value:uint, money:uint) : void
      {
      }
      
      public function selectProfile(value:uint) : void
      {
      }
      
      public function setupProfiles(p:Array) : void
      {
      }
      
      public function onStageResize(event:Event = null) : void
      {
         invalidate(InvalidationType.SETTINGS);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         // Play greyed from the start until the presets' reasons are known
         // (presets.md): the game may show the lobby before its first word
         // on Play, and a quick press went through.
         if(!LocalPlayerProfile.reasonsKnown())
         {
            this.playBtn.enabled = false;
         }
         var textBonus:* = (this.right_corner.btnShowDailyMessage.content as MovieClip).getChildByName("textBonus");
         if((textBonus))
         {
            textBonus.visible = false;
         }
         this.labelInfo.mouseEnabled = false;
         this.labelInfo.visible = false;
         this.labelInfoAdmin.mouseEnabled = false;
         this.labelInfoAdmin.visible = false;
         var tx:TextField = this.txGeneric;
         var txF:TextFormat = tx.getTextFormat(0,1);
         tx.autoSize = TextFieldAutoSize.LEFT;
         tx.defaultTextFormat = txF;
         tx = this.txPremium;
         txF = tx.getTextFormat(0,1);
         tx.autoSize = TextFieldAutoSize.LEFT;
         tx.defaultTextFormat = txF;
         this.icon_premium.gotoAndStop(2);
         LocalizationData.addButton(this.playBtn,"Play");
         panel_games_block = this.mid_back.getChildByName("panel_games_block") as MovieClip;
         panel_players_block = this.mid_back.getChildByName("panel_players_block") as MovieClip;
         constraints.addElement("btn_settings",this.btn_settings,Constraints.RIGHT);
         constraints.addElement("btn_off",this.btn_off,Constraints.RIGHT);
         constraints.addElement("mid_back",this.mid_back,Constraints.CENTER_H);
         constraints.addElement("panel_info_blog",this.panel_info_blog,Constraints.LEFT);
         constraints.addElement("panel_player",this.panel_player,Constraints.LEFT);
         constraints.addElement("panel_time_block",this.panel_time_block,Constraints.RIGHT);
         constraints.addElement("panel_ping_block",this.panel_ping_block,Constraints.RIGHT);
         constraints.addElement("panel_message",this.panel_message,Constraints.LEFT);
         constraints.addElement("lobbyGameSearch",this.lobbyGameSearch,Constraints.CENTER_H);
         constraints.addElement("right_corner",this.right_corner,Constraints.RIGHT);
         constraints.addElement("left_corner",this.left_corner,Constraints.LEFT);
         constraints.addElement("centerContainerTop",this.centerContainerTop,Constraints.CENTER_H);
         this.panelFaction.hintButtonFactionProgress.toolTipRendererName = "SrvProfileTipRenderer";
         this.matchSettings.addEventListener(MatchMakingSettings.TOURNAMENT_DESELECTED,this.updateMatchSettings);
         this.matchSettings.addEventListener(MatchMakingSettings.TOURNAMENT_SELECTED,this.updateMatchSettings);
         if((this._topMenu))
         {
            this._topMenu.y = -4;
            addChildAt(this._topMenu,getChildIndex(this.play_container) - 1);
            constraints.addElement("top_menu",this._topMenu,Constraints.CENTER_H);
         }
         this.btn_premium.enabled = true;
         this.btn_premium.constraintsDisabled = true;
         this.btn_premium.preventAutosizing = true;
         this.btn_premium.addEventListener(ButtonEvent.CLICK,this.showPremiumMessage);
         this.btn_add_regular_money.enabled = true;
         this.btn_add_premium_money.enabled = true;
         this.btn_add_premium_money.addEventListener(ButtonEvent.CLICK,this.onPremiumMoneyAddClick);
         this.btn_add_regular_money.addEventListener(ButtonEvent.CLICK,this.onRegulatMoneyAddClick);
         this.left_corner.btnKnowledgeBase.addEventListener(ButtonEvent.CLICK,this.onKnowlengeBaseClick);
         var date:Date = new Date();
         _time = getTimer() - date.milliseconds - int(date.seconds) * 1000;
         this.updateTime();
         this.btn_settings.addEventListener(MouseEvent.CLICK,this.onClickSettings);
         this.btn_off.addEventListener(MouseEvent.CLICK,this.onClickOff);
         this.playBtn.addEventListener(ButtonEvent.CLICK,this.play_button_clicked);
         addEventListener(Event.ENTER_FRAME,this.onEnterFrame);
         this.panel_player.top_rank_icon.mouseEnabled = false;
         this.panel_player.top_rank_icon.visible = false;
         this.lobbyGameSearch.mouseEnabled = false;
         this.lobbyGameSearch.visible = false;
         this.panel_player.addEventListener(MouseEvent.CLICK,this.showProfilesList);
         stage.addEventListener(MouseEvent.CLICK,this.hideProfilesList);
         this.btn_settings.tooltipRendererName = "SrvTextHintRenderer";
         this.btn_off.tooltipRendererName = "SrvProfileTipRenderer";
         this.btnChangeName.tooltipRendererName = "SrvProfileTipRenderer";
         this.factionHint.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.btn_add_premium_money.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_add_regular_money.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_premium.tooltipRendererName = "SrvProfileTipRenderer";
         this.panel_player.hintButton.tooltipRendererName = "SrvProfileTipRenderer";
         this.right_corner.btnShowDailyMessage.tooltipRendererName = "SrvProfileTipRenderer";
         this.left_corner.btnKnowledgeBase.tooltipRendererName = "SrvProfileTipRenderer";
         this.left_corner.videoTutorial.visible = false;
         this.left_corner.videoTutorial.closeBtn.addEventListener(MouseEvent.CLICK,this.closeVT);
         this.left_corner.videoTutorial.clickArea.addEventListener(MouseEvent.CLICK,this.showVT);
         this.left_corner.notesTutorial.visible = false;
         this.left_corner.notesTutorial.clickArea.addEventListener(MouseEvent.CLICK,this.showNT);
         this.left_corner.notesTutorial.mouseChildren = true;
         if((this.right_corner.textFieldShop))
         {
            LocalizationData.addTextField(this.right_corner.textFieldShop,"online_store");
            this.right_corner.textFieldShop.mouseEnabled = false;
         }
         LocalizationData.addCallback(this.updateButtons,"btn_settings_label");
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         players = 0;
         games = 0;
      }
      
      public function showVideoTutorialLink(bool:Boolean) : void
      {
         this.left_corner.videoTutorial.visible = bool;
         this.sortAdminMessages();
      }
      
      public function showNotesTutorial(bool:Boolean, index:uint) : void
      {
         trace("showNotesTutorial",bool,index);
         if(bool)
         {
            _notesIndex = index;
         }
         this.left_corner.notesTutorial.mouseChildren = true;
         this.left_corner.notesTutorial.visible = bool;
         this.left_corner.notesTutorial.container.textField.text = LocalizationData.getName(this._notesLabels[index]);
         this.left_corner.notesTutorial.container.icon.gotoAndStop(index + 1);
         this.sortAdminMessages();
      }
      
      private function showNT(e:MouseEvent) : void
      {
         trace("survival_tutorial_clicked",this._notesIndex);
         ExternalInterface.call("survival_tutorial_clicked",this._notesIndex);
      }
      
      private function showVT(e:MouseEvent) : void
      {
         trace("video_tutorial_link_clicked");
         if((this._vtMessage))
         {
            return;
         }
         var object:Object = {
            text:LocalizationData.getName("go_to_video_tutorial_message"),
            onYes:this.gotoVideoTutorial,
            label_ok:LocalizationData.getName("btn_go_to_online_store"),
            label_cancel:LocalizationData.getName("cancel")
         };
         _vtMessage = Message.showMessage("apply",object,true);
         this._vtMessage.addEventListener(Event.REMOVED_FROM_STAGE,this.onVTRemoved);
      }
      
      private function onVTRemoved(e:Event) : void
      {
         e.target.removeEventListener(Event.REMOVED_FROM_STAGE,this.onVTRemoved);
         _vtMessage = null;
      }
      
      private function gotoVideoTutorial() : void
      {
         trace("video_tutorial_link_clicked");
         ExternalInterface.call("video_tutorial_link_clicked");
      }
      
      private function closeVT(e:MouseEvent) : void
      {
         trace("video_tutorial_hide_clicked");
         this.left_corner.videoTutorial.visible = false;
         ExternalInterface.call("video_tutorial_hide_clicked");
         this.sortAdminMessages();
      }
      
      private function updateMatchSettings(e:Event) : void
      {
         var selectedOption:String = (this.matchSettings.checkBoxClan.selected) ? LocalizationData.getName("match_settings_clan_match") : LocalizationData.getName("match_settings_checkbox_PVP");
         this.play_container.dropDownButton.label = selectedOption;
      }
      
      private function onRegulatMoneyAddClick(e:ButtonEvent) : void
      {
         MessageMoneyExchange.open(false);
      }
      
      public function setFactionProgress(value:Object) : void
      {
         trace("setVFactionProgress::",value,"|",this._activeFactionName);
         _factionData = value;
         this.updateFactionData(value);
      }
      
      protected function updateFactionData(value:Object) : void
      {
         var factionProgressText:String = null;
         var tipText:String = null;
         this.panelFaction.textFieldFactionName.text = (this._activeFactionName) ? "«" + this._activeFactionName.toUpperCase() + "»" : "";
         if((value))
         {
            this.panelFaction.textFieldFactionProgress.styleSheet = MainScene.textStyleSheet;
            factionProgressText = value.faction_rank + ", " + value.faction_label;
            this.panelFaction.textFieldFactionProgress.text = factionProgressText;
         }
         if((value))
         {
            tipText = Util.styledChar(this._activeFactionName,"white") + "\n" + value.faction_label;
            this.panelFaction.factionHint.tipData = {
               tipText:tipText,
               name:tipText,
               faction_label:value.faction_label,
               faction_rank:value.faction_rank
            };
            this.panelFaction.factionProgressIndicator.value = value.faction_progress;
         }
         else
         {
            this.panelFaction.factionHint.tipData = {
               tipText:this._activeFactionName,
               name:this._activeFactionName,
               faction_label:"",
               faction_rank:""
            };
            this.panelFaction.factionProgressIndicator.value = 0;
         }
      }
      
      override protected function draw() : void
      {
         var scale:Number = 1;
         var upscale:Number = 0;
         var newWidth:int;
         var originHeight:Number;
         newWidth = MainScene.stageWidth;
         originHeight = _height / scaleY;
         trace("SP::updateSize",newWidth,upscale,"|",height,_height,originHeight);
         if(isInvalid(InvalidationType.SETTINGS))
         {
            setActualSize(newWidth / scale,originHeight);
            setSize(newWidth / scale,originHeight);
            if((constraints))
            {
               constraints.update(newWidth / (scale - upscale),originHeight);
            }
            if(this.firstScale < 2)
            {
            }
            this.reflowPlayers();
         }
         var stScale:Number = 1;
         if(MainScene.stageWidth / MainScene.stageHeight < 1.85)
         {
            setActualScale(stScale - upscale,stScale - upscale);
         }
         trace("SP::",this.centerContainerTop,this.play_container,this.matchSettings);
         this.play_container.x = this.centerContainerTop.x;
         this.matchSettings.x = int(this.play_container.x + 8);
         firstScale += 1;
         if((this.labelInfo))
         {
         }
         dispatchEvent(new Event(LAID_OUT));
      }
      
      override protected function preInitialize() : void
      {
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
      }
      
      private function reflowPlayers(useAnimations:Boolean = false) : void
      {
         var panelGamesSize:int = this.panel_players_block.tx_online.textWidth + 50;
         var playersX:int;
         var gamesX:int;
         var totalWidth:int;
         var newPos:int;
         if(this.lobbyGameSearch.visible)
         {
            playersX = this.lobbyGameSearch.x - panelGamesSize;
            gamesX = this.lobbyGameSearch.x + this.lobbyGameSearch.width + 10;
         }
         else
         {
            totalWidth = panelGamesSize + this.panel_games_block.tx_online.textWidth + 22;
            newPos = this.centerContainerTop.x + (this.centerContainerTop.width - totalWidth) / 2 - 3;
            playersX = newPos;
            gamesX = newPos + panelGamesSize;
         }
         trace("reflowPlayers",playersX,gamesX,this.mid_back.width,this.play_container.x);
         if(!useAnimations)
         {
            this.panel_players_block.x = playersX;
            this.panel_games_block.x = gamesX;
         }
         else
         {
            TweenLite.to(this.panel_players_block,0.1,{x:playersX});
            TweenLite.to(this.panel_games_block,0.1,{x:gamesX});
         }
      }
      
      public function showChangeNameMessage(e:ButtonEvent = null) : void
      {
         var voucherId:uint = PlayerInfo.getVoucherId(VoucherType.RENAME);
         var messageData:Object = {
            nickname:PlayerInfo.nickname,
            premiumCost:this.changeNameCostPremium,
            vouchersCost:1,
            voucherId:voucherId,
            changeNameFailedDelay:this.changeNameFailedDelay,
            changeNameDelay:this.changeNameDelay,
            selectVoucher:Boolean(e == null)
         };
         _changeNicknameMessage = Message.showMessage("change_name",messageData);
      }
      
      private function updateButtons(value:*, value2:*) : void
      {
         this.btn_settings.data = {name:LocalizationData.getName("btn_settings_label")};
         this.btn_off.data = {name:LocalizationData.getName("btn_exit_label")};
         this.btnChangeName.data = {name:LocalizationData.getName("btn_change_name_label")};
         this.btn_add_premium_money.data = {name:LocalizationData.getName("btn_add_premium_label")};
         this.btn_add_regular_money.data = {name:LocalizationData.getName("btn_exchange_premium_money")};
         this.btn_premium.data = {name:LocalizationData.getName("btn_premium_hint")};
         this.left_corner.btnKnowledgeBase.data = {name:LocalizationData.getName("btn_knowledge_base")};
         this.left_corner.videoTutorial.textField.text = LocalizationData.getName("youtube_video_tutorial_button");
         this.left_corner.videoTutorial.closeBtn.x = this.left_corner.videoTutorial.textField.x + this.left_corner.videoTutorial.textField.textWidth + 10;
         this.right_corner.btnShowDailyMessage.data = {name:LocalizationData.getName("daily_bonus_hint")};
         this.play_container.dropDownButton.label = LocalizationData.getName("match_settings_checkbox_PVP");
      }
      
      public function set_day_progress(value:uint) : void
      {
         this.right_corner.btnShowDailyMessage.content.bonusProgressIndicator.gotoAndStop(value + 1);
         this.right_corner.btnShowDailyMessage.data = {name:LocalizationData.getName("daily_bonus_hint")};
         var textBonus:* = (this.right_corner.btnShowDailyMessage.content as MovieClip).getChildByName("textBonus");
         if((textBonus))
         {
            textBonus.visible = value != DAILY_PROGRESS_LAST;
         }
      }
      
      private function hideProfilesList(e:Event) : void
      {
      }
      
      private function showProfilesList(e:Event) : void
      {
      }
      
      private function play_button_clicked(e:ButtonEvent) : void
      {
         if(this.playRooms())
         {
            return;
         }
         this.playBtn.selected = false;
         this.playBtn.focused = 0;
         trace("play_button_clicked");
         ExternalInterface.call("play_button_clicked");
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.played();
         }
      }
      
      private function onClickOff() : void
      {
         var object:Object = {
            text:LocalizationData.getName("game_exit_message"),
            onYes:this.callExit,
            label_ok:LocalizationData.getName("yes"),
            label_cancel:LocalizationData.getName("no")
         };
         Message.showMessage("apply",object,true);
      }
      
      private function callExit() : void
      {
         ExternalInterface.call("power_off_button_click");
      }
      
      private function onClickSettings() : void
      {
         ExternalInterface.call("show_settings");
      }
      
      private function onClickMessage(e:MouseEventEx = null) : void
      {
         if(!e || e.buttonIdx == MouseEventEx.LEFT_BUTTON)
         {
            Message.showMessage("anno",{text:this._anno_message},true);
         }
      }
      
      private function onEnterFrame(e:Event) : void
      {
         if(getTimer() - this._time >= 60000)
         {
            _time = getTimer();
            this.updateTime();
         }
      }
      
      private function updateTime() : void
      {
         var date:Date = new Date();
      }
      
      private function updateIconPosition(e:Event = null) : void
      {
         var newWidth:int = this.panel_player.textFieldName.getWidth() + 32;
         this.btnChangeName.setSize(newWidth,this.btnChangeName.height);
         this.btnChangeName.setActualSize(newWidth,this.btnChangeName.height);
      }
      
      private function onPremiumMoneyAddClick(e:Event) : void
      {
         MessageMoneyExchange.open(true);
      }
      
      private function onKnowlengeBaseClick(e:Event) : void
      {
         var object:Object = {
            text:LocalizationData.getName("go_to_knowledge_base_message"),
            onYes:this.goToKnowledgeBase,
            label_ok:LocalizationData.getName("btn_go_to_online_store"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",object,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      protected function goToKnowledgeBase() : void
      {
         ExternalInterface.call("go_to_knowledge_base");
      }
      
      public function showPremium(value:Object = null) : void
      {
         var messData:Object = null;
         if((value))
         {
            messData = value;
         }
         else
         {
            messData = new Object();
         }
         if(!this.btn_premium.discount)
         {
            messData.options = this._premiumOptions;
            Message.showMessage("premium",messData,true);
         }
         else
         {
            messData.options = this._premiumOptionsDiscount;
            messData.money = this.postRewardData.money;
            messData.experience = this.postRewardData.experience;
            messData.reputation = this.postRewardData.reputation;
            messData.factionId = this.postRewardData.factionId;
            Message.showMessage("post_premium",messData,true);
         }
      }
      
      private function showPremiumMessage(e:ButtonEvent) : void
      {
         this.showPremium();
      }
      
      // Play in a room or a room's queue (survarium.rooms): "Leave room" or
      // "Leave queue" (`word`), red, on whatever the game's own Play is (a
      // squad's member, a kit the game refuses: Leave needs none of its
      // conditions), grey from a press until its answer (`asking`). A
      // squad's soldier elsewhere: greyed, "In squad" (its leader plays for
      // both, squads.md; the owner's of 2026-10-31).
      // Otherwise the game's own label and look. `each`: the frame's call,
      // which leaves the game's own Play enabled as the game sets it.
      public function showRoomsPlay(leaving:Boolean, queued:Boolean, asking:Boolean, each:Boolean, nativePlay:Boolean, word:String) : void
      {
         var btn:* = this.playBtn;
         if(leaving || queued || !each)
         {
            // Play greyed while a reason holds (the kit a match would take
            // can not play, the maintenance rule: the server's own rule,
            // presets.md); Leave never. Never lit where the game keeps it off.
            var blocked:Boolean = !leaving && !queued && !LocalPlayerProfile.mayPlay();
            var on:Boolean = !asking && (leaving || queued || nativePlay && !blocked);
            if(btn.enabled != on)
            {
               btn.enabled = on;
            }
            this.showPlayReasons(blocked);
         }
         var soldier:Boolean = !leaving && !queued && LocalPlayerProfile.squadSoldier();
         // In the case of the game's own label (its Play is in capitals).
         var gameLabel:String = this._roomsPlayLabel != null ? this._roomsPlayLabel : String(btn.label);
         var label:String = soldier ? LocalizationData.getName("st_srp_play_in_squad") : word;
         if(gameLabel != "" && gameLabel == gameLabel.toUpperCase())
         {
            label = label.toUpperCase();
         }
         if(leaving || queued || soldier)
         {
            // The game sets its own label again (its language): ours over it
            // each time.
            if(btn.label != label)
            {
               if(this._roomsPlayLabel == null)
               {
                  this._roomsPlayLabel = btn.label;
               }
               btn.label = label;
            }
            this._roomsOurLabel = label;
            this.tintPlay(soldier ? new ColorTransform() : RoomsLook.RED_TINT);
         }
         else if(this._roomsPlayLabel != null)
         {
            if(btn.label == this._roomsOurLabel)
            {
               btn.label = this._roomsPlayLabel;
            }
            this._roomsPlayLabel = null;
            this._roomsOurLabel = null;
            this.tintPlay(new ColorTransform());
         }
      }

      // The greyed Play's tip: every reason, a line each (the game's
      // tooltip; a disabled button takes no hover, so it lies over it).
      private var _playReasons:Sprite;
      
      private function showPlayReasons(blocked:Boolean) : void
      {
         var btn:* = this.playBtn;
         if(this._playReasons != null)
         {
            if(this._playReasons.parent != null)
            {
               this._playReasons.parent.removeChild(this._playReasons);
            }
            this._playReasons = null;
         }
         if(!blocked || btn.parent == null || LocalPlayerProfile.reasons() == "")
         {
            return;
         }
         this._playReasons = LocalPlayerProfile.reasonTip(btn.parent,btn.x,btn.y,btn.width,btn.height);
      }
      
      // Play's art tinted, its label not: the text stays white, as PLAY's
      // (the owner's of 2026-10-20). Every part of the button but the one
      // that holds its text field takes the tint (as the rooms window's
      // Switch room).
      private function tintPlay(tint:ColorTransform) : void
      {
         RoomsLook.skinTint(this.playBtn,tint);
      }
      
      // Rooms are srp's only PvP (survarium.rooms): Play goes straight into
      // a room, no search shown; in a room it is "Leave room", in a room's
      // queue "Leave queue" (true: done). The game's own Play, its search,
      // is never run from this button (a JOIN and a match against bots call
      // it themselves).
      private function playRooms() : Boolean
      {
         var rooms:RoomsWindow = RoomsWindow.me;
         try
         {
            if(rooms == null)
            {
               return false;
            }
            if(rooms.leavesRoom())
            {
               rooms.leaveRoom();
            }
            else if(rooms.leavesQueue())
            {
               rooms.leaveQueue();
            }
            else
            {
               rooms.quickPlay();
            }
            return true;
         }
         catch(err:Error)
         {
         }
         return false;
      }
   }
}

