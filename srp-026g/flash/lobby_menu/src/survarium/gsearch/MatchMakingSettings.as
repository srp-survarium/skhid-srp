package survarium.gsearch
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
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
   import survarium.constants.Language;
   import survarium.constants.MessageId;
   import survarium.constants.PvbCode;
   
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
      
      // the words of pvbWords (pvbText keys; PVB_WHY_* are some of them too)
      public static const PVB_WORD_TITLE:int = 0;
      
      public static const PVB_WORD_HINT:int = 1;
      
      public static const PVB_WORD_VS:int = 2;
      
      public static const PVB_WORD_BUSY:int = 3;
      
      public static const PVB_WORD_GEAR_MUST_BE:int = 4;
      
      public static const PVB_WORD_OR_LOWER:int = 5;
      
      public static const PVB_WORD_YOURS:int = 6;
      
      public static const PVB_WORD_CLOSE:int = 7;
      
      // PvbCode.IN_SQUAD..OFF have their words at PVB_WORD_REASONS + code (8..12)
      public static const PVB_WORD_REASONS:int = 6;
      
      public static const PVB_WORD_AT_LIMIT:int = 13;
      
      public static const PVB_WORD_TRY_LATER:int = 14;
      
      public static const PVB_WORD_MAINTENANCE:int = 15;
      
      public static const PVB_WORD_CLAN_MATCH:int = 16;
      
      public static const PVB_WORD_UNKNOWN_CHOICE:int = 21;
      
      public static const PVB_WORD_DEMO_START:int = 22;
      
      // a bot match's start request: PVB_CHOOSE + size * PVB_SIZE_STEP + map (the server's B2 rows)
      public static const PVB_SIZE_STEP:int = 1000;
      
      // the team sizes offered when the server gives none, and the one picked first
      public static const PVB_SIZE_SMALL:int = 6;
      
      public static const PVB_SIZE_DEFAULT:int = 8;
      
      public static const PVB_SIZE_LARGE:int = 10;
      
      // meaning unknown: pvbData drops team sizes above it
      public static const PVB_SIZE_MAX:int = 20;
      
      // the map level picked first: Rudnya (pvbLevelName)
      public static const PVB_LEVEL_DEFAULT:int = 1;
      
      // the most gear level shown when the server gives none (the server's default SRP_PVB_MAX_GEAR)
      public static const PVB_MAX_GEAR_DEFAULT:String = "5";
      
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
      
      protected var pvbLang:int;
      
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
      
      protected var pvbPopupDown:Boolean;
      
      protected var pvbOpenHeight:int;
      
      protected var pvbDemoStatus:int;
      
      protected var pvbDemoPlay:int;
      
      protected var pvbDemoRows:Array;
      
      protected var pvbDemoAt:int;
      
      protected var pvbWordsCache:Array;
      
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
            this.pvbSizes = [MatchMakingSettings.PVB_SIZE_SMALL,MatchMakingSettings.PVB_SIZE_DEFAULT,MatchMakingSettings.PVB_SIZE_LARGE];
            this.pvbMaps = [];
            this.pvbMapKey = "";
            this.pvbSizeKey = "";
            this.pvbSize = MatchMakingSettings.PVB_SIZE_DEFAULT;
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
            this.pvbPopupDown = false;
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
            if(stage)
            {
               stage.addEventListener(MouseEvent.MOUSE_DOWN,this.onPvbStageDown,true,100);
               stage.addEventListener(MouseEvent.CLICK,this.onPvbStageClick,false,100);
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
            this.pvbLang = this.pvbFindLang();
            this.pvbRadio.label = this.pvbText(MatchMakingSettings.PVB_WORD_TITLE).toUpperCase();
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
      
      protected function pvbFindLang() : int
      {
         var word:String = LocalizationData.getName("btn_friends");
         if(word == "Друзі")
         {
            return Language.UK;
         }
         if(word == "Друзья")
         {
            return Language.RU;
         }
         if(word == "Freunde")
         {
            return Language.DE;
         }
         if(word == "Znajomi")
         {
            return Language.PL;
         }
         if(word == "Prijatelji")
         {
            return Language.SR;
         }
         if(word == "Přátelé")
         {
            return Language.CS;
         }
         if(word == "Barátok")
         {
            return Language.HU;
         }
         if(word == "Kaverit")
         {
            return Language.FI;
         }
         return Language.EN;
      }
      
      protected function pvbWords() : Array
      {
         var words:Array = [["Player vs Bots","Гравець проти ботів","Игрок против ботов","Spieler vs. Bots","Gracz z botami", "Igrač protiv botova", "Hráč proti botům", "Játékos a botok ellen", "Pelaaja vastaan botit"],["A match against bots, with bots on your team too","Бій проти ботів, у вашій команді теж боти","Бой против ботов, в вашей команде тоже боты","Ein Match gegen Bots, auch in deinem Team spielen Bots","Rozgrywka z botami, w twojej drużynie też są boty", "Meč protiv botova, sa botovima i u tvom timu", "Zápas proti botům, s boty i ve tvém týmu", "Meccs botok ellen, a saját csapatodban is botokkal", "Ottelu botteja vastaan, myös omassa joukkueessasi on botteja"],[" vs "," на "," на "," vs. "," na ", " protiv ", " proti ", " ellen ", " vastaan "],[" (busy)"," (зайнято)"," (занято)"," (belegt)"," (zajęte)", " (zauzeto)", " (obsazeno)", " (foglalt)", " (varattu)"],["Equipment level must be ","Рівень спорядження має бути ","Уровень снаряжения должен быть ","Ausrüstung Niveau muss ","Poziom wyposażenia musi wynosić ", "Nivo opreme mora biti ", "Úroveň vybavení musí být ", "A felszerelés szintje legfeljebb ", "Varusteiden tason on oltava "],[" or lower"," або нижче"," или ниже"," oder niedriger sein"," lub mniej", " ili niži", " nebo nižší", " lehet", " tai alempi"],[" (yours: "," (у вас: "," (у вас: "," (deins: "," (twój: ", " (tvoj nivo: ", " (tvoje: ", " (a tiéd: ", " (omasi: "],[")",")",")",")",")", ")", ")", ")", ")"],["Leave your squad first","Спершу залиште загін","Сначала покиньте отряд","Verlasse zuerst deine Abteilung","Najpierw opuść brygadę", "Prvo napusti odred", "Nejdřív opusť četu", "Előbb lépj ki az osztagból", "Poistu ensin ryhmästä"],["Leave the search first","Спершу скасуйте пошук","Сначала отмените поиск","Brich zuerst die Suche ab"
         ,"Najpierw przerwij wyszukiwanie", "Prvo prekini pretragu", "Nejdřív zruš hledání", "Előbb állítsd le a keresést", "Lopeta ensin haku"],["You are already in a match","Ви вже в матчі","Вы уже в матче","Du bist bereits in einem Match","Jesteś już w rozgrywce", "Već si u meču", "Už jsi v zápase", "Már meccsben vagy", "Olet jo ottelussa"],["Update your game","Оновіть гру","Обновите игру","Aktualisiere dein Spiel","Zaktualizuj grę", "Ažuriraj igru", "Aktualizuj hru", "Frissítsd a játékot", "Päivitä peli"],["Matches with bots are switched off now","Матчі з ботами зараз вимкнені","Матчи с ботами сейчас отключены","Matches mit Bots sind gerade abgeschaltet","Rozgrywki z botami są teraz wyłączone", "Mečevi protiv botova su trenutno isključeni", "Zápasy s boty jsou teď vypnuté", "A botmeccsek most ki vannak kapcsolva", "Bottiottelut ovat nyt pois käytöstä"],["The most bot matches at a time are already running","Уже йде найбільше матчів з ботами одночасно","Уже идёт максимум матчей с ботами одновременно","Die Höchstzahl an Bot-Matches läuft bereits","Trwa już maksymalna liczba rozgrywek z botami", "Već je pokrenut najveći dozvoljeni broj mečeva protiv botova", "Už běží maximální počet zápasů s boty", "Már elindult a megengedett legtöbb botmeccs", "Bottiotteluita on jo enimmäismäärä käynnissä"],[", try again in a minute",", спробуйте за хвилину",", попробуйте через минуту",", versuche es in einer Minute erneut",", spróbuj ponownie za minutę", ", pokušaj ponovo za minut", ", zkus to za minutu", ", próbáld újra egy perc múlva", ", yritä uudelleen minuutin kuluttua"],["Maintenance starts soon, no new bot matches","Скоро технічне обслуговування, нові матчі з ботами не почнуться","Скоро техническое обслуживание, новые матчи с ботами не начнутся"
         ,"Die Wartung beginnt bald, keine neuen Bot-Matches","Wkrótce konserwacja, nowe rozgrywki z botami nie wystartują", "Održavanje uskoro počinje, nema novih mečeva protiv botova", "Brzy začne údržba, nové zápasy s boty se nespustí", "Hamarosan karbantartás, nem indul új botmeccs", "Huolto alkaa pian, uusia bottiotteluita ei aloiteta"],["Switch off \"Clan match\" first","Спершу вимкніть \"Клановий бій\"","Сначала отключите \"Клановый бой\"","Schalte zuerst \"Clan Kampf\" ab","Najpierw wyłącz \"Bitwa klanowa\"", "Prvo isključi \"Klanski meč\"", "Nejdřív vypni „Klanový zápas“", "Előbb kapcsold ki a „Klánmeccs” opciót", "Poista ensin klaaniottelu käytöstä"],["Checking…","Перевірка…","Проверка…","Prüfung…","Sprawdzanie…", "Provera…", "Kontrola…", "Ellenőrzés…", "Tarkistetaan…"],["No answer from the server","Сервер не відповідає","Сервер не отвечает","Keine Antwort vom Server","Brak odpowiedzi serwera", "Server ne odgovara", "Server neodpovídá", "A szerver nem válaszol", "Palvelin ei vastaa"],["No answer, try again","Немає відповіді, спробуйте ще","Нет ответа, попробуйте ещё","Keine Antwort, erneut versuchen","Brak odpowiedzi, spróbuj ponownie", "Nema odgovora, pokušaj ponovo", "Žádná odpověď, zkus to znovu", "Nincs válasz, próbáld újra", "Ei vastausta, yritä uudelleen"],["A bot match cannot start now","Матч з ботами зараз почати не можна","Матч с ботами сейчас начать нельзя","Ein Bot-Match kann gerade nicht starten","Rozgrywka z botami nie może teraz wystartować", "Meč protiv botova trenutno ne može da počne", "Zápas s boty teď nelze spustit", "Most nem indítható botmeccs", "Bottiottelua ei voi aloittaa nyt"],["This map or team size is not available now","Ця карта чи розмір команд зараз недоступні","Эта карта или размер команд сейчас недоступны","Diese Karte oder Teamgröße ist gerade nicht verfügbar"
         ,"Ta mapa lub wielkość drużyn jest teraz niedostępna", "Ova mapa ili veličina tima trenutno nije dostupna", "Tato mapa nebo velikost týmu teď není dostupná", "Ez a pálya vagy csapatméret most nem érhető el", "Tämä kartta tai joukkuekoko ei ole nyt saatavilla"],["Demo: the server would start the match now: ","Демо: сервер зараз почав би матч: ","Демо: сервер сейчас начал бы матч: ","Demo: der Server würde das Match jetzt starten: ","Demo: serwer uruchomiłby teraz rozgrywkę: ", "Demo: server bi sada pokrenuo meč: ", "Demo: server by teď spustil zápas: ", "Demó: a szerver most elindítaná a meccset: ", "Demo: palvelin aloittaisi nyt ottelun: "]];
         return words;
      }
      
      protected function pvbText(key:int) : String
      {
         var row:Array = null;
         if(!this.pvbWordsCache)
         {
            this.pvbWordsCache = this.pvbWords();
         }
         row = this.pvbWordsCache[key] as Array;
         var text:String = null;
         if(row)
         {
            text = row[this.pvbLang] as String;
            if(!text)
            {
               text = row[0] as String;
            }
         }
         if(!text)
         {
            text = "";
         }
         return text;
      }
      
      protected function pvbLevelName(level:int) : String
      {
         var names:Array = [null,["Rudnya","Рудня","Рудня","Rudnya","Rudnia", "Rudnja", "Rudňa", "Rudnya", "Rudnja"],["School","Школа","Школа","Schule","Szkoła", "Škola", "Škola", "Iskola", "Koulu"],["Vostok Radar Station","РЛС \"Восток\"","РЛС \"Восток\"","RS Vostok","Stacja Radarowa \"Vostok\"", "Radarska stanica Vostok", "Radarová stanice Vostok", "Vosztok radarállomás", "Vostokin tutka-asema"],["Chemical Plant","Хімзавод","Химзавод","Chemiebetrieb","Zakłady chemiczne", "Hemijska fabrika", "Chemička", "Vegyi üzem", "Kemiantehdas"],["Tarakanovsky Fort","Тараканівський форт","Таракановский форт","Tarakanowski Fort","Fort Tarakanów", "Tarakanovski fort", "Tarakanovská pevnost", "Tarakanovi erőd", "Tarakanovin linnoitus"],["Mamayev Kurgan","Мамаїв курган","Мамаев курган","Mamaew kurgan","Kurhan Mamaja", "Mamajev kurgan", "Mamajova mohyla", "Mamajev-kurgán", "Mamajevin kurgaani"],["Vector Laboratory","Лабораторія \"Вектор\"","Лаборатория \"Вектор\"","Labor Vektor","Laboratorium \"Wektor\"", "Laboratorija Vektor", "Laboratoř Vektor", "Vektor laboratórium", "Vektor-laboratorio"],["Cologne","Кельн","Кельн","Köln","Most w Kolonii", "Keln", "Kolín nad Rýnem", "Köln", "Köln"],["London","Лондон","Лондон","London","Londyn", "London", "Londýn", "London", "Lontoo"]];
         var row:Array = null;
         if(level >= 1 && level < names.length)
         {
            row = names[level] as Array;
         }
         if(row)
         {
            return String(row[this.pvbLang]);
         }
         return "";
      }
      
      protected function pvbModeName(mode:int) : String
      {
         var key:String = mode == GameMode.TEAM_DEATHMATCH ? "match_settings_checkbox_team_match" : "match_settings_checkbox_battery_charge";
         var word:String = LocalizationData.getName(key);
         var spare:Array = mode == GameMode.TEAM_DEATHMATCH ? ["Team Deathmatch","Командний бій","Командный бой","Kommandokampf","Walka drużynowa", "Timska borba", "Týmový deathmatch", "Csapatos deathmatch", "Joukkuekuolemanottelu"] : ["Protective device","Захисний пристрій","Защитное устройство","Laden der Schutzeinrichtung","Urządzenie ochronne", "Zaštitni uređaj", "Ochranné zařízení", "Védelmi berendezés", "Suojalaite"];
         if(!word || word.length == 0 || word.charAt(0) == "#")
         {
            word = String(spare[this.pvbLang]);
         }
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
         return String(size) + this.pvbText(MatchMakingSettings.PVB_WORD_VS) + String(size);
      }
      
      public function pvbReasonText(why:int) : String
      {
         var text:String = null;
         if(why == PvbCode.GEAR_TOO_HIGH)
         {
            text = this.pvbText(MatchMakingSettings.PVB_WORD_GEAR_MUST_BE);
            if(this.pvbMax >= 0)
            {
               text = text + String(this.pvbMax) + this.pvbText(MatchMakingSettings.PVB_WORD_OR_LOWER);
            }
            else
            {
               text = text + MatchMakingSettings.PVB_MAX_GEAR_DEFAULT + this.pvbText(MatchMakingSettings.PVB_WORD_OR_LOWER);
            }
            if(this.pvbGear >= 0)
            {
               text = text + this.pvbText(MatchMakingSettings.PVB_WORD_YOURS) + String(this.pvbGear) + this.pvbText(MatchMakingSettings.PVB_WORD_CLOSE);
            }
            return text;
         }
         if(why == PvbCode.AT_LIMIT)
         {
            text = this.pvbText(MatchMakingSettings.PVB_WORD_AT_LIMIT);
            if(this.pvbRunning >= 0 && this.pvbRunMax > 0)
            {
               text = text + " (" + String(this.pvbRunning) + "/" + String(this.pvbRunMax) + ")";
            }
            return text + this.pvbText(MatchMakingSettings.PVB_WORD_TRY_LATER);
         }
         if(why >= PvbCode.IN_SQUAD && why <= PvbCode.OFF)
         {
            return this.pvbText(MatchMakingSettings.PVB_WORD_REASONS + why);
         }
         if(why == PvbCode.MAINTENANCE)
         {
            return this.pvbText(MatchMakingSettings.PVB_WORD_MAINTENANCE);
         }
         if(why == PvbCode.CLAN_TEAM)
         {
            return this.pvbText(MatchMakingSettings.PVB_WORD_CLAN_MATCH);
         }
         if(why == PvbCode.UNKNOWN_CHOICE)
         {
            return this.pvbText(MatchMakingSettings.PVB_WORD_UNKNOWN_CHOICE);
         }
         if(why == MatchMakingSettings.PVB_WHY_CHECKING || why == MatchMakingSettings.PVB_WHY_NOANSWER || why == MatchMakingSettings.PVB_WHY_RETRY)
         {
            return this.pvbText(why);
         }
         return this.pvbText(MatchMakingSettings.PVB_WHY_GENERIC);
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
      
      protected function pvbContainer() : Object
      {
         var found:Object = null;
         try
         {
            found = public::Object(this.parent).play_container;
         }
         catch(e:Error)
         {
            found = null;
         }
         return found;
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
            text = this.pvbText(MatchMakingSettings.PVB_WORD_TITLE);
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
               label += this.pvbText(MatchMakingSettings.PVB_WORD_BUSY);
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
         this.pvbRadio.data = {"name":(why >= 0 ? this.pvbReasonText(why) : this.pvbText(MatchMakingSettings.PVB_WORD_HINT))};
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
      
      protected function onPvbStageDown(e:MouseEvent) : void
      {
         try
         {
            this.pvbPopupDown = false;
            if(!this.visible || !this.pvbAnyMenuOpen())
            {
               return;
            }
            if(e.target is DisplayObject && this.contains(e.target as DisplayObject))
            {
               return;
            }
            this.pvbPopupDown = true;
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onPvbStageClick(e:MouseEvent) : void
      {
         try
         {
            if(this.pvbPopupDown)
            {
               this.pvbPopupDown = false;
               e.stopImmediatePropagation();
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function pvbAnyMenuOpen() : Boolean
      {
         if(this.pvbSizeMenu && this.pvbSizeMenu.selected)
         {
            return true;
         }
         if(this.pvbModeMenu && this.pvbModeMenu.selected)
         {
            return true;
         }
         if(this.pvbMapMenu && this.pvbMapMenu.selected)
         {
            return true;
         }
         if(this.pvbDemoStatusMenu && this.pvbDemoStatusMenu.selected)
         {
            return true;
         }
         if(this.pvbDemoPlayMenu && this.pvbDemoPlayMenu.selected)
         {
            return true;
         }
         return false;
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
         if(sizes.length == 0)
         {
            sizes = [MatchMakingSettings.PVB_SIZE_SMALL,MatchMakingSettings.PVB_SIZE_DEFAULT,MatchMakingSettings.PVB_SIZE_LARGE];
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
               Message.showMessage("ok",this.pvbText(MatchMakingSettings.PVB_WORD_DEMO_START) + this.pvbSizeName(this.pvbChooseSize) + ", " + this.pvbChosenMapName());
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

