package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.layout.LayoutData;
   import scaleform.clik.motion.Tween;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.MouseEventEx;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.constants.GameMode;
   import survarium.core.Icon;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.constants.ClaimReason;
   import survarium.constants.CommentKind;
   import survarium.constants.Compliment;
   import survarium.constants.MatchResult;
   import survarium.constants.PremiumState;
   import survarium.constants.VoucherType;
   
   public class VictoryScreen extends MovieClip
   {
      
      // The score icons' frames (set_score_value); the reputation of faction n is SCORE_ICON_REPUTATION_BASE + n.
      private static const SCORE_ICON_MONEY:int = 1;
      
      private static const SCORE_ICON_EXPERIENCE:int = 4;
      
      private static const SCORE_ICON_REPUTATION_BASE:int = 5;
      
      private static const SCORE_ICON_RATING:int = 10;
      
      // The best players' title icons' frames (set_title_value): titles 0..3 of the results (docs/notes/results.md).
      private static const TITLE_ICON_BEST_PLAYER:int = 1;
      
      private static const TITLE_ICON_SHOOTER:int = 2;
      
      private static const TITLE_ICON_SUPPORT:int = 3;
      
      private static const TITLE_ICON_EXPLORER:int = 4;
      
      // A premium account's share of the match rewards, and the bonus shown for it.
      private static const PREMIUM_MULTIPLIER:Number = 1.5;
      
      private static const PREMIUM_BONUS_PERCENT:int = 50;
      
      public var team1_ponts:MovieClip;
      
      public var team2_ponts:MovieClip;
      
      public var layoutData:LayoutData;
      
      public var best_players_container:MovieClip;
      
      public var score_container:MovieClip;
      
      public var btn_close:Button;
      
      public var textFieldStats:TextField;
      
      public var ds:MovieClip;
      
      public var syncImage:MovieClip;
      
      public var loading_text:TextField;
      
      public var backImage:DisplayObject;
      
      public var btnShare:Button;
      
      public var btnSave:Button;
      
      public var premium_button:Button;
      
      public var buyTreasureMaps:Button;
      
      public var victory_background:MovieClip;
      
      public var txAlly:TextField;
      
      public var noDroppedItemDescr:TextField;
      
      public var treasureTextfield:TextField;
      
      public var txEnemy:TextField;
      
      public var txSelectedNickname:TextField;
      
      public var player_rank:MovieClip;
      
      public var txRewardsLabel:TextField;
      
      public var players_list:PlayerList;
      
      public var item_slot_renderer:Button;
      
      public var item_slot2_renderer:Button;
      
      public var premiumAd:TextField;
      
      public var txPremiumLeft:TextField;
      
      public var top_container:MovieClip;
      
      public var droppedItem:MovieClip;
      
      public var dropChanceLabel:TextField;
      
      public var dropChanceValueLabel:TextField;
      
      public var hintChanceArea:ToolTip;
      
      public var searchIcon:MovieClip;
      
      protected var _premiumOptions:Array = [];
      
      protected var tm:int;
      
      protected var str:String = "";
      
      protected var _match_id:uint = 0;
      
      protected var _foundRenderers:Array = [];
      
      protected var _maxMatchResult:uint = 0;
      
      protected var _unlockProgress:int = 0;
      
      protected var _data:Object;
      
      protected var _victory:Boolean = false;
      
      protected var _cache:Boolean = false;
      
      // The matches whose drop card is used or refused this session (by
      // match id): their slot stays grey, here and on a screen reopened.
      // The server takes one card a player a match (`drop_cards.md`).
      protected static var _cardsDone:Object = {};
      
      // The match a card is asked for, till its answer (0: none).
      protected var _cardAsked:uint = 0;
      
      protected var _lastName:String = "";
      
      protected var _totalWidth:Number = 583;
      
      protected var _mapIcon:Icon;
      
      protected var _hintRenderer:*;
      
      private var delay:Number = 0.3;
      
      private var length:Number = 0.2;
      
      public var victoryPlate:MovieClip;
      
      public var totalScore:MovieClip;
      
      public var percentSprite:MovieClip;
      
      public var treasureCount:TextField;
      
      public var iconsTeam1:MovieClip;
      
      public var iconsTeam2:MovieClip;
      
      public var spriteTreasure:MovieClip;
      
      public var spriteArrow:MovieClip;
      
      protected var bonusMatchId:uint = 0;
      
      protected var bonusMatchValue:Boolean = false;
      
      protected var _rewardsMultiplied:Boolean = false;
      
      private var _visibleComponents:int = 0;
      
      private var _profilesNames:Array = [];
      
      private var _profilesPlayTime:Array = [];
      
      private var scoreData:Object = {};
      
      protected var _autobuy_money_ammo:int = 0;
      
      protected var _autobuy_money_consulables:int = 0;
      
      protected var _autorepair_money:int = 0;
      
      protected var _autobuy_money_ammo_arr:Array = [];
      
      protected var _autobuy_money_consulables_arr:Array = [];
      
      protected var _autorepair_money_arr:Array = [];
      
      public var treasureItem:MovieClip;
      
      public var progressIndicator:MovieClip;
      
      public function VictoryScreen()
      {
         super();
         LocalizationData.addTextField(this.txAlly,"label_teamA");
         LocalizationData.addTextField(this.txEnemy,"label_teamB");
         this.players_list.addEventListener(MouseEvent.DOUBLE_CLICK,this.onListDoubleClick);
         this.players_list.addEventListener(MouseEvent.CLICK,this.onListItemClick);
         LocalizationData.addTextField(this.best_players_container.title2.textFieldValueName2,"title_2_value_1");
         LocalizationData.addTextField(this.best_players_container.title2.textFieldValueName2,"title_2_value_1");
         LocalizationData.addTextField(this.best_players_container.title0.textFieldValueName,"title_0_value_0");
         LocalizationData.addTextField(this.best_players_container.title1.textFieldValueName,"title_1_value_0");
         LocalizationData.addTextField(this.best_players_container.title1.textFieldValueName2,"title_1_value_1");
         LocalizationData.addTextField(this.best_players_container.title2.textFieldValueName,"title_2_value_0");
         LocalizationData.addTextField(this.best_players_container.title2.textFieldValueName2,"title_2_value_1");
         LocalizationData.addTextField(this.best_players_container.title3.textFieldValueName,"title_3_value_0");
         LocalizationData.addTextField(this.best_players_container.title3.textFieldValueName2,"title_3_value_1");
         LocalizationData.addButton(this.btn_close,"close");
         LocalizationData.addTextField(this.dropChanceLabel,"item_drop_chance_label");
         this.onLocal();
         LocalizationData.addCallback(this.onLocal,"item_drop_chance_label");
         this.btn_close.addEventListener(ButtonEvent.CLICK,this.closeWindow);
         this.players_list.autoSizeLists = true;
         this.premium_button.addEventListener(ButtonEvent.CLICK,this.showPremiumMessage);
         this.treasureCount.mouseEnabled = false;
         this.searchIcon.addEventListener(MouseEvent.CLICK,this.onTreasureMapClick);
         this.buyTreasureMaps.addEventListener(ButtonEvent.CLICK,this.gotoTreasureStore);
         this.treasureItem.addEventListener(MouseEvent.ROLL_OVER,this.onTreasureMouseOver);
         this.treasureItem.addEventListener(MouseEvent.ROLL_OUT,this.onTreasureMouseOut);
         if(this.progressIndicator)
         {
            this.progressIndicator.mouseEnabled = false;
            this.progressIndicator.gotoAndStop(1);
         }
         if(this.hintChanceArea)
         {
            this.hintChanceArea.tooltipRendererName = "SrvProfileTipRendererStyled";
         }
         this.treasureItem.addEventListener(MouseEvent.ROLL_OVER,this.showItemHint);
         this.droppedItem.addEventListener(MouseEvent.ROLL_OVER,this.showItemHint);
         this.treasureItem.addEventListener(MouseEvent.ROLL_OUT,this.hideItemHint);
         this.droppedItem.addEventListener(MouseEvent.ROLL_OUT,this.hideItemHint);
      }
      
      override public function get width() : Number
      {
         return 1199;
      }
      
      override public function get height() : Number
      {
         return 730;
      }
      
      private function onListDoubleClick(e:Event) : void
      {
         trace("onListDoubleClick::",e.type,e.target,e.currentTarget);
         this.reguestPlayerInfo(e.target.data);
      }
      
      private function onLocal(value:String = null) : void
      {
         trace("onLocal",this.totalScore.section_2);
         this.totalScore.section_2.scoreQuests.label.text = LocalizationData.getName("statistics_score_quests");
         this.totalScore.section_2.scoreEquipment.label.text = LocalizationData.getName("statistics_equipment_bonus");
         this.totalScore.section_2.scoreDaily.label.text = LocalizationData.getName("statistics_daily_bonus");
         this.totalScore.section_2.scorePremium.label.text = LocalizationData.getName("statistics_score_premium");
         this.totalScore.section_3.scoreWasted.label.text = LocalizationData.getName("wasted");
         this.totalScore.section_3.scoreTotal.label.text = LocalizationData.getName("total");
         this.totalScore.section_3.wastedRepair.label.text = LocalizationData.getName("autorepair_money");
         this.totalScore.section_3.wastedAmmo.label.text = LocalizationData.getName("autobuy_money_ammo");
         this.totalScore.section_3.wastedItems.label.text = LocalizationData.getName("autobuy_money_consulabels");
         this.totalScore.columns.textFieldMoney.text = LocalizationData.getName("money");
         this.totalScore.columns.textFieldExp.text = LocalizationData.getName("experience");
         this.totalScore.columns.textFieldRep.text = LocalizationData.getName("reputation");
         this.best_players_container.title0.textFieldTitle.text = LocalizationData.getName("best").toUpperCase() + " " + LocalizationData.getName("title_0_player");
         this.best_players_container.title1.textFieldTitle.text = LocalizationData.getName("best").toUpperCase() + " " + LocalizationData.getName("title_1_player");
         this.best_players_container.title2.textFieldTitle.text = LocalizationData.getName("best").toUpperCase() + " " + LocalizationData.getName("title_2_player");
         this.best_players_container.title3.textFieldTitle.text = LocalizationData.getName("best").toUpperCase() + " " + LocalizationData.getName("title_3_player");
      }
      
      public function registerHintRenderer(value:Object) : void
      {
         _hintRenderer = value;
         addChild(this._hintRenderer);
         this._hintRenderer.x = 100;
         this._hintRenderer.y = 100;
         this._hintRenderer.visible = false;
         this._hintRenderer.displaylabel(false);
      }
      
      private function hideItemHint(e:MouseEvent) : void
      {
         trace("hideItemHint");
         if(this._hintRenderer.alpha != 0)
         {
            TweenLite.killTweensOf(this._hintRenderer);
            TweenLite.to(this._hintRenderer,this.length,{
               delay:this.delay,
               alpha:0
            });
         }
      }
      
      private function showItemHint(e:MouseEvent) : void
      {
         var target:MovieClip = e.currentTarget as MovieClip;
         var itemData:Object = target.itemSlot.data;
         if(!itemData)
         {
            return;
         }
         this._hintRenderer.mouseEnabled = false;
         this._hintRenderer.mouseChildren = false;
         this._hintRenderer.data = target.itemSlot.data;
         trace("showItemHint::",target,this._hintRenderer.height,target.y);
         this._hintRenderer.x = int(int(target.x + target.width - this._hintRenderer.background.width - 10) * MainScene.scaleFactor) / MainScene.scaleFactor;
         this._hintRenderer.y = int(int(target.y - this._hintRenderer.background.height - 5) * MainScene.scaleFactor) / MainScene.scaleFactor;
         this._hintRenderer.visible = true;
         TweenLite.killTweensOf(this._hintRenderer);
         TweenLite.to(this._hintRenderer,this.length,{
            delay:this.delay,
            alpha:1
         });
      }
      
      protected function onListItemClick(e:MouseEventEx) : void
      {
         var button:Button;
         if(!this._data)
         {
            return;
         }
         if(e.buttonIdx == MouseEventEx.RIGHT_BUTTON)
         {
            button = e.target as Button;
            if(!button || !button.data || button.data.name == PlayerInfo.nickname)
            {
               return;
            }
            this.showContextMenu(button.data);
         }
      }
      
      private function showContextMenu(_data:Object) : void
      {
         var arr:Array = [];
         var foundInIgnored:Boolean = false;
         var foundInFriends:Boolean = false;
         var isFriend:Boolean = foundInFriends;
         if(!foundInFriends)
         {
            arr.push({
               name:LocalizationData.getName("add_to_friends"),
               click_function:this.addToFriends,
               nickname:_data.name,
               callBack:true
            });
         }
         if(!foundInIgnored)
         {
            arr.push({
               name:LocalizationData.getName("add_to_ignore"),
               click_function:this.addToIgnore,
               nickname:_data.name,
               callBack:true
            });
         }
         trace("VS::nickname",_data.name);
         var subOptions:Array = [];
         subOptions.push({
            name:LocalizationData.getName("bot_or_cheater_claim"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.CLAIM,
            subIndex:ClaimReason.BOT_OR_CHEATER
         });
         subOptions.push({
            name:LocalizationData.getName("swearing_claim"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.CLAIM,
            subIndex:ClaimReason.SWEARING
         });
         subOptions.push({
            name:LocalizationData.getName("non_sport_behavior_claim"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.CLAIM,
            subIndex:ClaimReason.NON_SPORT_BEHAVIOR
         });
         subOptions.push({
            name:LocalizationData.getName("account_trading_claim"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.CLAIM,
            subIndex:ClaimReason.ACCOUNT_TRADING
         });
         var subOptions2:Array = [];
         subOptions2.push({
            name:LocalizationData.getName("friendly_user_compliment"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.COMPLIMENT,
            subIndex:Compliment.FRIENDLY_USER
         });
         subOptions2.push({
            name:LocalizationData.getName("good_player_compliment"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.COMPLIMENT,
            subIndex:Compliment.GOOD_PLAYER
         });
         subOptions2.push({
            name:LocalizationData.getName("born_leader_compliment"),
            nickname:_data.name,
            click_function:this.flagPlayer,
            callBack:true,
            index:CommentKind.COMPLIMENT,
            subIndex:Compliment.BORN_LEADER
         });
         arr.push({
            name:LocalizationData.getName("claim_player"),
            options:subOptions
         });
         arr.push({
            name:LocalizationData.getName("compliment_player"),
            options:subOptions2
         });
         SrvContextMenu.show(arr,0,0,this);
      }
      
      protected function flagPlayer(value:Object) : void
      {
         trace("comment_about_user",value.nickname,value.index,value.subIndex);
         ExternalInterface.call("comment_about_user",value.nickname,value.index,value.subIndex);
      }
      
      public function addToFriends(value:Object) : void
      {
         ExternalInterface.call("add_friend_name",value.nickname);
      }
      
      public function addToIgnore(value:Object) : void
      {
         ExternalInterface.call("add_ignore_name",value.nickname);
      }
      
      private function onTreasureMouseOver(e:MouseEvent) : void
      {
         if(this.treasureItem.mouseEnabled)
         {
         }
      }
      
      private function onTreasureMouseOut(e:MouseEvent) : void
      {
      }
      
      private function gotoTreasureStore(e:ButtonEvent) : void
      {
         dispatchEvent(new Event("try_buy_maps",true));
      }
      
      private function onTreasureMapClick(e:Event) : void
      {
         trace("onTreasureMapClick::",this._match_id,this.treasureItem.mouseEnabled);
         if(this.searchIcon.visible == false || _cardsDone[this._match_id])
         {
            return;
         }
         var itemVoucherId:int = (PlayerInfo.getVoucherId(VoucherType.DROP_CARD));
         this.progressIndicator.gotoAndPlay(1);
         this.searchIcon.visible = false;
         _cardAsked = this._match_id;
         ExternalInterface.call("use_drop_card",this._match_id,itemVoucherId);
      }
      
      // The card slot greyed: no card for this screen's match.
      protected function greyCard() : void
      {
         this.treasureItem.alpha = 0.5;
         this.treasureItem.mouseEnabled = false;
         this.treasureItem.mouseChildren = false;
         this.searchIcon.visible = false;
         this.progressIndicator.gotoAndStop(1);
      }
      
      // No card for this match: used or refused, or a room with a PIN's
      // (LobbyMenu.fill_victory_screen), which the server refuses.
      public function noCard(matchId:uint) : void
      {
         _cardsDone[matchId] = true;
         if(matchId == this._match_id)
         {
            this.greyCard();
         }
      }
      
      // The lobby's refusal of the card asked (`84 46`,
      // st_cannot_use_drop_card), which the exe shows as a message box.
      protected function cardRefused() : void
      {
         if(this._cardAsked != 0)
         {
            this.noCard(this._cardAsked);
            _cardAsked = 0;
         }
      }
      
      // The layer messages are drawn in (Message.initStage): a match's
      // loading hid it (set_loading_level), and show_match_making leaves it
      // hidden while this screen is up.
      protected function showTop() : void
      {
         if(this.top_container)
         {
            this.top_container.visible = true;
         }
      }
      
      // A message box of the lobby's (LobbyMenu.show_system_message) while
      // this screen is up: true when it is to be drawn now, over this
      // screen. show_victory_screen keeps messages back till this screen
      // closes (Message.hideMessages), so a refusal or an error ("debug",
      // "ok") showed only once it was closed, as if under it. A box while
      // a card is asked is that card's refusal: the slot greys.
      public function messageOver(type:String) : Boolean
      {
         if(!parent || !visible || type != "debug" && type != "ok")
         {
            return false;
         }
         this.cardRefused();
         this.showTop();
         return true;
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function setDropUnlockPositions(value:Array, maxValue:uint) : void
      {
      }
      
      public function setMatchItemsDrop(value:Object) : void
      {
         var renderer:MovieClip = this.droppedItem;
         var descriptions:Array;
         var l:uint;
         var str:String;
         var i:uint;
         trace("setMatchItemsDrop::",value,renderer,this.dropChanceValueLabel);
         renderer.itemSlot.data = value;
         if(this._data && this._data.dropChance != undefined)
         {
            this.noDroppedItemDescr.visible = false;
            descriptions = this._data.description;
            if(!descriptions)
            {
               descriptions = [];
            }
            l = descriptions.length;
            str = "";
            for(i = 0; i < l; i++)
            {
               str += descriptions[i].name + " " + Util.styledChar(descriptions[i].value,"highlight") + "\n";
            }
            this.dropChanceValueLabel.width = 100;
            this.dropChanceValueLabel.text = String(Math.round(this._data.dropChance * 100));
            this.hintChanceArea.tipData = {name:str};
            trace("dropChanceValueLabel",this._data.dropChance,this.dropChanceValueLabel.text);
            this.dropChanceValueLabel.x = 180 - this.dropChanceValueLabel.textWidth;
            this.dropChanceLabel.x = this.dropChanceValueLabel.x - 120;
         }
         else
         {
            this.dropChanceValueLabel.text = "";
         }
         if(!value)
         {
            this.noDroppedItemDescr.visible = true;
            return;
         }
         this.noDroppedItemDescr.visible = false;
         trace("renderer::",renderer.mouseEnabled,renderer.itemSlot.mouseEnabled,renderer.mouseChildren);
         renderer.mouseChildren = true;
         this.handleItem(renderer,value);
      }
      
      public function setPremiumStore(value:Array) : void
      {
         _premiumOptions = value;
      }
      
      public function closeWindow(event:Event = null) : void
      {
         trace("closeVS");
         ExternalInterface.call("close_victory_screen");
         FocusManager.setModalClip(null);
         dispatchEvent(new Event(Event.CLOSE,true));
         if(parent)
         {
            parent.removeChild(this);
         }
         this.players_list.team1list.selectedIndex = -1;
         this.players_list.team2list.selectedIndex = -1;
         this.players_list.team1list.validateNow();
         this.players_list.team2list.validateNow();
         this.players_list.addPlayersList([]);
         _lastName = "";
         this.setPlayerData(null);
         this.progressIndicator.gotoAndStop(1);
         _data = null;
         _autobuy_money_ammo = 0;
         _autorepair_money = 0;
         bonusMatchId = 0;
         _rewardsMultiplied = false;
      }
      
      protected function getSelectedName() : String
      {
         return this._lastName;
      }
      
      public function getPlayerData() : Object
      {
         return this._data.playersCache[this._lastName];
      }
      
      public function setPlayerData(value:Object) : void
      {
         var playerInfo:Object;
         if(value)
         {
            if(!this._data.playersCache)
            {
               this._data.playersCache = new Object();
            }
            if(!(value is Array))
            {
               value.name = "";
               value.level = 1;
               value = [value];
            }
            playerInfo = {};
            playerInfo.nickname = this._lastName;
            playerInfo.profiles = value;
            this._data.playersCache[this._lastName] = playerInfo;
            trace("setPlayerData::",value.name,value.id,this._lastName,value.level,value.weapon1_data,value.weapon2_data);
            this.showTop();
            dispatchEvent(new Event(Events.SHOW_PLAYER_INFO));
         }
      }
      
      public function prepare() : void
      {
         scoreData = {};
         this.treasureTextfield.visible = false;
         this.buyTreasureMaps.visible = false;
         this.txAlly.visible = false;
         this.txEnemy.visible = false;
         this.iconsTeam1.visible = false;
         this.iconsTeam2.visible = false;
         this.treasureCount.visible = false;
         this.spriteTreasure.visible = false;
         this.spriteArrow.visible = false;
         this.dropChanceLabel.visible = false;
         this.dropChanceValueLabel.visible = false;
         this.percentSprite.visible = false;
         this.premium_button.visible = false;
         this.btnShare.label = "";
         this.btnShare.enabled = false;
         this.btnSave.label = "";
         this.btnSave.enabled = false;
         this.totalScore.visible = false;
         this._hintRenderer.visible = false;
         this.syncImage.alpha = 1;
         this.syncImage.visible = true;
         this.syncImage.loading.playAnimation(true);
         this.best_players_container.visible = false;
         this.searchIcon.visible = false;
         this.victoryPlate.visible = false;
         this.score_container.visible = false;
         this.treasureItem.visible = false;
         this.droppedItem.visible = false;
         this.treasureItem.itemSlot.data = null;
         var mapsCount:uint = PlayerInfo.getVoucherCount(VoucherType.DROP_CARD);
         this.treasureCount.text = "x" + mapsCount.toString();
         this.treasureItem.mouseEnabled = mapsCount > 0;
         this.treasureItem.mouseChildren = this.treasureItem.mouseEnabled;
         this.noDroppedItemDescr.visible = false;
         this.noDroppedItemDescr.text = LocalizationData.getName("victory_screen_no_dropped_item");
         this.noDroppedItemDescr.mouseEnabled = false;
         TextFieldEx.setVerticalAlign(this.noDroppedItemDescr,TextFieldEx.VALIGN_CENTER);
         this.buyTreasureMaps.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.buyTreasureMaps.tipData = {name:LocalizationData.getName("buy_treasure_maps")};
         this.buyTreasureMaps.visible = false;
         this.treasureTextfield.text = LocalizationData.getName("treasure_map_advert");
         this.dropChanceValueLabel.text = "0";
         this.totalScore.section_3.wastedRepair.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.totalScore.section_3.wastedAmmo.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.totalScore.section_3.wastedItems.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.totalScore.section_3.wastedRepair.mouseChildren = false;
         this.totalScore.section_3.wastedAmmo.mouseChildren = false;
         this.totalScore.section_3.wastedItems.mouseChildren = false;
      }
      
      public function setBonus(matchId:uint, bonus:Boolean) : void
      {
         bonusMatchId = matchId;
         bonusMatchValue = bonus;
         if(this._match_id == this.bonusMatchId)
         {
            _rewardsMultiplied = this.bonusMatchValue;
         }
      }
      
      public function updateVoucherCount() : void
      {
         var mapsCount:uint;
         trace("updateVoucherCount",this.treasureItem.itemSlot.data);
         if(this.treasureItem.itemSlot.data == null)
         {
            mapsCount = PlayerInfo.getVoucherCount(VoucherType.DROP_CARD);
            this.treasureCount.text = "x" + mapsCount.toString();
            if(!this._cache && !_cardsDone[this._match_id] && (!this._data || this._data && this._data.allow_drop_card != false))
            {
               this.treasureItem.mouseEnabled = mapsCount > 0;
               this.treasureItem.mouseChildren = this.treasureItem.mouseEnabled;
               this.searchIcon.visible = this.treasureItem.mouseEnabled;
            }
         }
      }
      
      private function showPremiumMessage(e:ButtonEvent) : void
      {
         if(parent)
         {
            this.showTop();
            parent["statusPanel"].showPremium({
               matchId:this._match_id,
               victory:true
            });
         }
      }
      
      protected function addToHint(string:String, toAdd:*, name:String, canBeBonus:Boolean = true) : String
      {
         var value:int = toAdd;
         string += (LocalizationData.getName(name) + ": " + Util.styledChar(value.toString(),"yellow") + (this._rewardsMultiplied && canBeBonus && value != 0 ? Util.styledChar(" x2","gold") : "") + "\n");
         return string;
      }
      
      public function updatePremiumState() : void
      {
         if(!this._data)
         {
            return;
         }
         if(this._data.statistics_reputation == undefined)
         {
            this._data.statistics_reputation = {};
         }
         var isPremium:Boolean = PlayerInfo.premiumState != PremiumState.NONE;
         var premiumMultilplier:Number = !isPremium ? 0 : PREMIUM_MULTIPLIER;
         var reputation:Object;
         trace("updatePremiumState",this._data.statistics_reputation,this._data.statistics_reputation_faction);
         this.premium_button.label = LocalizationData.getName("activate_premium").toUpperCase();
         this.premium_button.visible = !isPremium;
         this.totalScore.section_2.scorePremium.alpha = isPremium ? 1 : 0.5;
         if(this._data.statistics_reputation is Array)
         {
            reputation = this._data.statistics_reputation[0];
         }
         else
         {
            reputation = this._data.statistics_reputation;
         }
         var hintStr:String = "";
         var i:uint = 0;
         var statistics_reputation:int = int(reputation.match_rep) + int(reputation.quest_rep) + int(reputation.loyalty_bonus_rep) + int(reputation.premium_bonus_rep) + int(reputation.premium_items_rep);
         var target:MovieClip = i == 0 ? this.score_container.score_1 : this.score_container.score_2;
         this.set_score_value(target,statistics_reputation,statistics_reputation * premiumMultilplier,reputation.faction_id + SCORE_ICON_REPUTATION_BASE);
         hintStr = "";
         hintStr = this.addToHint(hintStr,reputation.match_rep,"rep_reward_match");
         hintStr = this.addToHint(hintStr,reputation.quest_rep,"rep_reward_quest");
         hintStr = this.addToHint(hintStr,reputation.premium_bonus_rep,"rep_reward_premium");
         hintStr = this.addToHint(hintStr,reputation.premium_items_rep,"statistics_equipment_bonus",true);
         if(statistics_reputation)
         {
            this.score_container.score_1.visible = true;
            this.score_container.score_2.visible = false;
            this.score_container.score_3.y = this.score_container.score_1.y + 30;
         }
         else
         {
            this.score_container.score_1.visible = false;
            this.score_container.score_2.visible = false;
            this.score_container.score_3.y = this.score_container.score_0.y + 30;
         }
         var statistics_rating_delta:int = this._data.statistics_rating.elo_after - this._data.statistics_rating.elo_before;
         hintStr = "";
         hintStr = this.addToHint(hintStr,this._data.statistics_rating.elo_before,"elo_before",false);
         hintStr = this.addToHint(hintStr,this._data.statistics_rating.elo_after,"elo_after",false);
         hintStr = this.addToHint(hintStr,statistics_rating_delta,"statistics_rating_delta",false);
         this.score_container.score_4.hintArea.data = {name:hintStr};
         var statistics_rating:int = (this._data.statistics_rating.elo_after);
         this._data.rating = statistics_rating;
         this.set_score_value(this.score_container.score_4,statistics_rating,statistics_rating + statistics_rating_delta,SCORE_ICON_RATING);
         hintStr = "";
         hintStr = this.addToHint(hintStr,this._data.statistics_exp.match_exp,"exp_reward_match");
         hintStr = this.addToHint(hintStr,this._data.statistics_exp.premium_bonus_exp,"exp_reward_premium");
         hintStr = this.addToHint(hintStr,this._data.statistics_exp.premium_items_exp,"statistics_equipment_bonus",true);
         this.score_container.score_0.hintArea.data = {name:hintStr};
         this.updateMoney();
         this.premium_button.enabled = true;
         var totalrep:int = this.updateTotalScore(reputation,"rep");
         this.totalScore.section_3.scoreTotal.rep.text = totalrep.toString();
         var totalexp:int = this.updateTotalScore(this._data.statistics_exp,"exp");
         this.totalScore.section_3.scoreTotal.exp.text = totalexp.toString();
         reputation.statistics_reputation = totalrep;
         this.set_score_value(target,totalrep,totalrep * premiumMultilplier,reputation.faction_id + SCORE_ICON_REPUTATION_BASE);
         this.set_score_value(this.score_container.score_0,totalexp,totalexp * premiumMultilplier,SCORE_ICON_EXPERIENCE);
         this._data.exp = totalexp;
         this.totalScore.section_2.scorePremium.value.text = premiumMultilplier ? "+" + String((premiumMultilplier - 1) * 100) + "%" : "";
         this.updateScroreLayout();
      }
      
      private function updateMoney() : void
      {
         var premiumMultilplier:Number = PlayerInfo.premiumState != PremiumState.NONE ? 0 : PREMIUM_MULTIPLIER;
         var hintStr:String = "";
         trace("updateMoney",this._data);
         if(!this._data)
         {
            return;
         }
         if(this._data.statistics_money == undefined)
         {
            this._data.statistics_money = {};
         }
         trace("updateMoney",this._data,this._data ? this._data.statistics_money : "no_data");
         hintStr = this.addToHint(hintStr,this._data.statistics_money.match_money,"money_reward_match",false);
         hintStr = this.addToHint(hintStr,this._data.statistics_money.quest_money,"money_reward_quest",false);
         hintStr = this.addToHint(hintStr,this._data.statistics_money.premium_bonus_money,"money_reward_premium",false);
         hintStr = this.addToHint(hintStr,this._data.statistics_money.premium_items_money,"statistics_equipment_bonus",false);
         hintStr = this.addToHint(hintStr,this._autobuy_money_ammo,"autobuy_money_ammo",false);
         hintStr = this.addToHint(hintStr,this._autobuy_money_consulables,"autobuy_money_consulabels",false);
         hintStr = this.addToHint(hintStr,this._autorepair_money,"autorepair_money",false);
         var statistics_money:int = int(this._data.statistics_money.match_money) + int(this._data.statistics_money.quest_money) + int(this._data.statistics_money.premium_bonus_exp) + int(this._data.statistics_money.premium_bonus_money) + int(this._data.statistics_money.premium_items_money) + int(this._autorepair_money) + int(this._autobuy_money_consulables) + int(this._autobuy_money_ammo);
         trace("VS:updateMoney",statistics_money,this._data.statistics_money.autobuy_money,this._data.statistics_money.autorepair_money,this._autobuy_money_ammo);
         var wasted:int = this._autorepair_money + this._autobuy_money_ammo + this._autobuy_money_consulables;
         var totalmoney:int = this.updateTotalScore(this._data.statistics_money,"money");
         this.totalScore.section_3.scoreTotal.money.text = (totalmoney + wasted).toString();
         this._data.money = totalmoney + wasted;
         this.set_score_value(this.score_container.score_3,totalmoney + wasted,(totalmoney + wasted) * premiumMultilplier,SCORE_ICON_MONEY);
         this.totalScore.section_3.scoreWasted.money.text = wasted.toString();
         this.totalScore.section_3.wastedRepair.value.text = this._autorepair_money;
         this.totalScore.section_3.wastedAmmo.value.text = this._autobuy_money_ammo;
         this.totalScore.section_3.wastedItems.value.text = this._autobuy_money_consulables;
         this.updateScroreLayout();
      }
      
      private function scoreTotal(value:*) : int
      {
         var total:int;
         var l:uint;
         var i:uint;
         if(value is Array)
         {
            total = 0;
            l = (value.length);
            for(i = 0; i < l; i++)
            {
               total += int(value[i]);
            }
            return total;
         }
         return int(value);
      }
      
      private function updateScoreComponents(value:Array) : void
      {
         var renderer:MovieClip;
         if(!value)
         {
            return;
         }
         for(var i:uint = 0; i < 3; i++)
         {
            renderer = this.totalScore.section_1["score" + i.toString()];
            renderer.visible = false;
         }
         var l:uint = value.length;
         _visibleComponents = l;
         for(i = 0; i < l; i++)
         {
            var data:Object = value[i];
            renderer = this.totalScore.section_1["score" + i.toString()];
            renderer.visible = true;
            renderer.label.text = data.name;
            renderer["money_value"] = this.scoreTotal(data.money);
            renderer["exp_value"] = this.scoreTotal(data.experience);
            renderer["rep_value"] = this.scoreTotal(data.reputation);
            trace("renderer[exp_value]",renderer["exp_value"]);
            renderer.value.text = data.amount != undefined ? this.scoreTotal(data.amount) : "";
            renderer.money.text = this.scoreTotal(data.money);
            renderer.exp.text = this.scoreTotal(data.experience);
            renderer.rep.text = this.scoreTotal(data.reputation);
            if(data.profile_name)
            {
               _profilesNames = data.profile_name is Array ? data.profile_name : [data.profile_name];
            }
            if(data.profile_play_time)
            {
               _profilesPlayTime = data.profile_play_time is Array ? data.profile_play_time : [data.profile_play_time];
            }
            var columns:Array = [];
            columns.push({
               label:LocalizationData.getName("profile"),
               values:data.profile_name
            });
            columns.push({
               label:LocalizationData.getName("profile_play_time"),
               values:data.profile_play_time
            });
            columns.push({
               label:data.name,
               values:data.amount
            });
            columns.push({
               label:LocalizationData.getName("money"),
               values:data.money
            });
            columns.push({
               label:LocalizationData.getName("experience"),
               values:data.experience
            });
            columns.push({
               label:LocalizationData.getName("reputation"),
               values:data.reputation
            });
            if(this._profilesNames && this._profilesNames.length)
            {
               renderer.hintArea.tooltipRendererName = "SrvColumnsTip";
               renderer.hintArea.tipData = {
                  columnsData:columns,
                  showValues:data.amount != undefined
               };
            }
         }
      }
      
      private function updateScroreLayout() : void
      {
         var renderer:MovieClip;
         this.totalScore.section_2.y = this._visibleComponents * 19 + 66;
         renderer = this.totalScore.section_2.scoreDaily;
         renderer.visible = int(renderer["money_value"]) + int(renderer["exp_value"]) + int(renderer["rep_value"]) > 0;
         renderer = this.totalScore.section_2.scorePremium;
         renderer.y = this.totalScore.section_2.scoreDaily.visible ? 100 : 79;
         var premiumPos:int = this.totalScore.section_2.y + this.totalScore.section_2.scorePremium.y;
         var premiumOffset:int = this.premium_button.visible ? 30 : 0;
         this.premium_button.y = this.totalScore.y + premiumPos + premiumOffset;
         this.totalScore.section_3.y = premiumPos + (this.premium_button.visible ? 35 : 0) + 30;
      }
      
      protected function updateTotalScore(data:Object, key:String) : int
      {
         var value:int = 0;
         function getValue(valueName:String):int
         {
            if(!data)
            {
               return 0;
            }
            var scroreValue:Object = data[valueName + "_" + key];
            scoreData[valueName + "_" + key] = scroreValue is Array ? scroreValue : [scroreValue];
            var scrore:int = scoreTotal(scroreValue);
            trace("updateTotalScore::getValue",scrore,scroreValue);
            return scrore;
         }
         trace("updateTotalScore",data,key);
         var totalValue:int = 0;
         var bonusValue:int = 0;
         var sectionScore:int = 0;
         sectionScore += int(this.totalScore.section_1.score0[key + "_value"]);
         trace("aftersection1Score");
         sectionScore += int(this.totalScore.section_1.score1[key + "_value"]);
         trace("aftersection2Score");
         sectionScore += int(this.totalScore.section_1.score2[key + "_value"]);
         trace("aftersection3Score");
         if(sectionScore > 0)
         {
            totalValue += sectionScore;
         }
         trace("totalValue",totalValue,key + "_value",sectionScore);
         value = getValue("quest");
         totalValue += value;
         this.totalScore.section_2.scoreQuests[key].text = value.toString();
         value = getValue("premium_items");
         totalValue += value;
         bonusValue = getValue("premium_items_bonus");
         if(bonusValue)
         {
            this.totalScore.section_2.scoreEquipment[key].text = value.toString() + "(+" + bonusValue.toString() + "%)";
         }
         else
         {
            this.totalScore.section_2.scoreEquipment[key].text = "";
         }
         value = getValue("daily");
         totalValue += value;
         bonusValue = getValue("daily_bonus");
         if(bonusValue)
         {
            this.totalScore.section_2.scoreDaily[key].text = value.toString() + " (+" + bonusValue.toString() + "%)";
         }
         else
         {
            this.totalScore.section_2.scoreDaily[key].text = "";
         }
         this.totalScore.section_2.scoreDaily[key + "_value"] = value;
         var isPremium:Boolean = PlayerInfo.premiumState != 0;
         value = getValue("premium");
         if(isPremium)
         {
            totalValue += value;
         }
         this.totalScore.section_2.scorePremium[key].text = value.toString();
         this.updateLocalScore("premium_items",this.totalScore.section_2.scoreEquipment,"premium_items_bonus");
         this.updateLocalScore("daily",this.totalScore.section_2.scoreDaily,"daily_bonus");
         var bonus:Array = [PREMIUM_BONUS_PERCENT,PREMIUM_BONUS_PERCENT,PREMIUM_BONUS_PERCENT];
         bonus.length = this._profilesNames ? this._profilesNames.length : 1;
         this.updateLocalScore("premium",this.totalScore.section_2.scorePremium,null,bonus);
         return totalValue;
      }
      
      private function updateLocalScore(dataName:String, rendrer:MovieClip, bonus:String = null, values:Array = null) : void
      {
         trace("updateLocalScore",dataName,rendrer,bonus,values,rendrer.hintArea);
         var columns:Array = [];
         columns.push({
            label:LocalizationData.getName("profile"),
            values:this._profilesNames
         });
         columns.push({
            label:LocalizationData.getName("profile_play_time"),
            values:this._profilesPlayTime
         });
         columns.push({
            label:rendrer.label.text,
            values:(values ? values : null)
         });
         columns.push({
            label:LocalizationData.getName("money"),
            values:this.scoreData[dataName + "_money"],
            bonus:this.scoreData[bonus + "_money"]
         });
         columns.push({
            label:LocalizationData.getName("experience"),
            values:this.scoreData[dataName + "_exp"],
            bonus:this.scoreData[bonus + "_exp"]
         });
         columns.push({
            label:LocalizationData.getName("reputation"),
            values:this.scoreData[dataName + "_rep"],
            bonus:this.scoreData[bonus + "_rep"]
         });
         if(this._profilesNames && this._profilesNames.length)
         {
            rendrer.hintArea.tooltipRendererName = "SrvColumnsTip";
            rendrer.hintArea.tipData = {
               columnsData:columns,
               showValues:values != null
            };
         }
      }
      
      public function setSpendMoneyDetailed(autobuy_ammo:Array, autobuy_consulables:Array, repair:Array) : void
      {
         if(!autobuy_ammo)
         {
            autobuy_ammo = [];
         }
         if(!autobuy_consulables)
         {
            autobuy_consulables = [];
         }
         if(!repair)
         {
            repair = [];
         }
         _autobuy_money_ammo_arr = autobuy_ammo;
         _autobuy_money_consulables_arr = autobuy_consulables;
         _autorepair_money_arr = repair;
         this.calculateSpendMoney();
      }
      
      protected function calculateSpendMoney() : void
      {
         function fillWastedData(arr:Array, target:MovieClip):int
         {
            var i:uint;
            var value:int = 0;
            var str:String = "";
            var l:uint = arr.length;
            trace("fillWastedData",arr.length,target);
            for(i = 0; i < l; i++)
            {
               var moneyData:Object = arr[i];
               value += int(moneyData.value);
               str += _itemdescriptor.getItemDescription(moneyData.dict_id).name + " " + moneyData.value + "\n";
            }
            target.tipData = {name:str};
            return value;
         }
         var _itemdescriptor:ItemDescriptor;
         _itemdescriptor = ItemDescriptor.getItemDescriptor("InventoryItems");
         _autobuy_money_ammo = fillWastedData(this._autobuy_money_ammo_arr,this.totalScore.section_3.wastedAmmo);
         _autobuy_money_consulables = fillWastedData(this._autobuy_money_consulables_arr,this.totalScore.section_3.wastedItems);
         _autorepair_money = fillWastedData(this._autorepair_money_arr,this.totalScore.section_3.wastedRepair);
         trace("calculateSpendMoney",this._autobuy_money_ammo,this._autobuy_money_consulables,this._autorepair_money);
         if(this._data && this._data.statistics_money)
         {
            this._data.statistics_money.autobuy_money_ammo_arr = this._autobuy_money_ammo_arr;
            this._data.statistics_money.autobuy_money_consulables_arr = this._autobuy_money_consulables_arr;
            this._data.statistics_money.autorepair_money_arr = this._autorepair_money_arr;
            this._data.statistics_money.autobuy_money_ammo = this._autobuy_money_ammo;
            this._data.statistics_money.autobuy_money_consulables = this._autobuy_money_consulables;
            this._data.statistics_money.autorepair_money = this._autorepair_money;
         }
         this.updateMoney();
      }
      
      public function setSpendMoney(autobuy:int, repair:int) : void
      {
         _autobuy_money_ammo = autobuy;
         _autorepair_money = repair;
         if(this._data && this._data.statistics_money)
         {
            this._data.statistics_money.autobuy_money_ammo = autobuy;
            this._data.statistics_money.autobuy_money_consulables = autobuy;
            this._data.statistics_money.autorepair_money = repair;
            this.updateMoney();
         }
      }
      
      private function set_score_value(clip:MovieClip, value:int, value_prem:int, icon:int, isPrimary:Boolean = false) : void
      {
         var rateStr:String;
         clip.icon.gotoAndStop(icon);
         if(isPrimary)
         {
            rateStr = (value > 0 ? "+" : "") + value.toString();
            clip.textField.text = rateStr;
         }
         else
         {
            clip.textField.text = value.toString();
         }
         if(icon == SCORE_ICON_RATING && value_prem - value && value_prem)
         {
            clip.textField.text += " (" + (value_prem - value > 0 ? "+" : "") + String(value_prem - value) + ")";
         }
      }
      
      public function getData() : Object
      {
         return this._data;
      }
      
      public function setData(value:Object, cache:Boolean = false) : void
      {
         function set_title_value(clip:MovieClip, icon:int, name:String, value:String, value2:String = null):void
         {
            trace("set_title_value",clip);
            clip.icon.gotoAndStop(icon);
            clip.textFieldName.text = name;
            clip.textFieldValue.text = value;
            clip.textFieldValue.x = clip.textFieldValueName.x + clip.textFieldValueName.textWidth + 5;
            clip.size = 255;
            clip.shift = 0;
            if(value2 != null)
            {
               clip.textFieldValueName2.visible = true;
               clip.textFieldValue2.text = value2;
               clip.textFieldValue2.x = clip.textFieldValueName2.x + clip.textFieldValueName2.textWidth + 5;
            }
            else
            {
               clip.textFieldValue2.text = "";
               clip.textFieldValueName2.visible = false;
            }
         }
         if(this._autobuy_money_ammo != 0)
         {
            value.statistics_money.autobuy_money_ammo = this._autobuy_money_ammo;
         }
         if(this._autobuy_money_consulables != 0)
         {
            value.statistics_money.autobuy_money_consulables = this._autobuy_money_consulables;
         }
         if(this._autorepair_money != 0)
         {
            value.statistics_money.autorepair_money = this._autorepair_money;
         }
         this.totalScore.visible = true;
         this.buyTreasureMaps.visible = true;
         this.searchIcon.visible = this.treasureItem.mouseEnabled;
         _cache = cache;
         if(this._cache || value.allow_drop_card == false)
         {
            this.treasureItem.alpha = 0.5;
            this.treasureItem.mouseEnabled = false;
            this.treasureItem.mouseChildren = false;
         }
         else
         {
            this.treasureItem.alpha = 1;
         }
         this.treasureItem.visible = true;
         this.droppedItem.visible = true;
         this.treasureTextfield.visible = true;
         this.buyTreasureMaps.visible = true;
         this.txAlly.visible = true;
         this.txEnemy.visible = true;
         this.iconsTeam1.visible = true;
         this.iconsTeam2.visible = true;
         this.treasureCount.visible = true;
         this.spriteTreasure.visible = true;
         this.spriteArrow.visible = true;
         if(!parent)
         {
            trace("setDataVS::failed");
            return;
         }
         trace("setDataVS",this.treasureItem.alpha,value.allow_drop_card);
         _data = value;
         _victory = value.match_result > MatchResult.DEFEAT_MAX;
         _match_id = value.match_id;
         if(_cardsDone[this._match_id])
         {
            this.greyCard();
         }
         _rewardsMultiplied = value.is_bonus_match;
         this.players_list.set_local_name(PlayerInfo.nickname);
         FocusManager.setModalClip(this);
         MainScene.activeWindow = this;
         this.syncImage.visible = false;
         this.syncImage.loading.playAnimation(false);
         this.score_container.visible = true;
         this.dropChanceLabel.visible = true;
         this.dropChanceValueLabel.visible = true;
         this.percentSprite.visible = true;
         this.updateScoreComponents(this._data.score_components);
         this.updatePremiumState();
         trace("best_players_container",this.best_players_container);
         var posX:uint = 0;
         var step:uint = 255;
         var startX:uint = 0;
         if(value.title_0_player)
         {
            set_title_value(this.best_players_container.title0,TITLE_ICON_BEST_PLAYER,value.title_0_player,value.title_0_value_0);
            this.best_players_container.title0.x = posX + this.best_players_container.title0.shift;
            posX += this.best_players_container.title0.size + this.best_players_container.title0.shift;
            this.best_players_container.title0.visible = true;
         }
         else
         {
            this.best_players_container.title0.x = startX;
            this.best_players_container.title0.visible = false;
         }
         if(value.title_1_player)
         {
            set_title_value(this.best_players_container.title1,TITLE_ICON_SHOOTER,value.title_1_player,value.title_1_value_0,value.title_1_value_1);
            this.best_players_container.title1.x = posX + this.best_players_container.title1.shift;
            posX += this.best_players_container.title1.size + this.best_players_container.title1.shift;
            this.best_players_container.title1.visible = true;
         }
         else
         {
            this.best_players_container.title1.x = startX;
            this.best_players_container.title1.visible = false;
         }
         if(value.title_2_player)
         {
            set_title_value(this.best_players_container.title2,TITLE_ICON_SUPPORT,value.title_2_player,value.title_2_value_0,value.title_2_value_1);
            this.best_players_container.title2.x = posX + this.best_players_container.title2.shift;
            posX += this.best_players_container.title2.size + this.best_players_container.title2.shift;
            this.best_players_container.title2.visible = true;
         }
         else
         {
            this.best_players_container.title2.x = startX;
            this.best_players_container.title2.visible = false;
         }
         if(value.title_3_player)
         {
            set_title_value(this.best_players_container.title3,TITLE_ICON_EXPLORER,value.title_3_player,value.title_3_value_0,value.title_3_value_1);
            this.best_players_container.title3.x = posX + this.best_players_container.title3.shift;
            posX += this.best_players_container.title3.size + this.best_players_container.title3.shift;
            this.best_players_container.title3.visible = true;
         }
         else
         {
            this.best_players_container.title3.x = startX;
            this.best_players_container.title3.visible = false;
         }
         this.best_players_container.x = (1199 - posX) / 2 + 30;
         this.best_players_container.visible = true;
         trace("victoryPlate");
         this.victoryPlate.visible = true;
         var showScore:Boolean = value.show_score == true || value.show_score == undefined;
         value.result = value.team1_score + ":" + value.team2_score;
         if(showScore)
         {
            this.victoryPlate.textScore.text = value.result;
         }
         else
         {
            this.victoryPlate.textScore.text = "";
         }
         this.victoryPlate.gotoAndStop(this._victory ? 1 : 2);
         this.victoryPlate.textField.text = LocalizationData.getName(this._victory ? "victory" : "defeat");
         var showProgress:Boolean = value.game_mode == GameMode.TEAM_DEATHMATCH && value.team1_score == 0 && value.team2_score == 0;
         this.victoryPlate.team1LastPlayers.visible = this.victoryPlate.team2LastPlayers.visible = showProgress;
         trace("matchResult::",showProgress,value.game_mode == GameMode.TEAM_DEATHMATCH,value.team1_score,value.team2_score);
         if(showProgress == true)
         {
            this.victoryPlate.team1LastPlayers.rendererStep = 20;
            this.victoryPlate.team1LastPlayers.maximum = value.team1_param;
            this.victoryPlate.team1LastPlayers.value = value.team_1_players_count;
            this.victoryPlate.team2LastPlayers.rendererStep = 20;
            this.victoryPlate.team2LastPlayers.maximum = value.team2_param;
            this.victoryPlate.team2LastPlayers.value = value.team_2_players_count;
         }
         this.animateLayout();
         if(value.players)
         {
            this.players_list.addPlayersList(value.players);
         }
         if(value.dropped_item)
         {
            this.setMatchItemsDrop(value.dropped_item);
         }
         else
         {
            this.setMatchItemsDrop(null);
         }
      }
      
      protected function handleItem(target:MovieClip, data:Object) : void
      {
      }
      
      public function setDropItem(value:ItemData) : void
      {
         // The card's item: its match takes no other card.
         var matchId:uint = this._cardAsked != 0 ? this._cardAsked : this._match_id;
         if(matchId != 0)
         {
            _cardsDone[matchId] = true;
         }
         _cardAsked = 0;
         this.treasureItem.alpha = 1;
         this.treasureItem.itemSlot.data = value;
         this.treasureItem.mouseEnabled = true;
         this.treasureItem.mouseChildren = this.treasureItem.mouseEnabled;
         this.searchIcon.visible = false;
         this.progressIndicator.gotoAndStop(1);
      }
      
      private function onListIndexChange(e:ListEvent) : void
      {
         trace("onListIndexChange::",e.itemData,e.target,e.currentTarget);
         this.reguestPlayerInfo(e.itemData);
      }
      
      private function reguestPlayerInfo(rendererData:Object) : void
      {
         var playerData:Object;
         if(rendererData)
         {
            _lastName = rendererData.name;
            trace("onListIndexChange::",this._data.playersCache,this._lastName,this._data.playersCache ? this._data.playersCache[this._lastName] : null);
            if(this._data && this._data.playersCache && this._data.playersCache[this._lastName])
            {
               playerData = this._data.playersCache[this._lastName];
               trace("items_data_is_chached::id",playerData.id,playerData.name,"|",this.getSelectedName());
               dispatchEvent(new Event(Events.SHOW_PLAYER_INFO));
            }
            else
            {
               trace("items_data_is::id",rendererData.id,rendererData.name,"|",this.getSelectedName());
               ExternalInterface.call("victory_screen_request_player_data",uint(rendererData.id),this._match_id);
            }
         }
      }
      
      private function animateLayout() : void
      {
         for(var i:* = 0; i < this.best_players_container.numChildren; i++)
         {
            var chd:* = this.best_players_container.getChildAt(i);
            if(chd.visible)
            {
               chd.alpha = 0;
               var tween:* = new Tween(500,chd,{alpha:1},{delay:i * 50});
            }
         }
         for(i = 0; i < this.score_container.numChildren; i++)
         {
            chd = this.score_container.getChildAt(i);
            if(chd.visible)
            {
               chd.alpha = 0;
               tween = new Tween(500,chd,{alpha:1},{delay:i * 50});
            }
         }
      }
   }
}

