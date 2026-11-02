package
{
   import com.greensock.TweenLite;
   import flash.display.Bitmap;
   import flash.utils.Dictionary;
   import flash.geom.Matrix;
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.InteractiveObject;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Point;
   import flash.text.Font;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import flash.text.TextFormat;
   import flash.ui.Keyboard;
   import flash.utils.Timer;
   import flash.utils.clearTimeout;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import flash.utils.setTimeout;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.layout.Layout;
   import scaleform.clik.layout.LayoutData;
   import scaleform.clik.managers.DragManager;
   import scaleform.clik.managers.PopUpManager;
   import scaleform.clik.ui.InputDetails;
   import scaleform.gfx.Extensions;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.InteractiveObjectEx;
   import scaleform.gfx.MouseCursorEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.data.Relocation;
   import survarium.Util;
   import survarium.ammo.AmmoShareControll;
   import survarium.ammo.AmmoShareElement;
   import survarium.ammo.AmmoShareLite;
   import survarium.buttons.TabToggleButton;
   import survarium.char.CharWindow;
   import survarium.chat.LobbyChat;
   import survarium.clan.ClanWindow;
   import survarium.clan.TournamentWindow;
   import survarium.constants.ChatChannel;
   import survarium.constants.ClanState;
   import survarium.constants.ClanTeamRole;
   import survarium.constants.Cursor;
   import survarium.constants.Events;
   import survarium.constants.Faction;
   import survarium.constants.FriendStatus;
   import survarium.constants.GameMode;
   import survarium.constants.ImportantMessageType;
   import survarium.constants.InventoryTab;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemDictId;
   import survarium.constants.ItemPlace;
   import survarium.constants.KeyCode;
   import survarium.constants.MatchMakingState;
   import survarium.constants.MessageId;
   import survarium.constants.ModifierType;
   import survarium.constants.MoneyType;
   import survarium.constants.NotificationType;
   import survarium.constants.PlayButtonState;
   import survarium.constants.PremiumState;
   import survarium.constants.ProfileSlot;
   import survarium.constants.ProfileState;
   import survarium.constants.QuestStatus;
   import survarium.constants.QuickSlotSize;
   import survarium.constants.Reward;
   import survarium.constants.SuperCategory;
   import survarium.constants.TeamSide;
   import survarium.constants.UiSound;
   import survarium.constants.VoucherType;
   import survarium.controls.BottomPanel;
   import survarium.controls.DragRegion;
   import survarium.controls.Transient;
   import survarium.controls.GangShop;
   import survarium.controls.InventoryList;
   import survarium.controls.ItemCompare;
   import survarium.controls.ItemInfoRenderer;
   import survarium.controls.Message;
   import survarium.controls.Modals;
   import survarium.controls.NotificationList;
   import survarium.controls.NotificationListScrollContent;
   import survarium.controls.PlayerInfo;
   import survarium.controls.PlayerRanking;
   import survarium.lobbypick.LobbyWindow;
   import survarium.controls.QuestWindow;
   import survarium.controls.SrvContextMenu;
   import survarium.controls.StatusPanel;
   import survarium.controls.TopMenu;
   import survarium.controls.VictoryScreen;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.friends.FriendsMenu;
   import survarium.friends.ChatMute;
   import survarium.friends.SquadInviteList;
   import survarium.friends.SquadList;
   import survarium.gsearch.LobbyGameLoad;
   import survarium.gsearch.MatchMakingSettings;
   import survarium.interfaces.IItemFactory;
   import survarium.interfaces.IPaperDollSlot;
   import survarium.interfaces.IPlayerProfile;
   import survarium.item.InventoryItem;
   import survarium.item.ItemFactory;
   import survarium.item.ModificationRolls;
   import survarium.managers.SubsystemManager;
   import survarium.messages.MessageAnnounce;
   import survarium.messages.MessageApply;
   import survarium.messages.MessageApplyExternal;
   import survarium.messages.MessageAsk;
   import survarium.messages.MessageAskRelocate;
   import survarium.messages.MessageAskSell;
   import survarium.messages.MessageBox;
   import survarium.messages.MessageBase;
   import survarium.messages.MessageCraftItem;
   import survarium.messages.MessageCreateChar;
   import survarium.messages.MessageExit;
   import survarium.messages.MessageNoClose;
   import survarium.messages.MessagePostPremium;
   import survarium.messages.MessageQuestInfo;
   import survarium.paperdoll.PaperDollBase;
   import survarium.player.LocalPlayerProfile;
   import survarium.player.WeightEffects;
   import survarium.player.KitModifiers;
   import survarium.player.ProfileSelector;
   import survarium.quest.QuestList;
   import survarium.rooms.RoomsWindow;
   import survarium.upgrade.UpgradeWindow;
   
   public class LobbyMenu extends MovieClip
   {
      
      public static var scaleFactor:Number = 1;
      
      public static var profileCost:uint;
      
      public static var id:LobbyMenu;
      
      
      public static const INVENTORY_ELEMNENT_ID:uint = 2;
      
      public static const SHOP_ELEMNENT_ID:uint = 1;
      
      public static const PAPERDOLL_ELEMNENT_ID:uint = 0;
      
      public static const PLAYER_ELEMNENT_ID:uint = 3;
      
      public static const CHARWINDOW_ELEMNENT_ID:uint = 4;

      // "Show player's information" outside the results screen (srp's own;
      // the owner's of 2026-10-20: "you should be able to go to rankings and
      // rmb to do the same", "for every single player"). A right-click on a
      // player opens the game's right-click menu (SrvContextMenu, as the
      // results screen's) with the game's own "Show player's information";
      // it asks as the results screen's double-click on a row does
      // (`victory_screen_request_player_data`, `3D 00 <player> <match>`),
      // with a player no results row has (as srp's History asks for a
      // replay), and the answer (`93`, fill_victory_screen_player) opens the
      // game's own window (show_player_info), as the results screen's
      // double-click does. The server answers from the account as it is now
      // (servers/lobby-server/src/v026g/gear.rs; docs/notes/player_gear.md).
      //
      // The request's `player` (`3D 00 <how> <player>`): its `match` is then
      // the place of a row of the ranking (1 the first; srp's
      // lobby::GEAR_BY_PLACE), an account's id (lobby::GEAR_BY_ACCOUNT, the
      // Rooms list's members) or a place of the asker's room with its check
      // (lobby::GEAR_IN_ROOM, the room's window).
      public static const PLAYER_INFO_BY_PLACE:uint = 0xFFFFFF03;

      public static const PLAYER_INFO_BY_ACCOUNT:uint = 0xFFFFFF02;

      public static const PLAYER_INFO_BY_ROOM_PLACE:uint = 0xFFFFFF04;

      // The server answers at once or never (no such place or member): an
      // ask with no answer by then is taken as unanswered.
      private static const PLAYER_INFO_WAIT_MS:int = 3000;

      // set_admin_message's type: by the code, 0 shows the text as a warning (set_warning_text), any other
      // as an admin text; the exe never calls it.
      private static const ADMIN_MESSAGE_WARNING:int = 0;

      // set_autobuy's value: bit 0 the autobuy box, bit 1 the auto-repair box (gone: an account
      // that had it ticked still sends 3).
      private static const AUTO_BUY:int = 1;

      private static const AUTO_BUY_AND_REPAIR:int = 3;

      // meaning unknown: the progress show_daily_reward_window takes a day's reward to need.
      private static const DAILY_REWARD_PROGRESS_REQUIRED:int = 3;

      // ui_properties prop_id (gameplay/static_game_parameters) the test items use: st_inv_arp and
      // st_inv_item_weight.
      private static const PROP_ARMOR_PIERCING:int = 3;

      private static const PROP_ITEM_WEIGHT:int = 15;

      // Ids of the test functions' messages and chat tabs (no meaning beyond the tests).
      private static const TEST_INVITE_ID:int = 23;

      // FriendListRender does not answer a message of this id to the exe.
      public static const TEST_IMPORTANT_MESSAGE_ID:int = 666;

      private static const TEST_CHAT_TAB_ID:int = 900;

      // the tab test_chat_messsage_second adds (TEST_CHAT_TAB_ID + 4)
      private static const TEST_CHAT_TAB_SECOND:int = 904;

      // The ask in flight: its name and when it was sent. One at a time: a
      // `93` carries nothing of the player it is about, so an answer is ours
      // only as the answer to the one ask in flight.
      private static var _playerInfoName:String;

      private static var _playerInfoAt:int;

      // The newest ask made while one was in flight, as [nickname, player,
      // how]: sent when that one is answered (its answer, now stale, is
      // dropped) or has waited PLAYER_INFO_WAIT_MS.
      private static var _playerInfoNext:Array;

      private static var _playerInfoWait:Timer;
      
      public var item_control:SrvSelectedItemControl;
      
      public var bottom_panel:BottomPanel;
      
      public var _itemDescriptor:ItemDescriptor;
      
      public var try_relocate_item:Function;
      
      public var shop_ready:Function;
      
      public var try_relocate_perk:Function;
      
      public var topMenu:TopMenu;
      
      public var ammoShare:AmmoShareLite;
      
      public var statusPanel:StatusPanel;
      
      public var inventory_list:InventoryList;
      
      public var shop_list:GangShop;
      
      public var lobbyGameLoad:LobbyGameLoad;
      
      public var ranking:PlayerRanking;
      
      public var chat:LobbyChat;
      
      public var victory_screen:VictoryScreen;
      
      public var friendsMenu:FriendsMenu;
      
      public var player_profile:LocalPlayerProfile;
      
      public var charWindow:CharWindow;
      
      public var messagesBar:TileList;
      
      public var questWindow:QuestWindow;
      
      public var modifierWindow:UpgradeWindow;
      
      public var playerLoadingIndicator:MovieClip;
      
      public var invitesContainer:SquadInviteList;
      
      public var clanWindow:ClanWindow;
      
      public var squadList:SquadList;
      
      public var questList:QuestList;
      
      public var notificationList:NotificationListScrollContent;
      
      public var notificationListTemp:NotificationList;
      
      public var profileSelector:ProfileSelector;
      
      public var dragRegion:DragRegion;
      
      public var itemHintRenderer:ItemInfoRenderer = new InventoryItemHint();
      
      public var itemHintRendererSec:ItemInfoRenderer = new InventoryItemHint();
      
      public var tipContainer:Sprite;
      
      protected var _itemFactory:IItemFactory = new ItemFactory();
      
      protected var _lasm:String = "";
      
      protected var _tm:int = -1;
      
      protected var _back_sprites:MovieClip;
      
      protected var _top_container:MovieClip = new MovieClip();
      
      protected var _top2_container:MovieClip = new MovieClip();
      
      protected var _front_container:MovieClip = new MovieClip();
      
      protected var _importantMessages:Array = new Array();
      
      protected var _reports:Array = new Array();
      
      protected var _quests:Array = new Array();
      
      protected var _back_back:Sprite = new BackBlack();
      
      protected var _time:int = 0;
      
      protected var _layout:Layout;
      
      protected var _iconContainer_profile:IconContainer;
      
      protected var _equipment_i:IconContainer;
      
      protected var _ammo_i:IconContainer;
      
      protected var _weapons_i:IconContainer;
      
      protected var _modifiers_i:IconContainer;
      
      protected var _ammoControl:AmmoShareControll = new AmmoShareControll();
      
      protected var _superHeight:uint = 1080;
      
      protected var _superWidth:uint = 1920;
      
      protected var _load_images:Array = [Level1_day,Level2_day,Level3_day,Level3_evn,Level3_rain,Level4_day,Level1_rain,Level1_evn,Level2_rain,Level2_evn,Level4_rain,Level4_evn,Level5_day,Level5_rain,Level5_evn,Level6_day,Level6_rain,Level6_evn,Level7_day,Level7_rain,Level7_evn,Level8_day,Level8_rain,Level8_evn,Level8_night,Level8_prerain,Level9_day,Level9_evn,Level9_rain,Level_v011e,Level_v069d_rudnya];
      
      private var spriteHighlightContainer:MovieClip = new MovieClip();
      
      private var highlightArray:Array = new Array();
      
      private var _removedIndex:uint = 0;
      
      private var _inventoryHighlight:Boolean = false;
      
      private var _pdTarget:MovieClip;
      
      private var _wheelBack:MovieClip = new FadeBackground();
      
      private var _wheelsMC:MovieClip = new LoadingWheelsShared();
      
      private var _craftData:Object;
      
      protected var _ignoreMM:Boolean = false;
      
      protected var _messageShowed:Boolean = false;
      
      private var _tournament:TournamentManagment = new TournamentManagment();
      
      private var backWhite:MovieClip = new BackWhite();
      
      private var craftWindow:MessageCraftItem;
      
      protected var _invitesTemp:uint = 0;
      
      protected var _playerRequestTarget:MovieClip;
      
      private var _premiumStoreDiscount:Array = [];
      
      protected var lastClick:Number = 0;
      
      protected var messageIndex:uint = 0;
      
      protected var messageTimeOut:int = -1;
      
      protected var _playerProfiles:Array = [];
      
      protected var _profileItems:Object;

      // the upgrade window told the game (start_item_upgrade), so its close does too
      protected var _upgradeShown:Boolean = false;

      // the upgrade window was opened from the shop: its list shows the inventory while it is open,
      // the shop again after (shop-character-menu.md)
      protected var _upgradeFromShop:Boolean = false;

      // the craft window (Upgrade) likewise: its list shows the owned donors while it is open
      protected var _craftFromShop:Boolean = false;

      protected var backBlack:Sprite = new BackBlack();
      
      private var _dailyBonusData:Object = null;
      
      protected var _friendInvites:Array = [];
      
      protected var _messageIndex:int = 0;
      
      protected var _unlockData:Object;
      
      protected var _friends:Array;
      
      protected var _isWheeled:Boolean = false;
      
      private var fadeBack:MovieClip = new FadeBackground();

      // The black over the lobby's 3D scene while the exe puts another lobby in (srp_scene_fade),
      // and the loading spinner on it.
      private var _sceneFade:Sprite;

      private var _sceneSpinner:MovieClip;

      // The lobby window (the lobby button's) and what the exe last told it, [pick, shown].
      private var _lobbyWindow:LobbyWindow;

      private var _lobbyState:Array;
      
      protected var _tempIndex:uint = 0;
      
      private var _postRewardData:Object;
      
      private var _discountTimer:Timer;
      
      private var toggle:Boolean = false;
      
      private var toggleIndex:int = 0;
      
      protected var _escapeHandled:Boolean = false;

      // The interface is this movie's root, every layer of it (docs/notes/hide-ui-key.md): the panels,
      // bars, windows and chat are the root's children, and what the game first put on the stage is
      // in _stageLayer, the root's last child, drawn at the stage's scale: _layers (every layer put
      // above the lobby: the front, top and top2 containers with messages, the wheels, the loading
      // screen, the fades, the right-click menu), over them _transients (the transient windows
      // while open, docs/notes/transient-windows.md, and CLIK's popups) and over it _overlay, above
      // every modal: _corner (the exit and settings buttons while the exe wants them above all, drawn as
      // the status panel is), _cards (the item cards, drawn as the root is), CLIK's drag canvas and
      // the tooltips. toggle_interface hides the root, so all of it, in one step; nothing of the
      // interface is on the stage beside it.
      private var _stageLayer:Sprite;

      private var _layers:Sprite;

      private var _transients:Sprite;

      private var _overlay:Sprite;

      private var _corner:Sprite;

      private var _cards:Sprite;

      // where enable_element took a corner button from: its index in the status panel
      private var _cornerHome:Dictionary = new Dictionary(true);

      // Hidden by toggle_interface (exe patch 62, F11).
      private var _interfaceHidden:Boolean;

      // The key that showed the hidden interface: its release is taken too (onHiddenKeyUp); 0 when
      // none.
      private var _hiddenKeyUp:uint;
      
      public function LobbyMenu()
      {
         super();
         trace("LobbyMenu::super");
         id = this;
         MainScene.stage = this;
         this.makeStageLayer();
         this.init();
         this.initSubsystems();
         this._itemFactory.registerItemDescriptor(this._itemDescriptor);
         var rewardIcons:Object = new Object();
         var rewardExperience:Icon = new Icon();
         rewardExperience.loadImage(new Bitmap(new ExperienceBitmapData()));
         rewardIcons[Reward.EXPERIENCE] = rewardExperience;
         (this._itemFactory as ItemFactory).setRewardIcons(rewardIcons);
         this.setlayout();
         stage.stageFocusRect = false;
         stage.focusRect = null;
         var timer:Timer = MainScene.getTimerInstance();
         timer.delay = 1000 * 60;
         timer.start();
         this.tipContainer = new Sprite();
         this._overlay.addChild(this.tipContainer);
         ToolTip.tipContainer = this.tipContainer;
      }

      // The layers the game first put on the stage, in the root (see _stageLayer): made before
      // anything is added; CLIK's popup canvas, the right-click menu and the transient windows are
      // told where theirs go.
      private function makeStageLayer() : void
      {
         _stageLayer = new Sprite();
         this._stageLayer.name = "stageLayer";
         _layers = new Sprite();
         _transients = new Sprite();
         _overlay = new Sprite();
         _corner = new Sprite();
         _cards = new Sprite();
         itemHintRenderer.addEventListener(Event.RESIZE,onWeaponDetailsResize);
         itemHintRendererSec.addEventListener(Event.RESIZE,onWeaponDetailsResize);
         this._stageLayer.addChild(this._layers);
         this._stageLayer.addChild(this._transients);
         this._stageLayer.addChild(this._overlay);
         this._overlay.addChild(this._corner);
         this._overlay.addChild(this._cards);
         super.addChild(this._stageLayer);
         PopUpManager.layer = this._transients;
         Transient.layer = this._transients;
      }

      // _stageLayer stays the root's last child, whatever is added or moved to the top later.
      override public function addChild(child:DisplayObject) : DisplayObject
      {
         super.addChild(child);
         this.keepStageLayerOnTop();
         return child;
      }

      override public function addChildAt(child:DisplayObject, index:int) : DisplayObject
      {
         super.addChildAt(child,index);
         this.keepStageLayerOnTop();
         return child;
      }

      override public function setChildIndex(child:DisplayObject, index:int) : void
      {
         super.setChildIndex(child,index);
         this.keepStageLayerOnTop();
      }

      private function onStatusPanelLaidOut(e:Event) : void
      {
         this.placeOverlay();
      }

      // _corner is drawn as the status panel, _cards as the root: their coordinates are those.
      private function placeOverlay() : void
      {
         this._cards.scaleX = this._cards.scaleY = scaleX;
         if(this.statusPanel)
         {
            var place:Matrix = this.statusPanel.transform.matrix.clone();
            place.scale(scaleX,scaleY);
            this._corner.transform.matrix = place;
         }
      }

      private function keepStageLayerOnTop() : void
      {
         if(this._stageLayer != null && this._stageLayer.parent == this && getChildIndex(this._stageLayer) != numChildren - 1)
         {
            super.setChildIndex(this._stageLayer,numChildren - 1);
         }
      }
      
      public static function get stageWidth() : Number
      {
         return id.stage.stageWidth / id.scaleX;
      }
      
      public static function get stageHeight() : Number
      {
         return id.stage.stageHeight / id.scaleY;
      }
      
      public static function getInstance() : LobbyMenu
      {
         return id;
      }
      
      // The tutorial's notes link: the exe hides it at each match's loading (0x220400).
      public function show_notes_tutorial_link(bool:Boolean, index:uint) : void
      {
      }
      
      public function setTDMEnabled(value:Boolean) : void
      {
      }
      
      public function setBatteryEnabled(value:Boolean) : void
      {
      }
      
      public function remove_highlight(value:*) : void
      {
         var target:MovieClip = value as MovieClip;
         trace("remove_highlight",target);
         if(!target)
         {
            return;
         }
         if(target is AmmoShareElement)
         {
            (target as MovieClip).slider.hoverCells(false);
         }
         this.setTopmostLevel(target,false);
         var spriteHighlight:MovieClip = (target as MovieClip).highlightRender;
         trace("spriteHighlight",spriteHighlight);
         if((spriteHighlight) && this.spriteHighlightContainer.contains(spriteHighlight))
         {
            this.spriteHighlightContainer.removeChild(spriteHighlight);
         }
      }
      
      // The exe calls this at every entry of the lobby's state (0x2A3EA0, lobby_menu_state's) with
      // the exit and settings buttons (root.statusPanel.btn_off, btn_settings) and true, and at a
      // match's loading (0x220400) with false; its other callers are the tutorial's states, which
      // are no longer entered (no-tutorial.md). The stock movie put them on Scaleform's topmost
      // level (above every modal: a message's dim, the results, the wheels; drawn whatever their
      // parents' visibility). Here the two buttons go to _corner, above every modal and inside the
      // root, drawn where the status panel draws them (placeOverlay), and back into the status panel
      // with false; anything else is the tutorial's, ignored.
      public function enable_element(value:*, value2:Boolean = true) : void
      {
         var button:DisplayObject = value as DisplayObject;
         if(button == null || !this.statusPanel || button != this.statusPanel.btn_off && button != this.statusPanel.btn_settings)
         {
            return;
         }
         if(value2 && button.parent == this.statusPanel)
         {
            this._cornerHome[button] = this.statusPanel.getChildIndex(button);
            this.placeOverlay();
            this._corner.addChild(button);
         }
         else if(!value2 && button.parent == this._corner)
         {
            this.statusPanel.addChildAt(button,Math.min(int(this._cornerHome[button]),this.statusPanel.numChildren));
         }
      }

      public function highlight_element(value:*, value2:Boolean = true) : void
      {
         var target:InteractiveObject;
         var spriteHighlight:MovieClip;
         var origin:Point;
         var point:Point;
         var xOffset:int;
         var yOffset:int;
         var sizeOffset:int;
         trace("highlight_element",value,value2);
         if(value2)
         {
            target = value as InteractiveObject;
            spriteHighlight = new SpriteHighlight();
            origin = new Point(target.x,target.y);
            point = target.parent.localToGlobal(origin);
            trace("highlight_element",point,origin);
            xOffset = -6;
            yOffset = -6;
            sizeOffset = 6;
            if(target is TabToggleButton)
            {
               xOffset = 8;
               yOffset = 6;
               sizeOffset = -3;
            }
            else if(target is InventoryItem)
            {
               sizeOffset = 3;
            }
            else if(target is AmmoShareElement)
            {
               sizeOffset = 5;
               xOffset = -4;
               target.addEventListener(Event.RENDER,this.onRendererUpdate);
               target.addEventListener(Event.REMOVED_FROM_STAGE,this.onRendererRemoved);
               (target as MovieClip).slider.hoverCells(true);
            }
            else if(target is QuestWindow)
            {
               spriteHighlight.alpha = 0.5;
            }
            else if(target === this.player_profile.btnSetAmmo)
            {
               target.addEventListener(Event.RENDER,this.onRendererUpdate);
            }
            (target as MovieClip).highlightRender = spriteHighlight;
            this.setTopmostLevel(target,true);
            spriteHighlight.x = xOffset + point.x / MainScene.scaleFactor;
            spriteHighlight.y = yOffset + point.y / MainScene.scaleFactor;
            this.spriteHighlightContainer.addChild(spriteHighlight);
            spriteHighlight.width = target.width + sizeOffset * 2;
            spriteHighlight.height = target.height + sizeOffset * 2;
            spriteHighlight.mouseEnabled = false;
            spriteHighlight.mouseChildren = false;
            addEventListener(Event.RENDER,this.onStageUpdate);
            this.highlightArray.push(value);
         }
         else
         {
            this.remove_highlight(value);
         }
      }
      
      private function onRendererRemoved(e:Event) : void
      {
         var target:AmmoShareElement = e.target as AmmoShareElement;
         this.setTopmostLevel(target,false);
         target.removeEventListener(Event.REMOVED_FROM_STAGE,this.onRendererRemoved);
         _removedIndex = target.arrayIndex;
         var spriteHighlight:MovieClip = (target as MovieClip).highlightRender;
         if((spriteHighlight) && this.spriteHighlightContainer.contains(spriteHighlight))
         {
            this.spriteHighlightContainer.removeChild(spriteHighlight);
         }
         addEventListener(Event.RENDER,this.onStageUpdateAfterRemoved);
         addEventListener(Event.RENDER,this.onStageUpdate);
      }
      
      private function onStageUpdateAfterRemoved(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.onStageUpdateAfterRemoved);
         this.highlight_element(this.ammoShare.getAmmoRenderer(this._removedIndex));
      }
      
      private function onRendererUpdate(e:Event) : void
      {
         var target:MovieClip = e.target as MovieClip;
         var spriteHighlight:MovieClip = target.highlightRender;
         var xOffset:int = -6;
         var yOffset:int = -6;
         var sizeOffset:int = 6;
         var origin:Point = new Point(target.x,target.y);
         var point:Point = target.parent.localToGlobal(origin);
         if(target is AmmoShareElement)
         {
            sizeOffset = 0;
            xOffset = -6;
         }
         spriteHighlight.x = xOffset + point.x / MainScene.scaleFactor;
         spriteHighlight.y = yOffset + point.y / MainScene.scaleFactor;
      }
      
      public function highlight_inventory_items(value:Array) : void
      {
         this.inventory_list.filterItems(value);
         this.inventory_list.ignoreSelection = true;
         _inventoryHighlight = true;
         for(var i:uint = 0; i < value.length; i++)
         {
            this.highlight_element(this.inventory_list.getItemRenderer(value[i]),true);
         }
         addEventListener(Event.RENDER,this.onStageUpdate);
      }
      
      private function onStageUpdate(e:Event) : void
      {
         removeEventListener(e.type,this.onStageUpdate);
         this.setTopmostLevel(this.spriteHighlightContainer,true);
         this.setTopmostLevel(DragManager.getDragCanvas(),true);
         this.setTopmostLevel(this.tipContainer,true);
         trace("stageUpdated");
         InteractiveObjectEx.setHitTestDisable(this.spriteHighlightContainer,true);
      }
      
      public function clean_highlight() : void
      {
         while((this.spriteHighlightContainer.numChildren))
         {
            this.spriteHighlightContainer.removeChildAt(0);
         }
         for(var i:uint = 0; i < this.highlightArray.length; i++)
         {
            var renderer:MovieClip;
            renderer = this.highlightArray[i];
            this.setTopmostLevel(renderer,false);
            renderer.removeEventListener(Event.RENDER,this.onRendererUpdate);
            if(renderer is AmmoShareElement)
            {
               (renderer as MovieClip).slider.hoverCells(false);
               renderer.removeEventListener(Event.REMOVED_FROM_STAGE,this.onRendererRemoved);
            }
         }
         this.highlightArray.length = 0;
         this.setTopmostLevel(this.spriteHighlightContainer,false);
         this.setTopmostLevel(DragManager.getDragCanvas(),false);
         this.setTopmostLevel(this.tipContainer,false);
         if(this._inventoryHighlight)
         {
            _inventoryHighlight = false;
         }
      }
      
      public function remove_items_filter() : void
      {
         this.inventory_list.filterItems(null);
         this.inventory_list.ignoreSelection = false;
      }
      
      private function setTopmostLevel(target:*, value:Boolean) : void
      {
         trace("setTopmostLevel::",target.name,target,value);
         InteractiveObjectEx.setTopmostLevel(target,value);
      }
      
      // The exe's default state (0x2A4240) calls it at each sign-in: no slide is open, the sound stays.
      public function close_slide() : void
      {
         ExternalInterface.call("sound_play",UiSound.TUTORIAL_SUCCESS);
      }
      
      // The exe gives the tutorial's awards at each 9D (0x283190); nothing shows them.
      public function set_tutorial_rewards(value:Array) : void
      {
      }
      
      public function show_video_tutorial_link(bool:Boolean) : void
      {
         this.statusPanel.showVideoTutorialLink(bool);
      }
      
      private function initSubsystems() : void
      {
         this.charWindow.subsystem_name = "SkillTree";
         this.inventory_list.subsystem_name = "Inventory";
         this.inventory_list.shopSubsystem.subsystem_name = "Shop";
         this.clanWindow.subsystem_name = "Clan";
         this.ranking.subsystem_name = "Rating";
         this.questWindow.subsystem_name = "Quests";
         SubsystemManager.addSubsystem("SkillTree",this.charWindow);
         SubsystemManager.addSubsystem("Inventory",this.inventory_list);
         SubsystemManager.addSubsystem("Shop",this.inventory_list.shopSubsystem as DisplayObject);
         SubsystemManager.addSubsystem("Clan",this.clanWindow);
         SubsystemManager.addSubsystem("Rating",this.ranking);
         SubsystemManager.addSubsystem("FriendList",this.friendsMenu);
         SubsystemManager.addSubsystem("Quests",this.questWindow);
         SubsystemManager.addSubsystem("PlayButton",this.statusPanel.playBtn);
         this.charWindow.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.inventory_list.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.inventory_list.shopSubsystem.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.clanWindow.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.ranking.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.questWindow.addEventListener(Events.SUBSYSTEM_ACTIVATED,this.handleTopWindowActivated);
         this.charWindow.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.inventory_list.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.inventory_list.shopSubsystem.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.clanWindow.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.ranking.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.questWindow.addEventListener(Events.SUBSYSTEM_DEACTIVATED,this.handleTopWindowDeactivated);
         this.charWindow.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.inventory_list.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.inventory_list.shopSubsystem.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.clanWindow.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.ranking.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.questWindow.addEventListener(Events.SUBSYSTEM_ENABLED,this.handleTopWindowEnabled);
         this.charWindow.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
         this.inventory_list.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
         this.inventory_list.shopSubsystem.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
         this.clanWindow.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
         this.ranking.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
         this.questWindow.addEventListener(Events.SUBSYSTEM_DISABLED,this.handleTopWindowDisabled);
      }
      
      private function handleTopWindowDeactivated(e:Event) : void
      {
         trace("handleTopWindowDeactivated::",e);
         this.topMenu.selectActiveWindow(-1,false);
      }
      
      public function set_sybsystem_enabled(subsystem:String, value:Boolean) : void
      {
         SubsystemManager.setSybsystemEnabled(subsystem,value);
      }
      
      public function set_sybsystem_active(subsystem:String, value:Boolean) : void
      {
         if(subsystem == "Shop")
         {
            this.inventory_list.toggleShopMode(true);
         }
         else if(subsystem == "Inventory")
         {
            this.inventory_list.toggleShopMode(false);
         }
         trace("set_sybsystem_active::",subsystem,value);
         SubsystemManager.setSybsystemActive(subsystem,value);
      }
      
      private function getWindowIndex(value:Sprite) : int
      {
         if(value == this.charWindow)
         {
            return this.topMenu.CHAR_INDEX;
         }
         if(value == this.inventory_list)
         {
            return this.topMenu.INV_INDEX;
         }
         if(value == this.inventory_list.shopSubsystem)
         {
            return this.topMenu.SHOP_INDEX;
         }
         if(value == this.clanWindow)
         {
            return this.topMenu.CLAN_INDEX;
         }
         if(value == this.ranking)
         {
            return this.topMenu.RANK_INDEX;
         }
         if(value == this.questWindow)
         {
            return this.topMenu.QUESTS_INDEX;
         }
         return 0;
      }
      
      private function handleTopWindowEnabled(e:Event) : void
      {
         trace("handleTopWindowEnabled::",e);
         var index:int = this.getWindowIndex(e.target as Sprite);
         this.topMenu.lockIndex(index,false);
         if(this.topMenu.QUESTS_INDEX == index)
         {
            this.questList.enabled = true;
         }
      }
      
      private function handleTopWindowDisabled(e:Event) : void
      {
         trace("handleTopWindowDisabled::",e);
         var index:int = this.getWindowIndex(e.target as Sprite);
         this.topMenu.lockIndex(index,true);
         if(this.topMenu.QUESTS_INDEX == index)
         {
            this.questList.enabled = false;
         }
      }
      
      private function handleTopWindowActivated(e:Event) : void
      {
         trace("handleTopWindowActivated::",e);
         var index:int = this.getWindowIndex(e.target as Sprite);
         this.topMenu.selectActiveWindow(index,false);
      }
      
      private function init() : void
      {
         var style:StyleSheet = new StyleSheet();
         var styleObject:Object;
         styleObject = new Object();
         styleObject.color = "#b3ea97";
         style.setStyle("current",styleObject);
         styleObject = new Object();
         styleObject.color = "#FEE2B2";
         styleObject.size = 22;
         styleObject.fontSize = 22;
         style.setStyle("highlight_big",styleObject);
         styleObject = new Object();
         styleObject.color = "#079DD5";
         style.setStyle("blue",styleObject);
         styleObject = new Object();
         styleObject.color = "#D79342";
         style.setStyle("yellow",styleObject);
         styleObject = new Object();
         styleObject.color = "#5b6261";
         style.setStyle("gray",styleObject);
         styleObject = new Object();
         styleObject.color = "#F2C98F";
         style.setStyle("beige",styleObject);
         styleObject = new Object();
         styleObject.color = "#4A4C4B";
         style.setStyle("descr",styleObject);
         styleObject = new Object();
         styleObject.color = "#FEE2B2";
         style.setStyle("highlight",styleObject);
         styleObject = new Object();
         styleObject.color = "#e9b838";
         style.setStyle("gold",styleObject);
         style.setStyle(WeightEffects.WARNING_STYLE,{color:WeightEffects.WARNING_COLOR,display:"inline"});
         styleObject = new Object();
         styleObject.color = "#FEE2B2";
         style.setStyle("beige_bright",styleObject);
         styleObject = new Object();
         styleObject.color = "#5b6261";
         styleObject.size = 2;
         styleObject.fontSize = 2;
         style.setStyle("small",styleObject);
         styleObject = new Object();
         styleObject.color = "#5c7d47";
         style.setStyle("green",styleObject);
         styleObject = new Object();
         styleObject.color = "#cf4242";
         style.setStyle("red",styleObject);
         styleObject = new Object();
         styleObject.color = "#BCCBCB";
         style.setStyle("white",styleObject);
         styleObject = new Object();
         styleObject.color = "#8B8B8B";
         style.setStyle("gray_highlight",styleObject);
         styleObject = new Object();
         styleObject.color = "#FEE1B5";
         style.setStyle("tutorial_highlight",styleObject);
         MainScene.textStyleSheet = style;
         topMenu = new SrvTopMenu();
         lobbyGameLoad = new SrvLobbyGameLoad();
         ranking = new SrvPlayerRanking();
         victory_screen = new SrvVictoryScreenReskin();
         this.victory_screen.registerHintRenderer(new InventoryItemHint());
         this.victory_screen.addEventListener("try_buy_maps",this.onBuyMaps);
         this.victory_screen.addEventListener(Events.SHOW_PLAYER_INFO,this.onVSPlayerInfo);
         friendsMenu = new SrvFriendsMenuReskin();
         clanWindow = new SrvClanWindow();
         item_control = new SrvSelectedItemControl();
         dragRegion = new SrvDragRegion();
         bottom_panel = new SrvBottomPanel();
         invitesContainer = new InviteListContainer();
         charWindow = new SrvCharWindow();
         this.charWindow.registerRenderer(SrvSkillRendererCh);
         player_profile = new SrvLocalPlayerProfile();
         ammoShare = AmmoShareLite.getInstance();
         this.ammoShare.registerRenderer(SrvAmmoElement);
         this.player_profile.setAmmoShare(this.ammoShare);
         profileSelector = new SrvProfileSelector();
         this.profileSelector.tabPage.addEventListener(MouseEvent.CLICK,this.onProfileContextMenu);
         this.profileSelector.tabPage.addEventListener(Events.RENAME,this.onProfileRenameClicked);
         this.player_profile.addChild(this.profileSelector);
         this.profileSelector.x = 9;
         this.profileSelector.y = 42;
         modifierWindow = new SrvModifiersControlReskin();
         notificationList = new SrvNotificationListSC();
         this.notificationList.lobby = this;
         notificationListTemp = new SrvNotificationList();
         this.notificationList.registerRenderer(NotificationType.PLAIN,NotificationRendererSkinned);
         this.notificationList.registerRenderer(NotificationType.QUEST,NotificationQuestRendererSkinned);
         this.notificationList.registerRenderer(NotificationType.INVITED_REWARD,NotificationInvitedRewardRenderer);
         this.notificationList.registerRenderer(NotificationType.MATCH,NotificationMatchRenderer);
         this.notificationList.registerRenderer(NotificationType.PREMIUM,NotificationPremDiscount);
         this.notificationList.registerRenderer(NotificationType.FACTION_RANK,FactionRankNotif);
         this.notificationList.registerRenderer(NotificationType.PLAYER_RANK,PlayerRankNotif);
         this.notificationList.visible = false;
         this.notificationListTemp.registerRenderer(NotificationType.PLAIN,NotificationRendererSkinned);
         this.notificationListTemp.registerRenderer(NotificationType.QUEST,NotificationQuestRendererSkinned);
         this.notificationListTemp.registerRenderer(NotificationType.INVITED_REWARD,NotificationInvitedRewardRenderer);
         this.notificationListTemp.registerRenderer(NotificationType.MATCH,NotificationMatchRenderer);
         this.notificationListTemp.registerRenderer(NotificationType.PREMIUM,NotificationPremDiscount);
         this.notificationListTemp.registerRenderer(NotificationType.FACTION_RANK,FactionRankNotif);
         this.notificationListTemp.registerRenderer(NotificationType.PLAYER_RANK,PlayerRankNotif);
         this.notificationListTemp.visible = true;
         chat = new SrvLobbyChat();
         inventory_list = new SrvInventoryReskin();
         this.inventory_list.name = "inventory_list";
         this.inventory_list.itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OVER,this.showItemHint);
         this.inventory_list.itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OUT,this.hideItemHint);
         this.inventory_list.addEventListener(InventoryList.HIDE_ITEM_HINT,this.hideItemHint);
         this.inventory_list.bundleList.list.addEventListener(ListEvent.ITEM_ROLL_OVER,this.showItemHint);
         this.inventory_list.bundleList.list.addEventListener(ListEvent.ITEM_ROLL_OUT,this.hideItemHint);
         this.inventory_list.addEventListener(Event.CLOSE,this.hideItemHint);
         this.inventory_list.addEventListener(Events.OPEN_MODIFIERS,this.openModifierWindow);
         this.inventory_list.addEventListener(Events.CRAFFT_ITEM,this.openCraftWindow);
         this.inventory_list.itemControlPanel.addEventListener(Events.CRAFFT_ITEM,this.openCraftWindow);
         trace("ROOT:player_profile",this.player_profile);
         this.player_profile.profileItems.addEventListener(Events.OPEN_MODIFIERS,this.openModifierWindow);
         this.player_profile.profileItems.addEventListener(ListEvent.ITEM_ROLL_OVER,this.showPdItemHint);
         this.player_profile.profileItems.addEventListener(ListEvent.ITEM_ROLL_OUT,this.hideItemHint);
         this.player_profile.profileItems.addEventListener(PaperDollBase.PAPERDOLL_UPDATED,this.updateHintItem);
         shop_list = new SrvGangShop();
         this.shop_list.name = "inventory_list";
         questWindow = new SrvQuestWindowSkinned();
         this.questWindow.addEventListener(Event.CONNECT,this.showActivedQuestList);
         this.questWindow.addEventListener(QuestWindow.GOTO_SHOP,this.showFactionInShop);
         squadList = new SrvSquadPanel();
         this.squadList.setFriendsList(this.friendsMenu);
         questList = new SrvQuestsPanel();
         playerLoadingIndicator = new LoadingIndicator();
         this.questWindow.register_external_renderer(this.questList);
         this.chat.tapPageRenderer = this.bottom_panel.tabsBar;
         this.shop_list.registerReputationsContainer(this.questWindow);
         addChild(this.dragRegion);
         this.playerLoadingIndicator.mouseEnabled = false;
         this.playerLoadingIndicator.visible = false;
         addChild(this.playerLoadingIndicator);
         addChild(this.charWindow);
         addChild(this.invitesContainer);
         addChild(this.friendsMenu);
         addChild(this.victory_screen);
         this.victory_screen.top_container = this._top_container;
         addChild(this.chat);
         addChild(this.ranking);
         addChild(this.lobbyGameLoad);
         addChild(this.shop_list);
         addChild(this.clanWindow);
         addChild(this.questList);
         addChild(this.squadList);
         addChild(this.questWindow);
         addChild(this.inventory_list);
         addChild(this.item_control);
         addChild(this.modifierWindow);
         addChild(this.notificationList);
         addChild(this.notificationListTemp);
         addChild(this.bottom_panel);
         this.questWindow.visible = false;
         this.friendsMenu.addEventListener(Event.CONNECT,this.onRequestPlayers);
         this.squadList.addEventListener(Event.CONNECT,this.onRequestPlayers);
         this.clanWindow.addEventListener(Event.CONNECT,this.onRequestPlayers);
         this._tournament.addEventListener(Event.CONNECT,this.onRequestPlayers);
         this._tournament.addEventListener(Event.CLOSE,this.onTournamentClose);
         this._tournament.addEventListener(TournamentWindow.TOGGLE_FOLD,this.onTournamentFold);
         this.player_profile.profileItems.addEventListener(ListEvent.ITEM_ROLL_OVER,this.onPaperDollItemRollOver);
         this.player_profile.profileItems.addEventListener(ListEvent.ITEM_ROLL_OUT,this.onPaperDollItemRollOut);
         this.inventory_list.itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OVER,this.onInventoryListItemRollOver);
         this.inventory_list.itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OUT,this.onInventoryListItemRollOut);
         this.player_profile.profileItems.addEventListener(Event.COMPLETE,this.updateItemCard);
         SrvContextMenu.registerStage(stage,this._layers);
         SrvContextMenu.setPatternt(new ContextPattern());
         DragManager.init(this.stage,this._overlay);
         Message.registerMessage("ask",MessageAsk);
         Message.registerMessage("repair",SrvMessageRepairSkinned);
         Message.registerMessage("askr",MessageAskRelocate);
         Message.registerMessage("premium",SrvMessagePremium);
         Message.registerMessage("post_premium",SrvMessagePremiumLost);
         Message.registerMessage("premium_discount",SrvMessagePremiumDiscount);
         Message.registerMessage("asks",MessageAskSell);
         Message.registerMessage("debug",MessageBox);
         Message.registerMessage("reroll",SrvMessageRerollSkills);
         Message.registerMessage("anno",MessageAnnounce);
         Message.registerMessage("apply",MessageApply);
         Message.registerMessage("ask_system_message",MessageApplyExternal);
         Message.registerMessage("exit",MessageExit);
         Message.registerMessage("ok",MessageBox);
         Message.registerMessage("char",MessageCreateChar);
         Message.registerMessage("no_faction",MessageNoFactionSelected);
         Message.registerMessage("exchange",SrvMessageExchangeSkinned);
         Message.registerMessage("craft",SrvMessageCraftItemSkinned);
         Message.registerMessage("rename",SrvMessageEnterName);
         Message.registerBackground(BackBlack);
         Message.registerMessage("debug",MessageBox);
         Message.registerMessage("noclose",MessageNoClose);
         Message.registerMessage("sync",SrvSyncClip);
         Message.registerMessage("quest_message",MessageQuestInfo);
         Message.registerMessage("invite",SrvMessageInvited);
         Message.registerMessage("message_unlock",MessageUnlock);
         Message.registerMessage("message_unlock_rank",SrvMessageFactionRank);
         Message.registerMessage("message_unlock_level",MessageLevelUnlocked);
         Message.registerMessage("change_name",SrvMessageEnterNamePlayer);
         Message.registerMessage("upgrade_item",SrvMessageRerollItemSkinned);
         Message.registerMessage("daily_reward",SrvMessageDailyReward);
         Message.registerMessage("player_info",SrvMessagePlayerProfile);
         this._layers.addChild(this._front_container);
         this._layers.addChild(this._top_container);
         this._layers.addChild(this._top2_container);
         Message.initStage(this._top_container);
         this.bottom_panel.btnFriends.tooltipRendererName = "SrvProfileTipRenderer";
         LocalizationData.addCallback(this.updateButtonsLocal,"btn_friends");
         this.bottom_panel.btnFriends.addEventListener(ButtonEvent.CLICK,this.toggleFriends);
         this.bottom_panel.btnShowLogList.tooltipRendererName = "SrvProfileTipRenderer";
         this.bottom_panel.btnShowLogList.toggle = true;
         this.bottom_panel.btnShowLogList.addEventListener(ButtonEvent.CLICK,this.toggleReports);
         this.friendsMenu.addEventListener(Event.CLOSE,this.onFrClose);
         this.shop_list.addEventListener(GangShop.NEW_ITEMS_COUNT_CHANGED,this.onNewItemsCountChange);
         this.inventory_list.addEventListener(GangShop.NEW_ITEMS_COUNT_CHANGED,this.inventoryItemUnlocked);
         this.notificationList.matchCalback = this.showFullReaport;
         this.notificationListTemp.matchCalback = this.showFullReaport;
         this.set_log_list(this._reports);
         this.squadList.visible = true;
         this.victory_screen.visible = false;
         this.friendsMenu.visible = false;
         this.item_control.visible = false;
         this.clanWindow.visible = false;
         this.item_control.setInventory(this.inventory_list);
         this.modifierWindow.setInventory(this.inventory_list);
         this.modifierWindow.visible = false;
         DragRegion.playerProfile = this.player_profile;
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onHiddenKey,true,100);
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onHiddenKey,false,100);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onHiddenKeyUp,true,100);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onHiddenKeyUp,false,100);
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onKeyDownStage);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onKeyUpStage);
         stage.addEventListener(Event.DEACTIVATE,this.clearDetailKeys);
         stage.addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         stage.addEventListener(MouseCursorEvent.CURSOR_CHANGE,this.onChangeCursor);
         this.parseSpriteSheets();
         this.item_control.setModifiersIcons(this._modifiers_i);
         _itemDescriptor = new ItemDescriptor("InventoryItems");
         this.inventory_list.registerItemDescriptor(this._itemDescriptor);
         this.inventory_list.addEventListener(DragEvent.DRAG_START,this.dragCallback,false,-1);
         this.inventory_list.addEventListener(DragEvent.DRAG_END,this.dragCallback,false,-1);
         this.player_profile.registerInventory(this.inventory_list);
         this.player_profile.paperDoll.ammoControl = this._ammoControl;
         this.player_profile.paperDoll.ammoControl.registerShop(this.shop_list);
         this.player_profile.paperDoll.ammoControl.registerInventory(this.inventory_list);
         this.shop_list.registerItemDescriptor(this._itemDescriptor);
         this.shop_list.registerBuyFunction(this.try_buy_item);
         MainScene.buyFunction = this.try_buy_item;
         this.shop_list.setInventory(this.inventory_list);
         this.questWindow.register_inventory(this.inventory_list);
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         statusPanel = new SrvTopMenuContainer();
         this.statusPanel.addEventListener(StatusPanel.SHOW_DAILY_MESSAGE,this.show_daily_reward_window_on_event);
         this.statusPanel.addEventListener(StatusPanel.MATCH_SETTINGS,this.onMatchSettigsToggle);
         this.statusPanel.addEventListener(StatusPanel.LAID_OUT,this.onStatusPanelLaidOut);
         this.statusPanel.addEventListener(StatusPanel.LOBBY_WINDOW,this.onLobbyWindow);
         this.statusPanel.matchSettings.addEventListener(MatchMakingSettings.TOURNAMENT_SELECTED,this.openTournamentWindow2);
         this.statusPanel.matchSettings.addEventListener(MatchMakingSettings.TOURNAMENT_DESELECTED,this.openTournamentWindow2);
         this.statusPanel.matchSettings.addEventListener(Event.CONNECT,this.onRequestPlayers);
         addChild(this.statusPanel);
         addChild(this.player_profile);
         addChild(this.squadList);
         this.player_profile.visible = false;
         this.topMenu.tabPage.buttonWidth = 125;
         this.topMenu.tabPage.width = 990;
         this.topMenu.tabPage.validateNow();
         this.statusPanel.setTopMenu(this.topMenu);
         this._wheelsMC.playAnimation(false);
         stage.addEventListener(MouseEvent.MOUSE_WHEEL,this.onMouseFnx);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.onMouseStageClick);
         stage.addEventListener(MouseEvent.MOUSE_DOWN,this.onMouseStageClick);
         this.bottom_panel.btnShowLogList.enabled = false;
         this.inventory_list.item_control = this.item_control;
         AmmoShareLite.setItemCard(this.item_control);
         trace("Lobby::fonts_embed");
         var fonts:Array = Font.enumerateFonts();
         var font:Font;
         for each(font in fonts)
         {
            trace(font.fontName + ":" + font.fontType);
         }
         this.updateInvitesPos();
      }
      
      private function onProfileIndexChange(e:ButtonEvent) : void
      {
      }
      
      private function onProfileRenameClicked(e:Event) : void
      {
         var profileId:int = (e.target.data.index);
         var profileName:String = e.target.data.name;
         trace("onProfileRenameClicked",profileId,profileName);
         this.showProfileRename({
            profileId:profileId,
            profileName:profileName
         });
      }
      
      private function onProfileContextMenu(e:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = e as MouseEventEx;
         trace("onProfileContextMenu",sfEvent,e.target,e.currentTarget);
         var mouseIdx:uint = sfEvent == null ? MouseEventEx.LEFT_BUTTON : sfEvent.buttonIdx;
         var options:Array;
         trace("mouseIdx",mouseIdx,sfEvent.mouseIdx,sfEvent.buttonIdx);
         if(mouseIdx == MouseEventEx.LEFT_BUTTON && e.target is Button)
         {
            trace("onProfileIndexChange");
            Util.logObject(e.target.data);
            if(e.target.data.state == ProfileState.PREMIUM_ONLY)
            {
               if(this.player_profile.buyPremium.visible == false)
               {
                  this.player_profile.buyPremium.visible = true;
                  this.player_profile.buyPremium.setData({options:this.statusPanel.getPremiumStore()});
               }
            }
            else
            {
               this.player_profile.buyPremium.visible = false;
            }
         }
         else if(mouseIdx == MouseEventEx.RIGHT_BUTTON && e.target is Button && (e.target.enabled))
         {
            options = [];
            options.push({
               name:LocalizationData.getName("profile_rename"),
               profileId:e.target.data.index,
               profileName:e.target.data.name,
               click_function:this.showProfileRename,
               callBack:true
            });
            SrvContextMenu.show(options,0,0,this.player_profile);
         }
      }
      
      private function showProfileRename(value:Object) : void
      {
         Message.showMessage("rename",{
            windowLabel:LocalizationData.getName("profile_rename_label"),
            acceptLabel:LocalizationData.getName("ok"),
            cancelLabel:LocalizationData.getName("cancel"),
            defaultText:LocalizationData.getName("enter_profile_name"),
            currentName:value.profileName,
            externalInterface:"profile_rename",
            externalData:value.profileId
         });
      }
      
      private function onVSPlayerInfo(e:Event) : void
      {
         this.show_player_info(this.victory_screen.getPlayerData());
      }
      
      protected function showLocalPlayer() : void
      {
         var playerInfo:Object = {};
         var data:Array = this._playerProfiles;
         for(var i:uint = 0; i < data.length; i++)
         {
            data[i].items = i == 0 ? this._profileItems.items : [];
         }
         playerInfo.profiles = data;
         trace("showLocalPlayer",this._playerProfiles);
         playerInfo.nickname = PlayerInfo.nickname;
         this.show_player_info(playerInfo);
      }
      
      public function show_player_info(value:Object) : void
      {
         value.inventoryList = this.inventory_list;
         Message.showMessage("player_info",value,true);
      }
      
      public function exit_decal_edit() : void
      {
         this.modifierWindow.exitDecalEdit();
      }
      
      private function show_daily_reward_window_on_event(e:Event) : void
      {
         this.show_daily_reward_window();
      }
      
      private function showPdItemHint(e:ListEvent) : void
      {
         if(!e.itemData)
         {
            return;
         }
         this.itemHintRenderer.displayOrange(true);
         this.itemHintRenderer.displaylabel(false);
         this.itemHintRenderer.mouseEnabled = this.itemHintRenderer.mouseChildren = false;
         this.itemHintRendererSec.mouseEnabled = this.itemHintRendererSec.mouseChildren = false;
         this.itemHintRenderer.x = this.player_profile.x + e.itemRenderer.x + e.itemRenderer.width + 3;
         this.itemHintRenderer.y = this.player_profile.y + e.itemRenderer.y - 5;
         this._cards.addChild(this.itemHintRenderer);
         this.itemHintRenderer.data = e.itemData;
         this.itemHintRenderer.itemCompare.setSecondaryWeapon(null);
         var maxPox:int = this.itemHintRenderer.y + this.itemHintRenderer.height;
         if(maxPox > MainScene.stageHeight - 2)
         {
            this.itemHintRenderer.y -= maxPox - MainScene.stageHeight;
         }
         var delay:Number = 0.3;
         var length:Number = 0.2;
         if(this.itemHintRenderer.alpha == 1)
         {
            this.itemHintRenderer.alpha = 0;
            TweenLite.to(this.itemHintRenderer,length,{
               delay:delay,
               alpha:1
            });
         }
      }
      
      private function updateHintItem(e:Event) : void
      {
         trace("updateHintItem",this.itemHintRendererSec.data);
         var hintTarget:ItemInfoRenderer = (this.itemHintRendererSec.parent) ? this.itemHintRendererSec : this.itemHintRenderer;
         var oldData:ItemData;
         var curerrentSlot:IPaperDollSlot;
         var newData:ItemData;
         if(hintTarget == this.itemHintRendererSec && this.craftOpen())
         {
            // The craft window's item, not an equipped one: the paper doll does not change it.
            return;
         }
         if((hintTarget.parent) && (hintTarget.data))
         {
            oldData = hintTarget.data as ItemData;
            curerrentSlot = this.player_profile.profileItems.getSlotAt(oldData.slotId);
            newData = curerrentSlot.data as ItemData;
            trace("newData",newData);
            if(!newData)
            {
               hintTarget.data = null;
               if(hintTarget == this.itemHintRendererSec)
               {
                  this.itemHintRenderer.itemCompare.setSecondaryWeapon(null);
               }
               removeChild(hintTarget);
               return;
            }
            trace("CompareData::",oldData.name,newData.name,newData.slotId,oldData.slotId,newData.descriptor,oldData.descriptor,newData.tipText);
            if(oldData.slotId == newData.slotId)
            {
               hintTarget.data = newData;
               if(hintTarget == this.itemHintRendererSec)
               {
                  this.itemHintRenderer.itemCompare.setSecondaryWeapon(newData);
               }
            }
         }
      }
      
      // Whether the craft (Upgrade) window is open.
      private function craftOpen() : Boolean
      {
         return this.craftWindow != null && this.craftWindow.parent != null;
      }

      private function showItemHint(e:ListEvent) : void
      {
         var secondData:Object;
         if(!e.itemData || this.inventory_list.bundleList.visible && e.itemData.dictId == this.inventory_list.bundleList.itemId)
         {
            return;
         }
         trace("showItemHint::",e.itemData,e.target,e.currentTarget);
         this.itemHintRenderer.mouseEnabled = this.itemHintRenderer.mouseChildren = false;
         this.itemHintRendererSec.mouseEnabled = this.itemHintRendererSec.mouseChildren = false;
         this.itemHintRenderer.displayOrange(true);
         if((this._pdTarget))
         {
            this._pdTarget.removeEventListener(Event.RENDER,this.onHintTargetRender);
            _pdTarget = null;
         }
         _pdTarget = e.itemRenderer as MovieClip;
         this._pdTarget.addEventListener(Event.RENDER,this.onHintTargetRender);
         this._cards.addChild(this.itemHintRenderer);
         if(e.target is ScrollingList)
         {
            this.itemHintRenderer.x = this.inventory_list.x - 195 - this.itemHintRenderer.width;
         }
         else
         {
            this.itemHintRenderer.x = this.inventory_list.x + this._pdTarget.x - this.itemHintRenderer.width + 6;
         }
         if(InventoryList.AllOW_COMPARE && this.craftOpen())
         {
            // With the item being upgraded, not the equipped one; none while the window has none
            // (craft-menu.md, "Comparing items").
            secondData = this.craftWindow.compareWith(e.itemData);
         }
         else if(InventoryList.AllOW_COMPARE)
         {
            secondData = this.player_profile.getRelativeData(e.itemData);
         }
         else
         {
            secondData = null;
         }
         if((secondData))
         {
            this.itemHintRenderer.displaylabel(true);
            this.itemHintRendererSec.displaylabel(true);
            this.itemHintRenderer.itemCompare.setSecondaryWeapon(secondData);
            this.itemHintRendererSec.data = secondData;
            // Both cards coloured, each against the other: the equipped (or the craft window's) one
            // too (craft-menu.md, "Comparing items").
            this.itemHintRendererSec.itemCompare.setSecondaryWeapon(e.itemData);
            this._cards.addChild(this.itemHintRendererSec);
         }
         else
         {
            this.itemHintRenderer.displaylabel(false);
            this.itemHintRenderer.itemCompare.setSecondaryWeapon(null);
            if(this._cards.contains(this.itemHintRendererSec))
            {
               this._cards.removeChild(this.itemHintRendererSec);
            }
         }
         this.itemHintRenderer.data = e.itemData;
         this.itemHintRenderer.alignComparisonHeader(secondData != null ? this.itemHintRendererSec : null);
         var delay:Number = 0.3;
         var length:Number = 0.2;
         this.itemHintRenderer.alpha = 0;
         TweenLite.killTweensOf(this.itemHintRenderer);
         TweenLite.to(this.itemHintRenderer,length,{
            delay:delay,
            alpha:1
         });
         if((this.itemHintRendererSec.parent))
         {
            this.itemHintRendererSec.alpha = 0;
            TweenLite.killTweensOf(this.itemHintRendererSec);
            TweenLite.to(this.itemHintRendererSec,length,{
               delay:delay,
               alpha:1
            });
         }
      }
      
      // A craft slot's item left of the window, compared with the other slot's; the item being
      // upgraded always the left card (craft-menu.md, "Comparing items").
      private function showCraftSlotHint(e:ListEvent) : void
      {
         if(!e.itemData || !this.craftOpen())
         {
            return;
         }
         this.hideItemHint(null);
         var secondData:Object = InventoryList.AllOW_COMPARE ? this.craftWindow.slotCompareWith(e.itemRenderer) : null;
         this.itemHintRenderer.mouseEnabled = this.itemHintRenderer.mouseChildren = false;
         this.itemHintRendererSec.mouseEnabled = this.itemHintRendererSec.mouseChildren = false;
         this.itemHintRenderer.displayOrange(true);
         this.itemHintRenderer.displaylabel(secondData != null);
         this.itemHintRenderer.itemCompare.setSecondaryWeapon(secondData);
         this.itemHintRenderer.data = e.itemData;
         this._cards.addChild(this.itemHintRenderer);
         this.itemHintRenderer.x = this.craftWindow.x - this.itemHintRenderer.width - 3;
         this.itemHintRenderer.y = this.craftWindow.y;
         if(secondData)
         {
            this.itemHintRendererSec.displaylabel(true);
            this.itemHintRendererSec.data = secondData;
            // both cards coloured, each against the other
            this.itemHintRendererSec.itemCompare.setSecondaryWeapon(e.itemData);
            this._cards.addChild(this.itemHintRendererSec);
            this.itemHintRendererSec.y = this.itemHintRenderer.y;
            var hoveredMain:Boolean = e.itemRenderer == this.craftWindow.dragItemSlot1;
            this.itemHintRenderer.alignComparisonHeader(this.itemHintRendererSec);
            var left:ItemInfoRenderer = hoveredMain ? this.itemHintRenderer : this.itemHintRendererSec;
            var right:ItemInfoRenderer = hoveredMain ? this.itemHintRendererSec : this.itemHintRenderer;
            right.x = this.craftWindow.x - right.width - 3;
            left.x = right.x - left.width - 1;
            if(left.x < 0)
            {
               // no room on the left: both right of the window, in the same order
               left.x = this.craftWindow.x + this.craftWindow.width + 3;
               right.x = left.x + left.width + 1;
            }
         }
      }

      private function onHintTargetRender(e:Event) : void
      {
         this.itemHintRenderer.alignComparisonHeader(this.itemHintRendererSec.parent != null ? this.itemHintRendererSec : null);
         if((this._pdTarget) && (this._pdTarget.data) && this._pdTarget.data.descriptor == ItemPlace.NONE)
         {
            this.itemHintRenderer.y = this.inventory_list.y + this._pdTarget.y + this.inventory_list.bundleList.y + 55;
         }
         else
         {
            this.itemHintRenderer.y = this.inventory_list.y + this._pdTarget.y + this.inventory_list.itemsTileList.y - 3;
         }
         var itemHintBottomPos:int = this.itemHintRenderer.y + this.itemHintRenderer.height;
         var maxPos:int;
         var itemHintSecBottomPos:int;
         if((this.itemHintRendererSec.parent))
         {
            itemHintSecBottomPos = this.itemHintRenderer.y + this.itemHintRendererSec.height;
            maxPos = Math.max(itemHintBottomPos,itemHintSecBottomPos);
         }
         else
         {
            maxPos = itemHintBottomPos;
         }
         if(maxPos > MainScene.stageHeight - 2)
         {
            this.itemHintRenderer.y -= maxPos - MainScene.stageHeight;
         }
         if((this.itemHintRendererSec.parent))
         {
            this.itemHintRendererSec.y = this.itemHintRenderer.y;
            this.itemHintRendererSec.x = this.itemHintRenderer.x - this.itemHintRenderer.width - 1;
         }
      }

      private function onWeaponDetailsResize(e:Event) : void
      {
         if(itemHintRenderer.parent == null) return;
         itemHintRenderer.alignComparisonHeader(itemHintRendererSec.parent != null ? itemHintRendererSec : null);
         if(_pdTarget != null && !craftOpen())
         {
            onHintTargetRender(null);
            return;
         }
         var h:Number = itemHintRenderer.height;
         if(itemHintRendererSec.parent != null) h = Math.max(h,itemHintRendererSec.height);
         itemHintRenderer.y = Math.max(0,Math.min(itemHintRenderer.y,MainScene.stageHeight - h - 2));
         if(itemHintRendererSec.parent != null) itemHintRendererSec.y = itemHintRenderer.y;
      }
      
      private function hideItemHint(e:Event) : void
      {
         if((this._pdTarget))
         {
            this._pdTarget.removeEventListener(Event.RENDER,this.onHintTargetRender);
            _pdTarget = null;
         }
         trace("hideItemHint");
         if(this._cards.contains(this.itemHintRenderer))
         {
            this._cards.removeChild(this.itemHintRenderer);
         }
         if(this._cards.contains(this.itemHintRendererSec))
         {
            this._cards.removeChild(this.itemHintRendererSec);
         }
      }
      
      private function showFullReaport(e:Object) : void
      {
         ExternalInterface.call("show_match_result",e.match_id);
         var reportData:Object = e;
         this.show_victory_screen();
         this.victory_screen.setData(reportData,true);
         if((reportData) && (reportData.statistics_money))
         {
            trace("showFullReaport::",reportData.statistics_money.autobuy_money,reportData.statistics_money.autorepair_money);
            this.victory_screen.setSpendMoneyDetailed(reportData.statistics_money.autobuy_money_ammo_arr,reportData.statistics_money.autobuy_money_consulables_arr,reportData.statistics_money.autorepair_money_arr);
         }
      }
      
      public function show_loading_wheels(value:Boolean = true) : void
      {
         this._wheelsMC.playAnimation(true);
         TweenLite.killTweensOf(this._wheelBack);
         TweenLite.killTweensOf(this._wheelsMC);
         this._wheelBack.width = stage.stageWidth;
         this._wheelBack.height = stage.stageHeight;
         this._wheelBack.alpha = 1;
         this._wheelsMC.alpha = 1;
         this._layers.addChild(this._wheelBack);
         this._wheelsMC.x = (stage.stageWidth - this._wheelsMC.width) / 2;
         this._wheelsMC.y = (stage.stageHeight - this._wheelsMC.height) / 2;
         this._layers.addChild(this._wheelsMC);
         TweenLite.to(this._wheelBack,1,{
            delay:2,
            alpha:0,
            onComplete:this.hideWheels
         });
         TweenLite.to(this._wheelsMC,0,{
            delay:2,
            alpha:0
         });
      }
      
      private function hideWheels() : void
      {
         if((this._wheelBack.parent))
         {
            this._wheelsMC.playAnimation(false);
            TweenLite.killTweensOf(this._wheelBack);
            TweenLite.killTweensOf(this._wheelsMC);
            this._layers.removeChild(this._wheelBack);
            this._layers.removeChild(this._wheelsMC);
         }
      }
      
      public function set_debug_items() : void
      {
         MainScene.showDebugInfo = true;
      }
      
      public function fill_item_reroll_data(value:Object) : void
      {
         this.item_control.fillItemModifiers(value);
      }
      
      public function fill_item_reroll_options(value:Array) : void
      {
         this.item_control.fillItemRerollOptions(value);
      }
      
      private function test_craft_data() : void
      {
         var testObject:Object = {
            craft_time:90 * 60,
            regular_cost:12,
            premium_cost:22,
            success_chance_regular:12,
            fail_chance_regular:88,
            success_chance_gold:50,
            fail_chance_gold:50
         };
         this.fill_craft_data(testObject);
      }
      
      public function fill_craft_data(value:Object) : void
      {
         _craftData = value;
         if((this.craftWindow))
         {
            this.craftWindow.fillCraftData(value);
         }
      }
      
      public function fill_crafted_item(value:Object) : void
      {
         // The exe names the donor's dictionary entry; the upgraded item is
         // the window's first one, which differs for a golden item upgraded
         // with its plain item (ItemFactory.isCraftDonor).
         var main:Object = this.craftWindow ? this.craftWindow.dragItemSlot1.data : null;
         var itemData:ItemData = this.inventory_list.createItem(main ? main.dictId : value.dict_id);
         if((value.modifiers))
         {
            itemData.modifiers = (value.modifiers as Array).concat(itemData.modifiersDefault);
         }
         itemData.value = value.condition_or_stack;
         var rank:int = (value.rarity_level);
         itemData.rank = rank;
         itemData.clipSize = 1;
         itemData.upgrades = [];
         this.inventory_list.fillModifiersDescription(itemData);
         var modifiersIconContainer:IconContainer = IconContainer.getInstance("modifiers");
         var newModifier:Object = value.new_modifier;
         var icon:Icon;
         if((newModifier))
         {
            this.inventory_list.fillModifierDescription(newModifier);
            icon = modifiersIconContainer.getIcon(newModifier.icon);
            newModifier.icon = icon;
            newModifier.label = newModifier.textValue;
            // its range on hover, as in the reroll window (max-bonus.md)
            ModificationRolls.withRange(newModifier,newModifier);
         }
         this.craftWindow.fillCraftedItem(itemData,value.result,newModifier);
      }
      
      public function fill_victory_screen_match_bonus(mathId:uint, bonus:Boolean) : void
      {
         this.victory_screen.setBonus(mathId,bonus);
      }
      
      public function fill_victory_screen_spend_money_detailed(autobuy_ammo:Array, autobuy_consulables:Array, repair:Array) : void
      {
         trace("fill_victory_screen_spend_money_detailed",autobuy_ammo,autobuy_consulables);
         this.victory_screen.setSpendMoneyDetailed(autobuy_ammo,autobuy_consulables,repair);
         this.updateReport(this.victory_screen.getData());
         this.set_log_list(this._reports);
      }
      
      public function fill_victory_screen_spend_money(autobuy:int, repair:int) : void
      {
         trace("fill_victory_screen_spend_money",autobuy,repair);
         this.victory_screen.setSpendMoney(autobuy,repair);
         this.updateReport(this.victory_screen.getData());
         this.set_log_list(this._reports);
      }
      
      private function hideTop() : void
      {
         this._top_container.visible = false;
      }
      
      private function onBuyMaps(e:Event) : void
      {
         var pp:IPlayerProfile = this.player_profile;
         var dict:uint = PlayerInfo.getVoucherDictId(VoucherType.DROP_CARD);
         var id:uint = PlayerInfo.getVoucherId(VoucherType.DROP_CARD);
         trace("onBuyMaps::",pp.profileId,id,dict);
         this.try_buy_item(pp.profileId,id,dict,ItemPlace.SHOP,ItemPlace.INVENTORY,1,GangShop.getInstance().getItemPrice(dict),Faction.ANY);
      }
      
      public function show_choose_faction_message() : void
      {
         var messageData:Object = {acceptCallback:this.accept_show_quests_toggle};
         Message.showMessage("no_faction",messageData,true);
      }
      
      public function on_item_sold(value:uint) : void
      {
         var itemData:ItemData = this.inventory_list.getInventoryItem(value) as ItemData;
         trace("on_item_sold::",value,itemData,(itemData) ? itemData.equipped : "nl",(itemData) ? itemData.slotTarget : "nl");
         if((itemData) && (itemData.equipped) && (itemData.slotTarget))
         {
            trace("on_item_sold::drop_upgrade_item",itemData.slotTarget,uint(itemData.itemId));
            ExternalInterface.call("drop_upgrade_item",itemData.slotTarget,uint(itemData.itemId));
         }
      }
      
      private function accept_show_quests_toggle() : void
      {
         if(this.questWindow.visible == false)
         {
            this.topMenu.selectActiveWindow(this.topMenu.QUESTS_INDEX);
         }
      }
      
      public function handle_nickname_validation(result:Boolean, name:String = null, message:String = null) : void
      {
         trace("handle_nickname_validation",result,name,"message",message);
         this.statusPanel.handleNicknameValidation(result,name,message);
      }
      
      public function handle_daily_message_validation(result:Boolean, name:String = null, message:String = null) : void
      {
         trace("handle_daily_message_validation",result,name,message);
      }
      
      public function handle_clan_name_validation(result:Boolean, name:String = null, message:String = null) : void
      {
         trace("handle_clan_name_validation",result,name,message);
         this.clanWindow.createClanRenderer.handleClanNameValidation(result,name,message);
      }
      
      public function handle_clan_abbreviation_validation(result:Boolean, name:String = null, message:String = null) : void
      {
         trace("handle_clan_abbreviation_validation",result,name,message);
         this.clanWindow.createClanRenderer.handleClanAbbreviationValidation(result,name,message);
      }
      
      private function showFactionInShop(e:Event) : void
      {
         this.show_shop_toggle();
         this.inventory_list.optionControl.reset();
         this.inventory_list.inventoryTabPage.selectedIndex = SuperCategory.ALL;
         if(this.inventory_list.checkbox_all_traders.selected)
         {
            this.inventory_list.checkbox_all_traders.selected = false;
            this.inventory_list.optionControl.dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
         }
         var selectedFaction:int = this.questWindow.getSelectedFaction() + 1;
         trace("showFactionInShop::",selectedFaction);
         this.inventory_list.sellersTabPage.selectedIndex = selectedFaction;
      }
      
      private function test_active_quest() : void
      {
         trace("update_inventory_item");
         var temp_object:Object = {
            id:8910,
            condition_or_stack:75
         };
         this.update_inventory_item(temp_object);
      }
      
      private function test_fin_matchmaking() : void
      {
         var testProperties:Array = [99,10,10,10,99,99,10,10,10,99,99,10,10,10,99,99];
         this.statusPanel.lobbyGameSearch.setSearchProperties(testProperties);
         this.statusPanel.lobbyGameSearch.setSearchRange(1,15);
      }
      
      private function test_matchmaking() : void
      {
         var getSearchProp:Function = function():Object
         {
            ++level;
            return {
               value:1,
               level:level
            };
         };
         var level:int;
         trace("test_matchmaking");
         level = -1;
         var sp:Array = [];
         for(var i:uint = 0; i < 30; i++)
         {
            sp.push(getSearchProp());
         }
         this.show_match_making(true);
         this.statusPanel.lobbyGameSearch.setSearchProperties(sp);
         this.statusPanel.lobbyGameSearch.setSearchRange(3,18);
      }
      
      public function set_quest_active_faction(value:uint) : void
      {
         PlayerInfo.activeFaction = value;
         this.topMenu.setActiveFaction(value);
         this.questWindow.setActiveFaction(value);
      }
      
      public function allow_match_making_settings(value:Boolean) : void
      {
         this.statusPanel.matchSettings.setEnabled(value);
      }
      
      public function set_mm_search_properties(value:Array) : void
      {
         this.statusPanel.lobbyGameSearch.setSearchProperties(value);
      }
      
      public function set_mm_search_range(start:uint, end:uint) : void
      {
         this.statusPanel.lobbyGameSearch.setSearchRange(start,end);
      }
      
      private function dragCallback(e:DragEvent) : void
      {
         trace("DRAG_CALL_BACK::",e.type,this.player_profile.visible);
         if((this.craftWindow) && (this.craftWindow.parent))
         {
            this.craftWindow.onDrag(e);
         }
         if(this.player_profile.visible)
         {
            this.player_profile.onDrag(e);
         }
         else
         {
            this.modifierWindow.onDrag(e);
         }
      }
      
      private function closeModifierWindow(e:Event) : void
      {
         if(!this.modifierWindow.visible)
         {
            return;
         }
         this.modifierWindow.removeEventListener(Event.CLOSE,this.closeModifierWindow);
         this.inventory_list.removeEventListener(Event.CLOSE,this.closeModifierWindow);
         if(this._upgradeFromShop)
         {
            this.inventory_list.toggleShopMode(true);
            _upgradeFromShop = false;
         }
         this.inventory_list.focusCategory(SuperCategory.NO_FOCUS);
         this.inventory_list.mainTabBar.enabled = true;
         this.inventory_list.inventoryTabPage.enabled = true;
         this.inventory_list.updateItemCard = true;
         trace("closeModifierWindow::",this.item_control.mouseChildren);
         // The keys go back to the lobby and the window leaves the display list (it was only
         // hidden): the game turns the camera only when nothing of the movie is under the cursor
         // (get_mouse_over) and no control has the keys (lobby.md, "The upgrade window's close").
         if(stage != null && stage.focus != null && this.modifierWindow.contains(stage.focus as DisplayObject))
         {
            stage.focus = this;
         }
         this.modifierWindow.visible = false;
         if(this.modifierWindow.parent == this)
         {
            removeChild(this.modifierWindow);
         }
         this.player_profile.visible = this.inventory_list.visible;
         this.dragRegion.mouseEnabled = true;
         if(this._upgradeShown)
         {
            trace("exit_item_upgrade",this.modifierWindow.getItemId());
            ExternalInterface.call("exit_item_upgrade",this.modifierWindow.getItemId());
            this._upgradeShown = false;
         }
         this.item_control.removeEventListener(Event.ACTIVATE,this.updateSelectedItem);
         MainScene.activePop = null;
         if(!this.item_control.mouseChildren)
         {
            trace("item_control.vs",this.inventory_list.visible);
            this.item_control.visible = this.inventory_list.visible;
            this.item_control.mouseChildren = true;
         }
         this.inventory_list.toggleUpgradeMode();
      }
      
      private function test_tournament_data() : void
      {
         var arr:Array = [];
         arr.push({
            name:"ilya@vostok.com",
            status:ClanTeamRole.COMMANDER
         });
         arr.push({
            name:"ilya10@vostok.com",
            status:ClanTeamRole.SOLDIER
         });
         for(var i:uint = 0; i < 9; i++)
         {
            arr.push({
               name:"ilia" + i.toString(),
               status:int(i / 3) + 1,
               ready:true
            });
         }
         this.fill_clan_team(arr);
      }
      
      public function fill_clan_team(value:Array) : void
      {
         this._tournament.setPlayersData(value);
         this.squadList.lock(PlayerInfo.isInTeam && (this._tournament.parent));
      }
      
      private function onMatchSettigsToggle() : void
      {
         if((this._tournament))
         {
            this.updateTournamentPosition();
         }
      }
      
      private function onTournamentClose(e:Event) : void
      {
         if((this._tournament.parent))
         {
            this.statusPanel.matchSettings.deselectClanMatchCheckbox();
         }
      }
      
      private function onTournamentFold(e:Event) : void
      {
         if((this._tournament.parent))
         {
            this.updateTournamentPosition();
         }
      }
      
      public function checkbox_tournament_match_clicked(value:Boolean) : void
      {
         this.statusPanel.matchSettings.setTournamentResult(value);
         if((this.statusPanel.matchSettings.checkBoxClan.selected))
         {
            if(!this._tournament.parent)
            {
               this.openTournamentWindow();
            }
         }
         else if((this._tournament.parent))
         {
            this.closeTournamentWindow();
         }
      }
      
      public function show_clan_match_window() : void
      {
         if(!this._tournament.parent)
         {
            this.statusPanel.matchSettings.selectClanMatchCheckbox();
         }
      }
      
      private function openTournamentWindow2(e:Event) : void
      {
         trace("openTournamentWindow2");
         this.checkbox_tournament_match_clicked(true);
      }
      
      private function testRank() : void
      {
         this.checkbox_tournament_match_clicked(false);
      }
      
      private function openTournamentWindow(e:Event = null) : void
      {
         trace("openTournamentWindow");
         addChild(this._tournament);
         this._tournament.init();
         this._tournament.x = (MainScene.stageWidth - this._tournament.background.width) / 2;
         this.updateTournamentPosition();
         // a modal while unfolded: Esc folds it
         Modals.open(this._tournament,Modals.LOBBY,function():void
         {
            _tournament.fold();
         },function():Boolean
         {
            return _tournament.parent != null && _tournament.visible && !_tournament.isFold();
         });
      }
      
      private function updateTournamentPosition() : void
      {
         if(this._tournament.isFold())
         {
            this._tournament.y = this.inventory_list.y + 902 - 23 - 13;
         }
         else
         {
            this._tournament.y = this.inventory_list.y + 902 - this._tournament.height + 14 - 13;
         }
      }
      
      private function closeTournamentWindow(e:Event = null) : void
      {
         if((this._tournament))
         {
         }
         ExternalInterface.call("clan_match_closed");
         Modals.leave(this._tournament);
         removeChild(this._tournament);
         this.squadList.lock(false);
      }
      
      private function openCraftWindow(e:Event) : void
      {
         var itemData:ItemData = this.item_control.getItemData().clone();
         craftWindow = new SrvMessageCraftItemSkinned();
         this.craftWindow.setMessageData(0,itemData);
         if((this._craftData))
         {
            this.craftWindow.fillCraftData(this._craftData);
         }
         this.craftWindow.setMatchState(MainScene.activeMatchMaking ? MatchMakingState.SEARCHING : MatchMakingState.IDLE);
         this.craftWindow.x = (MainScene.stageWidth - this.craftWindow.width) / 2;
         this.craftWindow.y = (MainScene.stageHeight - this.craftWindow.height) / 2;
         this.craftWindow.btnCancel.addEventListener(ButtonEvent.CLICK,this.onCloseCraft);
         this.craftWindow.btnAccept.addEventListener(ButtonEvent.CLICK,this.onCloseCraft);
         this.craftWindow.btnClose.addEventListener(ButtonEvent.CLICK,this.onCloseCraft);
         this.craftWindow.addEventListener("closeCraft",this.closeCraftWindow);
         this.craftWindow.addEventListener(ListEvent.ITEM_ROLL_OVER,this.showCraftSlotHint);
         this.craftWindow.addEventListener(ListEvent.ITEM_ROLL_OUT,this.hideItemHint);
         this.craftWindow.dragItemSlot1.dragEnabled = false;
         this.craftWindow.dragItemSlot1.setData(itemData);
         addChild(this.backBlack);
         this.backBlack.width = MainScene.stageWidth;
         this.backBlack.height = MainScene.stageHeight;
         addChild(this.craftWindow);
         // a modal: Esc is its close (its own messages are modals over it)
         var craft:MessageCraftItem = this.craftWindow;
         Modals.open(craft,Modals.LOBBY,function():void
         {
            onCloseCraft();
         },function():Boolean
         {
            return craft.parent != null;
         });
         this.handleFrontInventory();
         this.inventory_list.lockControls(true);
         this.backBlack.alpha = 0.5;
         // the item being upgraded is in the window's main slot: not listed (craft-menu.md)
         this.inventory_list.hideCraftSlotted([itemData.itemId]);
         // from the shop (a kit item's menu): the donors are owned items, not the shop's entries
         _craftFromShop = this.inventory_list.isShop;
         if(this._craftFromShop)
         {
            this.inventory_list.toggleShopMode(false);
         }
         this.inventory_list.filterItem(itemData.dictId);
         this.inventory_list.selectItem(itemData.itemId);
      }
      
      private function handleFrontInventory() : void
      {
         addChild(this.inventory_list);
         addChild(this.notificationListTemp);
      }
      
      private function onCloseCraft(e:ButtonEvent = null) : void
      {
         if((e) && e.target == this.craftWindow.btnAccept)
         {
            if(this.craftWindow.btnAccept.label != LocalizationData.getName("btn_craft_accept"))
            {
               return;
            }
         }
         this.closeCraftWindow(null);
      }
      
      private function closeCraftWindow(e:Event) : void
      {
         // the compare card showed the window's item (MessageCraftItem.compareWith)
         this.hideItemHint(null);
         // The keys leave the window before it goes: a focus left on its removed buttons made the
         // game's own stage.focus read crash on the next key (exe patch 52 guards the read).
         if(stage != null && stage.focus != null && this.craftWindow.contains(stage.focus as DisplayObject))
         {
            stage.focus = this;
         }
         this.inventory_list.lockControls(false);
         Modals.leave(this.craftWindow);
         removeChild(this.craftWindow);
         removeChild(this.backBlack);
         if(this._craftFromShop)
         {
            this.inventory_list.toggleShopMode(true);
            _craftFromShop = false;
         }
         this.inventory_list.filterItem(-1);
         if((this.craftWindow.dragItemSlot1.data))
         {
            this.inventory_list.selectItem(this.craftWindow.dragItemSlot1.data.itemId);
         }
      }
      
      private function openModifierWindow(e:Event) : void
      {
         if(this.modifierWindow.visible == true)
         {
            trace("!!modifierWindowIsOPened");
            return;
         }
         var itemData:ItemData = this.item_control.getItemData();
         // Only an owned item: the game's window looks the item up and crashed on one it lacks (a
         // shop item on the card; shop-character-menu.md).
         if(itemData == null || !this.inventory_list.owns(itemData.itemId))
         {
            return;
         }
         var weapon:Boolean = UpgradeWindow.isWeaponCategory(int(itemData.category));
         // from the shop: the window lists what is owned, as from the inventory (the shop's list
         // has no owned decals or camouflages)
         _upgradeFromShop = this.inventory_list.isShop;
         if(this._upgradeFromShop)
         {
            this.inventory_list.toggleShopMode(false);
         }
         this.inventory_list.focusCategory(SuperCategory.UPGRADES);
         this.inventory_list.mainTabBar.selectedIndex = InventoryTab.INVENTORY;
         this.inventory_list.mainTabBar.enabled = false;
         this.inventory_list.inventoryTabPage.enabled = false;
         // what fits the window: an armour's decals and camouflages; a weapon's window chooses by the
         // page it opens on (its scopes, or its camouflages and decals: UpgradeWindow.showPage)
         if(!weapon)
         {
            this.inventory_list.selectCategory([ItemCategory.CAMOUFLAGES,ItemCategory.DECALS],false,true);
         }
         this.inventory_list.toggleUpgradeMode();
         this.modifierWindow.visible = true;
         this.dragRegion.mouseEnabled = false;
         MainScene.activePop = this.modifierWindow;
         // on the display list before setData, which places it by its parent's inventory list:
         // the close takes it off (#302), and a second open threw there and stopped half way
         addChild(this.modifierWindow);
         this.modifierWindow.setData(itemData);
         // the game shows a weapon in the character's hands (patch 32), any other item as before
         this._upgradeShown = Boolean(itemData);
         if(this._upgradeShown)
         {
            trace("start_item_upgrade");
            ExternalInterface.call("start_item_upgrade",itemData.itemId);
         }
         this.modifierWindow.addEventListener(Event.CLOSE,this.closeModifierWindow);
         this.inventory_list.addEventListener(Event.CLOSE,this.closeModifierWindow);
         this.player_profile.visible = false;
         this.item_control.addEventListener(Event.ACTIVATE,this.updateSelectedItem);
         trace("openModifierWindow",this.item_control.visible);
         if(this.item_control.visible)
         {
            trace("item_control.vs",false);
            this.item_control.visible = false;
            this.item_control.mouseChildren = false;
         }
         trace("openModifierWindow",this.item_control.visible,this.player_profile.visible);
      }
      
      private function updateSelectedItem(e:Event) : void
      {
         var otherData:ItemData = this.item_control.getItemData();
         if((otherData) && this.modifierWindow.getItemId() == otherData.itemId)
         {
            this.modifierWindow.setData(otherData,false);
         }
      }
      
      public function lock_paperdoll(value:Boolean) : void
      {
         this.topMenu.lockInventory(value);
      }
      
      private function showActivedQuestList(e:Event) : void
      {
         if(!this.questWindow.visible)
         {
            this.show_quests_toggle();
         }
      }
      
      public function set_premium_exchange_rate(value:uint) : void
      {
         PlayerInfo.premiumExchangeRate = value;
      }
      
      public function set_item_props_data(value:Array) : void
      {
         trace("set_item_props_data::",value.length);
         var descr:PropertiesDescriptor = new PropertiesDescriptor("properties");
         descr.setPropertiesDictionary(value);
         ItemCompare.setPropsDescriptor(descr);
         this.item_control.setPropsDescriptor(descr);
         this.charWindow.setPropsDescriptor(descr);
      }
      
      public function set_props_data(value:Array) : void
      {
         trace("set_props_data::",value.length);
         var descr:PropertiesDescriptor = new PropertiesDescriptor("properties");
         descr.setPropertiesDictionary(value);
         ItemCompare.setPropsDescriptor(descr);
         this.item_control.setPropsDescriptor(descr);
         this.charWindow.setPropsDescriptor(descr);
      }
      
      private function updateItemCard(e:Event) : void
      {
         trace("Lobby::updateItemCard",e.target,e.currentTarget);
         var value:ItemData = e.target.data as ItemData;
         var profile:int;
         var relocate_object:Object;
         if(value.isStack)
         {
            profile = this.player_profile.profileId;
            relocate_object = {
               profile:profile,
               id:value.itemId,
               dict_id:value.dictId,
               sourceSlot:ItemPlace.INVENTORY,
               targetSlot:value.slotId,
               count:value.value,
               faction_id:value.factionId
            };
            relocate_object.condition_or_stack = value.count;
            AmmoShareLite.getInstance().updateElement(relocate_object);
         }
      }
      
      private function inventoryItemUnlocked(e:Event) : void
      {
         trace("inventoryItemUnlocked");
         this.shop_list.unclockItem();
         this.updateNewItems(this.shop_list.getNewItems());
      }
      
      private function onNewItemsCountChange(e:Event) : void
      {
         this.updateNewItems(this.shop_list.getNewItems());
      }
      
      public function updateNewItems(value:uint) : void
      {
         this.statusPanel.setNewItems(value);
      }
      
      private function test_update_invenory_item() : void
      {
         var modifier:Object = {
            name:"sdf",
            props:[{
               prop_id:PROP_ITEM_WEIGHT,
               prop_value:-0.8,
               modifier_type:ModifierType.MULTIPLY
            },{
               prop_id:PROP_ARMOR_PIERCING,
               prop_value:10,
               modifier_type:ModifierType.ADD
            }]
         };
         var data:Object = {
            id:8994,
            condition_or_stack:95,
            modifiers:[modifier]
         };
         this.update_inventory_item(data);
      }
      
      private function test_clan_window() : void
      {
         this.handle_clan_name_validation(true,"dadada","tetete");
      }
      
      private function test_sync_screen() : void
      {
         this.show_match_making(true);
      }
      
      private function test_sync_screen1() : void
      {
         this.set_mm_sync_state("AAAA",0);
      }
      
      private function test_sync_screen2() : void
      {
         this.set_mm_sync_state("BBBB");
      }
      
      private function testUnclokedFactionRank() : void
      {
         trace("testUnclokedFaction");
         var data:Object = {
            faction_id:Faction.BORDER,
            label:"Рептилоиды",
            message_body:"Вы открыли путь к солнцу",
            type:ImportantMessageType.REPUTATION_LEVEL
         };
         this.add_important_message(data);
         this.setup_player_progress(Faction.BORDER,2,222);
      }
      
      private function testUnclokedFaction() : void
      {
         trace("testUnclokedFaction");
         var data:Object = {
            faction_id:Faction.BORDER,
            label:"Рептилоиды",
            message_body:"Вы открыли путь к солнцу",
            type:ImportantMessageType.REPUTATION_LEVEL
         };
         this.add_important_message(data);
      }
      
      private function testUnclokedFaction2() : void
      {
         trace("testUnclokedFaction");
         var data:Object = {
            faction_id:Faction.SCAVENGERS,
            label:"Доктор Сринтус",
            message_body:"Мантра очищена",
            type:ImportantMessageType.REPUTATION_LEVEL
         };
         this.add_important_message(data);
      }
      
      private function testInviteToSquad() : void
      {
         trace("testInviteToSquad");
         var data:Object = {
            id:TEST_INVITE_ID + this._invitesTemp,
            sender_name:"123456789123456789" + this._invitesTemp,
            message_body:"сквадиеще",
            type:ImportantMessageType.CLAN_TEAM_INVITE
         };
         this.add_important_message(data);
         ++this._invitesTemp;
         if(this._invitesTemp < 7)
         {
         }
      }
      
      private function testInviteToSquad2() : void
      {
         trace("testInviteToSquad2");
         var data:Object = {
            id:2,
            sender_name:"Timka2",
            message_body:"Vazzap2",
            type:ImportantMessageType.SQUAD_INVITE
         };
         this.remove_important_message(TEST_INVITE_ID);
      }
      
      public function add_squad_invite(value:Object) : void
      {
         trace("add_squad_invite");
         this.bottom_panel.addIvite(value);
      }
      
      private function testPremiumSettings() : void
      {
         this.set_premium_state(PremiumState.ACTIVE,"458д");
      }
      
      private function onRequestPlayers(e:Event) : void
      {
         trace("onRequstPlayers",e.target);
         _playerRequestTarget = e.target as MovieClip;
      }
      
      private function testIMChange(e:Event) : void
      {
         this.test_important_messsage2();
      }
      
      private function testRewardItems() : void
      {
         var test:Array = [];
         for(var i:uint = 0; i < 6; i++)
         {
            test.push({
               time:"01:22",
               label:"supra_level",
               result:"1:2",
               match_result:3 * (i % 2),
               reward:[{
                  type:Reward.EXPERIENCE,
                  amount:6500
               },{
                  type:Reward.REPUTATION,
                  amount:50,
                  param:Faction.SCAVENGERS
               },{
                  type:Reward.MONEY,
                  amount:50,
                  param:2
               }]
            });
         }
         this.set_log_list(test);
      }
      
      private function testQuestChange(e:Event) : void
      {
         trace("testQuestChange::",e);
         var array:Array = [];
         array.push({
            faction_id:Faction.SCAVENGERS,
            quest_icon:0,
            quest_label:"ex1TraQuestRRR",
            text:"1Sil\'veple la mue de cuartassone vi sonne juperne \nel cracasson si\nnme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:6500
            },{
               type:Reward.REPUTATION,
               amount:50,
               param:Faction.SCAVENGERS
            },{
               type:Reward.MONEY,
               amount:50,
               param:2
            },{
               type:Reward.ITEM,
               amount:50,
               param:82
            }],
            task:{
               text:"*KILL BIL/n\nL1",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            },
            tasks:[{
               text:"*KILL BIL/n\nL1",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:50
         });
         array.push({
            faction_id:Faction.SCAVENGERS,
            quest_icon:1,
            quest_label:"ex2TraQuestRRR",
            text:"2Sil\'veple la mue de cuartassone vi sonne jup\nerne el cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.ITEM,
               amount:40,
               param:68
            },{
               type:Reward.REPUTATION,
               amount:40,
               param:Faction.SCAVENGERS
            },{
               type:Reward.EXPERIENCE,
               amount:40
            },{
               type:Reward.MONEY,
               amount:40,
               param:2
            }],
            task:{
               text:"*KILL BILL2",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:1
            },
            tasks:[{
               text:"*KILL BIL/n\nL1",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:60 * 120
         });
         trace("testQuestChange::",array.length);
         this.show_quest_selector(array);
      }
      
      public function set_premium_state(state:uint, label:String) : void
      {
         PlayerInfo.premiumState = state;
         PlayerInfo.premiumLeft = label;
         if(state != PremiumState.NONE && (this.statusPanel.premiumDiscount))
         {
            this.statusPanel.premiumDiscount = 0;
            this.updatePremiumDiscountTime();
         }
         if(state != PremiumState.NONE && this.player_profile.buyPremium.visible == true)
         {
            this.player_profile.buyPremium.visible = false;
         }
         if(this.victory_screen.visible)
         {
            this.victory_screen.updatePremiumState();
         }
      }
      
      public function set_premium_discount_store(value:Array) : void
      {
         trace("set_premium_discount_store::",value);
         _premiumStoreDiscount = value;
         this.statusPanel.setPremiumStoreDiscount(value);
      }
      
      public function set_premium_store(value:Array) : void
      {
         value.reverse();
         this.statusPanel.setPremiumStore(value);
         this.victory_screen.setPremiumStore(value);
      }
      
      public function set_squad_commander_mode(value:Boolean) : void
      {
         PlayerInfo.commander = value;
      }
      
      public function show_squad(value:Boolean) : void
      {
      }
      
      public function set_squad_player_type(index:uint, type:uint, weapon_dict_id:uint = 0) : void
      {
         var iconId:uint;
         if((weapon_dict_id))
         {
            iconId = (this._itemDescriptor.getItemDescription(weapon_dict_id).icon);
         }
         trace("set_squad_player_type::",weapon_dict_id,iconId);
         this.squadList.set_squad_player_type(index,type,iconId);
      }
      
      public function set_squad_player(index:uint, name:String, id:uint, ready:Boolean, type:uint = 0, weapon_dict_id:uint = 0) : void
      {
         var iconId:uint;
         if((weapon_dict_id))
         {
            iconId = (this._itemDescriptor.getItemDescription(weapon_dict_id).icon);
         }
         this.squadList.set_squad_player(index,id,name,ready,type,iconId);
         this.updateInvitesPos();
         this.statusPanel.matchSettings.updateSquadStatus();
      }
      
      public function remove_squad_player(index:uint) : void
      {
         this.squadList.remove_squad_player(index);
         this.updateInvitesPos();
         this.statusPanel.matchSettings.updateSquadStatus();
      }
      
      private function updateInvitesPos() : void
      {
         this.invitesContainer.y = this.squadList.y + this.squadList.background.height + 10;
      }
      
      public function set_warning_text(value:String, time:String = "") : void
      {
         this.statusPanel.set_info_label(value,time);
      }
      
      public function set_admin_text(label:String, message:String, timeout:int = 5000) : void
      {
         this.statusPanel.set_admin_label(label,message);
      }
      
      public function set_admin_message(type:uint, text1:String, text2:String = "") : void
      {
         if(type == ADMIN_MESSAGE_WARNING)
         {
            this.set_warning_text(text1,text2);
         }
         else
         {
            this.set_admin_text(text1,text2);
         }
      }
      
      private function onQuestTxClick(e:MouseEvent) : void
      {
         this.clear_quests();
         this.test_quest_messsage();
      }
      
      protected function test_victory_screenData2() : void
      {
         this.fill_victory_screen_drop_item({
            dict_id:ItemDictId.AMMO_9X18_MAKAROV,
            condition_or_stack:101,
            value:45,
            item_id:12534,
            rarity_level:5
         });
      }
      
      protected function test_victory_screenData() : void
      {
         var modifiers:Array = [{
            name:"sdf",
            props:[{
               prop_id:PROP_ITEM_WEIGHT,
               prop_value:-0.8,
               modifier_type:ModifierType.MULTIPLY
            },{
               prop_id:PROP_ARMOR_PIERCING,
               prop_value:10,
               modifier_type:ModifierType.ADD
            }]
         },{
            name:"asdasd",
            props:[{
               prop_id:PROP_ITEM_WEIGHT,
               prop_value:-0.8,
               modifier_type:ModifierType.MULTIPLY
            },{
               prop_id:PROP_ARMOR_PIERCING,
               prop_value:10,
               modifier_type:ModifierType.ADD
            }]
         }];
         var scoreData:Object = {
            name:"score_name",
            profile_name:"name",
            profile_play_time:"123c",
            amount:100,
            money:100,
            experience:100,
            reputation:100
         };
         var data:Object = {
            score_components:[scoreData,scoreData,scoreData],
            statistics_rating:new Object(),
            statistics_exp:{
               match_exp:33,
               daily_exp:200,
               daily_bonus_exp:100,
               daily_exp:50,
               quest_exp:50
            },
            statistics_money:new Object(),
            statistics_reputation:{
               faction_id:Faction.SCAVENGERS,
               quest_rep:50
            },
            title_1_player:"Cibbie&Huide4",
            title_1_value_0:22,
            title_0_player:"Where\'sYourWaterDude",
            title_0_value_0:22,
            title_2_player:"Krepish1998",
            title_2_value_0:22,
            title_3_player:"Cibbie&Huide",
            title_3_value_0:22,
            team1_score:0,
            team2_score:0,
            allow_drop_card:true,
            dropped_item:{value:80.5}
         };
         data.dropped_item = {
            dict_id:46,
            condition_or_stack:101,
            value:0.333336,
            item_id:12534,
            rarity_level:5
         };
         this.set_drop_items_properties([1000,5000,9000],15000);
         if(!data.players)
         {
            data.players = this.get_temp_players();
         }
         data.game_mode = GameMode.TEAM_DEATHMATCH;
         data.team_1_players_count = 7;
         data.team1_param = 2;
         data.team_2_players_count = 10;
         data.team2_param = 5;
         data.map_name = "someMapName";
         this.fill_victory_screen(data);
         var data2:Array = [{
            faction_id:Faction.SCAVENGERS,
            bonus:22
         }];
         this.set_faction_bonus(data2);
         var detailedArrmo:Array = [{
            dict_id:243,
            value:100
         },{
            dict_id:46,
            value:20
         },{
            dict_id:ItemDictId.AMMO_9X18_MAKAROV,
            value:3
         }];
         this.fill_victory_screen_spend_money_detailed(detailedArrmo,[],[]);
      }
      
      private function get_temp_players() : Array
      {
         var temp_players:Array = [{
            id:0,
            team:TeamSide.NONE,
            name:"Cibbie&Huide4",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Cibbie&Huide",
            kills:23,
            deaths:18,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Cibbie&Huide2",
            kills:23,
            deaths:18,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Cibbie&Huide3",
            kills:23,
            deaths:18,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.NONE,
            name:"Where\'sYourWaterDude",
            kills:12,
            deaths:43,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Krepish1998",
            kills:1,
            deaths:12,
            ping:22,
            artifacts:33,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Tongar",
            kills:78,
            deaths:2,
            ping:22,
            artifacts:12,
            rank:int(Math.random() * 99)
         },{
            id:0,
            team:TeamSide.ALLY,
            name:"Ashtar",
            kills:0,
            deaths:14,
            ping:22,
            artifacts:12,
            rank:int(Math.random() * 99)
         }];
         return temp_players;
      }
      
      protected function test_victory_screen() : void
      {
         PlayerInfo.nickname = "Where\'sYourWaterDude";
         this.inventory_list.visible = true;
         this.update_profile_slots_count(2,1);
         this.show_victory_screen();
         setTimeout(this.test_victory_screenData,1000);
      }
      
      protected function test_squad_messsage4() : void
      {
      }
      
      protected function test_squad_messsage() : void
      {
         trace("test_squad_messsage");
         this.squadList.visible = true;
         this.set_squad_commander_mode(true);
         this.set_squad_player(0,"ilya@vostok.com",22,false);
         this.set_squad_player(1,"leha2",22,true);
         this.set_squad_player_type(0,6,149);
         this.set_squad_player_type(1,7,77);
         this.set_squad_player_type(2,8,14);
         var data:Object = {
            id:2,
            sender_name:"Timka2",
            message_body:"Vazzap2",
            type:SquadInviteList.SQUAD_INVITE
         };
         this.add_important_message(data);
      }
      
      protected function test_squad_messsage2() : void
      {
         this.remove_squad_player(0);
         this.remove_squad_player(1);
         this.remove_squad_player(2);
         this.set_squad_commander_mode(false);
         trace("test_squad_messsage2");
         stage.addEventListener(FocusEvent.FOCUS_IN,this.logFocud);
         this.remove_tab(TEST_CHAT_TAB_SECOND);
      }
      
      private function logFocud(e:FocusEvent) : void
      {
         trace("newFF::",e.target);
      }
      
      protected function test_squad_messsage3() : void
      {
         this.set_squad_player(2,"mexanikyu",22,false);
      }
      
      protected function test_someMessage(def:int = 400) : void
      {
         testF = function():void
         {
            show_system_message(0,"quest_message",value2,true);
         };
         trace("test_someMessage");
         var value:Object = {
            faction_id:Faction.SCAVENGERS,
            quest_icon:5,
            quest_label:"ex1TraQuestRRR",
            text:"1Sil\'veple la mue de cfd\nsfsdf sdf sdfsd fsdf sdf sdf sdf sdf sdf sdf suartassone\n  vi sonne juperne \nel cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:6500
            },{
               type:Reward.REPUTATION,
               amount:50,
               param:Faction.SCAVENGERS
            },{
               type:Reward.MONEY,
               amount:50,
               param:2
            }],
            tasks:[{
               text:"*KILL asdsadasdasdasdasd asd asdsa dsa dasdas dasdas das dasd asd asd asd asd asd as",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:2
            },{
               text:"*KILL asdsadasdasdasdasd asd asdsa dsa dasdas dasdas das dasd asd asd asd asd asd as",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            status:QuestStatus.COMPLETED
         };
         var value2:Object;
         var testF:Function;
         value2 = {
            faction_id:Faction.BLACK_MARKET,
            quest_icon:6,
            quest_label:"ex1TraQuestRRR",
            text:"1Sil\'veple la mue de cfd\nsfsdf sdf sdfsd fsdf sdf sdf sdf sdf sdf sdf suartassone\n  vi sonne juperne \nel cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:7500
            },{
               type:Reward.REPUTATION,
               amount:150,
               param:Faction.SCAVENGERS
            },{
               type:Reward.MONEY,
               amount:150,
               param:2
            }],
            tasks:[{
               text:"*KILL asdsadasdasdasdasd asd asdsa dsa dasdas dasdas das dasd asd asd asd asd asd as",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:2
            },{
               text:"*KILL asdsadasdasdasdasd asd asdsa dsa dasdas dasdas das dasd asd asd asd asd asd as",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            status:QuestStatus.FAILED
         };
         this.show_system_message(0,"quest_message",value,true);
         setTimeout(testF,def);
      }
      
      protected function onMouseStageClick(e:Event) : void
      {
         trace("stageClicked::",e.type,e.target,e.currentTarget);
         if(e.type == MouseEvent.MOUSE_DOWN)
         {
            return;
         }
         if(e.target == stage)
         {
            stage.focus = null;
         }
      }
      
      protected function getItemTopParent(value:DisplayObject) : DisplayObject
      {
         var target:DisplayObject = value;
         while((target))
         {
            if(target.parent == this || !target.parent || target.parent == stage)
            {
               return target;
            }
            target = target.parent;
         }
         return target;
      }
      
      protected function test_important_messsage() : void
      {
         trace("test_important_messsage");
         for(var i:int = 0; i < 1; i++)
         {
            var value:Object;
            value = new Object();
            value.id = TEST_IMPORTANT_MESSAGE_ID;
            value.request = true;
            value.sender_name = "VikgNickName";
            value.message_body = "\'МнеПлеватЙожики\'";
            value.type = ImportantMessageType.CLAN_INVITE;
            this.add_important_message(value);
         }
      }
      
      protected function test_important_messsage2() : void
      {
         this.remove_important_message(TEST_IMPORTANT_MESSAGE_ID);
      }
      
      protected function test_quest_messsage(e:Event = null) : void
      {
         this.clear_quests();
         trace("test_quest_messsage");
         this.add_quest({
            status:QuestStatus.OFFERED,
            faction_id:Faction.SCAVENGERS,
            quest_icon:0,
            quest_label:"ex1TraQuestRRR",
            text:"1Sil\'veple la mue de cuartassone vi sonne juperne \nel cracasson si\nnme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:1800
            },{
               type:Reward.REPUTATION,
               amount:140,
               param:Faction.SCAVENGERS
            },{
               type:Reward.MONEY,
               amount:50,
               param:2
            }],
            task:{
               text:"Убить за бой 5 представителей бродяг",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            },
            tasks:[{
               text:"Вы должны быт одеты в экипировку Черного Рынка",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:50
         });
         this.add_quest({
            status:QuestStatus.OFFERED,
            faction_id:Faction.BLACK_MARKET,
            quest_icon:1,
            quest_label:"Охота за головами",
            text:"Нельзя сказать, что бродяги наши злейшие враги. Но, время от времени, их нужно ставить на место. Лучше всего это сделать на наглядном примере.",
            reward:[{
               type:Reward.REPUTATION,
               amount:40,
               param:Faction.SCAVENGERS
            },{
               type:Reward.EXPERIENCE,
               amount:40
            },{
               type:Reward.MONEY,
               amount:40,
               param:2
            }],
            task:{
               text:"Убить за бой 5 представителей бродяг",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            },
            tasks:[{
               text:"Убить за бой 5 представителей бродяг",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            },{
               text:"Вы должны быт одеты в экипировку Черного Рынка",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:60 * 120
         });
         this.add_quest({
            status:QuestStatus.OFFERED,
            faction_id:Faction.RENAISSANCE,
            quest_icon:2,
            quest_label:"ex3TraQuestRRR",
            text:"sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:30
            },{
               type:Reward.REPUTATION,
               amount:30,
               param:Faction.SCAVENGERS
            },{
               type:Reward.MONEY,
               amount:30,
               param:2
            }],
            task:{
               text:"*KILL BILL3",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:1
            },
            tasks:[{
               text:"*aaaa bbbb fg df dfg dfg dfg fdg fdg dfg fg dfg dfgdf gd dfg dfgdf gdfg dfg dfg dfg d121 123 12312312",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            },{
               text:"*vvvvv",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:15
         });
         this.add_quest({
            status:QuestStatus.OFFERED,
            faction_id:Faction.RENAISSANCE,
            quest_icon:3,
            quest_label:"ex4TraQuestRRR%%%",
            text:"4Sil\'veple la mue de cuartassone vi sonne juperne el cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:20,
               param:0
            },{
               type:Reward.REPUTATION,
               amount:20,
               param:Faction.BLACK_MARKET
            },{
               type:Reward.MONEY,
               amount:20,
               param:2
            }],
            task:{
               text:"*KILL BILL4",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:1
            },
            tasks:[{
               text:"*KILL SRERE",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:20
         });
         this.add_quest({
            status:QuestStatus.OFFERED,
            faction_id:Faction.RENAISSANCE,
            quest_icon:4,
            quest_label:"ex5TraQuestRRR",
            text:"5Sil\'veple la mue de cuartassone vi sonne juperne el cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:10
            },{
               type:Reward.REPUTATION,
               amount:10,
               param:Faction.BLACK_MARKET
            },{
               type:Reward.MONEY,
               amount:10,
               param:2
            },{
               type:Reward.EXPERIENCE,
               amount:10,
               param:66
            }],
            task:{
               text:"*KILL BILL5",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:1
            },
            tasks:[{
               text:"*KILL BILL5",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:30
         });
         this.add_quest({
            status:QuestStatus.STATUS_1,
            faction_id:Faction.BLACK_MARKET,
            quest_icon:4,
            quest_label:"ex5TraQuestRRR",
            text:"5Sil\'veple la mue de cuartassone vi sonne juperne el cracasson sinme fur majleminto.",
            reward:[{
               type:Reward.EXPERIENCE,
               amount:10
            },{
               type:Reward.REPUTATION,
               amount:10,
               param:Faction.BLACK_MARKET
            },{
               type:Reward.MONEY,
               amount:10,
               param:2
            },{
               type:Reward.EXPERIENCE,
               amount:10,
               param:66
            }],
            task:{
               text:"*KILL BILL5",
               hint_text:"ARE YOU STUPID? KILL THE FUCKING BILL",
               progress:0,
               amount:1
            },
            tasks:[{
               text:"*KILL BILL5",
               hint_text:"ARE YOU STUPID? KILL \nTHE FUCKING BILL",
               progress:0,
               amount:2
            }],
            info:"WarningText",
            time_limit:30
         });
      }
      
      protected function test_chat_messsage_add(e:Event = null) : void
      {
         this.add_chat_message({
            type:ChatChannel.GENERAL,
            text:"FFF fg dfg dfg d dfgfd g dfg44xx " + this.messageIndex,
            name:"leha",
            color:"Blue"
         });
         this.add_chat_message({
            type:ChatChannel.GENERAL,
            text:"FFF fg dfg dfg d dfgfd g dfg44xx " + this.messageIndex,
            name:"[VG]leha"
         });
         this.add_chat_message({
            type:ChatChannel.GENERAL,
            text:"FFF fg dfg dfg d dfgfd g dfg44xx " + this.messageIndex,
            name:"[admin]leha"
         });
      }
      
      protected function test_chat_messsage_add_stop() : void
      {
         if(this.messageTimeOut != -1)
         {
            clearTimeout(this.messageTimeOut);
            messageTimeOut = -1;
         }
      }
      
      protected function test_chat_messsage_second(e:Event = null) : void
      {
         var i:int = 4;
         this.add_tab({
            id:i + TEST_CHAT_TAB_ID,
            name:"newOne" + i,
            channel_to_send:ChatChannel.MATCH,
            closable:false,
            channels:[ChatChannel.MATCH,ChatChannel.TEAM_1,ChatChannel.TEAM_2]
         });
      }
      
      protected function test_chat_messsage(e:Event = null) : void
      {
         for(var i:uint = 0; i < 3; i++)
         {
            this.add_tab({
               id:TEST_CHAT_TAB_ID,
               name:"leha" + i,
               channel_to_send:ChatChannel.MATCH,
               closable:(i == 2 ? false : true),
               channels:[ChatChannel.MATCH,ChatChannel.TEAM_1,ChatChannel.TEAM_2]
            });
         }
      }
      
      public function set_vouchers_data(value:Array) : void
      {
         PlayerInfo.setVouchers(value);
      }
      
      public function set_active_faction(icon:uint, faction_name:String) : void
      {
         trace("Lobby:: set_active_faction::",icon,faction_name);
         this.statusPanel.active_faction = icon;
         this.statusPanel.active_faction_name = faction_name;
      }
      
      public function set_player_speed(value:uint) : void
      {
         this.player_profile.setPlayerSpeed(value);
      }
      
      public function set_char_stats(array:Array) : void
      {
         this.charWindow.set_char_stats(array);
      }
      
      public function set_autobuy(value:uint) : void
      {
         trace("set_auto_buy::",value);
         this.player_profile.checkbox_autobuy.selected = value == AUTO_BUY_AND_REPAIR || value == AUTO_BUY;
      }
      
      public function try_buy_item(profile:uint, uid:uint, dict_uid:uint, sourceSlot:uint, targetSlot:uint, count:int, price:uint, seller:int = -1, onComplete:Function = null, ownItemMenu:Boolean = false) : Boolean
      {
         trace("Try_to_Buy_pr:" + profile + " uid:" + dict_uid + " src:" + sourceSlot + " tgt:" + targetSlot + " cnt:" + count + " price:" + price + " seller:" + seller);
         var _obj:Object = new Object();
         var _descr:Object = this._itemDescriptor.getItemDescription(dict_uid);
         if(sourceSlot == ItemPlace.BUNDLE)
         {
            itemData = this.inventory_list.getBundleData(dict_uid);
            _descr = {
               icon:itemData.iconId,
               is_stack:itemData.isStack,
               sellers:itemData.sellers,
               name:itemData.name,
               is_premium:itemData.isPremium
            };
         }
         if(count != -1)
         {
            _obj.count = 1;
         }
         else
         {
            _obj.count = int(((_descr.is_premium) ? PlayerInfo.premiumMoney : PlayerInfo.genericMoney) / price);
         }
         var lockedPremiumCost:uint = this.shop_list.getItemLockedCost(dict_uid);
         var icons:IconContainer;
         _obj.price = (lockedPremiumCost) ? lockedPremiumCost : price;
         _obj.seller = seller != Faction.ANY ? seller : this.shop_list.getItemSeller(dict_uid);
         _obj.faction = _obj.seller;
         _obj.sellers = _descr.sellers;
         // the buy dialog's starting count reads it (MessageAsk.startCount); a bundle has none
         _obj.category = _descr.category;
         _obj.uid = dict_uid;
         _obj.name = _descr.name;
         _obj.is_stack = _descr.is_stack;
         _obj.weight = _descr.weight;
         if((_descr.clip_size))
         {
            _obj.clip_size = _descr.clip_size;
         }
         _obj.quick_slot_item_size = (_descr.fills_whole_pocket) ? QuickSlotSize.WHOLE : QuickSlotSize.HALF;
         _obj.currentCount = this.inventory_list.getItemCount(dict_uid);
         var category:int = ItemFactory.getCategory(_descr.category);
         if(sourceSlot == ItemPlace.BUNDLE)
         {
            icons = IconContainer.getInstance("bundles");
         }
         else if(category == SuperCategory.EQUIPMENT)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(category == SuperCategory.WEAPONS)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(category == SuperCategory.UPGRADES)
         {
            if(_descr.category == ItemCategory.VOUCHERS || _descr.category == ItemCategory.GENERIC_MONEY || _descr.category == ItemCategory.PREMIUM_MONEY)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(_descr.category == ItemCategory.DECALS)
            {
               icons = IconContainer.getInstance("decals");
            }
            else
            {
               icons = IconContainer.getInstance("camo");
            }
         }
         else
         {
            icons = IconContainer.getInstance("consumables");
         }
         var iconIndex:int = 0;
         if(sourceSlot == ItemPlace.BUNDLE)
         {
            iconIndex = _descr.icon;
         }
         else
         {
            iconRow = _descr.icon_row - 1;
            iconColumn = _descr.icon_column - 1;
            iconIndex = iconColumn * icons.rows + iconRow;
         }
         var icon:Sprite = icons.getIcon(iconIndex);
         var itemData:Object;
         var iconRow:int;
         var iconColumn:int;
         _obj.icon = icon;
         _obj.sourceSlot = sourceSlot;
         _obj.targetSlot = targetSlot;
         _obj.isPremium = lockedPremiumCost || _descr.is_premium;
         if(targetSlot != ItemPlace.INVENTORY)
         {
            _obj.onYes = this.relocate_to_paperdoll;
         }
         if(onComplete != null)
         {
            _obj.onYes = onComplete;
         }
         trace("try_buy_info::",_descr.is_stack,_descr.name,this.inventory_list.hasItem(dict_uid),onComplete);
         // Every buy comes here (GangShop.buyFunction, MainScene.buyFunction). An item owned already,
         // not a stack, is asked first (the shop, the card's Buy, a drop, the craft and attachment
         // windows), but for Buy from the right-click menu of an item the player owns (ownItemMenu:
         // ItemCard.buyFromOwnMenu), a wish for another. A bundle's id is no item's
         // (docs/notes/items.md §6).
         if(!ownItemMenu && !_descr.is_stack && sourceSlot != ItemPlace.BUNDLE && this.inventory_list.hasItem(dict_uid))
         {
            _obj.text = LocalizationData.getName("already_in_inventory");
            _obj.onYes = this.show_buy_message;
            _obj.onComplete = onComplete;
            Message.showMessage("apply",_obj,true);
         }
         else
         {
            Message.showMessage("ask",_obj,true);
         }
         return true;
      }
      
      private function relocate_to_paperdoll(value:Object = null) : void
      {
         trace("relocate_to_paperdoll::valueIs",value.id,value.itemId);
         var id:uint = (value.id);
         var profile:uint = (this.player_profile.profileId);
         var dict_id:uint = (value.uid);
         var sourceSlot:uint = (value.sourceSlot);
         var targetSlot:uint = (value.targetSlot);
         var count:uint = (value.count);
         var faction_id:uint = (value.seller);
         trace("relocate_to_paperdoll::",profile,id,dict_id,sourceSlot,targetSlot,count,"faction",faction_id,value.isStack,value.is_stack);
         var relocate_object:Object = Relocation.move(profile,id,dict_id,sourceSlot,targetSlot,count);
         relocate_object.faction_id = faction_id;
         var new_data:ItemData;
         var _relocateArray:Array;
         var relocate:Function;
         if((value.is_stack))
         {
            relocate_object.condition_or_stack = this.inventory_list.getItemCount(dict_id);
            new_data = null;
            if(!id)
            {
               new_data = this.inventory_list.createItem(dict_id);
               if(ProfileSlot.isPocket(targetSlot))
               {
                  // A buy dropped on a pocket: the stack holds what is owned and what is bought (the
                  // ammo window's units wanted in all: the server buys what is beyond the stack,
                  // move_items), the pocket as many of them as it takes; the rest stays in the
                  // inventory (docs/notes/items.md §8).
                  new_data.count = uint(relocate_object.condition_or_stack) + count;
                  new_data.value = int(Math.min(new_data.count,value.quick_slot_item_size == QuickSlotSize.WHOLE ? 1 : PlayerInfo.pocketSize));
               }
               else
               {
                  new_data.value = count;
                  new_data.count = count;
               }
               new_data.slotId = targetSlot;
               trace("createNewData::",dict_id,"inv::",count,"|",new_data.count);
            }
            AmmoShareLite.getInstance().updateElement(relocate_object,new_data);
         }
         else
         {
            _relocateArray = [relocate_object];
            // Read first, then called: Scaleform throws "is not a function" when the
            // class's getter is called in place (as DragRegion does it).
            relocate = this.player_profile._relocateFunction;
            relocate(_relocateArray);
         }
      }
      
      private function show_buy_message(value:Object) : void
      {
         if(value.targetSlot != ItemPlace.INVENTORY)
         {
            value.onYes = this.relocate_to_paperdoll;
         }
         else if((value.onComplete))
         {
            value.onYes = value.onComplete;
         }
         Message.showMessage("ask",value,true);
      }
      
      public function set_item_props_min_max_values(array:Array) : void
      {
         ItemCompare.setItemsMaxValues(array);
      }
      
      public function set_localization_data(value:Object) : void
      {
         LocalizationData.setData(value);
      }
      
      public function is_typing() : Boolean
      {
         var result:Boolean = stage.focus is TextField && (stage.focus as TextField).type == TextFieldType.INPUT;
         return result;
      }
      
      public function get_mouse_over() : Boolean
      {
         if(this.interface_hidden())
         {
            return false;
         }
         var topMost:* = Extensions.getTopMostEntity(mouseX * scaleX,mouseY * scaleY,true);
         if((topMost) && (!(topMost is DragRegion)) && !(topMost is Stage) && topMost != this._sceneFade)
         {
            return true;
         }
         return false;
      }
      
      public function set_profile_cost(value:uint, money:uint = 0) : void
      {
         this.statusPanel.setProfileCost(value,money);
      }
      
      public function set_player_region(posX:uint, posY:uint, pos2X:uint, pos2Y:uint) : void
      {
         posX /= scaleFactor;
         posY /= scaleFactor;
         pos2X /= scaleFactor;
         pos2Y /= scaleFactor;
         this.dragRegion.x = posX;
         this.dragRegion.y = posY;
         this.dragRegion.scaleX = (pos2X - posX) / 350;
         this.dragRegion.scaleY = (pos2Y - posY) / 450;
      }
      
      public function set_items_compatibility(value:Array) : void
      {
         ItemFactory.setItemsCompatibility(value);
      }
      
      public function set_profile_slots_restrictions(value:Array) : void
      {
         this.player_profile.paperDoll.setSlotsRestrictions(value);
      }
      
      public function set_upgrade_slots_restrictions(value:Array) : void
      {
         this.modifierWindow.setSlotsRestrictions(value);
      }
      
      public function select_player_profile(value:uint) : void
      {
         this.profileSelector.selectProfile(value);
      }
      
      public function setup_player_profiles(value:Array) : void
      {
         trace("setup_player_profiles",(value) ? value.length : null);
         _playerProfiles = value;
         this.profileSelector.setupProfiles(value);
      }
      
      public function fill_player_profile_items(value:Object) : void
      {
         _profileItems = value;
         this.player_profile.fillProfileItems(value);
      }
      
      public function fill_player_data(value:Object) : void
      {
      }
      
      public function set_equipment_level(value:uint) : void
      {
         // The player's own level: the rooms' make form starts its max level there.
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.setGear(int(value));
         }
      }
      
      public function update_profile_slots_count(artifactSlots:uint, itemSlots:uint) : void
      {
         trace("update_profile_slots_count::",artifactSlots,itemSlots);
         this.player_profile.updateSlots(artifactSlots,itemSlots);
      }
      
      public function update_player_items_weight(cur:Number, max:Number, movement:Number = NaN, spending:Number = NaN, sprint:Number = NaN, skills:Number = NaN, bearArms:Number = NaN,
         base:Number = NaN, limitSkills:Number = NaN, packrat:Number = NaN, spending19:Number = NaN, spending67:Number = NaN) : void
      {
         trace("load_weigth:",cur,max);
         PlayerInfo.maxWeight = max;
         PlayerInfo.curWeight = cur;
         this.player_profile.updateWeight(cur,max,WeightEffects.parse(movement,spending,sprint,skills,bearArms,base,limitSkills,packrat,spending19,spending67));
      }
      
      // The selected kit's own player modifiers (the skills' sums), sent before each weight
      // callback (client-patches/weight_ui_026g.c); a weapon card's Alt view adds them.
      public function set_kit_modifiers(... values) : void
      {
         this.player_profile.setKitModifiers(KitModifiers.parse(values));
      }
      
      public function set_minimum_ammo(value1:uint, value2:uint) : void
      {
      }
      
      public function set_ammo_bags_count(value:uint) : void
      {
         PlayerInfo.ammoBags = value;
         this.player_profile.updateAmmoBags(value);
      }
      
      public function set_quick_slot_size(value:uint) : void
      {
         PlayerInfo.pocketSize = value;
         this.player_profile.updatePocketSize(value);
      }
      
      public function fill_auto_buy_items(value:Array) : void
      {
         trace("fill_auto_buy_items::",value);
         this.player_profile.fillAutoBuyItems(value);
         this.player_profile.checkbox_autobuy.selected = value.length != 0;
      }
      
      public function set_account_name(value:String) : void
      {
         if(PlayerInfo.nickname != "")
         {
            this.replacePlayerName(PlayerInfo.nickname,value);
         }
         PlayerInfo.nickname = value;
         this.statusPanel.setPlayerName(value);
         // A lobby sign-in: the rooms' create window's start is asked anew.
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.signedIn(value);
         }
      }
      
      public function set_account_money(value:Object) : void
      {
         PlayerInfo.genericMoney = value.generic_money;
         PlayerInfo.premiumMoney = value.premium_money;
         this.statusPanel.setMoneyData(value.generic_money,value.premium_money);
         this.inventory_list.onPlayerMoneyUpdate();
         this.clanWindow.createClanRenderer.updateClanCreationEnabled();
      }
      
      public function setPlayerInfo(value:Object) : void
      {
         PlayerInfo.nickname = value.nickname;
         PlayerInfo.genericMoney = value.generic_money;
         PlayerInfo.premiumMoney = value.premium_money;
         this.statusPanel.setMoneyData(value.generic_money,value.premium_money);
         this.statusPanel.setPlayerName(value.nickname);
         this.inventory_list.onPlayerMoneyUpdate();
      }
      
      public function set_ui_element_lock_state(element_id:uint, locked:Boolean, arg:uint = 0) : void
      {
         if(element_id == PAPERDOLL_ELEMNENT_ID)
         {
            this.player_profile.lock(locked,arg);
            this.dragRegion.mouseEnabled = !locked;
            this.item_control.checkButtonEnabled();
         }
         else if(element_id == CHARWINDOW_ELEMNENT_ID)
         {
            this.charWindow.lock(locked);
         }
         else if(element_id == PLAYER_ELEMNENT_ID)
         {
            this.dragRegion.mouseEnabled = !locked;
            if(locked && this.playerLoadingIndicator.currentFrame == 1)
            {
               this.backBlack.alpha = 0;
               this.playerLoadingIndicator.alpha = 0;
               TweenLite.to(this.backBlack,0.1,{alpha:1});
               TweenLite.to(this.playerLoadingIndicator,0.1,{alpha:1});
               addChildAt(this.backBlack,0);
               this.backBlack.width = stageWidth;
               this.backBlack.height = stageHeight;
               this.playerLoadingIndicator.visible = true;
               this.playerLoadingIndicator.gotoAndPlay(1);
            }
            else
            {
               removeChild(this.backBlack);
               this.playerLoadingIndicator.visible = false;
               this.playerLoadingIndicator.gotoAndStop(1);
            }
         }
         else
         {
            this.inventory_list.lock(element_id,locked);
            this.item_control.checkButtonEnabled();
         }
      }
      
      public function apply_char_stats(value:Array) : void
      {
         this.charWindow.apply_char_stats(value);
      }
      
      public function name_change_answer(value:Boolean, message:String) : void
      {
      }
      
      public function setup_shop_items(value:Array) : void
      {
         this.shop_list.setupShopItems(value);
      }
      
      public function setup_level_data(value:Array) : void
      {
         this.shop_list.setupLevelData(value);
      }
      
      public function setup_shop_data(seller:uint, value:Array, index:uint, name:String, reputation:uint) : void
      {
         this.shop_list.setupShopDataLevel(seller,value,index,name,reputation);
      }
      
      public function setup_player_progress(seller:uint, level:uint, progress:uint, disableText:String = "") : void
      {
         this.shop_list.setPlayerProgress(seller,level,progress,disableText);
      }
      
      public function show_sync() : void
      {
         var sync:SrvSyncClip = this._top_container.getChildByName("sync") as SrvSyncClip;
         if((sync))
         {
            return;
         }
         // it covers the lobby: the transient windows (over the layers) close
         Transient.closeAll();
         var _back_back:MovieClip = new BackBlack();
         this._top_container.addChild(_back_back);
         _back_back.width = stage.stageWidth;
         _back_back.height = stage.stageHeight;
         sync = new SrvSyncClip();
         sync.name = "sync";
         _back_back.name = "backblack";
         this.placeSync(sync);
         this._top_container.addChild(sync);
         stage.addEventListener(Event.RESIZE,this.updateSyncScreen);
      }
      
      private function updateSyncScreen(e:Event) : void
      {
         var sync:SrvSyncClip = this._top_container.getChildByName("sync") as SrvSyncClip;
         var _back_back:MovieClip = this._top_container.getChildByName("backblack") as MovieClip;
         _back_back.width = stage.stageWidth;
         _back_back.height = stage.stageHeight;
         this.placeSync(sync);
      }
      
      // The sync clip is on the stage (in _top_container, which is not scaled):
      // drawn at the lobby's scale, in the middle of the screen.
      private function placeSync(sync:SrvSyncClip) : void
      {
         var scale:Number = MainScene.overlayScale;
         sync.scaleX = sync.scaleY = scale;
         sync.x = stage.stageWidth / 2 - sync.width / 2 + 30 * scale;
         sync.y = stage.stageHeight / 2 - sync.height / 2;
      }
      
      public function close_sync() : void
      {
         trace("close_sync");
         var _back_back:MovieClip = this._top_container.getChildByName("backblack") as MovieClip;
         this._top_container.removeChild(_back_back);
         var sync:SrvSyncClip = this._top_container.getChildByName("sync") as SrvSyncClip;
         this._top_container.removeChild(sync);
         sync.clean();
         stage.removeEventListener(Event.RESIZE,this.updateSyncScreen);
      }
      
      public function set_player_level(value:uint) : void
      {
         this.statusPanel.player_level = value;
         PlayerInfo.currentLevel = value;
      }
      
      public function has_active_message() : Boolean
      {
         return Message.isAnyMessage() != -1 || MainScene.activePop != null;
      }
      
      // The exe asks this on every key (0x26CB80) and, on Esc, calls close_active_window when it
      // says true; false lets Esc open the game's menu. True: a modal is open (Modals) or a text
      // box has the keys.
      public function has_active_window() : Boolean
      {
         // the hidden interface is the top: Esc shows it (close_active_window)
         if(this.interface_hidden())
         {
            return true;
         }
         // a transient window (History, a dropdown's list) is over the modals: Esc closes it first
         var isAnyWindow:Boolean = !this._escapeHandled && (Transient.any() || Modals.any() || stage.focus is TextField);
         _escapeHandled = false;
         return isAnyWindow;
      }

      // The one Esc handler (docs/notes/esc.md), from the exe and from handleInput, the same press
      // handled once (_escapeHandled: handleInput takes the press's own key events). Esc first
      // closes the top transient window (History, the match settings, a dropdown's list), then
      // takes the keys from a text box (unless the right-click menu is open), then closes the
      // topmost modal only: every modal (windows, popups, the Upgrade window, the rooms window,
      // messages, the right-click menu...) enters Modals as it opens and leaves it as it closes.
      public function close_active_window() : void
      {
         _escapeHandled = true;
         trace("close_active_window");
         if(this.showInterface())
         {
            _hiddenKeyUp = Keyboard.ESCAPE;
            return;
         }
         if(Transient.escape())
         {
            return;
         }
         if(!SrvContextMenu.visisble && this.leaveTextBox())
         {
            return;
         }
         Modals.escape();
      }

      private function leaveTextBox() : Boolean
      {
         // The item list's search box (docs/notes/item-search.md): Esc in it empties it and leaves
         // it. Under a message or a popup Esc only leaves the box; the next Esc closes the message.
         if(this.inventory_list.searchFocused())
         {
            if(!Message.isMessage(Modals.top()) && (MainScene.activePop == null || MainScene.activePop == this.modifierWindow))
            {
               this.inventory_list.clearSearch();
               return true;
            }
            stage.focus = this.inventory_list;
            return true;
         }
         if(stage.focus is TextField)
         {
            if(stage.focus.parent is TextInput)
            {
               stage.focus = stage.focus.parent.parent;
            }
            else
            {
               stage.focus = stage.focus.parent;
            }
            return true;
         }
         return false;
      }

      // The friends list (F) is a modal while open: Esc closes it.
      private function friendsToggled() : void
      {
         if(!this.friendsMenu.visible)
         {
            Modals.leave(this.friendsMenu);
            return;
         }
         Modals.open(this.friendsMenu,Modals.LOBBY,function():void
         {
            friendsMenu.visible = false;
            bottom_panel.btnFriends.selected = false;
            Modals.leave(friendsMenu);
         },function():Boolean
         {
            return friendsMenu.visible;
         });
      }
      
      public function show_character_toggle() : void
      {
         if(this.windowKeysOff())
         {
            return;
         }
         if(!this.charWindow.visible)
         {
            this.topMenu.selectActiveWindow(this.topMenu.CHAR_INDEX);
         }
         else
         {
            this.topMenu.selectActiveWindow(-1);
         }
      }
      
      // The exe's window keys (character, friends, shop, inventory): not over a message, and not
      // over an item list that takes typing, where letters are its search's (InventoryList.onSearchKey;
      // the exe acts on a window key before the movie sees it).
      // A window key that acts may open an item list before the movie sees the same key: the search
      // then leaves that key alone (InventoryList.windowKeyActed).
      private function windowKeysOff() : Boolean
      {
         if(this.has_active_message() || this.inventory_list.visible && this.inventory_list.takesTyping())
         {
            return true;
         }
         this.inventory_list.windowKeyActed();
         return false;
      }
      
      public function show_friends_toggle() : void
      {
         trace("show_friends_toggle");
         if(this.windowKeysOff())
         {
            return;
         }
         this.friendsMenu.visible = !this.friendsMenu.visible;
         this.bottom_panel.btnFriends.selected = this.friendsMenu.visible;
         this.friendsToggled();
      }
      
      public function show_shop_toggle() : void
      {
         if(this.windowKeysOff())
         {
            return;
         }
         if(!this.inventory_list.visible || this.inventory_list.visible && this.inventory_list.mainTabBar.selectedIndex != InventoryTab.SHOP)
         {
            this.inventory_list.mainTabBar.selectedIndex = InventoryTab.SHOP;
            this.topMenu.selectActiveWindow(this.topMenu.SHOP_INDEX);
         }
         else
         {
            this.topMenu.selectActiveWindow(-1);
         }
      }
      
      public function show_inventory_toggle() : void
      {
         if(this.windowKeysOff())
         {
            return;
         }
         if(!this.inventory_list.visible || this.inventory_list.visible && this.inventory_list.mainTabBar.selectedIndex != InventoryTab.INVENTORY)
         {
            if(this.topMenu.inventoryLocked())
            {
               return;
            }
            this.inventory_list.mainTabBar.selectedIndex = InventoryTab.INVENTORY;
            this.topMenu.selectActiveWindow(this.topMenu.INV_INDEX);
         }
         else
         {
            this.topMenu.selectActiveWindow(-1);
         }
      }
      
      public function show_quests_toggle() : void
      {
         if(this.has_active_message())
         {
            return;
         }
         if(!this.questWindow.visible)
         {
            this.topMenu.selectActiveWindow(this.topMenu.QUESTS_INDEX);
         }
         else
         {
            this.topMenu.selectActiveWindow(-1);
         }
      }
      
      public function test_daily_reward_window() : void
      {
         var getDayRewardData:Function = function(value:int):Object
         {
            var dropData:Object = {
               dict_id:(value == 2 ? 18 : 185),
               condition_or_stack:100,
               rarity_level:0
            };
            dropData.progress = value;
            return dropData;
         };
         var tempDailyRewardData:Object = {
            current_day:4,
            reward:4,
            reward_item:{
               dict_id:46,
               condition_or_stack:12
            },
            data:[getDayRewardData(1),getDayRewardData(2),getDayRewardData(3),getDayRewardData(2),getDayRewardData(0),getDayRewardData(0),getDayRewardData(2)]
         };
         this.set_daily_bonus_data(tempDailyRewardData);
      }
      
      public function set_daily_bonus_data(value:Object) : void
      {
         var progress:int;
         _dailyBonusData = value;
         if((value))
         {
            progress = (value.data[value.current_day - 1]) ? (value.data[value.current_day - 1].progress) : 0;
            this.statusPanel.set_day_progress(progress);
         }
      }
      
      public function show_daily_reward_window(value:Boolean = true) : void
      {
         if(!this._dailyBonusData)
         {
            return;
         }
         if(value == false)
         {
            if(Message.isAnyMessage() != -1)
            {
               Message.close(Message.isAnyMessage());
            }
            return;
         }
         var newData:Object = new Object();
         var itemsArray:Array = [];
         newData.data = itemsArray;
         newData.current_day = this._dailyBonusData.current_day;
         newData.reward = this._dailyBonusData.reward;
         var messData:Array = this._dailyBonusData.data as Array;
         trace("messData::",messData.length);
         for(var i:uint = 0; i < messData.length; i++)
         {
            var itemData:ItemData;
            var dropItem:Object;
            var dayData:Object;
            var dayIndex:int;
            var rank:int;
            dropItem = messData[i];
            dayData = new Object();
            if(!dropItem)
            {
               dropItem = new Object();
               messData[i] = dropItem;
            }
            dayIndex = i + 1;
            if(dayIndex == messData.length)
            {
               dayData.last = true;
            }
            dayData.index = i;
            dayData.progressRequired = DAILY_REWARD_PROGRESS_REQUIRED;
            dayData.progress = dropItem.progress;
            dayData.completed = dayIndex < this._dailyBonusData.current_day || dayIndex == this._dailyBonusData.current_day && dropItem.progress == dayData.progressRequired;
            dayData.dayLabel = LocalizationData.getName("day") + " " + dayIndex.toString();
            dayData.showProgress = true;
            itemsArray.push(dayData);
         }
         newData.completed = this._dailyBonusData.completed;
         trace("dailyBonusData.reward_item.dict_id",this._dailyBonusData.reward_item.dict_id);
         if((this._dailyBonusData.reward_item.dict_id))
         {
            itemData = this.inventory_list.createItem(this._dailyBonusData.reward_item.dict_id);
            if((this._dailyBonusData.reward_item.modifiers))
            {
               itemData.modifiers = (this._dailyBonusData.reward_item.modifiers as Array).concat(itemData.modifiersDefault);
            }
            itemData.value = this._dailyBonusData.reward_item.condition_or_stack;
            rank = (this._dailyBonusData.reward_item.rarity_level);
            itemData.rank = rank;
            itemData.clipSize = 1;
            this.inventory_list.fillModifiersDescription(itemData);
            if((itemData.itemDescription && itemData.superCategory == SuperCategory.UPGRADES) && (itemData.category != ItemCategory.GENERIC_MONEY) && itemData.category != ItemCategory.PREMIUM_MONEY)
            {
               itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.itemDescription;
            }
            else
            {
               itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.modifiersDescription;
            }
         }
         else
         {
            itemData = null;
         }
         newData.itemData = itemData;
         Message.showMessage("daily_reward",newData);
      }
      
      public function set_drop_items_properties(value:Array, maxValue:uint) : void
      {
         this.victory_screen.setDropUnlockPositions(value,maxValue);
      }
      
      public function fill_victory_screen_drop_item(value:Object) : void
      {
         var dropItem:Object = value;
         var itemData:ItemData = this.inventory_list.createItem(dropItem.dict_id);
         var rank:int;
         trace("fill_victory_screen_drop_item::",itemData);
         if((itemData))
         {
            if((dropItem.modifiers))
            {
               itemData.modifiers = (dropItem.modifiers as Array).concat(itemData.modifiersDefault);
            }
            trace("itemData::",itemData.name,itemData.dictId,itemData.modifiers,"|",itemData.modifiersDefault,"filledModifiers::",dropItem.modifiers);
            itemData.value = dropItem.condition_or_stack;
            rank = (dropItem.rarity_level);
            itemData.rank = rank;
            itemData.clipSize = 1;
            this.inventory_list.fillModifiersDescription(itemData);
            if((itemData.itemDescription && itemData.superCategory == SuperCategory.UPGRADES) && (itemData.category != ItemCategory.GENERIC_MONEY) && itemData.category != ItemCategory.PREMIUM_MONEY)
            {
               itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.itemDescription;
            }
            else
            {
               itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.modifiersDescription;
            }
            itemData.enabled = true;
         }
         else
         {
            itemData = new ItemData();
            itemData.dictId = dropItem.dict_id;
         }
         this.victory_screen.setDropItem(itemData);
         this.victory_screen.getData().treasureItem = itemData;
      }
      
      public function fill_victory_screen(value:Object) : void
      {
         trace("fill_victory_screen::data",value);
         if((value.dropped_item))
         {
            trace("fill_victory_screen::",value.dropped_item);
            if((value.dropped_item.dict_id))
            {
               dropItem = value.dropped_item;
               itemData = this.inventory_list.createItem(dropItem.dict_id);
               if((itemData))
               {
                  if((dropItem.modifiers))
                  {
                     itemData.modifiers = (dropItem.modifiers as Array).concat(itemData.modifiersDefault);
                  }
                  trace("droppedItem::",itemData.name,itemData.category,itemData.dictId,itemData.modifiers,"|",itemData.modifiersDefault,"filledModifiers::",dropItem.modifiers);
                  rank = (dropItem.rarity_level);
                  itemData.rank = rank;
                  itemData.value = dropItem.condition_or_stack;
                  itemData.count = dropItem.value;
                  value.dropChance = dropItem.value;
                  value.description = dropItem.description;
                  value.penalty = dropItem.penalty;
                  itemData.clipSize = 1;
                  itemData.enabled = true;
                  this.inventory_list.fillModifiersDescription(itemData);
                  trace("modifiersDescription::",itemData.modifiersDescription);
                  if((itemData.itemDescription && itemData.superCategory == SuperCategory.UPGRADES) && (itemData.category != ItemCategory.GENERIC_MONEY) && itemData.category != ItemCategory.PREMIUM_MONEY)
                  {
                     itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.itemDescription;
                  }
                  else
                  {
                     itemData.tipText = Util.styledChar(itemData.name,"white") + "\n" + itemData.modifiersDescription;
                  }
               }
               else
               {
                  itemData = null;
               }
            }
            else
            {
               trace("noDroppedItem");
               value.dropChance = value.dropped_item.value;
               value.description = value.dropped_item.description;
               value.penalty = value.dropped_item.penalty;
               itemData = null;
            }
            value.dropped_item = itemData;
         }
         else
         {
            value.dropChance = 0;
            value.description = [];
            value.penalty = 0;
            itemData = null;
            value.dropped_item = itemData;
         }
         var progressData:Object = this.questWindow.getFactionProgress(value.statistics_reputation.faction_id);
         value.factionCurrent = progressData.faction_current;
         value.factionTotal = progressData.faction_next;
         value.playerCurrent = PlayerInfo.experienceCurrent;
         value.playerTotal = PlayerInfo.experienceNext;
         var date:Date = new Date();
         var dateM:int = date.minutes;
         var dateH:int = date.hours;
         var minutes:Number = date.time / (1000 * 60);
         var time:String = Util.getZeroString(dateH) + ":" + Util.getZeroString(dateM);
         var dropItem:Object;
         var itemData:ItemData;
         var rank:int;
         trace("initVSTime::",minutes,time,dateM,dateH);
         value.time = time;
         this.victory_screen.setData(value);
         // A room with a PIN's match takes no drop card (`rooms.md`).
         if(RoomsWindow.me != null && RoomsWindow.me.inPinRoom)
         {
            this.victory_screen.noCard(value.match_id);
         }
         value.notificationType = NotificationType.MATCH;
         this.add_notification(value);
         this.updateReport(this.victory_screen.getData());
         this.set_log_list(this._reports);
         if((value.treasureItem))
         {
            this.fill_victory_screen_drop_item(value.treasureItem);
         }
      }
      
      public function setup_bundle_data(value:Array) : void
      {
         trace("setup_bandle_data::",value.length);
         this.inventory_list.setupBundleData(value);
      }
      
      public function setup_inventory_data(value:Array) : void
      {
         trace("setup_inventory_data::",value.length);
         this.inventory_list.setupInventoryData(value);
         if(this.victory_screen.visible)
         {
            this.victory_screen.updateVoucherCount();
         }
      }
      
      public function setup_item_craft_prices_premium(value:Array) : void
      {
         trace("setup_item_craft_prices_premium",value);
         MessageCraftItem.setCraftPremiumCost(value);
      }
      
      public function setup_item_craft_prices_general(value:Array) : void
      {
         trace("setup_item_craft_prices_general",value);
         MessageCraftItem.setCraftRegularCost(value);
      }
      
      public function set_craft_regular_cost(cost:uint) : void
      {
         MessageCraftItem.setCraftRegularCost([cost]);
      }
      
      public function set_craft_premium_cost(cost:uint) : void
      {
         MessageCraftItem.setCraftPremiumCost([cost]);
      }
      
      public function setup_item_craft_chances(value:Array) : void
      {
         trace("setup_item_craft_chances",value);
         MessageCraftItem.setCraftChances(value);
      }
      
      public function update_inventory_item(value:Object) : void
      {
         this.inventory_list.updateInventoryItem(value);
      }
      
      public function fill_victory_screen_player(value:Object) : void
      {
         if(this.takePlayerInfo(value))
         {
            return;
         }
         this.victory_screen.setPlayerData(value);
      }
      
      public function show_victory_screen() : void
      {
         this.showInterface();
         this.showRoomUnderResults();
         dispatchEvent(new MouseEvent(MouseEvent.MOUSE_UP,true));
         trace("_top_container::this");
         this.notificationListTemp.stopTimer();
         Message.hideMessages();
         addChild(this._back_back);
         this._back_back.width = LobbyMenu.stageWidth;
         this._back_back.height = LobbyMenu.stageHeight;
         addChild(this.victory_screen);
         trace("Lobby::show_victory_screen");
         stage.focus = this.victory_screen;
         FocusManager.setModalClip(this.victory_screen);
         this.victory_screen.visible = true;
         this.victory_screen.addEventListener(Event.REMOVED_FROM_STAGE,this.onVsRemoved);
         this.victory_screen["prepare"]();
         MainScene.layout.reflow();
         this.topMenu.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.closeVS);
         this.chat.scrollTabsToLastMessage();
      }
      
      private function closeVS(e:IndexEvent) : void
      {
         this.victory_screen.closeWindow();
      }
      
      public function set_disk_query(value:uint) : void
      {
         this.statusPanel.query = value;
      }
      
      public function show_disk_query(value:Boolean) : void
      {
         this.statusPanel.show_query = value;
      }
      
      public function set_ping(value:uint) : void
      {
         this.statusPanel.ping = value;
      }
      
      public function set_players_online(value:uint) : void
      {
         this.statusPanel.players = value;
      }
      
      public function set_games_online(value:uint) : void
      {
         this.statusPanel.games = value;
      }
      
      public function set_status_message(value:String, firstTime:Boolean = false) : void
      {
         this.statusPanel.setStatusMessage(value,firstTime);
      }
      
      // the Play button's state (PlayButtonState), for Enter's Play (lobbyKey)
      private var _playState:uint = PlayButtonState.PLAY;
      
      public function set_play_button_state(value:uint) : void
      {
         _playState = value;
         var label:String = null;
         if(value == PlayButtonState.PLAY)
         {
            label = "Play";
         }
         else if(value == PlayButtonState.LEAVE)
         {
            label = "btnLeave";
         }
         else if(value == PlayButtonState.READY)
         {
            label = "btn_ready";
         }
         else
         {
            label = "btn_not_ready";
         }
         this.statusPanel.playBtn.label = LocalizationData.getName(label).toUpperCase();
      }
      
      public function set_play_button_enabled(bool:Boolean) : void
      {
         stackTrace("set_play_button_enabled::" + bool.toString());
         this.statusPanel.playBtn.enabled = bool;
         if(bool == true && !this.statusPanel.lobbyGameSearch.visible)
         {
            this.topMenu.lockInventory(false);
         }
         // In a room or a room's queue Play is the rooms' Leave (on);
         // otherwise the game's own word (StatusPanel.showRoomsPlay).
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.playEnabled(this,this.statusPanel.playBtn.enabled);
         }
      }
      
      public function set_add_profile_button_enabled(value:Boolean) : void
      {
      }
      
      public function set_add_profile_button_hint(value:String) : void
      {
      }
      
      public function update_profile_experience(value:uint, experience_current:uint, experience_next_level:uint) : void
      {
      }
      
      protected function updateReport(value:Object) : void
      {
         var rewards:Array = [];
         trace("updateReport",value);
         if(!value)
         {
            return;
         }
         rewards.push({
            type:Reward.RATING,
            amount:value.rating
         });
         rewards.push({
            type:Reward.EXPERIENCE,
            amount:value.exp
         });
         var reputations:Object = value.statistics_reputation;
         var reputation:Object;
         if((reputations))
         {
            reputation = reputations;
            rewards.push({
               type:Reward.REPUTATION,
               amount:reputation.statistics_reputation,
               param:reputation.faction_id
            });
         }
         trace("reportAddMOney::",value.money,reputations);
         rewards.push({
            type:Reward.MONEY,
            amount:value.money,
            param:MoneyType.REGULAR
         });
         value.reward = rewards;
         this.notificationList.updateNotification(value);
         this.notificationListTemp.updateNotification(value);
      }
      
      public function add_report(value:Object) : void
      {
         this.updateReport(value);
         this._reports.push(value);
      }
      
      public function remove_report(report_id:uint) : void
      {
         for(var i:uint = 0; i < this._reports.length; i++)
         {
            if(this._reports[i].id == report_id)
            {
               this._reports.splice(i,1);
               break;
            }
         }
      }
      
      public function replacePlayerName(nickname:String, newValue:String) : void
      {
         if(!this._reports)
         {
            return;
         }
         var reportL:uint = this._reports.length;
         for(var i:uint = 0; i < reportL; i++)
         {
            var players:Array;
            var playersL:uint;
            var j:uint;
            players = this._reports[i].players;
            playersL = players.length;
            for(j = 0; j < playersL; j++)
            {
               if(players[i].name == nickname)
               {
                  players[i].name = newValue;
               }
            }
         }
      }
      
      public function set_log_list(value:Array) : void
      {
      }
      
      private function test_clan_widow() : void
      {
         this.set_clan_progress(500,2500,517,10,2);
      }
      
      public function set_clan_name(name:String, abb:String) : void
      {
         this.clanWindow.setClanName(name,abb);
         this.statusPanel.setPlayerClanAbb(abb);
      }
      
      public function set_clan_window_state(value:uint) : void
      {
         PlayerInfo.clanState = value;
         this.clanWindow.setState(value);
         this.statusPanel.matchSettings.updateClanStatus();
         this.statusPanel.setPlayerClanState(value);
      }
      
      public function set_clan_players_list(value:Array) : void
      {
         this.clanWindow.setPlayerData(value);
      }
      
      public function set_clan_progress(minimum:uint, maximum:uint, current:uint, players:uint, level:uint) : void
      {
         this.clanWindow.manageClanRenderer.setClanProgress(minimum,maximum,current,players,level);
      }
      
      public function set_clan_manage_permissions(value:Object) : void
      {
         trace("set_clan_manage_permissions",value.battle_commander_permission);
         PlayerInfo.isTeamCommander = value.battle_commander_permission;
         PlayerInfo.isClanIviter = value.invite_permission;
         this.clanWindow.manageClanRenderer.setManagePermissions(value);
      }
      
      public function set_clan_ranks(value:Array, freeSlotsAvailable:Boolean = true) : void
      {
         this.clanWindow.manageClanRenderer.setRanksData(value,freeSlotsAvailable);
      }
      
      public function set_clan_daily_message(value:String) : void
      {
         this.clanWindow.manageClanRenderer.setDailyMessage(value);
      }
      
      public function set_clan_reservation_enabled(value:Boolean) : void
      {
         this.clanWindow.createClanRenderer.btnReserveClanName.enabled = value;
      }
      
      public function set_clan_creation_enabled(value:Boolean) : void
      {
         this.clanWindow.createClanRenderer.btnAccept.enabled = value;
      }
      
      public function set_clan_registration_paid(value:Boolean) : void
      {
         this.clanWindow.createClanRenderer.registrationPaid = value;
      }
      
      public function set_clan_players_required(value:uint) : void
      {
         this.clanWindow.createClanRenderer.setClanPlayersRequired(value);
      }
      
      public function set_clan_ready_players(value:uint, total:uint = 10) : void
      {
         this.clanWindow.createClanRenderer.totalPlayers = total;
         this.clanWindow.createClanRenderer.clanPlayers = value;
      }
      
      public function set_clan_creation_money_collected(value:uint = 0) : void
      {
         this.clanWindow.createClanRenderer.setClanCreationCostProgress(value);
      }
      
      public function set_clan_creation_cost(value:uint = 0) : void
      {
         this.clanWindow.createClanRenderer.setClanCreationCost(value);
      }
      
      public function add_notification(value:Object) : void
      {
         trace("add_notification");
         this.notificationList.addNotification(value);
         if(this.notificationList.visible == false)
         {
            this.notificationListTemp.addNotification(value);
         }
         this.bottom_panel.btnShowLogList.enabled = true;
      }
      
      public function add_important_message(value:Object) : void
      {
         var st:String;
         var mess:*;
         trace("addImportantmessage",value);
         for(st in value)
         {
            trace(st + ":" + value[st]);
         }
         ExternalInterface.call("sound_play",UiSound.INVITE);
         value.run = true;
         value.request = true;
         value.name = value.sender_name;
         if(value.type == ImportantMessageType.FRIEND_REQUEST || value.type == undefined)
         {
            this._friendInvites.unshift(value);
            value.status = FriendStatus.INVITE;
            value.status_text = value.message_body;
            this.friendsMenu.setInvites(this._friendInvites);
            this.bottom_panel.btnFriends.invites = this._friendInvites.length;
         }
         else if(value.type == ImportantMessageType.PREMIUM_ACTIVATED)
         {
            value.label = LocalizationData.getName("action_premium_activated");
            value.notificationType = NotificationType.PREMIUM;
            value.reward = [{
               type:Reward.MONEY,
               amount:this._postRewardData.money,
               param:MoneyType.REGULAR
            },{
               type:Reward.EXPERIENCE,
               amount:this._postRewardData.experience
            },{
               type:Reward.REPUTATION,
               amount:this._postRewardData.reputation,
               param:this._postRewardData.factionId
            }];
            value.hideTimer = 15000;
            this.add_notification(value);
         }
         else if(value.type == ImportantMessageType.REFERRAL_REWARD)
         {
            value.label = Util.styledChar(value.nickname,"gray_highlight") + "\n" + LocalizationData.getName("reach_level") + " " + Util.styledChar(value.level,"green");
            value.notificationType = NotificationType.INVITED_REWARD;
            value.hideTimer = 15000;
            this.add_notification(value);
         }
         else if(value.type == ImportantMessageType.SKILL_POINT_REWARD)
         {
            value.label = Util.styledChar(LocalizationData.getName("level") + " " + value.skill_level + " ","highlight") + LocalizationData.getName("achieved");
            value.hideTimer = 15000;
            value.notificationType = NotificationType.PLAYER_RANK;
            value.reward = [{
               type:Reward.MONEY,
               amount:value.reward_amount,
               param:value.reward_id
            },{
               type:Reward.POINT,
               amount:1
            }];
            this.add_notification(value);
            mess = Message.showMessage("message_unlock_level",value,false);
            mess.addEventListener(Event.OPEN,this.openCharItem,false,0,true);
         }
         else if(value.type == ImportantMessageType.REPUTATION_LEVEL)
         {
            value.items = GangShop.getUnlockedItems(value.faction_id,value.level);
            trace("value.items",value.items);
            value.factionName = "«" + GangShop.getSellerName(value.faction_id) + "»";
            value.rankName = GangShop.getSellerRank(value.faction_id);
            value.text = value.message_body;
            value.label = value.text;
            value.hideTimer = 15000;
            value.notificationType = NotificationType.FACTION_RANK;
            this.add_notification(value);
            if(!value.items || value.items.length == 0)
            {
               trace("waitForItems");
               _unlockData = value;
               this.shop_list.addEventListener(GangShop.NEW_ITEMS_COUNT_CHANGED,this.onNewItemsCountChangeShop,false,0,true);
            }
            else
            {
               mess = Message.showMessage("message_unlock_rank",value,false,MessageId.RANK_UNLOCK + this._messageIndex);
               mess.addEventListener(Event.OPEN,this.openUnlockedItem,false,0,true);
            }
         }
         else
         {
            if(value.type == ImportantMessageType.CLAN_INVITE || value.type == ImportantMessageType.CLAN_TEAM_INVITE)
            {
               value.text = "";
               value.label = value.sender_name;
               value.name = value.sender_name + "\n" + value.message_body;
            }
            else
            {
               value.text = value.message_body;
               value.label = value.sender_name;
            }
            trace("addImportantmessage::",value.text,value.label);
            this.invitesContainer.addInvite(value);
            this.updateInvitesPos();
         }
      }
      
      private function onNewItemsCountChangeShop(e:Event) : void
      {
         trace("onNewItemsCountChangeShop");
         this._unlockData.items = GangShop.getUnlockedItems(this._unlockData.faction_id,this._unlockData.level);
         this.shop_list.removeEventListener(GangShop.NEW_ITEMS_COUNT_CHANGED,this.onNewItemsCountChangeShop);
         var mess:* = Message.showMessage("message_unlock_rank",this._unlockData,false,MessageId.RANK_UNLOCK + this._messageIndex);
         mess.addEventListener(Event.OPEN,this.openUnlockedItem,false,0,true);
      }
      
      private function openCharItem(e:Event) : void
      {
         this.topMenu.selectActiveWindow(this.topMenu.CHAR_INDEX);
      }
      
      private function openUnlockedItem(e:Event) : void
      {
         var mess:Object = e.target;
         var data:Object = mess.getMessageData();
         var selectedFaction:int;
         trace("openUnlockedItem",data);
         if((data))
         {
            this.topMenu.selectActiveWindow(this.topMenu.SHOP_INDEX);
            this.inventory_list.optionControl.reset();
            this.inventory_list.inventoryTabPage.selectedIndex = 0;
            if(this.inventory_list.checkbox_all_traders.selected)
            {
               this.inventory_list.checkbox_all_traders.selected = false;
               this.inventory_list.optionControl.dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
            }
            selectedFaction = (data.faction_id);
            this.inventory_list.sellersTabPage.selectedIndex = selectedFaction;
            this.inventory_list.selectItemDict(data.dictId);
         }
      }
      
      public function remove_important_message(message_id:uint) : void
      {
         var removeType:int = -1;
         for(var i:uint = 0; i < this._friendInvites.length; i++)
         {
            if(this._friendInvites[i].id == message_id)
            {
               removeType = (this._friendInvites[i].type);
               this._friendInvites.splice(i,1);
               break;
            }
         }
         trace("remove_important_message",message_id,removeType);
         if(removeType == ImportantMessageType.FRIEND_REQUEST)
         {
            this.friendsMenu.setInvites(this._friendInvites);
            this.bottom_panel.btnFriends.invites = this._friendInvites.length;
         }
         else
         {
            this.invitesContainer.removeInvite(message_id);
         }
      }
      
      public function fill_players_search(value:Array) : void
      {
         trace("fill_players_search::",value.length,this._playerRequestTarget);
         this._playerRequestTarget.fillFoundPlayers(value);
      }
      
      public function set_ignored_list(value:Array) : void
      {
         this.friendsMenu.setPlayers(value,false);
      }
      
      protected function test_friends_list() : void
      {
         this.set_friends_list(this._friends);
      }
      
      public function set_friends_list(value:Array) : void
      {
         _friends = value;
         this.chat.setPlayers(value);
         this.friendsMenu.setPlayers(value);
         this.bottom_panel.btnFriends.label = String(this.friendsMenu.getPlayersOnline());
      }
      
      public function fill_skills_progress(value:Array) : void
      {
         this.charWindow.fillSkillsProgress(value);
      }
      
      public function fill_skills_properties(value:Array) : void
      {
         trace("root.fill_skills_properties");
         this.charWindow.fillSkillsProperties(value);
      }
      
      public function fill_skills(value:Array) : void
      {
         trace("root.fill_skills");
         this.charWindow.fillSkills(value);
      }
      
      public function set_achievements(value:Array) : void
      {
         this.charWindow.setAchievements(value);
      }
      
      public function create_perk_trees(value:Array) : void
      {
         this.charWindow.createPerkTrees(value);
      }
      
      public function create_perk_tree(value:Object) : void
      {
         this.charWindow.createPerkTree(value);
      }
      
      public function set_faction_bonus(value:Array) : void
      {
         this.shop_list.setFactionBonus(value);
      }
      
      public function fill_char_info(value:Object) : void
      {
         var useCurrent:Boolean = value.experience_next_level == 0 || value.experience_next_level == undefined;
         var nextExp:uint = useCurrent ? (value.experience_current) : (value.experience_next_level);
         trace("fill_char_info",value.experience_current,value.experience_next_level,nextExp,"useCurrent::",useCurrent,uint.MAX_VALUE);
         this.statusPanel.player_progress = 100 * (value.experience_current / nextExp);
         this.statusPanel.progress_hint = String(value.experience_current) + "/" + String(nextExp);
         PlayerInfo.experienceCurrent = value.experience_current;
         PlayerInfo.experienceNext = nextExp;
         this.set_player_level(value.points_unlocked);
         this.charWindow.setData(value);
      }
      
      public function set_reroll_cost(value:uint, value_prem:uint = 0) : void
      {
         this.charWindow.reroll_cost = value;
         this.charWindow.reroll_cost_premium = value_prem;
      }
      
      public function set_change_name_properties(delay:uint, failed_delay:uint) : void
      {
         this.statusPanel.changeNameDelay = delay;
         this.statusPanel.changeNameFailedDelay = failed_delay;
      }
      
      public function set_change_name_cost(value:uint) : void
      {
         this.statusPanel.changeNameCostPremium = value;
      }
      
      public function show_quest_selector(value:Array) : void
      {
         Message.showMessage("quest_selector",value);
      }
      
      public function add_quest(value:Object) : void
      {
         trace("LOBBY::add_quest");
         this.questWindow.addQuest(value);
      }
      
      public function update_quest(value:Object) : void
      {
         this.questWindow.updateQuest(value);
      }
      
      public function remove_quest(quest_id:uint) : void
      {
         this.questWindow.removeQuest(quest_id);
      }
      
      public function clear_quests() : void
      {
         trace("LOBBY::clear_quests");
         this.questWindow.clearQuests();
      }
      
      public function ranking_set_data(value:Array) : void
      {
         this.ranking.fillPlayers(value);
      }
      
      public function ranking_set_player(value:Object) : void
      {
         this.ranking.player_data = value;
      }
      
      public function ranking_set_players_total_count(value:uint) : void
      {
         this.ranking.totalPlayers = value;
      }
      
      public function start_message(value:String, timeout:uint = 100) : void
      {
         _lasm = value;
         if(this._tm != -1)
         {
            clearTimeout(this._tm);
         }
         _tm = setTimeout(this.actualInput,timeout);
      }
      
      public function active_tab_id() : uint
      {
         return this.chat.activeTabId();
      }
      
      public function show_chat(bool:Boolean) : void
      {
         this.chat.visible = bool;
      }
      
      public function set_mode(mode:uint) : void
      {
         this.chat.setMode(mode);
      }
      
      public function finish_typing_message() : void
      {
         this.chat.submitText();
      }
      
      public function focus_tab(id:uint) : void
      {
         this.chat.focusTab(id);
      }
      
      public function add_chat_message(value:Object = null) : void
      {
         if((value))
         {
            this.chat.got_message(value);
         }
      }
      
      public function clear_channel(value:uint) : void
      {
         this.chat.clearChannel(value);
      }
      
      public function set_local_player(value:String) : void
      {
         this.chat.setLocalPlayer(value);
      }
      
      public function add_tab(data:Object) : void
      {
         trace("chat_add_tab",data.id);
         this.chat.addTab(data);
         this.bottom_panel.addTab(data);
      }
      
      public function remove_tab(id:uint) : void
      {
         trace("removeTabAA");
         this.chat.removeTab(id);
         this.bottom_panel.removeTab(id);
      }
      
      public function set_channels(value:Array) : void
      {
         this.chat.setData(value);
      }
      
      public function focus_chat(bool:Boolean) : void
      {
         if(this.chat.visible)
         {
            this.chat.ingnoreFoucOut();
            this.chat.tabChildren = bool;
            if(bool)
            {
               stage.focus = this.chat.text_inpput.textField;
            }
            else
            {
               stage.focus = null;
            }
         }
      }
      
      public function next_tab() : void
      {
         if(this.bottom_panel.tabsBar.selectedIndex == this.bottom_panel.tabsBar.dataProvider.length - 1)
         {
            this.bottom_panel.tabsBar.selectedIndex = 0;
            this.bottom_panel.tabsBar.validateNow();
         }
         else
         {
            this.bottom_panel.tabsBar.selectedIndex += 1;
            this.bottom_panel.tabsBar.validateNow();
         }
      }
      
      public function previous_tab() : void
      {
         if(this.bottom_panel.tabsBar.selectedIndex == 0)
         {
            this.bottom_panel.tabsBar.selectedIndex = this.bottom_panel.tabsBar.dataProvider.length - 1;
         }
         else
         {
            this.bottom_panel.tabsBar.selectedIndex -= 1;
         }
      }
      
      public function set_mm_sync_state(value:String, showLeaveButton:uint = 0) : void
      {
         this.showInterface();
         _hiddenKeyUp = 0;
         if((value) && !showLeaveButton)
         {
            ExternalInterface.call("sound_play",UiSound.MATCH_FOUND);
         }
         this.statusPanel.lobbyGameSearch.sync_text = value;
         if(showLeaveButton == 1)
         {
         }
         else
         {
            this.topMenu.lockInventory(true);
            this.dragRegion.mouseEnabled = false;
            stage.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_UP));
            // the match is found: the message up goes (closed, not answered as Esc would)
            if(Message.isAnyMessage() != -1)
            {
               Message.close(Message.isAnyMessage());
            }
         }
      }
      
      public function set_mm_current_time(value:uint) : void
      {
         this.statusPanel.lobbyGameSearch.updateTime(value);
      }
      
      public function set_mm_average_time(value:String) : void
      {
         this.statusPanel.lobbyGameSearch.averageTime = value;
      }
      
      public function set_mm_place(value0:uint = 0, value1:uint = 0) : void
      {
         this.statusPanel.lobbyGameSearch.setPlace(value0,value1);
      }
      
      public function show_loading_screen(bool:Boolean) : void
      {
         this.lobbyGameLoad.visible = bool;
      }
      
      public function set_loading_level(level:uint) : void
      {
         this.showInterface();
         trace("set_loading_level",this._load_images[level]);
         addChild(this.chat);
         trace("_top_container::this");
         this._top_container.visible = false;
         this._layers.addChild(this.lobbyGameLoad);
         stage.focus = null;
         var image:Class = this._load_images[level];
         this.lobbyGameLoad.backgroundImage(image);
         // the match loads: the transient windows close (the loading screen is under their layer),
         // the message up goes (closed, not answered as Esc would), else the top modal closes
         Transient.closeAll();
         if(Message.isAnyMessage() != -1)
         {
            Message.close(Message.isAnyMessage());
         }
         else
         {
            this.close_active_window();
         }
         dispatchEvent(new MouseEvent(MouseEvent.MOUSE_UP,true));
      }
      
      public function switch_mm_to_loading() : void
      {
         this.set_loading_level(0);
         this.set_mm_percent(0);
      }
      
      public function show_match_making(bool:Boolean) : void
      {
         if(this._ignoreMM)
         {
            return;
         }
         this.statusPanel.show_match_making(bool);
         this.lobbyGameLoad.clearPlayers();
         MainScene.activeMatchMaking = bool;
         if((this.craftWindow))
         {
            this.craftWindow.setMatchState(MainScene.activeMatchMaking ? MatchMakingState.SEARCHING : MatchMakingState.IDLE);
         }
         if(!bool)
         {
            if(!this.victory_screen.parent || !this.victory_screen.visible)
            {
               trace("_top_container::stage");
               this._top_container.visible = true;
            }
            this.dragRegion.mouseEnabled = true;
            this.topMenu.lockInventory(false);
         }
      }
      
      public function set_mm_percent(value:uint) : void
      {
         this.lobbyGameLoad.loading.set_percent(value);
      }
      
      public function set_mm_progress(a1:uint, a2:uint) : void
      {
         this.lobbyGameLoad.loading.set_progress(a1,a2);
      }
      
      public function switch_mm_to_def() : void
      {
      }
      
      public function set_mm_level(value:String, mode:String, icon:uint) : void
      {
         this.lobbyGameLoad.level = value;
         this.lobbyGameLoad.setMode(mode,icon);
      }
      
      public function set_mm_mode(value:String, icon:uint) : void
      {
         this.lobbyGameLoad.setMode(value,icon);
      }
      
      public function set_mm_description(value:String) : void
      {
         this.lobbyGameLoad.description = value;
      }
      
      public function set_mm_players(value:Object) : void
      {
         this.lobbyGameLoad.setData(value);
      }
      
      public function set_mm_status(value:String) : void
      {
         this.lobbyGameLoad.status = value;
      }
      
      public function add_mm_player(value:Object, team:uint) : void
      {
         if(value.name == PlayerInfo.nickname)
         {
            value.is_local = true;
         }
         this.lobbyGameLoad.addPlayer(value,team);
      }
      
      public function remove_mm_player(value:String) : void
      {
         this.lobbyGameLoad.removePlayer(value);
      }
      
      public function apply_mm_players() : void
      {
         this.lobbyGameLoad.updateContent();
      }
      
      public function show_system_message(id:Number, type:String, data:Object, fill:Boolean = true) : void
      {
         var icons:IconContainer;
         var factionIcon:Icon;
         if(type == "quest_message")
         {
            icons = IconContainer.getInstance("factionItemIcons");
            factionIcon = icons.getIcon(data.faction_id);
            data.faction_icon = factionIcon;
            this.questWindow.fillRewardItems(data);
            data.label = Util.styledChar(LocalizationData.getName(String("quest_status_" + data.status)),data.status == QuestStatus.FAILED ? "red" : "gold") + Util.styledChar("\n«" + data.quest_label + "»","gray_highlight");
            data.hideTimer = data.status == QuestStatus.FAILED ? 15000 : 15000;
            trace("addFailedQuestNotification");
            data.notificationType = NotificationType.QUEST;
            this.add_notification(data);
            return;
         }
         if((id))
         {
            Message.close(id,true);
         }
         // Over the results screen, now (VictoryScreen.messageOver).
         Message.showMessage(type,data,this.victory_screen.messageOver(type),id);
      }
      
      public function close_system_message(id:uint) : void
      {
         Message.close(id);
      }
      
      private function onPaperDollItemRollOut(e:ListEvent) : void
      {
         if((this.inventory_list.item_compare))
         {
            this.inventory_list.item_compare.setSecondaryWeapon(null);
         }
         this.inventory_list.hoverItem();
         this.inventory_list.hoverScopesFor();
      }
      
      private function onPaperDollItemRollOver(e:ListEvent) : void
      {
         if((this.inventory_list) && (this.inventory_list.item_compare))
         {
            this.inventory_list.item_compare.setSecondaryWeapon(e.itemData);
         }
         if((e) && (e.itemData))
         {
            this.inventory_list.hoverItem(e.itemData.itemId);
            this.linkScopeHovers(e.itemData);
         }
      }
      
      private function onInventoryListItemRollOut(e:ListEvent) : void
      {
         this.player_profile.paperDoll.hoverItem();
         this.player_profile.paperDoll.hoverWeaponsFor();
         this.inventory_list.hoverScopesFor();
      }
      
      private function onInventoryListItemRollOver(e:ListEvent) : void
      {
         trace("onInventoryListItemRollOver",e.itemData);
         if((e.itemData))
         {
            this.player_profile.paperDoll.hoverItem(e.itemData.itemId);
            this.linkScopeHovers(e.itemData);
         }
      }
      
      // A hovered weapon: the listed sights that fit it look hovered; a hovered sight: the kit's
      // weapons it fits (weapon_scopes.md, section 4a).
      private function linkScopeHovers(item:Object) : void
      {
         if(item.isWeapon)
         {
            this.inventory_list.hoverScopesFor(int(item.dictId));
         }
         else if(item.category == ItemCategory.SCOPES)
         {
            this.player_profile.paperDoll.hoverWeaponsFor(int(item.dictId));
         }
      }
      
      private function setlayout() : void
      {
         trace("Stage_values_when_loaded",stage.stageHeight,stage.stageWidth);
         this.lobbyGameLoad.visible = false;
         this.victory_screen.layoutData = new LayoutData();
         this.victory_screen.layoutData.alignV = "center";
         this.victory_screen.layoutData.alignH = "center";
         this.victory_screen.layoutData.offsetV = 0;
         this.victory_screen.layoutData.offsetH = 0;
         this.notificationList.layoutData = new LayoutData();
         this.notificationList.layoutData.alignV = "bottom";
         this.notificationList.layoutData.alignH = "right";
         this.notificationList.layoutData.offsetV = -50;
         this.notificationList.layoutData.offsetH = -20;
         this.notificationListTemp.layoutData = new LayoutData();
         this.notificationListTemp.layoutData.alignV = "bottom";
         this.notificationListTemp.layoutData.alignH = "right";
         this.notificationListTemp.layoutData.offsetV = -50;
         this.notificationListTemp.layoutData.offsetH = -40;
         this.friendsMenu.layoutData = new LayoutData();
         this.friendsMenu.layoutData.alignV = "bottom";
         this.friendsMenu.layoutData.alignH = "left";
         this.friendsMenu.layoutData.offsetV = -55;
         this.friendsMenu.layoutData.offsetH = 55;
         this.squadList.layoutData = new LayoutData();
         this.squadList.layoutData.alignH = "left";
         this.squadList.layoutData.offsetH = -5;
         this.questList.layoutData = new LayoutData();
         this.questList.layoutData.alignH = "right";
         this.questList.layoutData.offsetH = 16;
         this.item_control.layoutData = new LayoutData();
         this.item_control.layoutData.alignH = "center";
         this.item_control.layoutData.offsetH = 0;
         this.modifierWindow.layoutData = new LayoutData();
         this.modifierWindow.layoutData.alignV = "bottom";
         this.modifierWindow.layoutData.alignH = "left";
         this.modifierWindow.layoutData.offsetV = -80;
         this.modifierWindow.layoutData.offsetH = 20;
         this.player_profile.layoutData = new LayoutData();
         this.player_profile.layoutData.alignV = "top";
         this.player_profile.layoutData.alignH = "left";
         this.player_profile.layoutData.offsetV = 123;
         this.player_profile.layoutData.offsetH = 112;
         this.inventory_list.layoutData = new LayoutData();
         this.inventory_list.layoutData.alignV = "top";
         this.inventory_list.layoutData.alignH = "right";
         this.inventory_list.layoutData.offsetV = 123;
         this.inventory_list.layoutData.offsetH = -112;
         this.shop_list.layoutData = new LayoutData();
         this.charWindow.layoutData = new LayoutData();
         this.charWindow.layoutData.alignV = "top";
         this.charWindow.layoutData.alignH = "right";
         this.charWindow.layoutData.offsetV = 123;
         this.charWindow.layoutData.offsetH = -112;
         this.ranking.layoutData = new LayoutData();
         this.ranking.layoutData.alignV = "center";
         this.ranking.layoutData.alignH = "center";
         this.ranking.layoutData.offsetV = 0;
         this.ranking.layoutData.offsetH = 0;
         this.questWindow.layoutData = new LayoutData();
         this.questWindow.layoutData.alignV = "top";
         this.questWindow.layoutData.alignH = "right";
         this.questWindow.layoutData.offsetV = 123;
         this.questWindow.layoutData.offsetH = -112;
         this.clanWindow.layoutData = new LayoutData();
         this.clanWindow.layoutData.alignV = "top";
         this.clanWindow.layoutData.alignH = "right";
         this.clanWindow.layoutData.offsetV = 123;
         this.clanWindow.layoutData.offsetH = -112;
         _layout = new Layout();
         this._layout.beforeReflow = this.updateRubberElements;
         this._layout.tiedToStageSize = true;
         MainScene.layout = this._layout;
         addChild(this._layout);
         this._layout.afterReflow = this.updateRubberElements;
      }
      
      private function clearType(e:Event) : void
      {
         _isWheeled = false;
         removeEventListener(Event.RENDER,this.clearType);
      }
      
      private function onMouseFnx(e:MouseEvent) : void
      {
         if(!this._isWheeled)
         {
            addEventListener(Event.RENDER,this.clearType);
         }
         else
         {
            e.stopImmediatePropagation();
         }
         trace("onMouseFnx",(e.target) ? e.target.name : "null",e.target,(e.currentTarget) ? e.currentTarget.name : "null",(e.target) && (e.target.parent) ? e.target.parent.name : null);
         _isWheeled = true;
         return;
         var crt:* = undefined;
         var point:Point = null;
         var point2:Point = null;
         var point3:Point;
      }
      
      private function updateRubberElements() : void
      {
         var inventoryMaxHeight:int = 0;
         if(LobbyMenu.stageWidth / LobbyMenu.stageHeight < 1.4)
         {
            trace("setBoxLayout",LobbyMenu.stageWidth / LobbyMenu.stageHeight);
         }
         else
         {
            trace("setDefaultlayout",LobbyMenu.stageWidth / LobbyMenu.stageHeight);
         }
         this.invitesContainer.x = this.squadList.x;
         this.updateInvitesPos();
         this._layout.reflow();
      }
      
      private function parseSpriteSheets() : void
      {

         var CamoIcons:Class = LobbyMenu_CamoIcons;
         var bitmapData:BitmapData = get_bitmap(CamoIcons);
         var _camo_i:IconContainer = new IconContainer("camo",72,72,bitmapData,true);
         bitmapData.dispose();
         var AmmoIcons:Class = LobbyMenu_AmmoIcons;
         bitmapData = get_bitmap(AmmoIcons);
         _ammo_i = new IconContainer("consumables",171,83,bitmapData,true,0,0,8,false);
         bitmapData.dispose();
         var DecalsIcons:Class = LobbyMenu_DecalsIcons;
         bitmapData = get_bitmap(DecalsIcons);
         _ammo_i = new IconContainer("decals",82,82,bitmapData,true,0,0,0,false);
         bitmapData.dispose();
         var BundleIcons:Class = LobbyMenu_BundleIcons;
         bitmapData = get_bitmap(BundleIcons);
         var _bundle_i:IconContainer = new IconContainer("bundles",171,92,bitmapData,true,0,0,0,false);
         bitmapData.dispose();
         var VoucherIcons:Class = LobbyMenu_VoucherIcons;
         bitmapData = get_bitmap(VoucherIcons);
         var _voucher_i:IconContainer = new IconContainer("vouchers",171,83,bitmapData,true,0,0,8,false);
         bitmapData.dispose();
         var CamoIconsBig:Class = LobbyMenu_CamoIconsBig;
         bitmapData = get_bitmap(CamoIconsBig);
         var _camo_big_i:IconContainer = new IconContainer("camo_big",140,140,bitmapData,true);
         bitmapData.dispose();
         var WeaponIcons:Class = LobbyMenu_WeaponIcons;
         bitmapData = get_bitmap(WeaponIcons);
         _weapons_i = new IconContainer("weapons",171,83,bitmapData,true,0,0,8,false);
         bitmapData.dispose();
         var EquipmentIcons:Class = LobbyMenu_EquipmentIcons;
         bitmapData = get_bitmap(EquipmentIcons);
         _equipment_i = new IconContainer("equipment",171,83,bitmapData,true,0,0,8,false);
         bitmapData.dispose();
         var ModifierIcons:Class = LobbyMenu_ModifierIcons;
         bitmapData = get_bitmap(ModifierIcons);
         _modifiers_i = new IconContainer("modifiers",38,38,bitmapData,true,0,0,0,false);
         bitmapData.dispose();
         var ProfileIcons:Class = LobbyMenu_ProfileIcons;
         bitmapData = get_bitmap(ProfileIcons);
         _iconContainer_profile = new IconContainer("profileIcons",64,58,bitmapData,true,5);
         bitmapData.dispose();
         var TopPanelIcons:Class = LobbyMenu_TopPanelIcons;
         bitmapData = get_bitmap(TopPanelIcons);
         new IconContainer("topIcons",91,91,bitmapData,false,0,2,2);
         bitmapData.dispose();
         var FactionIcons:Class = LobbyMenu_FactionIcons;
         bitmapData = get_bitmap(FactionIcons);
         new IconContainer("factionIcons",42,42,bitmapData,false,0,1,1);
         bitmapData.dispose();
         var FactionIconsBig:Class = LobbyMenu_FactionIconsBig;
         bitmapData = get_bitmap(FactionIconsBig);
         new IconContainer("factionIconsBig",66,66,bitmapData,false,0,2,2);
         bitmapData.dispose();
         var FactionItemIcons:Class = LobbyMenu_FactionItemIcons;
         bitmapData = get_bitmap(FactionItemIcons);
         new IconContainer("factionItemIcons",20,20,bitmapData,true);
         bitmapData.dispose();
         var FilersIcons:Class = LobbyMenu_FilersIcons;
         bitmapData = get_bitmap(FilersIcons);
         new IconContainer("filter_icons",26,26,bitmapData);
         bitmapData.dispose();
         var FilersOverIcons:Class = LobbyMenu_FilersOverIcons;
         bitmapData = get_bitmap(FilersOverIcons);
         new IconContainer("filter_over_icons",26,26,bitmapData);
         bitmapData.dispose();
         function get_bitmap(className:Class):BitmapData
         {
            var tempBitmap:Bitmap = new className();
            var tempBitmapData:BitmapData = new BitmapData(tempBitmap.width,tempBitmap.height,true,0);
            tempBitmapData.draw(tempBitmap);
            tempBitmap = null;
            return tempBitmapData;
         }
      }
      
      private function onChangeCursor(e:MouseCursorEvent) : void
      {
         var _new:uint;
         if(e.cursor == "ibeam")
         {
            _new = Cursor.IBEAM;
         }
         else if(e.cursor != "arrow")
         {
            _new = Cursor.BUTTON;
         }
         else
         {
            _new = Cursor.ARROW;
         }
         ExternalInterface.call("set_mouse_cursor",_new);
      }
      
      private function onStageResize(event:Event = null) : void
      {
         trace("new_stage_values",stage.stageHeight,stage.stageWidth,stage.stageWidth / stage.stageHeight);
         this.updateRubberElements();
         this._layout.beforeReflow = this.updateRubberElements;
         this._layout.afterReflow = this.updateRubberElements;
         _time = 0;
         if(stage.stageWidth / stage.stageHeight < 1.85)
         {
            scaleFactor = stage.stageWidth / this._superWidth;
         }
         else
         {
            scaleFactor = stage.stageHeight / this._superHeight;
         }
         Message.setScaleFactor(scaleFactor);
         MainScene.scaleFactor = scaleFactor;
         MainScene.overlayScale = Math.max(1,scaleFactor);
         if(this.tipContainer)
         {
            this.tipContainer.scaleX = this.tipContainer.scaleY = MainScene.overlayScale;
         }
         if(DragManager.getDragCanvas())
         {
            DragManager.getDragCanvas().scaleX = DragManager.getDragCanvas().scaleY = MainScene.overlayScale;
         }
         this._front_container.scaleX = this._front_container.scaleY = scaleFactor;
         scaleX = scaleFactor;
         scaleY = scaleFactor;
         // the layers the game first put on the stage keep the stage's scale
         this._stageLayer.scaleX = this._stageLayer.scaleY = 1 / scaleFactor;
         this.placeOverlay();
         trace("finalScaleFactor::",scaleFactor);
         this.statusPanel.onStageResize();
         this.playerLoadingIndicator.alpha = 0.7;
         this.playerLoadingIndicator.scaleX = this.playerLoadingIndicator.scaleY = 0.7;
         this.playerLoadingIndicator.x = stageWidth / 2;
         this.playerLoadingIndicator.y = stageHeight / 2;
         this._layout.setScaleFactor(scaleFactor);
         this.updateTournamentPosition();
         trace("squadList.y = questList.y",this.squadList.y,this.questList.y);
      }
      
      public function set_quest_reward_multiplier(valueReputation:Number, valueMoney:Number) : void
      {
         PlayerInfo.questRewardsMultiplierReputation = valueReputation;
         PlayerInfo.questRewardsMultiplierMoney = valueMoney;
         this.questWindow.questsContent.scrollList.dataProvider.invalidate();
      }
      
      private function showIM(e:ListEvent = null) : void
      {
      }
      
      private function onVsRemoved(e:Event) : void
      {
         this.topMenu.tabPage.removeEventListener(IndexEvent.INDEX_CHANGE,this.closeVS);
         this.victory_screen.removeEventListener(Event.REMOVED_FROM_STAGE,this.onVsRemoved);
         // The rooms window's tab, kept as it was under the results, follows it now (after closeVS
         // is off: its tab changes).
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.resultsClosed();
         }
         removeChild(this._back_back);
         FocusManager.setModalClip(null);
         MainScene.activeWindow = null;
         this.notificationListTemp.startTimer();
         Message.showMessages();
      }
      
      private function onFrClose(e:Event) : void
      {
         this.bottom_panel.btnFriends.selected = !this.bottom_panel.btnFriends.selected;
      }
      
      public function fade_in() : void
      {
         this.fadeBack.width = 2048;
         this.fadeBack.height = 2048;
         this._layers.addChild(this.fadeBack);
         this.fadeBack.alpha = 1;
         TweenLite.to(this.fadeBack,0.5,{
            alpha:0,
            onComplete:this.removeFadeSprite
         });
         stage.addEventListener(Event.RESIZE,this.onStageResizeFade);
      }
      
      private function onStageResizeFade(e:Event) : void
      {
         this.fadeBack.width = 2048;
         this.fadeBack.height = 2048;
      }
      
      private function removeFadeSprite() : void
      {
         if((this.fadeBack) && this._layers.contains(this.fadeBack))
         {
            this._layers.removeChild(this.fadeBack);
         }
      }
      
      public function fade_out(force:Boolean = false) : void
      {
         this.fadeBack.width = 2048;
         this.fadeBack.height = 2048;
         this._layers.addChild(this.fadeBack);
         if(force)
         {
            this.fadeBack.alpha = 0;
            TweenLite.to(this.fadeBack,0.5,{alpha:1});
         }
         else
         {
            this.fadeBack.alpha = 1;
         }
      }
      
      // The lobby's scene changing (exe patch 81, docs/notes/lobby-scenes.md): a black over the 3D
      // scene, under every panel (the root's first child), while the exe puts another lobby in;
      // perMille of black (0 gone), the exe stepping it a frame each; `loading`: the game's loading
      // spinner on it. The interface stays; no click lands on the black (get_mouse_over leaves it
      // out), and it is off the display list while there is none.
      public function srp_scene_fade(perMille:uint, loading:uint) : void
      {
         if(this._sceneFade == null)
         {
            this._sceneFade = new Sprite();
            this._sceneFade.graphics.beginFill(0,1);
            this._sceneFade.graphics.drawRect(-4096,-4096,12288,12288);
            this._sceneFade.graphics.endFill();
            this._sceneFade.mouseEnabled = false;
            this._sceneFade.mouseChildren = false;
            this._sceneSpinner = new LoadingIndicator();
            this._sceneSpinner.mouseEnabled = false;
            this._sceneSpinner.scaleX = this._sceneSpinner.scaleY = 0.7;
            this._sceneFade.addChild(this._sceneSpinner);
         }
         var fade:Sprite = this._sceneFade;
         if(perMille == 0)
         {
            if(fade.parent != null)
            {
               fade.parent.removeChild(fade);
            }
            this._sceneSpinner.stop();
            return;
         }
         fade.alpha = Math.min(perMille,1000) / 1000;
         if(fade.parent != this)
         {
            addChildAt(fade,0);
         }
         // the middle of the screen in the root's units (the root is scaled to the screen)
         if(stage != null)
         {
            this._sceneSpinner.x = stage.stageWidth / scaleX / 2;
            this._sceneSpinner.y = stage.stageHeight / scaleY / 2;
         }
         var spin:Boolean = loading != 0;
         if(spin != this._sceneSpinner.visible)
         {
            this._sceneSpinner.visible = spin;
            if(spin)
            {
               this._sceneSpinner.gotoAndPlay(1);
            }
            else
            {
               this._sceneSpinner.stop();
            }
         }
      }

      // What the lobby window shows (exe patch 81): the player's pick and the lobby shown, told by
      // the exe when one changes.
      public function srp_lobby_state(pick:uint, shown:uint) : void
      {
         this._lobbyState = [pick,shown];
         if(this._lobbyWindow != null)
         {
            this._lobbyWindow.state(pick,shown);
         }
      }

      // The lobby button (StatusPanel.LOBBY_WINDOW): the lobby window opened, or closed again; the
      // exe's diagnostics build opens it so too (srp_lobby_window).
      public function srp_lobby_window() : void
      {
         this.onLobbyWindow(null);
      }

      private function onLobbyWindow(e:Event) : void
      {
         if(this._lobbyWindow != null && this._lobbyWindow.parent != null)
         {
            this.closeLobbyWindow();
            return;
         }
         if(this._lobbyWindow == null)
         {
            // the font of the bottom bar's tabs (the game's own), as the rooms window's
            var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
            var sample:* = new tabClass();
            var tabText:TextField = sample["textField"] as TextField;
            var format:TextFormat = tabText != null ? tabText.getTextFormat() : new TextFormat();
            this._lobbyWindow = new LobbyWindow(format.font,format.bold);
            this._lobbyWindow.onClose = this.closeLobbyWindow;
            if(this._lobbyState != null)
            {
               this._lobbyWindow.state(this._lobbyState[0],this._lobbyState[1]);
            }
         }
         var window:LobbyWindow = this._lobbyWindow;
         if(stage != null)
         {
            window.x = int((stage.stageWidth / scaleX - window.width) / 2);
            window.y = int((stage.stageHeight / scaleY - window.height) / 2);
         }
         addChild(window);
         Modals.open(window,Modals.LOBBY,this.closeLobbyWindow,function():Boolean
         {
            return window.parent != null;
         });
      }

      private function closeLobbyWindow() : void
      {
         if(this._lobbyWindow != null && this._lobbyWindow.parent != null)
         {
            this._lobbyWindow.parent.removeChild(this._lobbyWindow);
         }
         Modals.leave(this._lobbyWindow);
      }

      private function test_clan_mamngment() : void
      {
         var createPermission:Function = function(value:String = null):Object
         {
            var object:Object = {
               name:value,
               invite_permission:true,
               kick_permission:true,
               change_rank_permission:false,
               daily_message_permission:true
            };
            return object;
         };
         this.clanWindow.setState(ClanState.OVERVIEW);
         this.set_clan_daily_message("some Very Awful Daily Message\nsecond line text");
         this.set_clan_ranks([createPermission("RankZero"),createPermission("Rank2"),createPermission("Rank3"),createPermission("Rank4")]);
         this.set_clan_manage_permissions(createPermission());
         this.set_clan_name("superClanName","SCN");
         var players:Array = [];
         players.push({
            name:"Гиганский кролик",
            label:"Status",
            status:0,
            rank:"Рядовой",
            level:23
         });
         for(var i:uint = 0; i < 10; i++)
         {
            players.push({
               name:"Member" + i.toString(),
               label:"Status",
               status:0,
               rank:"Рядовой",
               level:1
            });
         }
         this.set_clan_players_list(players);
      }
      
      private function toggleFriends(e:Event) : void
      {
         trace("toggleFriends::",e.target);
         this.friendsMenu.visible = !this.friendsMenu.visible;
         setChildIndex(this.friendsMenu,numChildren - 1);
         this.friendsToggled();
      }
      
      private function toggleInvites(e:Event) : void
      {
      }
      
      private function toggleReports(e:Event) : void
      {
         this.showHistory(!this.notificationList.visible);
      }

      // History (the bottom bar's button) is a transient window (Transient): open over the
      // windows; any other window opening, a press outside it or Esc closes it as its button does.
      public function showHistory(value:Boolean) : void
      {
         this.notificationList.visible = value;
         this.bottom_panel.btnShowLogList.selected = value;
         if(!value)
         {
            Transient.closed(this.notificationList);
            return;
         }
         this.notificationListTemp.clear(true);
         Transient.open(this.notificationList,this.bottom_panel.btnShowLogList,function():void
         {
            showHistory(false);
         });
      }
      
      private function updateButtonsLocal(value:String) : void
      {
         this.bottom_panel.btnFriends.data = {name:LocalizationData.getName("btn_friends")};
         this.bottom_panel.btnShowLogList.data = {name:LocalizationData.getName("btn_show_log_list_hint")};
         this.bottom_panel.btnShowLogList.label = LocalizationData.getName("btn_show_log_list").toUpperCase();
      }
      
      // The key that hides the interface (docs/notes/hide-ui-key.md): the exe calls this for F11,
      // the game's toggle_replay_ui key (exe patch 62), when no text box has the keys. The whole
      // interface is the root (see _stageLayer): hidden in one step, the character and the scene
      // stay; no click lands on the interface and the camera turns and zooms (get_mouse_over). The
      // item cards and an open right-click menu are closed first. Shown again
      // by the same key, any key the movie gets (onHiddenKey: that key does nothing else), Esc, a
      // match found, its loading or the results.
      public function toggle_interface() : void
      {
         if(this.showInterface())
         {
            return;
         }
         if(SrvContextMenu.visisble)
         {
            SrvContextMenu.hide();
         }
         this.hideItemHint(null);
         visible = false;
         _interfaceHidden = true;
      }

      // The exe asks this on every key in the lobby (exe patch 62): while it is true no key acts in
      // the game (a window's key, F6's Play); the key comes to the movie, which shows the interface.
      public function interface_hidden() : Boolean
      {
         return this._interfaceHidden;
      }

      // Shows what toggle_interface hid; true when it was hidden.
      private function showInterface() : Boolean
      {
         if(!this._interfaceHidden)
         {
            return false;
         }
         visible = true;
         _interfaceHidden = false;
         return true;
      }

      // A key while the interface is hidden shows it, and is taken, its release too: no other
      // handler of the movie gets it (Enter does not press Play nor answer a message hidden with the
      // interface, a letter does not start the item search).
      private function onHiddenKey(e:KeyboardEvent) : void
      {
         if(this.showInterface())
         {
            _hiddenKeyUp = e.keyCode;
            e.stopImmediatePropagation();
         }
         else if(e.keyCode == this._hiddenKeyUp)
         {
            // pressed again: its release was missed (the exe passes no key while a match is found)
            _hiddenKeyUp = 0;
         }
      }

      private function onHiddenKeyUp(e:KeyboardEvent) : void
      {
         if(this._hiddenKeyUp != 0 && e.keyCode == this._hiddenKeyUp)
         {
            _hiddenKeyUp = 0;
            e.stopImmediatePropagation();
         }
      }

      private function onKeyDownStage(e:KeyboardEvent) : void
      {
         if(e.keyCode == KeyCode.LEFT_SHIFT)
         {
            MainScene.isShift = true;
         }
         if(e.keyCode == KeyCode.LEFT_CONTROL || e.keyCode == KeyCode.RIGHT_CONTROL)
         {
            MainScene.isCtrl = true;
         }
         if(e.keyCode == KeyCode.LEFT_ALT || e.keyCode == KeyCode.RIGHT_ALT)
         {
            MainScene.isAlt = true;
         }
         if(e.keyCode == Keyboard.R || e.keyCode == Keyboard.ENTER)
         {
            this.lobbyKey(e);
         }
      }
      
      // the key of lobbyKey that is down (a held key repeats its KEY_DOWN: Play and the rooms take
      // the first only); 0 when none
      private var _lobbyKeyDown:uint;
      
      // Skhid's lobby keys (docs/notes/lobby-keys.md), not while a text box has the keys, a
      // message, popup, right-click menu or the results are up, nor with Ctrl or Alt:
      // - R opens the rooms window, or closes it (not over an item list taking typing: R is its
      //   search's there);
      // - Enter presses Play on the lobby's main screen (no window open, not in a room), once per
      //   press. Up, Down and Enter in the rooms list are RoomsList.onKey's.
      private function lobbyKey(e:KeyboardEvent) : void
      {
         var repeat:Boolean = this._lobbyKeyDown == e.keyCode;
         _lobbyKeyDown = e.keyCode;
         if(repeat || e.ctrlKey || e.altKey || stage.focus is TextField || this.has_active_message() || SrvContextMenu.visisble)
         {
            return;
         }
         if(this.victory_screen != null && this.victory_screen.visible && this.victory_screen.stage != null)
         {
            return;
         }
         if(RoomsWindow.me == null)
         {
            return;
         }
         if(e.keyCode == Keyboard.R)
         {
            if(!(this.inventory_list.visible && this.inventory_list.takesTyping()))
            {
               this.topMenu.toggleRoomsTab();
            }
            return;
         }
         // not in a room or a room's queue (Play is the rooms' Leave then), and with no window, the
         // friends list, the squad search or the tournament open
         if(RoomsWindow.me.shown || RoomsWindow.me.marked() || MainScene.activeWindow != null || MainScene.activeComponent != null || this.friendsMenu.visible || this.craftWindow && this.craftWindow.parent || this._tournament && this._tournament.parent && this._tournament.visible && !this._tournament.isFold())
         {
            return;
         }
         var play:Button = this.statusPanel.playBtn;
         if(this._playState == PlayButtonState.PLAY && play.enabled && play.visible)
         {
            play.dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
         }
      }
      
      private function test_bandle_data() : void
      {
         var arr:Array = [];
         arr.push({
            bundle_id:666,
            bundle_name:"666",
            bundle_descr:"descr 666 is 666",
            cost:666,
            currency_type:MoneyType.PREMIUM,
            bundle_icon:1
         });
         arr.push({
            bundle_id:555,
            bundle_name:"555",
            bundle_descr:"descr 555 is 555",
            cost:555,
            currency_type:MoneyType.REGULAR,
            bundle_icon:0
         });
         this.setup_bundle_data(arr);
      }
      
      private function testTRewards() : void
      {
         this.set_tutorial_rewards([{
            name:"tutorial_completed",
            reward:[{
               type:Reward.ITEM,
               param:ItemDictId.GENERIC_MONEY,
               amount:100
            },{
               type:Reward.EXPERIENCE,
               amount:5
            }]
         },{
            name:"survival_item_notes_completed",
            reward:[{
               type:Reward.ITEM,
               param:ItemDictId.GENERIC_MONEY,
               amount:200
            },{
               type:Reward.EXPERIENCE,
               amount:5
            }]
         },{
            name:"survival_prepare_to_battle_notes_completed",
            reward:[{
               type:Reward.ITEM,
               param:ItemDictId.GENERIC_MONEY,
               amount:300
            },{
               type:Reward.EXPERIENCE,
               amount:5
            }]
         }]);
      }
      
      public function show_premium_discount(dataObject:Object) : void
      {
         var data:Array;
         var discount:Number;
         var messData:Object;
         var i:uint;
         var discount_data:Object;
         data = dataObject.items;
         discount = (dataObject.discount);
         trace("show_premium_discount",data,discount);
         messData = {};
         for(i = 0; i < data.length; i++)
         {
            discount_data = data[i];
            discount_data.discount_value = discount;
            discount_data.value_old = Math.floor(discount_data.value / (1 - discount));
         }
         messData.options = data;
         Message.showMessage("premium_discount",messData);
      }
      
      public function show_premium_post_reward(money:uint, exp:uint, rep:uint, hours:uint = 0, timeLeft:uint = 3600.0, discount:Number = 1, factionId:int = Faction.ANY) : void
      {
         var messData:Object;
         var notif:Object;
         trace("show_premium_post_reward",money,exp,rep,hours,timeLeft,discount,factionId);
         if(factionId == Faction.ANY)
         {
            factionId = PlayerInfo.activeFaction;
         }
         MessagePostPremium.discount = discount;
         messData = {
            money:money,
            experience:exp,
            reputation:rep,
            factionId:factionId,
            options:this._premiumStoreDiscount
         };
         _postRewardData = messData;
         Message.showMessage("post_premium",messData);
         notif = {
            notificationType:NotificationType.PLAIN,
            label:LocalizationData.getName("premium_post_reward_label")
         };
         this._postRewardData.startTime = getTimer();
         if(!this._discountTimer)
         {
            _discountTimer = new Timer(1000);
            this._discountTimer.start();
            this._discountTimer.addEventListener(TimerEvent.TIMER,this.updatePremiumDiscountTime);
         }
         this.statusPanel.postRewardData = this._postRewardData;
         this.statusPanel.premiumDiscount = timeLeft;
         this.add_notification(notif);
      }
      
      private function updatePremiumDiscountTime(e:TimerEvent = null) : void
      {
         var curTime:int;
         var delta:int;
         if(this.statusPanel.premiumDiscount > 0)
         {
            curTime = getTimer();
            delta = Math.round((curTime - this._postRewardData.startTime) / 1000);
            trace("delta::",delta,"diff::",getTimer() - this._postRewardData.startTime);
            if(delta > 0)
            {
               this._postRewardData.startTime = curTime;
               this.statusPanel.premiumDiscount = Math.max(this.statusPanel.premiumDiscount - delta,0);
            }
         }
         if(this.statusPanel.premiumDiscount <= 0)
         {
            if((this._discountTimer))
            {
               this._discountTimer.removeEventListener(TimerEvent.TIMER,this.updatePremiumDiscountTime);
            }
         }
      }
      
      private function onKeyUpStage(e:KeyboardEvent) : void
      {
         if(e.keyCode == KeyCode.LEFT_SHIFT) MainScene.isShift = false;
         trace("KEY_DOWN_CODE_STAGE:",name,e.keyCode,e.target,e.currentTarget,e.isDefaultPrevented(),MainScene.isShift,"stage::",stage.focus);
         if(e.keyCode == KeyCode.LEFT_CONTROL || e.keyCode == KeyCode.RIGHT_CONTROL)
         {
            MainScene.isCtrl = false;
         }
         if(e.keyCode == KeyCode.LEFT_ALT || e.keyCode == KeyCode.RIGHT_ALT)
         {
            MainScene.isAlt = false;
         }
         if(e.keyCode == this._lobbyKeyDown)
         {
            _lobbyKeyDown = 0;
         }
         if(e.isDefaultPrevented())
         {
            return;
         }
         if(e.keyCode == Keyboard.M)
         {
            if(!this.toggle)
            {
            }
            toggle = !this.toggle;
            ++this.toggleIndex;
            return;
         }
         if(e.keyCode == KeyCode.LEFT_SHIFT)
         {
            MainScene.isShift = false;
         }
      }
      
      public function enable_second_weapon(boolean:Boolean) : void
      {
         trace("root.enable_second_weapon",boolean);
         this.player_profile.enableSecondWeapon(boolean);
      }

      private function clearDetailKeys(e:Event) : void
      {
         MainScene.isShift = false;
         MainScene.isCtrl = false;
         MainScene.isAlt = false;
      }
      
      public function allow_use_two_double_handed_weapons(boolean:Boolean) : void
      {
         trace("root.allow_use_two_double_handed_weapons",boolean);
         this.player_profile.allowSecondaryTwoHandedWeapon(boolean);
      }
      
      private function actualInput() : void
      {
         this.chat.srartMessage(this._lasm);
      }
      
      public function handleInput(event:InputEvent) : void
      {
         var details:InputDetails;
         trace("STAGE::handleInput::",event.isDefaultPrevented(),event.handled);
         details = event.details;
         var index:uint = details.controllerIndex;
         if(this._escapeHandled)
         {
            _escapeHandled = false;
            return;
         }
         switch(details.code)
         {
            case Keyboard.ESCAPE:
               if(details.value == InputValue.KEY_DOWN)
               {
                  trace("ESCAPEKeyDown");
                  if(event.isDefaultPrevented())
                  {
                     _escapeHandled = true;
                     addEventListener(Event.RENDER,this.cleanEscapeHandle);
                  }
                  else if(this.has_active_window())
                  {
                     this.close_active_window();
                     _escapeHandled = true;
                     addEventListener(Event.RENDER,this.cleanEscapeHandle);
                  }
               }
               break;
         }
      }
      
      private function cleanEscapeHandle(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.cleanEscapeHandle);
         _escapeHandled = false;
      }
      
      // Shared player actions for the Rankings window and both room lists.
      public static function showPlayerInfoMenu(nickname:String, player:uint, how:uint, owner:DisplayObject = null) : void
      {
         if(nickname == null || nickname == "")
         {
            return;
         }
         var options:Array = [{
            name:LocalizationData.getName("show_player_info_button_hint"),
            click_function:function(e:* = null):void
            {
               askPlayerInfo(nickname,player,how);
            }
         }];
         if(nickname != PlayerInfo.nickname)
         {
            var friend:Object = FriendsMenu.getFriend(nickname);
            options.push({
               name:LocalizationData.getName(friend != null ? "remove_from_friends" : "add_to_friends"),
               click_function:function():void
               {
                  if(friend != null)
                  {
                     ExternalInterface.call("remove_friend",uint(friend.id));
                  }
                  else
                  {
                     ExternalInterface.call("add_friend_name",nickname);
                  }
               }
            });
            options = options.concat(ChatMute.options(nickname));
            options.push({
               name:LocalizationData.getName("send_message"),
               click_function:function():void
               {
                  ExternalInterface.call("start_friend_message",nickname);
               }
            });
            if(PlayerInfo.commander && !PlayerInfo.isSquadFull && !SquadList.isInSquad(nickname))
            {
               options.push({
                  name:LocalizationData.getName("invite_to_squad"),
                  click_function:function():void
                  {
                     ExternalInterface.call("invite_to_squad",nickname);
                  }
               });
            }
         }
         SrvContextMenu.show(options,0,0,owner);
      }
      
      private static function askPlayerInfo(nickname:String, player:uint, how:uint) : void
      {
         if(!playerInfoInFlight())
         {
            sendPlayerInfoAsk(nickname,player,how);
            return;
         }
         _playerInfoNext = [nickname,player,how];
         if(_playerInfoWait == null)
         {
            _playerInfoWait = new Timer(PLAYER_INFO_WAIT_MS,1);
            _playerInfoWait.addEventListener(TimerEvent.TIMER_COMPLETE,onPlayerInfoWait);
         }
         _playerInfoWait.reset();
         _playerInfoWait.delay = Math.max(1,PLAYER_INFO_WAIT_MS - (getTimer() - _playerInfoAt));
         _playerInfoWait.start();
      }
      
      private static function playerInfoInFlight() : Boolean
      {
         return _playerInfoName != null && getTimer() - _playerInfoAt < PLAYER_INFO_WAIT_MS;
      }
      
      private static function sendPlayerInfoAsk(nickname:String, player:uint, how:uint) : void
      {
         _playerInfoNext = null;
         _playerInfoName = nickname;
         _playerInfoAt = getTimer();
         try
         {
            ExternalInterface.call("victory_screen_request_player_data",how,player);
         }
         catch(e:Error)
         {
         }
      }
      
      private static function sendNextPlayerInfoAsk() : void
      {
         var next:Array = _playerInfoNext;
         sendPlayerInfoAsk(String(next[0]),uint(next[1]),uint(next[2]));
      }
      
      // The ask in flight got no answer: the newest one goes now.
      private static function onPlayerInfoWait(e:TimerEvent) : void
      {
         if(_playerInfoNext != null && !playerInfoInFlight())
         {
            sendNextPlayerInfoAsk();
         }
      }
      
      // fill_victory_screen_player, first: the answer to our ask opens the
      // window here, as VictoryScreen.setPlayerData and onVSPlayerInfo do
      // for a row of the results (they keep it in the screen's data, which
      // no results screen has here). Any other answer is the results
      // screen's, as before.
      private function takePlayerInfo(value:Object) : Boolean
      {
         // The results screen is showing: a `93` now is its answer. It is
         // modal: no list of ours can be clicked; an ask of ours from
         // before it opened is given up.
         if(this.victory_screen != null && this.victory_screen.parent != null && this.victory_screen.visible)
         {
            _playerInfoName = null;
            _playerInfoNext = null;
            return false;
         }
         if(!playerInfoInFlight())
         {
            _playerInfoName = null;
            return false;
         }
         var nickname:String = _playerInfoName;
         _playerInfoName = null;
         if(_playerInfoNext != null)
         {
            // Asked again since: this answer is the older ask's.
            sendNextPlayerInfoAsk();
            return true;
         }
         if(!value)
         {
            return true;
         }
         if(!(value is Array))
         {
            value.name = "";
            value.level = 1;
            value = [value];
         }
         try
         {
            this.show_player_info({
               nickname:nickname,
               profiles:value
            });
         }
         catch(e:Error)
         {
         }
         return true;
      }
      
      // The rooms window while open: over the side panels (the lobby adds
      // them again over it, and their hidden areas then take its clicks);
      // the chat stays over it (the owner's of 2026-10-19).
      public function layerRoomsWindow(window:DisplayObject) : void
      {
         survarium.rooms.RoomLayers.belowResults(window,this.victory_screen,this._back_back);
         var panels:Array = [this.squadList,this.questList];
         for(var i:int = 0; i < panels.length; i++)
         {
            var panel:DisplayObject = panels[i] as DisplayObject;
            if(panel != null && panel.parent == this && getChildIndex(panel) > getChildIndex(window))
            {
               // Move the persistent side panel back, not the room forward:
               // a window opened more recently (History, for example) stays above.
               setChildIndex(panel,getChildIndex(window));
            }
         }
         if(this.chat != null && this.chat.parent == this && getChildIndex(this.chat) < getChildIndex(window))
         {
            setChildIndex(this.chat,getChildIndex(window));
         }
      }
      
      // A match's results: a player in a room gets the room's window opened
      // first, so the results go over it and closing them shows the room
      // (the owner's of 2026-10-20). Out of a room: nothing. The window is
      // told the match is over (its chat).
      private function showRoomUnderResults() : void
      {
         try
         {
            if(RoomsWindow.me == null)
            {
               return;
            }
            RoomsWindow.me.matchOver();
            if(RoomsWindow.me.inRoom && !RoomsWindow.me.shown)
            {
               RoomsWindow.me.show(true);
            }
         }
         catch(err:Error)
         {
         }
      }
   }
}

