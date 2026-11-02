package
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.BlendMode;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.Matrix;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.filters.ColorMatrixFilter;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import flash.utils.Timer;
   import flash.utils.clearTimeout;
   import flash.utils.getDefinitionByName;
   import flash.utils.setTimeout;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.layout.Layout;
   import scaleform.clik.layout.LayoutData;
   import scaleform.gfx.Extensions;
   import survarium.Util;
   import survarium.constants.BodyPartState;
   import survarium.constants.Colors;
   import survarium.constants.CombatAction;
   import survarium.constants.CombatLogTeam;
   import survarium.constants.Faction;
   import survarium.constants.MinimapPosition;
   import survarium.constants.ProfileState;
   import survarium.constants.SquadState;
   import survarium.constants.Team;
   import survarium.constants.TeamSide;
   import survarium.controls.DeathEyes;
   import survarium.controls.Minimap;
   import survarium.controls.PlayerProfileBase;
   import survarium.controls.PlayerRowMenu;
   import survarium.controls.ForcedView;
   import survarium.controls.KeptLayer;
   import survarium.controls.QuickSlots;
   import survarium.controls.ResearchUnits;
   import survarium.controls.ResearchCountdownLayout;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IItemFactory;
   import survarium.item.ItemFactory;
   
   [SWF(backgroundColor="#666666")]
   public class HUD extends MovieClip
   {
      
      public var chat:SrvGameChat;
      
      public var minimap:SrvMinimap;
      
      public var crosshair:SrvCrosshair;
      
      public var ammoInd:SrvAmmoIndicator;
      
      public var hud_overlay:SrvHudOverlay;
      
      public var damageDebug:SrvDebugDamage;
      
      public var quick_slots:SrvQuickSlots;
      
      public var combatLog:SrvCombatLogList;
      
      public var damageInd:SrvDamageIndicator;
      
      public var oxygen_count:SrvOxygenContainer;
      
      public var warning_indicator:SrvAsyncSymbol;
      
      public var connectionInfo:SrvWarningMessage;
      
      public var container_icon:SrvContainerSymbol;
      
      public var progress_indicator:SrvMatchProgress;
      
      public var pregameInfo:SrvContextInfoPregame;
      
      public var questList:HudQuestsTileList;
      
      public var questTaskList:HudTasksTileList;
      
      public var contextInfo3:SrvContextMessage3;
      
      public var contextInfo2:SrvContextMessage;
      
      public var contextInfo:SrvContextInfo;
      
      public var player_list:SrvPlayerList;

      // A player's menu on the Tab list's rows (Ctrl's cursor, exe patch 86).
      private var _rowMenu:PlayerRowMenu;
      
      public var critInd:SrvCriticalIndicator;
      
      public var playerInfo:MovieClip;
      
      public var gameInfo:MovieClip;
      
      public var layout:Layout;
      
      private var _hudPlayerProfile:PlayerProfileBase = new HUDPlayerProfile();
      
      private var _profilePanel:MovieClip = new HUDProfileSelector();
      
      // Whether the player is typing in the chat: focus_chat(true) to focus_chat(false). Not the
      // input's `visible`, which the chat's timeline starts true until the chat is first closed.
      private var _chatTyping:Boolean = false;

      private var _artefactOfferKey:String = "";
      // The offer text's value (%1$s): the dropped gun's name for patch 85's pickup.
      private var _artefactOfferValue:String = "";
      private var _nativeContext:Object = {text:"",value:0,visible:true};

      // One owner for the existing collection widget. Native context updates
      // may arrive after the carry hint; retain them without erasing its text.
      public function show_artefact_offer(key:String, value:String = "") : void
      {
         this._artefactOfferKey = key == null ? "" : key;
         this._artefactOfferValue = value == null ? "" : value;
         this.refresh_context();
      }

      private function refresh_context() : void
      {
         if(!this.contextInfo) return;
         var offer:Boolean = this._artefactOfferKey.length != 0;
         this.contextInfo.visible = offer ? !this._chatTyping && !this._profilePanel.visible : this._nativeContext.visible;
         if(!this.contextInfo.visible) return;
         var value:int = offer ? 0 : this._nativeContext.value;
         var frame:int = value == 0 ? 1 : 2;
         if(this.contextInfo.currentFrame != frame) this.contextInfo.gotoAndStop(frame);
         if(offer) this.contextInfo.contextText.text = LocalizationData.format(this._artefactOfferKey, this._artefactOfferValue);
         else this.contextInfo.contextText.text = this._nativeContext.text;
         if(value != 0)
         {
            this.contextInfo.progress_indicator.width = this.contextInfo.contextText.textWidth + 10;
            this.contextInfo.progress_indicator.set_progress(value);
         }
      }
      
      private var _itemFactory:IItemFactory = new ItemFactory();
      
      public var itemDescriptor:ItemDescriptor;
      
      protected const hitmarkContainer:Sprite = new Sprite();
      
      protected const effects:Array = [];
      
      protected const effectsDepth:Array = [];
      
      protected const effectsDict:Dictionary = new Dictionary();
      
      protected const effectContainer:Sprite = new Sprite();

      // The view fading over the respawn countdown (DeathEyes, docs/notes/death-eyes.md): on the
      // stage between the screen effects and the root, so the kits and the countdown are over it.
      protected const deathEyes:DeathEyes = new DeathEyes();
      
      protected const BLOOD_EFFECT:uint = 0;
      
      protected const BLOOD_DRIP1:uint = 1;
      
      protected const BLOOD_DRIP2:uint = 2;
      
      protected const BLOOD_DRIP3:uint = 3;
      
      protected const GRENADE_EFFECT:uint = 4;
      
      protected var ammo_size_last:int = -1;
      
      protected var tempo:Number = 0;
      
      protected var _timeout:int = -1;
      
      protected var _lastMessage:String = "";
      
      protected var _context_distance:Number = 0;
      
      protected var _last_rotation:Number = -1;
      
      protected var _parameterizedTimeout:uint;
      
      protected var _reflow:Boolean = false;
      
      protected var _artifacts:uint = 0;
      
      protected var _warningTimeout:uint;
      
      protected var _contextTimeout:uint;
      
      protected var _contextTimeout2:uint;
      
      protected var _gameType:uint = 0;
      
      private var textIndex:int = 0;
      
      private var _fps:uint;
      
      private var _latency:uint;
      
      private var _lowFps:Boolean;
      
      private var _lowLatency:Boolean;
      
      protected var indexOfLevel:uint = 0;
      
      protected var tabMinimap:Minimap;
      
      protected var _pointsTimer:Timer = new Timer(3000);
      
      protected const GAME_MODE_TEAMDEATHMATCH:uint = 4;
      
      // Research (mode 5): the teams' points in Team Deathmatch's bars
      // (progress_indicator's frame 5) and the units under them.
      protected const GAME_MODE_RESEARCH:uint = 5;
      
      public var researchUnits:ResearchUnits;

      private var _researchCountdowns:ResearchCountdownLayout;

      // The units' place in the indicator's own space (placeResearchUnits).
      protected var _researchX:Number = 0;

      protected var _researchY:Number = ResearchUnits.TOP;
      
      protected var grid_sprite:Sprite = new Sprite();
      
      protected var _superHeight:uint = 1080;
      
      protected var _superWidth:uint = 1920;
      
      protected var scaleFactor:Number;
      
      public function HUD()
      {
         super();
         trace("initHUD",name,parent.name,stage.name,rotation,numChildren);
         stage.visible = false;
         this.init();
         this.setlayout();
         this.placeMinimap();
         this.onResize(null);
         addEventListener(Event.RENDER,this.onInitRender);
      }
      
      // The match's own tutorial: the exe hides it at each match's start (0x27AE59).
      public function show_tutorial_slide(bool:Boolean, index:uint) : void
      {
      }
      
      public function create_bitmap_data() : void
      {
         var bitmapData:BitmapData = new BitmapData(256,256);
      }
      
      private function onInitRender(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.onInitRender);
         stage.visible = true;
      }
      
      private function init() : void
      {

         var style:StyleSheet = new StyleSheet();
         var styleObject:Object;
         styleObject = new Object();
         styleObject.color = "#b3ea97";
         style.setStyle("current",styleObject);
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
         MainScene.textStyleSheet = style;
         // The screen effects (add_effect: the pain's red edges, the blood drips, the grenade's flash)
         // are the scene's, not the interface (the owner, 2026-10-24): on the stage below the hud's
         // root, placed over the screen as the root is scaled (placeEffects), so hiding the
         // interface (the root) leaves them.
         this.effectContainer.mouseEnabled = this.effectContainer.mouseChildren = false;
         stage.addChildAt(this.effectContainer,stage.getChildIndex(this));
         stage.addChildAt(this.deathEyes,stage.getChildIndex(this));
         _keptLayer = new KeptLayer(this,this.keepLaidOut);
         _forcedView = new ForcedView(this._keptLayer,this.keptWhenForced,this.keptOnTab,this.keptWhileTyping,
                                      this.holdChatHistory);
         stage.addChildAt(this._keptLayer,stage.getChildIndex(this) + 1);
         _stageLayer = new Sprite();
         this._stageLayer.name = "stageLayer";
         super.addChild(this._stageLayer);
         addChild(this.hitmarkContainer);
         playerInfo = new HUDPlayerInfo() as MovieClip;
         hud_overlay = new SrvHudOverlay();
         addChild(this.hud_overlay);
         damageInd = new SrvDamageIndicator();
         addChild(this.damageInd);
         this.damageInd.setHitmarkContainer(this.hitmarkContainer as Sprite);
         critInd = this.playerInfo.criticalInd;
         this.damageInd.setCriticalIndicator(this.critInd);
         minimap = new SrvMinimap();
         addChild(this.minimap);
         oxygen_count = new SrvOxygenContainer();
         addChild(this.oxygen_count);
         warning_indicator = new SrvAsyncSymbol();
         addChild(this.warning_indicator);
         quick_slots = new SrvQuickSlots();
         addChild(this.quick_slots);
         ammoInd = this.playerInfo.ammoInd;
         addChild(this.playerInfo);
         questList = new HudQuestsTileList();
         addChild(this.questList);
         questTaskList = new HudTasksTileList();
         this.questTaskList.tileList.visible = false;
         addChild(this.questTaskList);
         this.questTaskList.visible = false;
         chat = new SrvGameChat();
         addChild(this.chat);
         combatLog = new SrvCombatLogList();
         addChild(this.combatLog);
         container_icon = new SrvContainerSymbol();
         addChild(this.container_icon);
         progress_indicator = new SrvMatchProgress();
         addChild(this.progress_indicator);
         // Beside the indicator, not in it (the layout centres the indicator by
         // its whole width), following it every frame (followIndicator).
         researchUnits = new ResearchUnits();
         this.researchUnits.visible = false;
         addChild(this.researchUnits);
         this.researchUnits.addEventListener(Event.ENTER_FRAME,this.onResearchFrame);
         pregameInfo = new SrvContextInfoPregame();
         addChild(this.pregameInfo);
         this.pregameInfo.premiumMembers.visible = false;
         damageDebug = new SrvDebugDamage();
         addChild(this.damageDebug);
         this.damageDebug.visible = false;
         crosshair = new SrvCrosshair();
         addChild(this.crosshair);
         connectionInfo = new SrvWarningMessage();
         addChild(this.connectionInfo);
         contextInfo3 = new SrvContextMessage3();
         addChild(this.contextInfo3);
         contextInfo2 = new SrvContextMessage();
         addChild(this.contextInfo2);
         contextInfo = new SrvContextInfo();
         addChild(this.contextInfo);
         player_list = new SrvPlayerList();
         addChild(this.player_list);
         _rowMenu = new PlayerRowMenu(this.player_list.players);
         this.player_list.addChild(this._rowMenu);
         gameInfo = new GameInfo();
         this._stageLayer.addChild(this.gameInfo);
         this.gameInfo.x = 20;
         this.gameInfo.y = 8;
         addChild(this._profilePanel);
         this._profilePanel.profileSelector.tabPage.addEventListener(MouseEvent.MOUSE_OVER,this.onProfileChanged);
         this._profilePanel.profileSelector.tabPage.addEventListener(MouseEvent.MOUSE_OUT,this.onProfileChanged);
         itemDescriptor = new ItemDescriptor("InventoryItems");
         this._itemFactory.registerItemDescriptor(this.itemDescriptor);
         this._hudPlayerProfile.registerInventory(this._itemFactory);
         this._profilePanel.addChild(this._hudPlayerProfile);
         this._hudPlayerProfile.x = 250;
         this._hudPlayerProfile.y = -258;
         this._hudPlayerProfile.visible = false;
         tabMinimap = this.player_list.minimap;
         this.tabMinimap.centerPlayer = false;
         this.minimap.addEventListener(Minimap.MAP_LOADED,this.onMinimapLoaded);
         this.tabMinimap.addEventListener(Minimap.MAP_LOADED,this.onMinimapLoaded);
         this.contextInfo3.visible = false;
         var _iconContainer:IconContainer = new IconContainer("inventoryitems",171,83,new hudItems(),true,0,0,0,false);
         this.quick_slots.registerIconContainer(_iconContainer);
         var grayBitmaps:Dictionary = new Dictionary();
         for(var i:uint = 0; i < 10; i++)
         {
            var index:uint;
            var icon:Icon;
            var newIcon:Icon;
            index = i;
            icon = _iconContainer.getIcon(index);
            newIcon = new Icon();
            newIcon.loadImage(new Bitmap(makeGrayscale(icon.bitmap.bitmapData)));
            trace("createGray::",i,newIcon,index);
            grayBitmaps[index] = newIcon;
         }
         // the exoskeletons' backs and srp's quick-slot items (QuickSlots.SRP_HUD_ICONS) gray when used up
         for(i = 28; i < QuickSlots.SRP_FIRST_CELL + QuickSlots.SRP_HUD_ICONS.length; i++)
         {
            index = i;
            icon = _iconContainer.getIcon(index);
            newIcon = new Icon();
            newIcon.loadImage(new Bitmap(makeGrayscale(icon.bitmap.bitmapData)));
            trace("createGray::",i,newIcon,index);
            grayBitmaps[index] = newIcon;
         }
         this.quick_slots.setGrayIcons(grayBitmaps);
         this.pregameInfo.visible = false;
         this.show_profiles_selector(false);
         this.show_quests_progress(false);
         this.show_ammo(false);
         this.show_slots(false);
         this.show_game_info(false);
         this.show_crosshair(false);
         this.show_player_list(false);
         this.show_oxygen(false);
         this.show_stamina(false);
         this.contextInfo.visible = true;
         this.contextInfo2.visible = false;
         this.connectionInfo.visible = false;
         this.connectionInfo.stop();
         this.damageInd.setHP(100);
         this.damageInd.scaleX = this.stageWidth / 1280;
         this.damageInd.scaleY = this.stageHeight / 720;
         stage.addEventListener(Event.RESIZE,this.onResize);
         this.container_icon.visible = false;
         this.effects[this.BLOOD_EFFECT] = BloodEffect;
         this.effects[this.BLOOD_DRIP1] = Blood_drip1;
         this.effects[this.BLOOD_DRIP2] = Blood_drip2;
         this.effects[this.BLOOD_DRIP3] = Blood_drip3;
         this.effects[this.GRENADE_EFFECT] = GrenadeEffect;
         if(Extensions.isGFxPlayer)
         {
            setTimeout(this.test_all,1000);
         }
         function makeGrayscale(bitmapData:BitmapData = null):BitmapData
         {
            var newBitmapData:BitmapData = bitmapData.clone();
            var rc:Number = 1 / 3;
            var gc:Number = 1 / 3;
            var bc:Number = 1 / 3;
            var filter:ColorMatrixFilter = new ColorMatrixFilter([rc,gc,bc,0,0,rc,gc,bc,0,0,rc,gc,bc,0,0,0,0,0,1,0]);
            newBitmapData.applyFilter(newBitmapData,newBitmapData.rect,new Point(),filter);
            return newBitmapData;
         }
         var textField:TextField = this.contextInfo.textPoints;
         textField.text = "";
         this._pointsTimer.addEventListener(TimerEvent.TIMER,this.removePoints);
      }
      
      private function test_minimap() : void
      {
      }
      
      private function test_minimap_3() : void
      {
         setTimeout(this.test_minimap_2,100);
         var m_objects:Array = [{
            player_id:10,
            team:Team.SECOND,
            player_pos_x:10,
            player_pos_y:-35 - Math.random() * 10,
            type:1,
            is_local_player:true
         },{
            player_id:23,
            team:Team.FIRST,
            player_pos_x:-0.0,
            player_pos_y:-35 - Math.random() * 10,
            type:10,
            is_squad_player:true,
            is_carrying_item:true
         },{
            player_id:24,
            team:Team.SECOND,
            player_pos_x:-10,
            player_pos_y:-35 - Math.random() * 10,
            type:9
         },{
            player_id:25,
            team:Team.FIRST,
            player_pos_x:-20,
            player_pos_y:-35 - Math.random() * 10,
            type:8
         },{
            player_id:26,
            team:Team.FIRST,
            player_pos_x:-30,
            player_pos_y:-35 - Math.random() * 10,
            type:2
         },{
            player_id:27,
            team:Team.FIRST,
            player_pos_x:-40,
            player_pos_y:-35 - Math.random() * 10,
            type:2,
            is_carrying_item:true
         },{
            player_id:30,
            team:Team.SECOND,
            player_pos_x:-50,
            player_pos_y:-35 - Math.random() * 10,
            type:10,
            is_squad_player:true,
            is_carrying_item:true
         }];
         this.update_players(m_objects);
         this.update_players([]);
      }
      
      private function test_minimap_1() : void
      {
         setTimeout(this.test_minimap_3,100);
         var m_objects:Array = [{
            player_id:10,
            team:Team.FIRST,
            player_pos_x:10,
            player_pos_y:-35 - Math.random() * 10,
            type:1,
            is_local_player:true
         },{
            player_id:23,
            team:Team.SECOND,
            player_pos_x:-0.0,
            player_pos_y:-35 - Math.random() * 10,
            type:10,
            is_squad_player:true,
            is_carrying_item:true
         },{
            player_id:24,
            team:Team.FIRST,
            player_pos_x:-10,
            player_pos_y:-35 - Math.random() * 10,
            type:9
         },{
            player_id:25,
            team:Team.SECOND,
            player_pos_x:-20,
            player_pos_y:-35 - Math.random() * 10,
            type:8
         },{
            player_id:26,
            team:Team.SECOND,
            player_pos_x:-30,
            player_pos_y:-35 - Math.random() * 10,
            type:2
         },{
            player_id:27,
            team:Team.SECOND,
            player_pos_x:-40,
            player_pos_y:-35 - Math.random() * 10,
            type:2,
            is_carrying_item:true
         }];
         this.update_players(m_objects);
      }
      
      private function test_minimap_2() : void
      {
         var m_objects:Array = [{
            player_id:23,
            team:Team.FIRST,
            player_pos_x:-0.0,
            player_pos_y:-35 - Math.random() * 10,
            type:10,
            is_squad_player:true
         },{
            player_id:70,
            team:Team.FIRST,
            player_pos_x:-60,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:80,
            team:Team.FIRST,
            player_pos_x:-70,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:90,
            team:Team.FIRST,
            player_pos_x:-80,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:10,
            team:Team.FIRST,
            player_pos_x:10,
            player_pos_y:-35 - Math.random() * 10,
            type:1,
            is_local_player:true
         },{
            player_id:26,
            team:Team.FIRST,
            player_pos_x:-30,
            player_pos_y:-35 - Math.random() * 10,
            type:2
         },{
            player_id:27,
            team:Team.FIRST,
            player_pos_x:-40,
            player_pos_y:-35 - Math.random() * 10,
            type:2,
            is_carrying_item:false
         }];
         this.update_players(m_objects);
         m_objects = [{
            player_id:23,
            team:Team.FIRST,
            player_pos_x:-0.0,
            player_pos_y:-35 - Math.random() * 10,
            type:10,
            is_squad_player:true
         },{
            player_id:70,
            team:Team.FIRST,
            player_pos_x:-60,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:80,
            team:Team.FIRST,
            player_pos_x:-70,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:90,
            team:Team.FIRST,
            player_pos_x:-80,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         }];
         this.update_players(m_objects);
         m_objects = [{
            id:23,
            type:"resupply",
            pos_x:-40,
            pos_y:-40 - Math.random() * 10
         },{
            id:26,
            type:"base_highlighted",
            pos_x:-50,
            pos_y:-40 - Math.random() * 10
         }];
         this.update_objects(m_objects);
      }
      
      private function test_all() : void
      {
         this.quick_slots.visible = true;
         this.fill_slots([{
            slot_id:1,
            name:"sdfsdfsd",
            icon_row:2,
            icon_column:2,
            icon:23,
            enabled:true
         },{
            icon_row:18,
            icon_column:1,
            slot_id:2,
            name:"sdfsdfsd",
            icon:49,
            enabled:true,
            cooldown:0,
            is_artefact:true
         },{
            icon_row:0,
            icon_column:0,
            slot_id:3,
            name:"sdfsdfsd",
            icon:0,
            enabled:true,
            cooldown:0,
            is_artefact:true
         }]);
         this.show_crosshair(true);
         stage.addEventListener(MouseEvent.CLICK,this.testClick);
         this.add_log_message({
            who_name:"Test",
            action_id:CombatAction.KILL,
            victim_name:"super_logn_extra_icvon",
            who_team:CombatLogTeam.ALLY,
            victim_team:CombatLogTeam.ENEMY,
            extra_icon:4
         });
      }
      
      private function onProfileChanged(e:Event) : void
      {
         var index:int;
         var enabled:Boolean;
         trace("onProfileChanged",e.target);
         if(e.target is Button)
         {
            if(e.type == MouseEvent.MOUSE_OUT)
            {
               this._hudPlayerProfile.visible = false;
            }
            else
            {
               index = (e.target.data.index);
               enabled = e.target.enabled;
               if(enabled)
               {
                  trace("onProfileChanged",index);
                  this._hudPlayerProfile.visible = true;
                  this._hudPlayerProfile.x = this._profilePanel.profileSelector.x + index * 187;
                  this.fill_player_profile_items(this._profilePanel.profileSelector.getProfileData(index));
               }
            }
         }
      }
      
      public function select_player_profile(value:uint) : void
      {
         this._profilePanel.profileSelector.selectProfile(value);
      }

      // The keys 1, 2 and 3 (exe patch 58 calls this for each going down while the cursor is
      // shown; docs/notes/match-hud-options.md): the kit of that tab, while the kits are shown
      // and the chat takes no typing (opened with its key, or its input clicked into).
      public function select_kit_key(index:uint) : void
      {
         var typing:Boolean = this._chatTyping
            || this.chat && this.chat.text_inpput && stage.focus != null && stage.focus == this.chat.text_inpput.textField;
         if(!this._profilePanel.visible || typing)
         {
            return;
         }
         this._profilePanel.profileSelector.pickByKey(index);
      }
      
      // The key that hides the interface (exe patch 62, F11; docs/notes/hide-ui-key.md). The interface
      // is the root: the hud, the crosshair, the minimap, the kits, the death screen, the kill feed,
      // the chat, the hit marks, and in _stageLayer (its last child, drawn at the stage's scale) the
      // fps line the game put on the stage. Hidden in one step; the movie goes on running, so what
      // happens meanwhile plays out unseen; the screen effects are not the root's and stay. The exe
      // asks interface_hidden on the key and calls this with the other state; a death or a respawn
      // keeps it.
      private var _stageLayer:Sprite;

      private var _interfaceHidden:Boolean;

      // Forced (Blackout, force_interface_hidden) the interface stays as forced: the key and a
      // match's reset do nothing to it; the exe's patch 65 unforces it for a match without.
      public function hide_interface(hidden:Boolean) : void
      {
         if(this._forcedView && this._forcedView.forced)
         {
            return;
         }
         this.setInterfaceHidden(hidden);
      }

      private function setInterfaceHidden(hidden:Boolean) : void
      {
         _interfaceHidden = hidden;
         visible = !hidden;
      }

      public function interface_hidden() : Boolean
      {
         return this._interfaceHidden;
      }

      // Forced hidden for a whole match (Blackout, exe patch 65; docs/notes/blackout.md): the interface
      // hidden as by the key, except what a forced match keeps (ForcedView): always the pre-match
      // countdown (keptWhenForced: the game shows it while it runs and hides it when play starts); while
      // the Tab list is asked for (show_player_list) the match's time left, the chat with its history and,
      // alive, the intact player-info panel (ammo and body state) and quick-use slots;
      // while the game shows the kits (the death screen, before the start), the
      // kits to pick from (keptOnTab), never the list; while the player types in the chat (focus_chat),
      // the chat (keptWhileTyping). A kept part is in _keptLayer (KeptLayer: on the stage over the root,
      // placed as the root is), so the hidden root leaves it, and works there as in the root; back at its
      // place in the root when not.
      private var _keptLayer:KeptLayer;

      private var _forcedView:ForcedView;

      public function force_interface_hidden(forced:Boolean) : void
      {
         this._forcedView.forced = forced;
         this.setInterfaceHidden(forced);
      }

      public function interface_forced() : Boolean
      {
         return this._forcedView.forced;
      }

      private function keptWhenForced() : Array
      {
         return [this.pregameInfo];
      }

      private function keptOnTab() : Array
      {
         if(this._profilePanel.visible)
         {
            return [this._profilePanel,this.matchTime(),this.chat];
         }
         // Ammo and the body doll share a size-dependent, bottom-right layout.
         // Keep their owner intact: removing either child changes its bounds
         // and a reflow can push the other child beyond the viewport.
         return [this.playerInfo,this.quick_slots,this.matchTime(),this.chat];
      }

      private function keptWhileTyping() : Array
      {
         return [this.chat];
      }

      // The chat's last lines held on screen (Tab's history), or let fade again as the chat does.
      private function holdChatHistory(hold:Boolean) : void
      {
         if(!this.chat || !this.chat.messageContainer)
         {
            return;
         }
         if(hold)
         {
            this.chat.messageContainer.showLastMessages();
            this.chat.messageContainer.stopHideout();
         }
         else
         {
            this.chat.messageContainer.resumeHideout();
         }
      }

      // A part the hud's layout places (its layoutData: the countdown, the quick-use slots) stays
      // its: placed by the same rule in the kept layer as in the root (the layer is placed as the
      // root), at every resolution and interface scale, also when it was kept before the layout's
      // first reflow (a match's start).
      private function keepLaidOut(part:DisplayObject) : void
      {
         if(this.layout != null && part is Sprite && Sprite(part)["layoutData"] != null)
         {
            this.layout.manage(Sprite(part));
         }
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

      private function keepStageLayerOnTop() : void
      {
         if(this._stageLayer != null && this._stageLayer.parent == this && getChildIndex(this._stageLayer) != numChildren - 1)
         {
            super.setChildIndex(this._stageLayer,numChildren - 1);
         }
      }

      // The screen effects' place: the screen at 1280x720 in the hud's units, seen from the stage.
      private function placeEffects() : void
      {
         var place:Matrix = new Matrix(this.stageWidth / 1280,0,0,this.stageHeight / 720,0,0);
         place.concat(this.transform.matrix);
         this.effectContainer.transform.matrix = place;
      }

      public function setup_player_profiles(value:Array) : void
      {
         var i:uint;
         trace("setup_player_profiles",value ? value.length : null);
         if(value)
         {
            for(i = 0; i < value.length; i++)
            {
               value[i].enabled = value[i].state == ProfileState.COMPLETE;
            }
         }
         this._profilePanel.profileSelector.setupProfiles(value);
      }
      
      public function fill_player_profile_items(value:Object) : void
      {
         this._hudPlayerProfile.fillProfileItems(value);
      }
      
      private function toggleMinimap() : void
      {
         ++this.indexOfLevel;
         if(this.indexOfLevel == 7)
         {
            indexOfLevel = 1;
         }
         this.set_minimap_props({map:"level_0" + this.indexOfLevel.toString()});
      }
      
      private function testClick(e:MouseEvent) : void
      {
         trace("testClick::",stage.stageWidth,stage.stageHeight);
         this.set_crosshair_damage(Math.random(),1000,true,true);
      }
      
      private function test_all2() : void
      {
         var m_objects:Array = [{
            player_id:10,
            team:Team.SECOND,
            player_pos_x:-2,
            player_pos_y:-35 - Math.random() * 10,
            type:1,
            is_local_player:true
         },{
            player_id:23,
            team:Team.FIRST,
            player_pos_x:-10,
            player_pos_y:-35 - Math.random() * 10,
            type:10
         },{
            player_id:25,
            team:Team.FIRST,
            player_pos_x:-20,
            player_pos_y:-35 - Math.random() * 10,
            type:8
         },{
            player_id:27,
            team:Team.SECOND,
            player_pos_x:-40,
            player_pos_y:-35 - Math.random() * 10,
            type:2,
            is_carrying_item:false
         }];
         m_objects = [{
            faction_id:Faction.SCAVENGERS,
            quest_icon:1,
            current_progress:4,
            required:5
         }];
         this.show_quests_progress(true);
         this.set_quests_progress(m_objects);
         this.set_body_part_hp(1,1,0.5);
         this.crosshair_enemy_hit(0.8);
         var temp_players:Array = [{
            id:10,
            team:TeamSide.ALLY,
            name:"fdgfdAAF",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:20,
            squad_state:SquadState.IN_SQUAD,
            squad_id:1
         },{
            id:9,
            team:TeamSide.ALLY,
            name:"fdgfd1F",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:21,
            squad_state:SquadState.STATE_2,
            squad_id:1
         },{
            id:8,
            team:TeamSide.NONE,
            name:"fdgfd2",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:4,
            squad_state:SquadState.NONE,
            squad_id:0
         },{
            id:7,
            team:TeamSide.NONE,
            name:"fdgfd3",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:20,
            squad_state:SquadState.STATE_2,
            squad_id:1
         },{
            id:6,
            team:TeamSide.NONE,
            name:"fdgfd4",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:4,
            squad_state:SquadState.STATE_2,
            squad_id:2
         },{
            id:5,
            team:TeamSide.NONE,
            name:"fdgfd5",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:4,
            squad_state:SquadState.STATE_2,
            squad_id:2
         },{
            id:4,
            team:TeamSide.NONE,
            name:"fdgfd6",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:4,
            squad_state:SquadState.STATE_2,
            squad_id:2
         },{
            id:3,
            team:TeamSide.NONE,
            name:"fdgfd6",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:6
         },{
            id:2,
            team:TeamSide.NONE,
            name:"fdgfd1",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:1
         },{
            id:1,
            team:TeamSide.NONE,
            name:"fdgfd8",
            kills:12,
            deaths:22,
            ping:22,
            artifacts:33,
            rank:8
         }];
         this.list_set_players(temp_players);
      }
      
      public function set_localization_data(value:Object) : void
      {
         LocalizationData.setData(value);
      }
      
      public function show_profiles_selector(value:Boolean) : void
      {
         this._profilePanel.visible = value;
         // A death/respawn switches the Tab view even while Tab stays held.
         // The game's show_player_list only invokes the movie when Tab changes.
         this._forcedView.refresh();
      }
      
      public function set_game_stats(state:uint, fps:uint, latency:uint, lowFps:Boolean = false, lowLatenxy:Boolean = false) : void
      {
         this.updateStatsWarning(state);
         this.set_fps_stats(fps,lowFps);
         this.set_latency_stats(latency,lowLatenxy);
      }
      
      public function set_fps_stats(value:uint, isLow:Boolean) : void
      {
         var color:uint;
         var textFieldFps:TextField;
         if(this._fps != value)
         {
            _fps = value;
            textFieldFps = this.gameInfo.container.textFieldFps;
            color = isLow ? 16711680 : 16777215;
            if(textFieldFps.textColor != color)
            {
               textFieldFps.textColor = color;
            }
            textFieldFps.text = this._fps.toString();
         }
      }
      
      public function set_latency_stats(value:uint, isLow:Boolean) : void
      {
         var color:uint;
         var textFieldPing:TextField;
         if(this._latency != value)
         {
            _latency = value;
            textFieldPing = this.gameInfo.container.textFieldPing;
            color = isLow ? 16711680 : 16777215;
            if(textFieldPing.textColor != color)
            {
               textFieldPing.textColor = color;
            }
            textFieldPing.text = this._latency.toString();
         }
      }
      
      public function updateStatsWarning(state:uint) : void
      {
         var frame:uint = state;
         if(this.gameInfo.icon.currentFrame != frame)
         {
            this.gameInfo.icon.gotoAndStop(frame);
         }
      }
      
      public function set_quests_alpha(value:Number) : void
      {
         this.questList.alpha = value;
      }
      
      // The minimap's position, the exe's setting (Settings, user.cfg's minimap_position): BOTTOM
      // until the exe says. The exe may say before or after a match's map loads (setProps): the
      // minimaps' visibility is the HUD's alone, from this and their maps (placeMinimap).
      private var _minimapPosition:int = MinimapPosition.BOTTOM;

      public function set_minimap_position(value:uint) : void
      {
         _minimapPosition = MinimapPosition.parse(value);
         this.placeMinimap();
      }

      // Runs on every position and every map loaded. The minimap shows with a map, unless hidden;
      // hidden keeps the last place. The Tab map shows with a map (its list shows it on Tab).
      private function placeMinimap() : void
      {
         if(this._minimapPosition != MinimapPosition.HIDDEN)
         {
            this.minimap.layoutData.alignV = this._minimapPosition == MinimapPosition.TOP ? "top" : "bottom";
            this.minimap.layoutData.offsetV = this._minimapPosition == MinimapPosition.TOP ? 36 : -36;
            if(this.layout)
            {
               this.layout.reflow();
            }
         }
         this.minimap.visible = this.minimap.hasMap && this._minimapPosition != MinimapPosition.HIDDEN;
         this.tabMinimap.visible = this.tabMinimap.hasMap;
      }

      private function onMinimapLoaded(e:Event) : void
      {
         this.placeMinimap();
      }
      
      public function set_quests_progress(value:Array) : void
      {
         trace("hud_quests::set_quests_progress",value);
         var list:TileList = this.questList.tileList as TileList;
         var listTasks:TileList = this.questTaskList.tileList as TileList;
         var size:Number = value.length * list.rowHeight + 10;
         list.height = size;
         list.width = list.columnWidth;
         list.dataProvider = new DataProvider(value);
         list.validateNow();
         var tasks:Array = new Array();
         var l:uint = value.length;
         for(var i:uint = 0; i < l; i++)
         {
            var objTasks:Array;
            var lt:uint;
            var str:String;
            var j:uint;
            objTasks = value[i].tasks;
            if(objTasks == null)
            {
               trace("PUSH_AT",i);
               tasks.push("");
            }
            else
            {
               lt = objTasks.length;
               str = "";
               for(j = 0; j < lt; j++)
               {
                  str += objTasks[j].text;
                  if(j + 1 != lt)
                  {
                     str += Util.styledChar("\n   \n","small");
                  }
               }
               tasks.push(str);
            }
         }
         trace("createTasks::",tasks,"size:",size);
         listTasks.height = size;
         listTasks.dataProvider = new DataProvider(tasks);
         listTasks.visible = tasks.length > 0;
         listTasks.validateNow();
         this.combatLog.layoutData.offsetV = this.questList.layoutData.offsetV + size + 20;
         if(this.layout)
         {
            this.layout.reflow();
         }
      }
      
      public function show_quests_progress(value:Boolean) : void
      {
         trace("hud::show_quests_progress",value);
         this.questList.visible = value;
      }
      
      private function setlayout() : void
      {
         var borderOffset:uint = 56;
         this.warning_indicator.layoutData = new LayoutData();
         this.warning_indicator.layoutData.alignV = "bottom";
         this.warning_indicator.layoutData.alignH = "right";
         this.warning_indicator.layoutData.offsetV = -180;
         this.warning_indicator.layoutData.offsetH = -23;
         this.questList.layoutData = new LayoutData();
         this.questList.layoutData.alignV = "top";
         this.questList.layoutData.alignH = "right";
         this.questList.layoutData.offsetV = 25;
         this.questList.layoutData.offsetH = -borderOffset + 100;
         this.questTaskList.layoutData = new LayoutData();
         this.questTaskList.layoutData.alignV = "top";
         this.questTaskList.layoutData.alignH = "right";
         this.questTaskList.layoutData.offsetV = 22;
         this.questTaskList.layoutData.offsetH = -borderOffset - 70;
         this.player_list.layoutData = new LayoutData();
         this.player_list.layoutData.alignV = "center";
         this.player_list.layoutData.alignH = "center";
         this.player_list.layoutData.offsetV = -100;
         this.player_list.layoutData.offsetH = -40;
         this.contextInfo.layoutData = new LayoutData();
         this.contextInfo.layoutData.alignV = "center";
         this.contextInfo.layoutData.alignH = "center";
         this.contextInfo.layoutData.offsetV = 0;
         this.contextInfo.layoutData.offsetH = 159 + 48 + 82;
         this.contextInfo2.layoutData = new LayoutData();
         this.contextInfo2.layoutData.alignV = "center";
         this.contextInfo2.layoutData.alignH = "center";
         this.contextInfo2.layoutData.offsetV = this.stageHeight / 3;
         this.contextInfo2.layoutData.offsetH = 0;
         this.connectionInfo.layoutData = new LayoutData();
         this.connectionInfo.layoutData.alignV = "top";
         this.connectionInfo.layoutData.alignH = "center";
         this.connectionInfo.layoutData.offsetV = 145;
         this.connectionInfo.layoutData.offsetH = 0;
         this.damageDebug.layoutData = new LayoutData();
         this.damageDebug.layoutData.alignV = "center";
         this.damageDebug.layoutData.alignH = "center";
         this.damageDebug.layoutData.offsetV = this.stageHeight / 4;
         this.damageDebug.layoutData.offsetH = 0;
         this.combatLog.layoutData = new LayoutData();
         this.combatLog.layoutData.alignV = "top";
         this.combatLog.layoutData.alignH = "right";
         this.combatLog.layoutData.offsetV = 120;
         this.combatLog.layoutData.offsetH = -borderOffset;
         this.progress_indicator.layoutData = new LayoutData();
         this.progress_indicator.layoutData.alignV = "top";
         this.progress_indicator.layoutData.alignH = "center";
         this.progress_indicator.layoutData.offsetV = 56;
         this.progress_indicator.layoutData.offsetH = 230;
         // its alignV and offsetV are the position's (placeMinimap)
         this.minimap.layoutData = new LayoutData();
         this.minimap.layoutData.alignH = "left";
         this.minimap.layoutData.offsetH = borderOffset + 22;
         this.container_icon.layoutData = new LayoutData();
         this.container_icon.layoutData.alignV = "center";
         this.container_icon.layoutData.alignH = "right";
         this.container_icon.layoutData.offsetV = 0;
         this.container_icon.layoutData.offsetH = -borderOffset;
         this.playerInfo.layoutData = new LayoutData();
         this.playerInfo.layoutData.alignV = "bottom";
         this.playerInfo.layoutData.alignH = "right";
         this.playerInfo.layoutData.offsetV = -51;
         this.playerInfo.layoutData.offsetH = -borderOffset + 30;
         this.pregameInfo.layoutData = new LayoutData();
         this.pregameInfo.layoutData.alignV = "top";
         this.pregameInfo.layoutData.alignH = "center";
         this.pregameInfo.layoutData.offsetV = 10;
         this.pregameInfo.layoutData.offsetH = 0;
         this.oxygen_count.layoutData = new LayoutData();
         this.oxygen_count.layoutData.alignV = "bottom";
         this.oxygen_count.layoutData.alignH = "center";
         this.oxygen_count.layoutData.offsetV = -20;
         this.oxygen_count.layoutData.offsetH = 5;
         this.quick_slots.layoutData = new LayoutData();
         this.quick_slots.layoutData.alignV = "bottom";
         this.quick_slots.layoutData.alignH = "center";
         this.quick_slots.layoutData.offsetV = -15;
         this.quick_slots.layoutData.offsetH = 0;
         this.contextInfo3.layoutData = new LayoutData();
         this.contextInfo3.layoutData.alignV = "top";
         this.contextInfo3.layoutData.alignH = "center";
         this.contextInfo3.layoutData.offsetV = this.stageHeight / 3;
         this.contextInfo3.layoutData.offsetH = 0;
         this.chat.layoutData = new LayoutData();
         this.chat.layoutData.alignV = "bottom";
         this.chat.layoutData.alignH = "center";
         this.chat.layoutData.offsetH = 0;
         this.chat.layoutData.offsetV = 0;
         layout = new Layout();
         this.layout.tiedToStageSize = true;
         addChild(this.layout);
      }
      
      public function update_current_player_weapon_stats(ammo:Array, ammo_type:uint, queue_size:uint, indicator_visible:Boolean, ammo_size:uint, crosshair_visible:Boolean, crosshair_size:Number, selectedIndex:int = -1) : void
      {
         trace("update_current_player_weapon_stats",ammo_type,ammo_size,queue_size,indicator_visible,crosshair_visible,crosshair_size);
         this.ammoInd.setAmmo(ammo);
         this.ammoInd.setActiveAmmo(ammo_type);
         this.ammoInd.setQueueSize(queue_size);
         this.show_ammo(indicator_visible);
         this.ammoInd.setAmmoClip(ammo_size);
         this.show_crosshair(crosshair_visible);
         this.crosshair.setCrosshair(crosshair_size);
         if(selectedIndex != -1)
         {
            this.ammoInd.setSelectedAmmo(selectedIndex);
         }
         else
         {
            this.ammoInd.setSelectedAmmo(ammo_type);
         }
      }
      
      public function add_effect(effect_id:uint, effect_priority:uint) : void
      {
         var effect:MovieClip = new this.effects[effect_id]();
         this.effectContainer.addChild(effect);
         this.effectsDict[effect_id] = effect;
         this.effectsDepth.push({
            depth:effect_priority,
            target:effect_id
         });
         this.sortEffects();
         if(effect_id == this.BLOOD_EFFECT)
         {
            effect.gotoAndStop(0);
            effect.blendMode = BlendMode.MULTIPLY;
         }
         else if(effect_id == this.BLOOD_DRIP1)
         {
            effect.x = 131;
            effect.y = -241;
            effect.blendMode = BlendMode.MULTIPLY;
         }
         else if(effect_id == this.BLOOD_DRIP2)
         {
            effect.x = 589;
            effect.y = -565;
            effect.blendMode = BlendMode.MULTIPLY;
         }
         else if(effect_id == this.BLOOD_DRIP3)
         {
            effect.x = 1005;
            effect.y = -365;
            effect.blendMode = BlendMode.MULTIPLY;
         }
      }
      
      public function update_effect(effect_id:uint, frame_id:Number) : void
      {
         if(!this.effectsDict[effect_id])
         {
            return;
         }
         if(effect_id == this.GRENADE_EFFECT)
         {
            (this.effectsDict[effect_id] as MovieClip).update(frame_id);
         }
         else if(effect_id == this.BLOOD_EFFECT)
         {
            (this.effectsDict[effect_id] as MovieClip).update(frame_id);
         }
         else
         {
            (this.effectsDict[effect_id] as MovieClip).gotoAndStop(frame_id);
         }
      }
      
      public function remove_effect(effect_id:uint) : void
      {
         if(!this.effectsDict[effect_id])
         {
            return;
         }
         this.effectContainer.removeChild(this.effectsDict[effect_id] as MovieClip);
         delete this.effectsDict[effect_id];
         for(var i:uint = 0; i < this.effectsDepth.length; i++)
         {
            if(this.effectsDepth[i].target == effect_id)
            {
               this.effectsDepth.splice(i,1);
               break;
            }
         }
      }
      
      public function show_players_list_button(bool:Boolean, label:String) : void
      {
         this.player_list.gotoAndStop(bool ? 2 : 1);
         if(bool)
         {
            this.player_list.btn_exit.label = label;
         }
      }
      
      public function set_online_player(name:String, bool:Boolean) : void
      {
         this.player_list.players.set_online_player(name,bool);
      }
      
      public function focus_chat(bool:Boolean) : void
      {
         this._chatTyping = bool;
         if(this.chat)
         {
            this.chat.ignoreFocus = true;
            this.chat.tabChildren = bool;
            if(bool)
            {
               this.chat.startEnterMessage();
            }
            else
            {
               this.chat.stopEnterMessage();
            }
         }
         // forced (Blackout): the chat shows while typed in (keptWhileTyping)
         this._forcedView.typing = bool;
      }
      
      public function set_friends_list(value:Array) : void
      {
         this.chat.setPlayers(value);
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
         if(value)
         {
            trace("add_chat_message::");
            this.chat.got_message(value);
         }
      }
      
      public function next_tab() : void
      {
         this.chat.nextTab();
      }
      
      public function previous_tab() : void
      {
         this.chat.prevTab();
      }
      
      public function clear_channel(value:uint) : void
      {
         this.chat.clearChannel(value);
      }
      
      public function set_local_player(value:String) : void
      {
         trace("root::set_local_player::",value);
         this.chat.setLocalPlayer(value);
         this.player_list.players.set_local_name(value);
      }
      
      public function add_tab(data:Object) : void
      {
         trace("HUD::addTab::",data.name);
         this.chat.addTab(data);
      }
      
      public function remove_tab(id:uint) : void
      {
         trace("HUD::removeTab::",id);
         this.chat.removeTab(id);
      }
      
      public function set_channels(value:Array) : void
      {
         this.chat.setData(value);
      }
      
      public function start_message(value:String, timeout:uint = 100) : void
      {
         _lastMessage = value;
         if(this._timeout != -1)
         {
            clearTimeout(this._timeout);
         }
         _timeout = setTimeout(this.actualInput,timeout);
      }
      
      public function actualInput() : void
      {
         this.chat.srartMessage(this._lastMessage);
      }
      
      public function tab_channel() : void
      {
         this.chat.tab_channel();
      }
      
      public function add_blood_drip() : void
      {
      }
      
      public function set_quick_slot_activity(slot:uint, phase:uint, remaining:uint, duration:uint) : void
      {
         if(this.quick_slots.setActivity(slot,phase,remaining,duration) && !this._reflow)
         {
            _reflow = true;
            addEventListener(Event.RENDER,this.onRender);
         }
      }

      public function fill_slot(_p:Object) : void
      {
         if(this.quick_slots.fillSlot(_p) && !this._reflow)
         {
            _reflow = true;
            addEventListener(Event.RENDER,this.onRender);
         }
      }
      
      public function onRender(e:Event = null) : void
      {
         _reflow = false;
         removeEventListener(Event.RENDER,this.onRender);
         this.layout.reflow();
      }
      
      public function fill_slots(_p:Array) : void
      {
         this.quick_slots.fillSlots(_p);
         addEventListener(Event.RENDER,this.onRender);
      }
      
      public function set_player_hp(value:uint) : void
      {
         this.damageInd.setHP(value);
      }
      
      public function damage_fill_names(value:Array) : void
      {
         this.damageDebug.setNames(value);
      }
      
      public function part_hp(part:uint, hp:Number, hp_max:Number, affect:String = null) : void
      {
         this.damageDebug.set(part,hp,hp_max,affect);
      }
      
      public function crit_part(value:uint) : void
      {
         this.damageInd.setHP(value);
      }
      
      public function set_radiation(vv:Number) : void
      {
         this.damageInd.setRadiation(vv);
      }
      
      public function show_stamina(bool:Boolean) : void
      {
         this.playerInfo.statminaProgress.visible = bool;
      }
      
      public function set_stamina(value:Number) : void
      {
         this.playerInfo.statminaProgress.set_progress(value);
      }
      
      public function set_toxic(vv:Number) : void
      {
         this.damageInd.setToxic(vv);
      }
      
      public function set_bleeding(vv:Number) : void
      {
         this.damageInd.setBleeding(vv);
      }
      
      public function hit_player(direction:Number, amount:Number) : void
      {
         trace("hit_player::",direction,amount);
         this.on_hit({
            direction:direction,
            amount:amount
         });
      }
      
      public function show_radiation_indicator(bool:Boolean) : void
      {
      }
      
      public function play_radiation_hit(bool:Boolean = false) : void
      {
      }
      
      public function set_radiation_progress(value:uint) : void
      {
      }
      
      public function add_log_message(description:Object) : void
      {
         this.combatLog.addItem(description);
      }
      
      public function set_crosshair_size(size:Number) : void
      {
         this.crosshair.setCrosshair(size);
      }
      
      public function set_crosshair_damage(value:Number, delay:uint = 1000, killed:Boolean = false, debug:Boolean = false) : void
      {
         this.crosshair.setDamage(value,delay,killed,debug);
      }
      
      public function crosshair_enemy_hit(size:Number, delay:uint = 1000, killed:Boolean = false, debug:Boolean = false) : void
      {
         this.crosshair.setDamage(size,delay,killed,debug);
      }
      
      // The Settings' "Hide hit markers", "Hide player scores" and "Hidden crosshair" (exe patch 83, told at
      // once and again at each change; docs/notes/hide-hit-markers-scores.md): the crosshair shows no hit
      // marker; the Tab list shows the players' names only; no crosshair at all.
      public function srp_hud_options(hideHitMarkers:Boolean, hideScores:Boolean, hideCrosshair:Boolean) : void
      {
         this.crosshair.hitMarkersHidden = hideHitMarkers;
         this.player_list.players.namesOnly = hideScores;
         // the whole crosshair, its hit marker with it (show_crosshair shows and hides its lines)
         this.crosshair.visible = !hideCrosshair;
      }
      
      public function show_oxygen(bool:Boolean) : void
      {
         this.oxygen_count.visible = false;
      }
      
      public function set_oxygen(percent:uint, time:uint) : void
      {
         this.oxygen_count.set_oxygen(percent,time);
      }
      
      public function set_teams_names(team1:String, team2:String) : void
      {
         this.player_list.players.team1.text = team1;
         this.player_list.players.team2.text = team1;
      }
      
      public function list_set_players(_players:Array) : void
      {
         this.player_list.players.addPlayersList(_players);
         this.tabMinimap.setPlayerNames(_players);
      }

      // The Tab map's names over the player icons, shown while Alt is held (the exe's patch 82 tells
      // the movie each change; docs/notes/tab-map-names.md). The map is the Tab list's, so they show
      // only with it: never in Blackout, whose Tab keeps the list hidden.
      public function show_map_names(shown:Boolean) : void
      {
         this.tabMinimap.namesShown = shown;
      }
      
      public function list_update_player(_player:Object) : void
      {
         this.player_list.players.updatePlayer(_player);
      }
      
      public function show_player_list(bool:Boolean) : void
      {
         if(!bool)
         {
            this._rowMenu.close();
         }
         // Forced (Blackout): Tab shows its view (keptOnTab), never the list.
         this._forcedView.tab = bool;
         if(this._forcedView.forced)
         {
            return;
         }
         this.player_list.visible = bool;
         this.questTaskList.visible = bool && this.questList.visible;
      }
      
      // The exe's answers to the Tab list's player menu (exe patch 86): what a place offers, and a friend
      // request or a chat mute sent or not.
      public function srp_player_menu(place:uint, addFriend:uint, mute:uint, muted:uint) : void
      {
         this._rowMenu.offered(place,addFriend != 0,mute != 0,muted);
      }

      public function srp_friend_request(place:uint, sent:uint) : void
      {
         this._rowMenu.requested(place,sent != 0);
      }

      public function srp_chat_mute(place:uint, sent:uint) : void
      {
         this._rowMenu.muted(place,sent != 0);
      }

      public function show_debug_damage(bool:Boolean) : void
      {
         if(!bool)
         {
            if(this.contains(this.damageDebug))
            {
               removeChild(this.damageDebug);
            }
         }
         else if(!this.contains(this.damageDebug))
         {
            addChild(this.damageDebug);
            this.damageDebug.visible = true;
            this.layout.reflow();
         }
         else if(!this.damageDebug.visible)
         {
            this.damageDebug.visible = true;
         }
      }
      
      public function show_premium_players(value:String, prefix:String = "", postfix:String = "", bonus:uint = 20) : void
      {
         var style:StyleSheet = new StyleSheet();
         var styleObject:Object;
         styleObject = new Object();
         styleObject.color = Colors.PLAYER_ST;
         style.setStyle("yellow",styleObject);
         this.pregameInfo.premiumMembers.textField.autoSize = TextFieldAutoSize.LEFT;
         this.pregameInfo.premiumMembers.textField.styleSheet = style;
         var players:String = value;
         this.pregameInfo.premiumMembers.visible = true;
         this.pregameInfo.premiumMembers.textField.text = prefix + "\n" + Util.styledChar(value,"yellow") + "\n" + postfix + Util.styledChar("+" + bonus.toString() + "%","yellow");
         this.pregameInfo.premiumMembers.background.height = this.pregameInfo.premiumMembers.textField.textHeight + 25;
      }
      
      public function show_pregame(bool:Boolean) : void
      {
         this.pregameInfo.visible = bool;
         if(bool == false)
         {
            this.pregameInfo.premiumMembers.visible = false;
         }
         this.placeResearchCountdowns();
      }
      
      public function set_pregame(value:String) : void
      {
         this.pregameInfo.contextText.autoSize = TextFieldAutoSize.LEFT;
         this.pregameInfo.contextText.text = value;
         if(this.pregameInfo.contextText.width > 400)
         {
            this.pregameInfo.contextText.x = 0;
            this.pregameInfo.premiumMembers.x = (this.pregameInfo.contextText.textWidth - this.pregameInfo.premiumMembers.width) / 2;
         }
         else
         {
            this.pregameInfo.contextText.x = (400 - this.pregameInfo.contextText.textWidth) / 2;
            this.pregameInfo.premiumMembers.x = 55;
         }
         this.placeResearchCountdowns();
      }
      
      public function show_game_info(bool:Boolean) : void
      {
         this.progress_indicator.visible = bool;
      }
      
      public function show_crosshair(bool:Boolean) : void
      {
         if(bool)
         {
            if(this.crosshair)
            {
               this.crosshair.crosshair_mc.visible = true;
            }
         }
         else if(this.crosshair)
         {
            this.crosshair.crosshair_mc.visible = false;
         }
         stage.invalidate();
      }
      
      public function show_ammo(bool:Boolean) : void
      {
         if(bool)
         {
            if(this.ammoInd)
            {
               this.ammoInd.visible = true;
            }
         }
         else if(this.ammoInd)
         {
            this.ammoInd.visible = false;
         }
      }
      
      public function show_slots(bool:Boolean) : void
      {
         trace("showSlots::",bool,this.quick_slots.slots_list.width,this.quick_slots.slots_list.visible);
         if(bool)
         {
            if(this.quick_slots)
            {
               this.quick_slots.visible = true;
            }
         }
         else if(this.quick_slots)
         {
            this.quick_slots.visible = false;
         }
      }
      
      public function set_weapon_fire_queue_size(size:uint) : void
      {
         this.ammoInd.setQueueSize(size);
      }
      
      public function set_weapon_ammo_size(size:uint) : void
      {
         trace("H:set_weapon_ammo_size",size);
         this.ammoInd.setAmmoClip(size);
      }
      
      public function set_weapon_ammo_type(ammo_type:uint) : void
      {
         trace("H:set_weapon_ammo_type",ammo_type);
         this.ammoInd.setActiveAmmo(ammo_type);
      }
      
      public function set_ammo(value:Array) : void
      {
         trace("H:set_ammo");
         this.ammoInd.setAmmo(value);
      }
      
      public function set_body_part_hp(part:uint, value:Number, break_point:Number) : void
      {
         this.damageInd.partHP(part,value,break_point);
      }
      
      public function crit_player_body_part(part:uint, value:uint = BodyPartState.BROKEN) : void
      {
         this.damageInd.crit(part,value);
      }
      
      public function set_body_part_state(part:uint, value:uint = BodyPartState.BROKEN) : void
      {
         this.damageInd.crit(part,value);
      }
      
      public function heal_player_body_part(part:uint) : void
      {
         this.damageInd.heal(part);
      }
      
      public function reset_damage_indicator() : void
      {
         removeChild(this.damageInd);
         this.damageInd.stop();
         this.damageInd.reset();
         damageInd = new SrvDamageIndicator();
         this.damageInd.setHitmarkContainer(this.hitmarkContainer);
         this.damageInd.setCriticalIndicator(this.critInd);
         addChildAt(this.damageInd,getChildIndex(this.hud_overlay));
         this.damageInd.scaleX = this.stageWidth / 1280;
         this.damageInd.scaleY = this.stageHeight / 720;
         this.damageInd.setHP(100);
      }
      
      public function reset() : void
      {
         trace("HUD::reset");
         this._artefactOfferKey = "";
         this._artefactOfferValue = "";
         this._nativeContext = {text:"",value:0,visible:true};
         // a new match starts with the interface shown (the movie is the last match's; the exe's
         // patch 62 shows the player icons' with it); a forced one stays forced until patch 65
         // tells the movie the new match's state
         this.hide_interface(false);
         this.clear_minimap();
         this.chat.clear();
         this.show_ammo(false);
         this.show_slots(false);
         this.show_game_info(false);
         this.show_crosshair(false);
         this.show_player_list(false);
         this.show_oxygen(false);
         this.show_stamina(false);
         this.combatLog.clear();
         this.crosshair.setDamage(0,0,false,false);
         this.damageInd.reset();
         this.damageInd.setHitmarkContainer(this.hitmarkContainer as Sprite);
         this.quick_slots.clean();
         this.contextInfo.visible = true;
         this.contextInfo2.visible = false;
         this.connectionInfo.visible = false;
         this.show_pregame(false);
         this.set_match_time("");
         this.deathEyes.countdown("");
         if(this.progress_indicator.respawn_time)
         {
            this.set_respawn_time("");
         }
      }
      
      public function clear_minimap() : void
      {
         this.minimap.clear_minimap();
         this.tabMinimap.clear_minimap();
      }
      
      public function set_minimap_props(value:Object) : void
      {
         this.minimap.setProps(value);
         this.tabMinimap.setProps(value);
         this.updateFullMinimapLayout();
      }
      
      private function updateFullMinimapLayout() : void
      {
         trace("updateMinimapLayout");
         var ST_X:uint = 440;
         var ST_Y:uint = 245;
         var AREA:uint = 460;
         var rect:Rectangle = this.tabMinimap.getVisibleRectangle();
         var mapSizeX:int = rect.width;
         var mapSizeY:int = rect.height;
         this.tabMinimap.x = ST_X - rect.x + (AREA - mapSizeX) / 2;
         this.tabMinimap.y = ST_Y - rect.y + (AREA - mapSizeY) / 2;
      }
      
      public function update_objects(objects_array:Array) : void
      {
         this.minimap.setObjectData(objects_array);
         this.tabMinimap.setObjectData(objects_array);
      }
      
      public function update_players(players_array:Array) : void
      {
         this.minimap.setPlayersData(players_array);
         this.tabMinimap.setPlayersData(players_array);
      }
      
      public function update_local_player(position_x:Number, position_y:Number, rotation:Number, team:uint) : void
      {
         trace("update_local_player",position_x,position_y,rotation,team);
         this.minimap.updatePlayerPos(position_x,position_y,rotation,team);
         this.tabMinimap.updatePlayerPos(position_x,position_y,rotation,team);
         if(this._last_rotation - rotation > 0.0001 || this._last_rotation - rotation < -0.0001)
         {
            _last_rotation = rotation;
            this.damageInd.updateHitmarks(rotation);
         }
      }
      
      public function set_rotable(rotable:Boolean) : void
      {
         this.minimap.setRotable(rotable);
      }
      
      public function set_game_type(type:uint) : void
      {
         if(this._gameType == this.GAME_MODE_RESEARCH && this.progress_indicator.layoutData)
         {
            this.progress_indicator.layoutData.alignH = "center";
         }
         if(this._researchCountdowns != null)
         {
            this._researchCountdowns.restore();
            this._researchCountdowns = null;
         }
         _gameType = type;
         this.researchUnits.visible = type == this.GAME_MODE_RESEARCH;
         if(type == this.GAME_MODE_RESEARCH)
         {
            // Research centres the visible score bars on the actual viewport.
            // Timeline labels can change width after generic layout offsets
            // were measured; Research must not wait for a resize to settle.
            if(this.progress_indicator.layoutData)
            {
               this.progress_indicator.layoutData.alignH = "none";
            }
            this.progress_indicator.gotoAndStop(this.GAME_MODE_TEAMDEATHMATCH + 1);
            if(this.progress_indicator.team1LastPlayers)
            {
               this.progress_indicator.team1LastPlayers.visible = false;
               this.progress_indicator.team2LastPlayers.visible = false;
            }
            return;
         }
         this.progress_indicator.gotoAndStop(this._gameType + 1);
      }
      
      public function set_match_time_progress(time_left:uint, time_total:uint) : void
      {
         this.progress_indicator.radialTime.setTime(time_left,time_total);
      }
      
      public function set_match_time(time:String) : void
      {
         this.matchTime().text = time;
      }

      // The match's time left as the mode shows it: Team Deathmatch's and Research's label, or the
      // other modes' (progress_indicator's frames).
      private function matchTime() : Object
      {
         if(this._gameType == this.GAME_MODE_TEAMDEATHMATCH || this._gameType == this.GAME_MODE_RESEARCH)
         {
            return this.progress_indicator.match_time_2_tdm;
         }
         return this.progress_indicator.match_time_2;
      }
      
      public function show_context(bool:Boolean) : void
      {
         this._nativeContext.visible = bool;
         this.refresh_context();
      }
      
      public function show_context_message(bool:Boolean) : void
      {
         this.contextInfo2.visible = bool;
      }
      
      public function set_parameterized_message(text:String, fonSize:uint, shift:uint, time:uint = 2500) : void
      {
         // %s in a message is its time on screen, in seconds (Onyx's activation hint: its active's time, exe patch 54).
         text = text.split("%s").join(String(Math.ceil(time / 1000)));
         if(!this.contextInfo3.visible)
         {
            this.contextInfo3.visible = true;
         }
         addChild(this.contextInfo3);
         if(this.contextInfo3.currentFrame == this.contextInfo3.totalFrames)
         {
            this.contextInfo3.gotoAndPlay(0);
         }
         var textField:TextField = this.contextInfo3.mc_text.contextText;
         var fm:TextFormat = textField.getTextFormat();
         fm.size = fonSize;
         textField.defaultTextFormat = fm;
         textField.autoSize = TextFieldAutoSize.LEFT;
         textField.setTextFormat(fm);
         if(textField.text != text)
         {
            this.contextInfo3.gotoAndPlay(0);
            textField.text = text;
         }
         if(this.contextInfo3.layoutData)
         {
            this.contextInfo3.layoutData.offsetV = this.stageHeight / 100 * shift - textField.textHeight / textField.numLines;
         }
         if(this.layout)
         {
            this.layout.reflow();
         }
         if(this._parameterizedTimeout)
         {
            clearTimeout(this._parameterizedTimeout);
         }
         if(time)
         {
            _parameterizedTimeout = setTimeout(this.hideParameterized,time);
         }
      }
      
      public function hideParameterized() : void
      {
         this.contextInfo3.gotoAndPlay("hide");
      }
      
      public function set_warning_message(text:String, time:int = 2500) : void
      {
         if(!this.connectionInfo.visible)
         {
            this.connectionInfo.visible = true;
         }
         addChild(this.connectionInfo);
         if(this.connectionInfo.currentFrame == this.connectionInfo.totalFrames)
         {
            this.connectionInfo.gotoAndPlay(0);
         }
         if(this.connectionInfo.mc_text.mc_text_w.contextText.text != text)
         {
            this.connectionInfo.gotoAndPlay(0);
            this.connectionInfo.mc_text.mc_text_w.setText(text);
         }
         if(this._warningTimeout)
         {
            clearTimeout(this._warningTimeout);
         }
         _warningTimeout = setTimeout(this.hideWarning,time);
      }
      
      public function hideWarning() : void
      {
         this.connectionInfo.gotoAndPlay("hide");
      }
      
      public function set_context_message(text:String, value:int = -1) : void
      {
         if(!this.contextInfo2.visible)
         {
            this.contextInfo2.visible = true;
         }
         addChild(this.contextInfo2);
         if(this.contextInfo2.currentFrame == this.contextInfo2.totalFrames)
         {
            this.contextInfo2.gotoAndPlay(0);
         }
         if(this.contextInfo2.mc_text.contextText.text != text)
         {
            this.contextInfo2.gotoAndPlay(0);
            this.contextInfo2.mc_text.contextText.text = text;
         }
         if(this._contextTimeout)
         {
            clearTimeout(this._contextTimeout);
         }
         _contextTimeout = setTimeout(this.hideContext,5000);
      }
      
      public function hideContext() : void
      {
         this.contextInfo2.gotoAndPlay(15);
      }
      
      public function add_points(text:String, value:int) : void
      {
         this.show_context(true);
         this._pointsTimer.reset();
         this._pointsTimer.start();
         var textField:TextField = this.contextInfo.textPoints;
         textField.border = true;
         textField.selectable = true;
         trace("points_added",text,textField.numLines,"/",textField.text == "","/");
         textField.appendText((textField.text == "" ? "" : "\n") + text + (value > 0 ? "+" : "") + String(value));
         trace("origin",textField.text,textField.numLines);
         if(textField.numLines > 4)
         {
            textField.text = textField.text.slice(textField.getLineOffset(2));
         }
         trace("add_points_2",textField.numLines);
      }
      
      protected function removePoints(e:Event = null) : void
      {
         var textField:TextField = this.contextInfo.textPoints;
         var originText:String = textField.text;
         var start:int;
         trace("removePoints",textField.numLines,"|");
         if(textField.numLines > 1)
         {
            start = textField.getLineLength(1);
            originText = originText.slice(start);
         }
         else
         {
            originText = "";
            this._pointsTimer.reset();
            this._pointsTimer.stop();
         }
         textField.text = originText;
         trace("removePoints_01::\n",start,textField.numLines,originText);
      }
      
      public function set_context(text:String, value:int = 0, time:int = 1000) : void
      {
         this._nativeContext = {text:text,value:value,visible:true};
         if(this._contextTimeout2) clearTimeout(this._contextTimeout2);
         this.refresh_context();
      }
      
      public function hide_context() : void
      {
         this.show_context(false);
      }
      
      public function set_tickets_required(value:uint) : void
      {
         if(this._gameType == this.GAME_MODE_TEAMDEATHMATCH || this._gameType == this.GAME_MODE_RESEARCH)
         {
            this.progress_indicator.team_1_score.maximum = value;
            this.progress_indicator.team_2_score.maximum = value;
         }
      }
      
      public function set_artifacts_required(value:uint) : void
      {
         _artifacts = value;
         this.set_artifacts_progress(Team.FIRST,0);
         this.set_artifacts_progress(Team.SECOND,0);
         if(this.progress_indicator.layoutData)
         {
            this.progress_indicator.layoutData.offsetH = Math.ceil(this.progress_indicator.width / 2);
         }
         if(this.layout)
         {
            this.layout.reflow();
         }
      }
      
      public function setTeams(value1:String, value2:String) : void
      {
         this.progress_indicator.team_1.text = value1;
         this.progress_indicator.team_2.text = value2;
      }
      
      public function set_last_players(team1Players:uint, team1TotalPlayers:uint, team2Players:uint, team2TotalPlayers:uint) : void
      {
         trace("set_last_players::",team1Players,team1TotalPlayers,team2Players,team2TotalPlayers,"GM::",this._gameType);
         if(this._gameType == this.GAME_MODE_TEAMDEATHMATCH)
         {
            this.progress_indicator.team1LastPlayers.rendererStep = 20;
            this.progress_indicator.team1LastPlayers.maximum = team1TotalPlayers;
            this.progress_indicator.team1LastPlayers.value = team1Players;
            this.progress_indicator.team2LastPlayers.rendererStep = 20;
            this.progress_indicator.team2LastPlayers.maximum = team2TotalPlayers;
            this.progress_indicator.team2LastPlayers.value = team2Players;
         }
      }
      
      public function set_game_progress(team1:uint, team2:uint) : void
      {
         trace("set_game_progress::",team1,team2);
         if(this._gameType == this.GAME_MODE_RESEARCH && this.progress_indicator.team_1_score)
         {
            this.progress_indicator.team1Progress.visible = this.progress_indicator.team_1_score.visible = true;
            this.progress_indicator.team2Progress.visible = this.progress_indicator.team_2_score.visible = true;
            this.progress_indicator.team1LastPlayers.visible = false;
            this.progress_indicator.team2LastPlayers.visible = false;
            this.progress_indicator.team_1_score.value = team1;
            this.progress_indicator.team_2_score.value = team2;
            this.progress_indicator.team1Progress.text = String(team1);
            this.progress_indicator.team2Progress.text = String(team2);
         }
         else if(this._gameType == this.GAME_MODE_TEAMDEATHMATCH)
         {
            this.progress_indicator.team1Progress.visible = this.progress_indicator.team_1_score.visible = team1 > 0;
            this.progress_indicator.team2Progress.visible = this.progress_indicator.team_2_score.visible = team2 > 0;
            this.progress_indicator.team1LastPlayers.visible = team1 == 0;
            this.progress_indicator.team2LastPlayers.visible = team2 == 0;
            this.progress_indicator.team_1_score.value = team1;
            this.progress_indicator.team_2_score.value = team2;
            this.progress_indicator.team1Progress.text = String(team1);
            this.progress_indicator.team2Progress.text = String(team2);
         }
         else
         {
            this.set_artifacts_progress(Team.FIRST,team1);
            this.set_artifacts_progress(Team.SECOND,team2);
         }
         if(this.progress_indicator.layoutData)
         {
            this.progress_indicator.layoutData.offsetH = Math.ceil(this.progress_indicator.width / 2);
         }
         if(this.layout)
         {
            this.layout.reflow();
         }
      }
      
      // Research (mode 5), every frame the state changes and every second
      // (research_arena::research_update_hud): the game's own team, the teams'
      // points, the points to win, the number of units, then nine numbers a
      // unit: cp_id, x, -z (the minimap's pos_x, pos_y), owner, the team
      // taking it, its capture 0..100, the players of team 0 and 1 inside,
      // whether the game's own player is inside. A team is 0, 1 or 2 (none).
      public function research_update(... values) : void
      {
         var ownTeam:uint = uint(values[0]);
         var goal:uint = uint(values[3]);
         var count:uint = uint(values[4]);
         var units:Array = [];
         var i:uint = 0;
         var at:uint;
         while(i < count && 5 + 9 * i + 8 < values.length)
         {
            at = 5 + 9 * i;
            units.push({
               "cp_id":uint(values[at]),
               "pos_x":Number(values[at + 1]),
               "pos_y":Number(values[at + 2]),
               "owner":uint(values[at + 3]),
               "capturing":uint(values[at + 4]),
               "progress":Number(values[at + 5]),
               "inside_0":uint(values[at + 6]),
               "inside_1":uint(values[at + 7]),
               "local":uint(values[at + 8]) != 0,
               "left":5 + 9 * count + i < values.length ? Number(values[5 + 9 * count + i]) : 0
            });
            i++;
         }
         if(this._gameType != this.GAME_MODE_RESEARCH)
         {
            this.set_game_type(this.GAME_MODE_RESEARCH);
         }
         if(this.progress_indicator.team_1_score && this.progress_indicator.team_1_score.maximum != goal)
         {
            this.progress_indicator.team_1_score.maximum = goal;
            this.progress_indicator.team_2_score.maximum = goal;
         }
         this.researchUnits.update(units,ownTeam);
         this.placeResearchUnits();
         this.minimap.setResearchUnits(units,ownTeam);
         this.tabMinimap.setResearchUnits(units,ownTeam);
      }
      
      // The units centred under Team Deathmatch's bars and the timer, measured
      // in the indicator's own space (its origin is its centre). Research's
      // respawn and pre-match countdowns get a separate row below the units.
      private function placeResearchUnits() : void
      {
         var space:DisplayObject = this.progress_indicator;
         var bars:Rectangle = null;
         var part:DisplayObject;
         var parts:Array = [this.progress_indicator.team_1_score,this.progress_indicator.team_2_score,this.progress_indicator.match_time_2_tdm];
         var top:Number = 0;
         for each(part in parts)
         {
            if(part != null && part.visible)
            {
               bars = bars == null ? part.getBounds(space) : bars.union(part.getBounds(space));
            }
         }
         if(bars != null)
         {
            top = bars.bottom;
         }
         _researchX = bars == null ? 0 : bars.x + bars.width / 2;
         _researchY = top + ResearchUnits.TOP;
         this.followIndicator();
      }

      // Centre Research on the live viewport, including before the first
      // resize. Layout owns the vertical position; the points follow the bars.
      private function followIndicator() : void
      {
         this.researchUnits.followIndicator(this.progress_indicator,this._researchX,this._researchY);
      }

      // Every frame of a Research match: the layout moves the indicator at
      // many calls (scores, messages, resizes), the units follow it.
      private function onResearchFrame(e:Event) : void
      {
         if(this.researchUnits.visible)
         {
            this.followIndicator();
            this.placeResearchCountdowns();
         }
      }

      private function placeResearchCountdowns() : void
      {
         if(this._gameType != this.GAME_MODE_RESEARCH || !this.progress_indicator.respawn_time ||
            !this.progress_indicator.respawn_time.textField || !this.pregameInfo.layoutData)
         {
            return;
         }
         if(this._researchCountdowns == null)
         {
            this._researchCountdowns = new ResearchCountdownLayout(this.progress_indicator.respawn_time,this.pregameInfo);
         }
         this._researchCountdowns.place(this.researchUnits);
      }

      public function set_artifacts_progress(team:uint, value:uint) : void
      {
         this.progress_indicator.art_teams.team = team;
         this.progress_indicator.art_teams.set_progress(value,this._artifacts);
         if(team == Team.FIRST)
         {
            this.progress_indicator.team_1_progress.text = String(value);
         }
         else
         {
            this.progress_indicator.team_2_progress.text = String(value);
         }
         this.progress_indicator.team_1_progress.visible = this.progress_indicator.team_2_progress.visible = true;
      }
      
      public function set_respawn_time(time:String) : void
      {
         this.deathEyes.countdown(time);
         this.progress_indicator.respawn_time.text = time;
         this.placeResearchCountdowns();
      }
      
      public function set_capture_progress(team:uint, text:String, value:uint) : void
      {
         if(this.progress_indicator.currentFrame == 1)
         {
            if(team == Team.SECOND)
            {
               this.progress_indicator.team_1_progress.text = text;
               this.progress_indicator.capture_progress_bar_team_1.value = value;
            }
            else
            {
               this.progress_indicator.team_2_progress.text = text;
               this.progress_indicator.capture_progress_bar_team_2.value = value;
            }
         }
         else if(this.progress_indicator.currentFrame == 2)
         {
            this.progress_indicator.capture_progress_bar.percent = value;
         }
         else if(this.progress_indicator.currentFrame == 3)
         {
            if(team == Team.SECOND)
            {
               this.progress_indicator.capture_progress_bar.percent = value;
               this.progress_indicator.team_1_progress.text = text;
            }
            else
            {
               this.progress_indicator.capture_progress_bar.percent = 100 - value;
               this.progress_indicator.team_2_progress.text = text;
            }
         }
      }
      
      public function on_hit(value:Object) : void
      {
         var angle:Number = (value.direction);
         var amount:Number = (value.amount);
         var point:Point = new Point(Math.cos(angle),Math.sin(angle));
         this.damageInd.hit(point,amount,angle);
      }
      
      public function on_hit_v(x_component:Number, y_component:Number, amount:Number) : void
      {
         var point:Point = new Point(x_component,y_component);
         this.damageInd.hit(point,amount);
      }
      
      public function show_container_icon(value:uint) : void
      {
         this.container_icon.visible = true;
         this.container_icon.gotoAndStop(value + 1);
      }
      
      public function hide_container_icon() : void
      {
         this.container_icon.visible = false;
      }
      
      public function show_warning_indicator() : void
      {
         this.gameInfo.syncIcon.gotoAndPlay(1);
         this.gameInfo.syncIcon.arrowsContainer.gotoAndPlay(1);
      }
      
      public function set_crosshair_position(x:uint, y:uint) : void
      {
         this.crosshair.x = x / this.scaleFactor;
         this.crosshair.y = y / this.scaleFactor;
      }
      
      protected function sortEffects() : void
      {
         this.effectsDepth.sortOn("depth",Array.DESCENDING);
         for(var i:uint = 0; i < this.effectsDepth.length; i++)
         {
            this.effectContainer.setChildIndex(this.effectsDict[this.effectsDepth[i].target],i);
         }
      }
      
      private function create_grid() : void
      {
         addChild(this.grid_sprite);
         var gr:Graphics = this.grid_sprite.graphics;
         var rect:Rectangle;
         gr.clear();
         gr.lineStyle(1,6946681,1,false);
         var hh:int = this.stageHeight;
         var ww:int = this.stageWidth;
         var posX:int;
         var posY:int;
         posX = 50;
         gr.moveTo(50,0);
         gr.lineTo(50,hh);
         posX = ww / 2;
         gr.moveTo(posX,0);
         gr.lineTo(posX,hh);
         posX = ww - 50;
         gr.moveTo(posX,0);
         gr.lineTo(posX,hh);
         posY = 50;
         gr.moveTo(0,posY);
         gr.lineTo(ww,posY);
         posY = hh / 3;
         gr.moveTo(0,posY);
         gr.lineTo(ww,posY);
         posY = hh / 2;
         gr.moveTo(0,posY);
         gr.lineTo(ww,posY);
         posY = hh - hh / 3;
         gr.moveTo(0,posY);
         gr.lineTo(ww,posY);
         posY = hh - 50;
         gr.moveTo(0,posY);
         gr.lineTo(ww,posY);
      }
      
      protected function get stageWidth() : int
      {
         return stage.stageWidth / this.scaleFactor;
      }
      
      protected function get stageHeight() : int
      {
         return stage.stageHeight / this.scaleFactor;
      }
      
      private function onResize(event:Event) : void
      {
         if(stage.stageWidth / stage.stageHeight < 1.85)
         {
            scaleFactor = stage.stageWidth / this._superWidth;
         }
         else
         {
            scaleFactor = stage.stageHeight / this._superHeight;
         }
         scaleX = this.scaleFactor;
         scaleY = this.scaleFactor;
         if(this._stageLayer)
         {
            this._stageLayer.scaleX = this._stageLayer.scaleY = 1 / this.scaleFactor;
         }
         trace("HUD::finalScaleFactor::",this.scaleFactor);
         this.layout.setScaleFactor(this.scaleFactor);
         this.damageInd.scaleX = this.stageWidth / 1280;
         this.damageInd.scaleY = this.stageHeight / 720;
         this.placeEffects();
         this._keptLayer.place();
         this.deathEyes.place(stage.stageWidth,stage.stageHeight);
         // The middle of the centre pixel in screen pixels (the crosshair is
         // drawn at 1:1, below): at a scale of 2 the half pixel in HUD units
         // was a whole pixel off.
         this.crosshair.x = (int(stage.stageWidth / 2) + 0.5) / this.scaleFactor;
         this.crosshair.y = (int(stage.stageHeight / 2) + 0.5) / this.scaleFactor;
         this.contextInfo2.layoutData.offsetV = this.stageHeight / 3;
         this.damageDebug.layoutData.offsetV = this.stageHeight / 4;
         if(this.chat.layoutData)
         {
            this.chat.layoutData.offsetV = -150;
         }
         this.hud_overlay.update(this.scaleFactor);
         this.crosshair.scaleX = this.crosshair.scaleY = 1 / this.scaleFactor;
         this._profilePanel.x = int(this.stageWidth - 594) / 2;
         this._profilePanel.y = int(this.stageHeight - 120);
         this.layout.reflow();
         if(this._gameType == this.GAME_MODE_RESEARCH)
         {
            this.placeResearchUnits();
         }
      }
   }
}

