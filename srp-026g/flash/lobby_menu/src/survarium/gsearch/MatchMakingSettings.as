package survarium.gsearch
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.buttons.CheckBoxIcon;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   import survarium.constants.CharCode;
   import survarium.constants.ClanState;
   import survarium.constants.GameMode;
   import survarium.constants.MessageId;
   import survarium.constants.PvbCode;
   import survarium.rooms.RoomsText;
   
   public class MatchMakingSettings extends MovieClip
   {
      
      public static const TOURNAMENT_SELECTED:String = "tournamentSelected";
      
      public static const TOURNAMENT_DESELECTED:String = "tournamentDeselected";
      
      public static const PVB_DEMO:Boolean = false;
      
      public static const PVB_STATUS:int = 2050000000;
      
      public static const PVB_CHOOSE:int = 2051000000;
      
      public static const PVB_END:int = 2052000000;
      
      public static const PVB_ROWS:int = 32;
      
      public static const PVB_WAIT_MS:int = 6000;
      
      public static const PVB_POLL_MS:int = 10000;
      
      public static const PVB_WHY_CHECKING:int = 17;
      
      public static const PVB_WHY_NOANSWER:int = 18;
      
      public static const PVB_WHY_RETRY:int = 19;
      
      public static const PVB_WHY_GENERIC:int = 20;
      
      // pvbWhy's "no reason": the player may start a bot match
      public static const PVB_WHY_NONE:int = -1;
      
      // the words (dictionary keys) of a PvbCode or a PVB_WHY_* reason (pvbReasonText)
      public static const PVB_REASONS:Object = {
         "2":"st_srp_pvb_in_squad",
         "3":"st_srp_pvb_searching",
         "4":"st_srp_pvb_in_match",
         "5":"st_srp_pvb_outdated",
         "6":"st_srp_pvb_off",
         "8":"st_srp_pvb_maintenance",
         "9":"st_srp_pvb_clan_match",
         "10":"st_srp_pvb_unknown_choice",
         "17":"st_srp_pvb_checking",
         "18":"st_srp_pvb_no_answer",
         "19":"st_srp_pvb_retry",
         "20":"st_srp_pvb_generic"
      };
      
      // a bot match's start request: PVB_CHOOSE + size * PVB_SIZE_STEP + map (the server's B2 rows)
      public static const PVB_SIZE_STEP:int = 1000;
      
      // meaning unknown: pvbData drops team sizes above it
      public static const PVB_SIZE_MAX:int = 20;
      
      // the map level picked first: Rudnya (pvbLevelName)
      public static const PVB_LEVEL_DEFAULT:int = 1;
      
      // the rows of the size, mode and map menus (the server offers 16 bot maps)
      public static const PVB_SIZE_ROWS:int = 3;
      
      public static const PVB_MODE_ROWS:int = 2;
      
      public static const PVB_MAP_ROWS:int = 16;
      
      // the demo menus' "no answer" rows (PVB_DEMO)
      public static const PVB_DEMO_STATUS_NO_ANSWER:int = 10;
      
      public static const PVB_DEMO_PLAY_NO_ANSWER:int = 11;
      
      // the demo menus' row counts (PVB_DEMO)
      public static const PVB_DEMO_STATUS_ROWS:int = 11;
      
      public static const PVB_DEMO_PLAY_ROWS:int = 12;
      
      public static var pvbHolder:MatchMakingSettings;
      
      public var checkBoxPVP:Button;
      
      public var checkBoxPVE:Button;
      
      public var checkBoxRankBattle:CheckBoxIcon;
      
      public var checkBoxTournamentBattle:CheckBoxIcon;
      
      public var checkBoxTrainingBattle:Button;
      
      private var _enabled:Boolean = true;
      
      public var pvpGame:MovieClip;
      
      public var clanGame:MovieClip;
      
      public var freeGame:MovieClip;
      
      public var trainingGame:MovieClip;
      
      public var background:MovieClip;
      
      public var checkBoxClan:Button;
      
      public var checkBoxCharging:CheckBoxIcon;
      
      public var checkBoxTeam:CheckBoxIcon;
      
      private var _selectCall:Boolean = false;
      
      protected var pvbReady:Boolean;
      
      protected var pvbSection:MovieClip;
      
      protected var pvbRadio:Button;
      
      protected var pvbSizeMenu:DropdownMenu;
      
      protected var pvbMapMenu:DropdownMenu;
      
      protected var pvbModeMenu:DropdownMenu;
      
      protected var pvbModeList:Array;
      
      protected var pvbLevelRows:Array;
      
      protected var pvbMode:int;
      
      protected var pvbLevel:int;
      
      protected var pvbDemoBox:MovieClip;
      
      protected var pvbDemoStatusMenu:DropdownMenu;
      
      protected var pvbDemoPlayMenu:DropdownMenu;
      
      protected var pvbOn:Boolean;
      
      protected var pvbCode:int;
      
      protected var pvbGear:int;
      
      protected var pvbMax:int;
      
      protected var pvbRunning:int;
      
      protected var pvbRunMax:int;
      
      protected var pvbSizes:Array;
      
      protected var pvbMaps:Array;
      
      protected var pvbMapKey:String;
      
      protected var pvbSizeKey:String;
      
      protected var pvbSize:int;
      
      protected var pvbMap:int;
      
      protected var pvbAsking:Boolean;
      
      protected var pvbAskedAt:int;
      
      protected var pvbHeardAt:int;
      
      protected var pvbChoosing:Boolean;
      
      protected var pvbChooseAt:int;
      
      protected var pvbChooseSize:int;
      
      protected var pvbChooseMap:int;
      
      protected var pvbWasVisible:Boolean;
      
      protected var pvbOpenHeight:int;
      
      protected var pvbDemoStatus:int;
      
      protected var pvbDemoPlay:int;
      
      protected var pvbDemoRows:Array;
      
      protected var pvbDemoAt:int;
      
      public function MatchMakingSettings()
      {
         super();
         stop();
         LocalizationData.addCallback(this.initLocal,"label_status");
         addEventListener(Event.RENDER,this.configUI);
      }
      
      public static function pvbTake(rows:Array) : Boolean
      {
         var first:Object = null;
         var name:String = null;
         var place:Number = 0;
         var taken:Boolean = false;
         try
         {
            if(rows && rows.length > 0)
            {
               first = rows[0];
               if(first)
               {
                  place = Number(first.place);
                  name = String(first.nickname);
                  if(place >= MatchMakingSettings.PVB_STATUS && place < MatchMakingSettings.PVB_END)
                  {
                     taken = true;
                  }
                  else if(name.indexOf("B1;") == 0 || name.indexOf("B2;") == 0)
                  {
                     taken = true;
                  }
                  if(taken && MatchMakingSettings.pvbHolder)
                  {
                     MatchMakingSettings.pvbHolder.pvbData(rows);
                  }
               }
            }
         }
         catch(e:Error)
         {
         }
         return taken;
      }
      
      private function configUI(event:Event) : void
      {
         removeEventListener(Event.RENDER,this.configUI);
         checkBoxClan = this.clanGame.checkBoxClan;
         checkBoxRankBattle = this.clanGame.checkBoxRankBattle;
         checkBoxTournamentBattle = this.clanGame.checkBoxTournamentBattle;
         checkBoxTrainingBattle = this.trainingGame.checkBoxTrainingBattle;
         checkBoxPVP = this.pvpGame.checkBoxPVP;
         checkBoxCharging = this.pvpGame.checkBoxCharging;
         checkBoxTeam = this.pvpGame.checkBoxTeam;
         checkBoxPVE = this.freeGame.checkBoxPVE;
         this.checkBoxClan.enabled = this.checkBoxPVP.enabled = false;
         this.checkBoxPVP.selected = true;
         this.checkBoxCharging.iconIndex = 6;
         this.checkBoxTeam.iconIndex = 0;
         this.checkBoxRankBattle.iconIndex = 1;
         this.checkBoxTournamentBattle.iconIndex = 4;
         this.updateLayout();
         this.pvbHookConfig();
      }
      
      public function setEnabled(value:Boolean) : void
      {
         _enabled = value;
         this.updateButtonEnabled();
      }
      
      public function selectClanMatchCheckbox() : void
      {
         _selectCall = true;
         this.setTournamentResult(true);
         dispatchEvent(new Event(TOURNAMENT_SELECTED));
      }
      
      public function deselectClanMatchCheckbox() : void
      {
         _selectCall = false;
         this.setTournamentResult(true);
         dispatchEvent(new Event(TOURNAMENT_DESELECTED));
      }
      
      public function updateClanStatus() : void
      {
         this.updateButtonEnabled();
      }
      
      public function updateSquadStatus() : void
      {
         this.updateButtonEnabled();
      }
      
      protected function updateButtonEnabled() : void
      {
         var canPlayClan:Boolean = (PlayerInfo.clanState == ClanState.OVERVIEW || PlayerInfo.clanState == ClanState.MANAGE) && !PlayerInfo.isInSquad;
         this.checkBoxPVP.enabled = this.checkBoxClan.enabled = this._enabled && canPlayClan;
         if(!canPlayClan)
         {
            if(this.checkBoxClan.selected)
            {
               this.checkBoxClan.selected = false;
               this.checkBoxPVP.selected = true;
               _selectCall = false;
               dispatchEvent(new Event(TOURNAMENT_DESELECTED));
            }
         }
         this.updateLayout();
         this.pvbHookButtons();
      }
      
      private function updateLayout() : void
      {
         trace("updateLayout",this.checkBoxPVP.selected,this.checkBoxClan.selected);
         this.checkBoxCharging.visible = this.checkBoxTeam.visible = this.checkBoxPVP.selected;
         this.checkBoxRankBattle.visible = this.checkBoxTournamentBattle.visible = this.checkBoxClan.selected;
         this.pvpGame.sectionBack.height = this.checkBoxPVP.selected == true ? 100 : 41;
         this.clanGame.sectionBack.height = this.checkBoxClan.selected == true ? 100 : 41;
         this.freeGame.y = int(this.pvpGame.y + this.pvpGame.sectionBack.height);
         this.clanGame.y = int(this.freeGame.y + this.freeGame.sectionBack.height);
         this.trainingGame.y = int(this.clanGame.y + this.clanGame.sectionBack.height);
         this.background.height = this.trainingGame.y + this.trainingGame.sectionBack.height + 10;
         this.pvbHookLayout();
      }
      
      protected function initLocal(value:String) : void
      {
         this.checkBoxPVP.label = LocalizationData.getName("match_settings_checkbox_PVP").toUpperCase();
         this.checkBoxClan.label = LocalizationData.getName("match_settings_clan_match").toUpperCase();
         this.checkBoxPVE.label = LocalizationData.getName("match_settings_checkbox_PVE").toUpperCase();
         this.checkBoxTrainingBattle.label = LocalizationData.getName("match_settings_checkbox_training_match").toUpperCase();
         this.checkBoxTournamentBattle.label = LocalizationData.getName("match_settings_checkbox_tournament_match");
         this.checkBoxCharging.label = LocalizationData.getName("match_settings_checkbox_battery_charge");
         this.checkBoxTeam.label = LocalizationData.getName("match_settings_checkbox_team_match");
         this.checkBoxRankBattle.label = LocalizationData.getName("match_settings_checkbox_rank_match");
         this.checkBoxTrainingBattle.focusable = this.checkBoxPVP.focusable = this.checkBoxTournamentBattle.focusable = false;
         this.checkBoxRankBattle.selected = false;
         this.checkBoxPVP.addEventListener(ButtonEvent.CLICK,this.onTournamentSelected);
         this.checkBoxClan.addEventListener(ButtonEvent.CLICK,this.onTournamentSelected);
         this.checkBoxPVP.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxCharging.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxTeam.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxPVE.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxClan.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxRankBattle.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxTournamentBattle.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxTrainingBattle.tooltipRendererName = "SrvTextHintRenderer";
         this.checkBoxPVP.data = {name:LocalizationData.getName("match_settings_checkbox_PVP_hint")};
         this.checkBoxCharging.data = {name:LocalizationData.getName("match_settings_checkbox_battery_charge_hint")};
         this.checkBoxTeam.data = {name:LocalizationData.getName("match_settings_checkbox_team_match_hint")};
         this.checkBoxPVE.data = {name:LocalizationData.getName("match_settings_checkbox_PVE_hint")};
         this.checkBoxClan.data = {name:LocalizationData.getName("match_settings_checkbox_clan_match_hint")};
         this.checkBoxRankBattle.data = {name:LocalizationData.getName("match_settings_checkbox_rank_match_hint")};
         this.checkBoxTournamentBattle.data = {name:LocalizationData.getName("match_settings_checkbox_tournament_match_hint")};
         this.checkBoxTrainingBattle.data = {name:LocalizationData.getName("match_settings_checkbox_training_match_hint")};
         this.pvbHookLocal();
      }
      
      public function setTournamentResult(accepted:Boolean) : void
      {
         var clanSelected:Boolean = accepted == true ? this._selectCall : !this._selectCall;
         trace("setTournamentResult::",clanSelected,accepted,this._selectCall);
         this.checkBoxClan.selected = clanSelected;
         this.checkBoxPVP.selected = !this.checkBoxClan.selected;
         this.updateLayout();
         this.pvbHookTournament();
      }
      
      protected function showCloseMessage() : void
      {
         var messageData:Object = {
            text:LocalizationData.getName(PlayerInfo.isCommander ? "clan_window_close_message_commander" : "clan_window_close_message"),
            onYes:this.closeWindow,
            label_ok:LocalizationData.getName("ok"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true,MessageId.AUTO,true);
      }
      
      protected function closeWindow() : void
      {
         trace("TW::closeWindow");
         this.deselectClanMatchCheckbox();
         var clanSelected:Boolean = this.checkBoxClan.selected;
         trace("checkbox_tournament_match_clicked",clanSelected);
         ExternalInterface.call("checkbox_tournament_match_clicked",clanSelected);
      }
      
      private function onTournamentSelected(event:ButtonEvent) : void
      {
         trace("onTournamentSelected");
         if(event.target == this.checkBoxPVP)
         {
            this.checkBoxClan.selected = !this.checkBoxPVP.selected;
         }
         var clanSelected:Boolean = this.checkBoxClan.selected;
         _selectCall = clanSelected;
         if(this.checkBoxClan.selected)
         {
            dispatchEvent(new Event(TOURNAMENT_SELECTED));
         }
         else
         {
            this.checkBoxPVP.selected = false;
            this.checkBoxClan.selected = true;
            this.showCloseMessage();
            this.updateLayout();
            return;
         }
         this.updateLayout();
         trace("checkbox_tournament_match_clicked",clanSelected,this._selectCall);
         ExternalInterface.call("checkbox_tournament_match_clicked",clanSelected);
      }
      
      public function pvbHookConfig() : void
      {
         var sectionClass:Class = null;
         var play:Button = null;
         try
         {
            MatchMakingSettings.pvbHolder = this;
            this.pvbOn = false;
            this.pvbCode = PvbCode.UNKNOWN;
            this.pvbGear = -1;
            this.pvbMax = -1;
            this.pvbRunning = -1;
            this.pvbRunMax = -1;
            // the team sizes, the one picked first and the most gear: the server's status's (pvbStatus)
            this.pvbSizes = [];
            this.pvbMaps = [];
            this.pvbMapKey = "";
            this.pvbSizeKey = "";
            this.pvbSize = 0;
            this.pvbMap = 0;
            this.pvbMode = GameMode.GATHER_VICTORY_ITEMS;
            this.pvbLevel = MatchMakingSettings.PVB_LEVEL_DEFAULT;
            this.pvbModeList = [];
            this.pvbLevelRows = [];
            this.pvbAsking = false;
            this.pvbAskedAt = 0;
            this.pvbHeardAt = 0;
            this.pvbChoosing = false;
            this.pvbChooseAt = 0;
            this.pvbWasVisible = false;
            this.pvbDemoStatus = 0;
            this.pvbDemoPlay = 0;
            this.pvbDemoRows = null;
            this.pvbDemoAt = 0;
            sectionClass = getDefinitionByName("top_fla.trainingGame_44") as Class;
            this.pvbSection = new sectionClass() as MovieClip;
            this.pvbSection.name = "pvbGame";
            this.pvbSection.x = this.trainingGame.x;
            this.pvbSection.y = this.trainingGame.y;
            addChild(this.pvbSection);
            this.pvbRadio = this.pvbSection.checkBoxTrainingBattle as Button;
            this.pvbRadio.selected = false;
            this.pvbRadio.enabled = false;
            this.pvbRadio.focusable = false;
            this.pvbRadio.tooltipRendererName = "SrvTextHintRenderer";
            this.pvbRadio.addEventListener(ButtonEvent.CLICK,this.onPvbRadio);
            this.checkBoxPVP.addEventListener(ButtonEvent.CLICK,this.onPvbPvpClick,false,100);
            this.checkBoxClan.addEventListener(ButtonEvent.CLICK,this.onPvbClanClick,false,100);
            this.pvbSizeMenu = this.pvbNewMenu(MatchMakingSettings.PVB_SIZE_ROWS);
            this.pvbModeMenu = this.pvbNewMenu(MatchMakingSettings.PVB_MODE_ROWS);
            this.pvbMapMenu = this.pvbNewMenu(MatchMakingSettings.PVB_MAP_ROWS);
            this.pvbSection.addChild(this.pvbSizeMenu);
            this.pvbSection.addChild(this.pvbModeMenu);
            this.pvbSection.addChild(this.pvbMapMenu);
            this.pvbPlaceMenus();
            if(MatchMakingSettings.PVB_DEMO)
            {
               this.pvbDemoSetup();
            }
            play = this.pvbPlayButton();
            if(play)
            {
               play.addEventListener(ButtonEvent.CLICK,this.onPvbPlay,false,100);
            }
            addEventListener(Event.ENTER_FRAME,this.onPvbFrame);
            this.pvbReady = true;
            this.pvbHookLocal();
            this.pvbButtons();
         }
         catch(e:Error)
         {
            trace("pvbHookConfig",e);
         }
      }
      
      public function pvbHookButtons() : void
      {
         try
         {
            this.pvbButtons();
         }
         catch(e:Error)
         {
         }
      }
      
      public function pvbHookLayout() : void
      {
         try
         {
            this.pvbLayout();
         }
         catch(e:Error)
         {
         }
      }
      
      public function pvbHookLocal() : void
      {
         try
         {
            if(!this.pvbReady)
            {
               return;
            }
            this.pvbRadio.label = LocalizationData.getName("st_srp_pvb_title").toUpperCase();
            this.pvbFillMenus(true);
            this.pvbButtons();
            this.pvbLabel();
         }
         catch(e:Error)
         {
         }
      }
      
      public function pvbHookTournament() : void
      {
         try
         {
            if(!this.pvbReady || !this.pvbOn)
            {
               return;
            }
            if(this.checkBoxClan.selected)
            {
               this.pvbSelect(false);
            }
            else
            {
               this.checkBoxPVP.selected = false;
               this.pvbButtons();
               this.pvbLabel();
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function pvbLevelName(level:int) : String
      {
         var key:String = level >= 1 ? RoomsText.levelKey(level) : null;
         return key != null ? LocalizationData.getName(key) : "";
      }
      
      // The game's own name of a mode, without its trailing spaces.
      protected function pvbModeName(mode:int) : String
      {
         var key:String = mode == GameMode.TEAM_DEATHMATCH ? "match_settings_checkbox_team_match" : "match_settings_checkbox_battery_charge";
         var word:String = LocalizationData.getName(key);
         while(word.length > 0 && word.charAt(word.length - 1) == " ")
         {
            word = word.substr(0,word.length - 1);
         }
         return word;
      }
      
      protected function pvbMapName(row:Object) : String
      {
         var name:String = this.pvbLevelName(int(row.level));
         if(name.length == 0)
         {
            name = "#" + String(row.id);
         }
         return name + " · " + this.pvbModeName(int(row.mode));
      }
      
      protected function pvbSizeName(size:int) : String
      {
         return LocalizationData.format("st_srp_pvb_team_size",size,size);
      }
      
      public function pvbReasonText(why:int) : String
      {
         if(why == PvbCode.GEAR_TOO_HIGH)
         {
            var most:String = String(this.pvbMax);
            return this.pvbGear >= 0 ? LocalizationData.format("st_srp_pvb_gear_max_yours",most,this.pvbGear)
               : LocalizationData.format("st_srp_pvb_gear_max",most);
         }
         if(why == PvbCode.AT_LIMIT)
         {
            return this.pvbRunning >= 0 && this.pvbRunMax > 0 ? LocalizationData.format("st_srp_pvb_at_limit_count",this.pvbRunning,this.pvbRunMax)
               : LocalizationData.getName("st_srp_pvb_at_limit");
         }
         var key:String = MatchMakingSettings.PVB_REASONS[String(why)] as String;
         return LocalizationData.getName(key != null ? key : MatchMakingSettings.PVB_REASONS[String(MatchMakingSettings.PVB_WHY_GENERIC)]);
      }
      
      protected function pvbWhy() : int
      {
         if(PlayerInfo.isInSquad)
         {
            return PvbCode.IN_SQUAD;
         }
         if(this.checkBoxClan && this.checkBoxClan.selected)
         {
            return PvbCode.CLAN_TEAM;
         }
         if(this.pvbCode == PvbCode.UNKNOWN)
         {
            return MatchMakingSettings.PVB_WHY_CHECKING;
         }
         if(this.pvbCode == PvbCode.NO_ANSWER)
         {
            return MatchMakingSettings.PVB_WHY_NOANSWER;
         }
         if(this.pvbCode != PvbCode.ALLOWED)
         {
            return this.pvbCode;
         }
         if(!this.pvbMaps || this.pvbMaps.length == 0)
         {
            return MatchMakingSettings.PVB_WHY_GENERIC;
         }
         return MatchMakingSettings.PVB_WHY_NONE;
      }
      
      protected function pvbNumber(value:Object) : int
      {
         var i:int = 0;
         var code:int = 0;
         var result:int = 0;
         var text:String = String(value);
         if(text.length == 0 || text.length > 9)
         {
            return -1;
         }
         i = 0;
         while(i < text.length)
         {
            code = int(text.charCodeAt(i));
            if(code < CharCode.ZERO || code > CharCode.NINE)
            {
               return -1;
            }
            result = result * 10 + (code - CharCode.ZERO);
            i++;
         }
         return result;
      }
      
      // The top bar's Play box (its Play and the arrow), told by the top bar: the settings are on
      // the transient layer while open, not in the bar.
      public var playContainer:Object;

      protected function pvbContainer() : Object
      {
         return this.playContainer;
      }
      
      protected function pvbPlayButton() : Button
      {
         var box:Object = this.pvbContainer();
         if(box)
         {
            return box.playBtn as Button;
         }
         return null;
      }
      
      protected function pvbChoiceButton() : Button
      {
         var box:Object = this.pvbContainer();
         if(box)
         {
            return box.dropDownButton as Button;
         }
         return null;
      }
      
      protected function pvbLabel() : void
      {
         var button:Button = this.pvbChoiceButton();
         var text:String = null;
         if(!button)
         {
            return;
         }
         if(this.pvbOn)
         {
            text = LocalizationData.getName("st_srp_pvb_title");
         }
         else if(this.checkBoxClan.selected)
         {
            text = LocalizationData.getName("match_settings_clan_match");
         }
         else
         {
            text = LocalizationData.getName("match_settings_checkbox_PVP");
         }
         if(button.label != text)
         {
            button.label = text;
         }
      }
      
      protected function pvbNewMenu(rows:int) : DropdownMenu
      {
         var menuClass:Class = getDefinitionByName("SrvDropDownMenuP") as Class;
         var menu:DropdownMenu = new menuClass() as DropdownMenu;
         menu.dropdown = "OptionScrollingList";
         menu.itemRenderer = "OptionsListRenderer";
         menu.scrollBar = "";
         menu.menuDirection = "down";
         menu.menuMargin = 0;
         menu.thumbOffsetTop = 0;
         menu.thumbOffsetBottom = 0;
         menu.menuRowsFixed = false;
         menu.menuRowCount = rows;
         menu.visible = false;
         menu.addEventListener(ListEvent.INDEX_CHANGE,this.onPvbMenu);
         return menu;
      }
      
      protected function pvbPlaceMenus() : void
      {
         var left:int = 20;
         var width:int = int(this.pvbSection.sectionBack.width) - left - 10;
         if(width < 120)
         {
            width = 196;
         }
         this.pvbSizeMenu.x = left;
         this.pvbSizeMenu.y = 40;
         this.pvbSizeMenu.width = 130;
         this.pvbModeMenu.x = left;
         this.pvbModeMenu.y = int(this.pvbSizeMenu.y + this.pvbSizeMenu.height + 6);
         this.pvbModeMenu.width = width;
         this.pvbModeMenu.menuWidth = width > 230 ? width : 230;
         this.pvbMapMenu.x = left;
         this.pvbMapMenu.y = int(this.pvbModeMenu.y + this.pvbModeMenu.height + 6);
         this.pvbMapMenu.width = width;
         this.pvbMapMenu.menuWidth = width > 260 ? width : 260;
         this.pvbOpenHeight = int(this.pvbMapMenu.y + this.pvbMapMenu.height + 10);
         if(this.pvbOpenHeight < 130)
         {
            this.pvbOpenHeight = 150;
         }
      }
      
      protected function pvbKind(mode:int) : int
      {
         return mode == GameMode.TEAM_DEATHMATCH ? GameMode.TEAM_DEATHMATCH : GameMode.GATHER_VICTORY_ITEMS;
      }
      
      protected function pvbMapRow(id:int) : Object
      {
         var i:int = 0;
         while(i < this.pvbMaps.length)
         {
            if(int(this.pvbMaps[i].id) == id)
            {
               return this.pvbMaps[i];
            }
            i++;
         }
         return null;
      }
      
      protected function pvbSettleChoice() : void
      {
         var i:int = 0;
         var row:Object = this.pvbMapRow(this.pvbMap);
         var level:int = row ? int(row.level) : this.pvbLevel;
         var first:Object = null;
         var same:Object = null;
         if(row && this.pvbKind(int(row.mode)) == this.pvbMode)
         {
            this.pvbLevel = int(row.level);
            return;
         }
         i = 0;
         while(i < this.pvbMaps.length)
         {
            if(this.pvbKind(int(this.pvbMaps[i].mode)) == this.pvbMode)
            {
               if(!first)
               {
                  first = this.pvbMaps[i];
               }
               if(!same && int(this.pvbMaps[i].level) == level)
               {
                  same = this.pvbMaps[i];
               }
            }
            i++;
         }
         if(!first)
         {
            if(row)
            {
               this.pvbMode = this.pvbKind(int(row.mode));
               this.pvbLevel = int(row.level);
               return;
            }
            if(this.pvbMaps.length == 0)
            {
               this.pvbMap = 0;
               return;
            }
            first = this.pvbMaps[0];
            this.pvbMode = this.pvbKind(int(first.mode));
         }
         row = same ? same : first;
         this.pvbMap = int(row.id);
         this.pvbLevel = int(row.level);
      }
      
      protected function pvbFillMenus(all:Boolean) : void
      {
         var i:int = 0;
         var rows:Array = null;
         var row:Object = null;
         var key:String = "";
         var label:String = null;
         var at:int = -1;
         var kind:int = 0;
         var kinds:Array = [];
         rows = [];
         i = 0;
         while(i < this.pvbSizes.length)
         {
            rows.push({
               "label":this.pvbSizeName(int(this.pvbSizes[i])),
               "id":int(this.pvbSizes[i])
            });
            key = key + String(this.pvbSizes[i]) + ";";
            if(int(this.pvbSizes[i]) == this.pvbSize)
            {
               at = i;
            }
            i++;
         }
         if(all || key != this.pvbSizeKey)
         {
            this.pvbSizeKey = key;
            this.pvbSizeMenu.menuRowCount = rows.length > 0 ? rows.length : 1;
            this.pvbSizeMenu.dataProvider = new DataProvider(rows);
         }
         this.pvbSizeMenu.selectedIndex = at;
         this.pvbSizeMenu.invalidateSelectedIndex();
         this.pvbSettleChoice();
         i = 0;
         while(i < this.pvbMaps.length)
         {
            kind = this.pvbKind(int(this.pvbMaps[i].mode));
            if(kinds.indexOf(kind) < 0)
            {
               kinds.push(kind);
            }
            i++;
         }
         this.pvbModeList = kinds;
         this.pvbLevelRows = [];
         i = 0;
         while(i < this.pvbMaps.length)
         {
            if(this.pvbKind(int(this.pvbMaps[i].mode)) == this.pvbMode)
            {
               this.pvbLevelRows.push(this.pvbMaps[i]);
            }
            i++;
         }
         key = String(this.pvbMode) + "|" + String(this.pvbMap) + "|" + kinds.join(",") + "|";
         i = 0;
         while(i < this.pvbLevelRows.length)
         {
            row = this.pvbLevelRows[i];
            key = key + String(row.id) + ":" + String(row.free) + ";";
            i++;
         }
         key += this.pvbCode == PvbCode.ALLOWED ? "a" : "b";
         if(!all && key == this.pvbMapKey)
         {
            return;
         }
         this.pvbMapKey = key;
         rows = [];
         at = -1;
         i = 0;
         while(i < kinds.length)
         {
            rows.push({
               "label":this.pvbModeName(int(kinds[i])),
               "id":int(kinds[i])
            });
            if(int(kinds[i]) == this.pvbMode)
            {
               at = i;
            }
            i++;
         }
         this.pvbModeMenu.menuRowCount = rows.length > 0 ? rows.length : 1;
         this.pvbModeMenu.dataProvider = new DataProvider(rows);
         this.pvbModeMenu.selectedIndex = at;
         this.pvbModeMenu.invalidateSelectedIndex();
         rows = [];
         at = -1;
         i = 0;
         while(i < this.pvbLevelRows.length)
         {
            row = this.pvbLevelRows[i];
            label = this.pvbLevelName(int(row.level));
            if(label.length == 0)
            {
               label = "#" + String(row.id);
            }
            if(int(row.free) == 0 && this.pvbCode == PvbCode.ALLOWED)
            {
               label = LocalizationData.format("st_srp_pvb_busy",label);
            }
            rows.push({
               "label":label,
               "id":int(row.id)
            });
            if(int(row.id) == this.pvbMap)
            {
               at = i;
            }
            i++;
         }
         this.pvbMapMenu.menuRowCount = rows.length > 0 ? (rows.length > MatchMakingSettings.PVB_MAP_ROWS ? MatchMakingSettings.PVB_MAP_ROWS : rows.length) : 1;
         this.pvbMapMenu.dataProvider = new DataProvider(rows);
         this.pvbMapMenu.selectedIndex = at;
         this.pvbMapMenu.invalidateSelectedIndex();
      }
      
      protected function pvbSelect(on:Boolean) : void
      {
         this.pvbOn = on;
         this.pvbRadio.selected = on;
         if(on)
         {
            this.checkBoxPVP.selected = false;
         }
         else if(!this.checkBoxClan.selected)
         {
            this.checkBoxPVP.selected = true;
         }
         this.pvbButtons();
         this.pvbLabel();
      }
      
      protected function pvbButtons() : void
      {
         var clanRule:Boolean = false;
         var why:int = 0;
         var allowed:Boolean = false;
         if(!this.pvbReady)
         {
            return;
         }
         clanRule = (PlayerInfo.clanState == ClanState.OVERVIEW || PlayerInfo.clanState == ClanState.MANAGE) && !PlayerInfo.isInSquad;
         if(this.pvbOn && (PlayerInfo.isInSquad || this.checkBoxClan.selected))
         {
            this.pvbOn = false;
            this.pvbRadio.selected = false;
            if(!this.checkBoxClan.selected)
            {
               this.checkBoxPVP.selected = true;
            }
            this.pvbLabel();
         }
         why = this.pvbWhy();
         allowed = this._enabled && why < 0;
         this.pvbRadio.enabled = allowed;
         this.pvbRadio.mouseEnabled = true;
         this.pvbRadio.selected = this.pvbOn;
         if(!clanRule)
         {
            this.checkBoxPVP.enabled = this._enabled && (allowed || this.pvbOn);
         }
         this.pvbRadio.data = {"name":(why >= 0 ? this.pvbReasonText(why) : LocalizationData.getName("st_srp_pvb_hint"))};
         this.updateLayout();
      }
      
      protected function pvbLayout() : void
      {
         var y:int = 0;
         var bottom:int = 0;
         if(!this.pvbReady)
         {
            return;
         }
         y = int(this.pvpGame.y + this.pvpGame.sectionBack.height);
         this.pvbSection.y = y;
         this.pvbSizeMenu.visible = this.pvbOn;
         this.pvbModeMenu.visible = this.pvbOn;
         this.pvbMapMenu.visible = this.pvbOn;
         this.pvbSection.sectionBack.height = this.pvbOn ? this.pvbOpenHeight : 41;
         this.freeGame.y = int(y + this.pvbSection.sectionBack.height);
         this.clanGame.y = int(this.freeGame.y + this.freeGame.sectionBack.height);
         this.trainingGame.y = int(this.clanGame.y + this.clanGame.sectionBack.height);
         bottom = int(this.trainingGame.y + this.trainingGame.sectionBack.height);
         if(this.pvbDemoBox)
         {
            this.pvbDemoBox.y = bottom + 6;
            bottom = int(this.pvbDemoBox.y + this.pvbDemoBox.height);
         }
         this.background.height = bottom + 10;
      }
      
      protected function onPvbRadio(e:ButtonEvent) : void
      {
         try
         {
            if(!this.pvbRadio.enabled)
            {
               this.pvbRadio.selected = this.pvbOn;
               return;
            }
            this.pvbSelect(true);
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onPvbPvpClick(e:ButtonEvent) : void
      {
         var clanRule:Boolean = false;
         try
         {
            if(this.pvbOn)
            {
               e.stopImmediatePropagation();
               this.pvbSelect(false);
               return;
            }
            clanRule = (PlayerInfo.clanState == ClanState.OVERVIEW || PlayerInfo.clanState == ClanState.MANAGE) && !PlayerInfo.isInSquad;
            if(!clanRule && !this.checkBoxPVP.selected && !this.checkBoxClan.selected)
            {
               e.stopImmediatePropagation();
               this.checkBoxPVP.selected = true;
               this.updateLayout();
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onPvbClanClick(e:ButtonEvent) : void
      {
         try
         {
            if(this.pvbOn)
            {
               this.pvbOn = false;
               this.pvbRadio.selected = false;
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onPvbMenu(e:ListEvent) : void
      {
         var menu:DropdownMenu = e.currentTarget as DropdownMenu;
         var at:int = 0;
         try
         {
            e.stopPropagation();
            at = menu.selectedIndex;
            if(menu == this.pvbSizeMenu)
            {
               if(at >= 0 && at < this.pvbSizes.length)
               {
                  this.pvbSize = int(this.pvbSizes[at]);
               }
            }
            else if(menu == this.pvbModeMenu)
            {
               if(at >= 0 && at < this.pvbModeList.length && int(this.pvbModeList[at]) != this.pvbMode)
               {
                  this.pvbMode = int(this.pvbModeList[at]);
                  this.pvbFillMenus(false);
               }
            }
            else if(menu == this.pvbMapMenu)
            {
               if(at >= 0 && at < this.pvbLevelRows.length)
               {
                  this.pvbMap = int(this.pvbLevelRows[at].id);
                  this.pvbLevel = int(this.pvbLevelRows[at].level);
                  this.pvbFillMenus(false);
               }
            }
            else if(menu == this.pvbDemoStatusMenu)
            {
               this.pvbDemoStatus = at;
               this.pvbCode = PvbCode.UNKNOWN;
               this.pvbButtons();
               this.pvbAsk(getTimer());
            }
            else if(menu == this.pvbDemoPlayMenu)
            {
               this.pvbDemoPlay = at;
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onPvbPlay(e:ButtonEvent) : void
      {
         var play:Button = e.currentTarget as Button;
         try
         {
            if(!this.pvbReady || !this.pvbOn || !play)
            {
               return;
            }
            if(String(play.label).toUpperCase() != LocalizationData.getName("Play").toUpperCase())
            {
               return;
            }
            e.stopImmediatePropagation();
            play.selected = false;
            play.focused = 0;
            this.pvbChoose(getTimer());
         }
         catch(err:Error)
         {
         }
      }
      
      protected function pvbRequest(start:int, count:int) : void
      {
         try
         {
            ExternalInterface.call("request_players_range",start,count);
         }
         catch(err:Error)
         {
         }
      }
      
      protected function pvbAsk(now:int) : void
      {
         this.pvbAsking = true;
         this.pvbAskedAt = now;
         if(this.pvbCode == PvbCode.NO_ANSWER)
         {
            this.pvbCode = PvbCode.UNKNOWN;
            this.pvbButtons();
         }
         if(MatchMakingSettings.PVB_DEMO)
         {
            this.pvbDemoStatusRows(now);
            return;
         }
         this.pvbRequest(MatchMakingSettings.PVB_STATUS,MatchMakingSettings.PVB_ROWS);
      }
      
      protected function pvbChoose(now:int) : void
      {
         var start:int = MatchMakingSettings.PVB_CHOOSE;
         var why:int = 0;
         if(this.pvbChoosing && now - this.pvbChooseAt >= 0 && now - this.pvbChooseAt < MatchMakingSettings.PVB_WAIT_MS)
         {
            return;
         }
         why = this.pvbWhy();
         if(why == PvbCode.IN_SQUAD || why == PvbCode.CLAN_TEAM)
         {
            Message.showMessage("ok",this.pvbReasonText(why));
            return;
         }
         if(this.pvbMap <= 0 || this.pvbSize <= 0)
         {
            Message.showMessage("ok",this.pvbReasonText(MatchMakingSettings.PVB_WHY_GENERIC));
            return;
         }
         this.pvbChoosing = true;
         this.pvbChooseAt = now;
         this.pvbChooseSize = this.pvbSize;
         this.pvbChooseMap = this.pvbMap;
         if(MatchMakingSettings.PVB_DEMO)
         {
            this.pvbDemoChooseRows(now);
            return;
         }
         start = start + this.pvbSize * MatchMakingSettings.PVB_SIZE_STEP + this.pvbMap;
         this.pvbRequest(start,1);
      }
      
      public function pvbData(rows:Array) : void
      {
         var i:int = 0;
         var name:String = null;
         try
         {
            i = 0;
            while(i < rows.length)
            {
               if(rows[i])
               {
                  name = String(rows[i].nickname);
                  if(name.indexOf("B1;") == 0)
                  {
                     this.pvbStatus(name,rows);
                     return;
                  }
                  if(name.indexOf("B2;") == 0)
                  {
                     this.pvbAnswer(name);
                     return;
                  }
               }
               i++;
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function pvbStatus(head:String, rows:Array) : void
      {
         var f:Array = head.split(";");
         var i:int = 0;
         var parts:Array = null;
         var n:int = 0;
         var sizes:Array = [];
         var maps:Array = [];
         var name:String = null;
         var row:Array = null;
         var defSize:int = -1;
         var defMap:int = -1;
         var keepSize:Boolean = false;
         var keepMap:Boolean = false;
         var now:int = getTimer();
         if(f.length < 2)
         {
            return;
         }
         this.pvbCode = this.pvbNumber(f[1]);
         if(this.pvbCode < 0)
         {
            this.pvbCode = MatchMakingSettings.PVB_WHY_GENERIC;
         }
         this.pvbGear = f.length > 2 ? this.pvbNumber(f[2]) : -1;
         this.pvbMax = f.length > 3 ? this.pvbNumber(f[3]) : -1;
         if(f.length > 4)
         {
            parts = String(f[4]).split(".");
            i = 0;
            while(i < parts.length)
            {
               n = this.pvbNumber(parts[i]);
               if(n >= 1 && n <= MatchMakingSettings.PVB_SIZE_MAX)
               {
                  sizes.push(n);
               }
               i++;
            }
         }
         defSize = f.length > 5 ? this.pvbNumber(f[5]) : -1;
         defMap = f.length > 6 ? this.pvbNumber(f[6]) : -1;
         this.pvbRunning = f.length > 7 ? this.pvbNumber(f[7]) : -1;
         this.pvbRunMax = f.length > 8 ? this.pvbNumber(f[8]) : -1;
         i = 0;
         while(i < rows.length)
         {
            if(rows[i])
            {
               name = String(rows[i].nickname);
               if(name.indexOf("B1;") != 0)
               {
                  row = name.split(";");
                  if(row.length >= 4 && this.pvbNumber(row[0]) > 0 && this.pvbNumber(row[1]) >= 0 && this.pvbNumber(row[2]) >= 0 && this.pvbNumber(row[3]) >= 0)
                  {
                     maps.push({
                        "id":this.pvbNumber(row[0]),
                        "level":this.pvbNumber(row[1]),
                        "mode":this.pvbNumber(row[2]),
                        "free":this.pvbNumber(row[3])
                     });
                  }
               }
            }
            i++;
         }
         this.pvbSizes = sizes;
         this.pvbMaps = maps;
         i = 0;
         while(i < sizes.length)
         {
            if(int(sizes[i]) == this.pvbSize)
            {
               keepSize = true;
            }
            i++;
         }
         if(!keepSize)
         {
            this.pvbSize = int(sizes[int(sizes.length / 2)]);
            i = 0;
            while(i < sizes.length)
            {
               if(int(sizes[i]) == defSize)
               {
                  this.pvbSize = defSize;
               }
               i++;
            }
         }
         i = 0;
         while(i < maps.length)
         {
            if(int(maps[i].id) == this.pvbMap)
            {
               keepMap = true;
            }
            i++;
         }
         if(!keepMap)
         {
            this.pvbMap = maps.length > 0 ? int(maps[0].id) : 0;
            i = 0;
            while(i < maps.length)
            {
               if(int(maps[i].id) == defMap)
               {
                  this.pvbMap = defMap;
               }
               i++;
            }
            if(this.pvbMapRow(this.pvbMap))
            {
               this.pvbMode = this.pvbKind(int(this.pvbMapRow(this.pvbMap).mode));
            }
         }
         this.pvbAsking = false;
         this.pvbHeardAt = now;
         this.pvbFillMenus(false);
         this.pvbButtons();
      }
      
      protected function pvbAnswer(head:String) : void
      {
         var f:Array = head.split(";");
         var code:int = 0;
         var now:int = getTimer();
         if(!this.pvbChoosing || f.length < 3)
         {
            return;
         }
         if(f[1] == "ok")
         {
            if(f.length >= 4 && (this.pvbNumber(f[2]) != this.pvbChooseSize || this.pvbNumber(f[3]) != this.pvbChooseMap))
            {
               return;
            }
            this.pvbChoosing = false;
            if(MatchMakingSettings.PVB_DEMO)
            {
               Message.showMessage("ok",LocalizationData.format("st_srp_pvb_demo_start",this.pvbSizeName(this.pvbChooseSize) + ", " + this.pvbChosenMapName()));
               return;
            }
            try
            {
               ExternalInterface.call("play_button_clicked");
            }
            catch(err:Error)
            {
            }
            return;
         }
         if(f[1] == "no")
         {
            this.pvbChoosing = false;
            code = this.pvbNumber(f[2]);
            if(f.length > 4)
            {
               this.pvbGear = this.pvbNumber(f[3]);
               this.pvbMax = this.pvbNumber(f[4]);
            }
            if(f.length > 6)
            {
               this.pvbRunning = this.pvbNumber(f[5]);
               this.pvbRunMax = this.pvbNumber(f[6]);
            }
            if(code < PvbCode.GEAR_TOO_HIGH || code > PvbCode.UNKNOWN_CHOICE)
            {
               code = MatchMakingSettings.PVB_WHY_GENERIC;
            }
            Message.showMessage("ok",this.pvbReasonText(code));
            this.pvbAsk(now);
         }
      }
      
      protected function pvbChosenMapName() : String
      {
         var i:int = 0;
         while(i < this.pvbMaps.length)
         {
            if(int(this.pvbMaps[i].id) == this.pvbChooseMap)
            {
               return this.pvbMapName(this.pvbMaps[i]);
            }
            i++;
         }
         return "#" + String(this.pvbChooseMap);
      }
      
      protected function onPvbFrame(e:Event) : void
      {
         var now:int = 0;
         var shown:Boolean = false;
         var rows:Array = null;
         try
         {
            if(!this.pvbReady)
            {
               return;
            }
            now = getTimer();
            shown = this.visible;
            if(shown && !this.pvbWasVisible)
            {
               this.pvbAsk(now);
            }
            else if(shown && !this.pvbAsking && (now - this.pvbAskedAt >= MatchMakingSettings.PVB_POLL_MS || now < this.pvbAskedAt))
            {
               this.pvbAsk(now);
            }
            this.pvbWasVisible = shown;
            if(this.pvbAsking && (now - this.pvbAskedAt >= MatchMakingSettings.PVB_WAIT_MS || now < this.pvbAskedAt))
            {
               this.pvbAsking = false;
               if(this.pvbHeardAt == 0 || now - this.pvbHeardAt > 3 * MatchMakingSettings.PVB_POLL_MS || this.pvbCode == PvbCode.UNKNOWN)
               {
                  this.pvbCode = PvbCode.NO_ANSWER;
                  this.pvbButtons();
               }
            }
            if(this.pvbChoosing && (now - this.pvbChooseAt >= MatchMakingSettings.PVB_WAIT_MS || now < this.pvbChooseAt))
            {
               this.pvbChoosing = false;
               Message.showMessage("ok",this.pvbReasonText(MatchMakingSettings.PVB_WHY_RETRY));
            }
            if(this.pvbDemoRows && now - this.pvbDemoAt >= 0)
            {
               rows = this.pvbDemoRows;
               this.pvbDemoRows = null;
               MatchMakingSettings.pvbTake(rows);
            }
            if(this.pvbOn)
            {
               this.pvbLabel();
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function pvbDemoSetup() : void
      {
         var status:Array = ["Status: allowed (gear 3)","Status: 1 gear too high (7 of 5)","Status: 2 squad","Status: 3 searching","Status: 4 in a match","Status: 5 old game version","Status: 6 mode off","Status: 7 bot-match limit (5/5)","Status: 8 maintenance soon","Status: 9 clan-team mode","Status: no answer"];
         var play:Array = ["Play: ok","Play: 1 gear too high (7 of 5)","Play: 2 squad","Play: 3 searching","Play: 4 in a match","Play: 5 old game version","Play: 6 mode off","Play: 7 bot-match limit (5/5)","Play: 8 maintenance soon","Play: 9 clan-team mode","Play: 10 unknown map or size","Play: no answer"];
         var rows:Array = [];
         var i:int = 0;
         var width:int = int(this.pvbSection.sectionBack.width) - 24 - 14;
         if(width < 120)
         {
            width = 220;
         }
         this.pvbDemoBox = new MovieClip();
         this.pvbDemoBox.x = this.trainingGame.x;
         addChild(this.pvbDemoBox);
         this.pvbDemoStatusMenu = this.pvbNewMenu(MatchMakingSettings.PVB_DEMO_STATUS_ROWS);
         this.pvbDemoPlayMenu = this.pvbNewMenu(MatchMakingSettings.PVB_DEMO_PLAY_ROWS);
         this.pvbDemoBox.addChild(this.pvbDemoStatusMenu);
         this.pvbDemoBox.addChild(this.pvbDemoPlayMenu);
         i = 0;
         while(i < status.length)
         {
            rows.push({
               "label":"DEMO " + String(status[i]),
               "id":i
            });
            i++;
         }
         this.pvbDemoStatusMenu.dataProvider = new DataProvider(rows);
         this.pvbDemoStatusMenu.selectedIndex = 0;
         rows = [];
         i = 0;
         while(i < play.length)
         {
            rows.push({
               "label":"DEMO " + String(play[i]),
               "id":i
            });
            i++;
         }
         this.pvbDemoPlayMenu.dataProvider = new DataProvider(rows);
         this.pvbDemoPlayMenu.selectedIndex = 0;
         this.pvbDemoStatusMenu.x = 24;
         this.pvbDemoStatusMenu.y = 0;
         this.pvbDemoStatusMenu.width = width;
         this.pvbDemoPlayMenu.x = 24;
         this.pvbDemoPlayMenu.y = int(this.pvbDemoStatusMenu.height + 6);
         this.pvbDemoPlayMenu.width = width;
         this.pvbDemoStatusMenu.visible = true;
         this.pvbDemoPlayMenu.visible = true;
      }
      
      protected function pvbDemoRow(text:String, place:int) : Object
      {
         return {
            "nickname":text,
            "place":place,
            "ranking":0,
            "ranking_diff":0
         };
      }
      
      protected function pvbDemoStatusRows(now:int) : void
      {
         var code:int = this.pvbDemoStatus;
         var rows:Array = [];
         var head:String = null;
         var i:int = 0;
         var free:String = code == PvbCode.AT_LIMIT ? "0" : "1";
         if(code >= MatchMakingSettings.PVB_DEMO_STATUS_NO_ANSWER)
         {
            return;
         }
         head = "B1;" + String(code) + ";" + (code == PvbCode.GEAR_TOO_HIGH ? "7" : "3") + ";5;6.8.10;8;61;" + (code == PvbCode.AT_LIMIT ? "5" : "2") + ";5";
         rows.push(this.pvbDemoRow(head,MatchMakingSettings.PVB_STATUS));
         i = 0;
         while(i < 8)
         {
            rows.push(this.pvbDemoRow(String(61 + 6 * i) + ";" + (String(i + 1)) + ";2;" + free,MatchMakingSettings.PVB_STATUS + 1 + i));
            i++;
         }
         i = 0;
         while(i < 8)
         {
            rows.push(this.pvbDemoRow(String(117 + 3 * i) + ";" + (String(i + 1)) + ";4;" + free,MatchMakingSettings.PVB_STATUS + 9 + i));
            i++;
         }
         this.pvbDemoRows = rows;
         this.pvbDemoAt = now + 400;
      }
      
      protected function pvbDemoChooseRows(now:int) : void
      {
         var code:int = this.pvbDemoPlay;
         var place:int = MatchMakingSettings.PVB_CHOOSE + this.pvbChooseSize * MatchMakingSettings.PVB_SIZE_STEP + this.pvbChooseMap;
         var text:String = null;
         if(code >= MatchMakingSettings.PVB_DEMO_PLAY_NO_ANSWER)
         {
            return;
         }
         if(code == PvbCode.ALLOWED)
         {
            text = "B2;ok;" + String(this.pvbChooseSize) + ";" + String(this.pvbChooseMap);
         }
         else
         {
            text = "B2;no;" + String(code) + ";" + (code == PvbCode.GEAR_TOO_HIGH ? "7" : "3") + ";5;" + (code == PvbCode.AT_LIMIT ? "5" : "2") + ";5";
         }
         this.pvbDemoRows = [this.pvbDemoRow(text,place)];
         this.pvbDemoAt = now + 700;
      }
   }
}

