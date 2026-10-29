package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.InteractiveObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import survarium.buttons.ButtonFriends;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsWindow;
   import survarium.constants.CharCode;
   import survarium.constants.ChatTab;
   import survarium.constants.Language;
   import survarium.constants.MatchJoinCode;
   import survarium.constants.MatchPhase;
   
   public class BottomPanel extends UIComponent
   {
      
      public static const MATCHES_DEMO:Boolean = false;
      
      public static var matchesHolder:BottomPanel;
      
      public static const MATCHES_W:int = 400;
      
      public static const MATCHES_CARD_W:int = 382;
      
      public static const MATCHES_LEVEL_W:int = 110;
      
      // the "matches now" wire (srp's M2): the list asked at MATCHES_START, a join at JOIN_START + the match's key
      public static const MATCHES_START:int = 2000000000;
      
      public static const JOIN_START:int = 2100000000;
      
      // the list's format: M1 rows, or M2 rows (with the join code, the key and the match level)
      public static const MATCHES_M1:int = 1;
      
      public static const MATCHES_M2:int = 2;
      
      // _matchesState: asked (no list yet), listed, no answer
      public static const MATCHES_ASKING:int = 0;
      
      public static const MATCHES_LISTED:int = 1;
      
      public static const MATCHES_NO_ANSWER:int = 2;
      
      // the words of matchesText; MATCHES_SAY_NONE: no word said
      public static const MATCHES_SAY_NONE:int = -1;
      
      public static const MATCHES_WORD_TITLE:int = 0;
      
      public static const MATCHES_WORD_NONE:int = 1;
      
      public static const MATCHES_WORD_WAITING:int = 2;
      
      public static const MATCHES_WORD_STARTING:int = 3;
      
      public static const MATCHES_WORD_JOINING:int = 4;
      
      public static const MATCHES_WORD_FULL:int = 5;
      
      public static const MATCHES_WORD_CLOSED:int = 6;
      
      public static const MATCHES_WORD_CANNOT_JOIN:int = 7;
      
      public static const MATCHES_WORD_NO_ANSWER:int = 8;
      
      // MatchJoinCode.BALANCED..QUEUED have their words at MATCHES_WORD_REASONS + code (9..20)
      public static const MATCHES_WORD_REASONS:int = 7;
      
      // meaning unknown: the movie drops a match key above it
      public static const MATCHES_KEY_MAX:int = 99999999;
      
      // meaning unknown: the movie drops a match level above it
      public static const MATCH_LEVEL_MAX:int = 999;
      
      public var tabsBar:ButtonBarBottom;
      
      public var btnFriends:ButtonFriends;
      
      public var btn1:Button;
      
      public var btn2:Button;
      
      public var btn3:Button;
      
      public var btnShowLogList:Button;
      
      public var background:Sprite;
      
      public var barInvites:ButtonBarBottom;
      
      public var debugLine:Sprite;
      
      public var matchesTab:Button;
      
      protected var _tabs:Array = [];
      
      protected var _invites:Array = [];
      
      protected var _matchesWindow:Sprite;
      
      protected var _matchesTitle:TextField;
      
      protected var _matchesShown:Boolean = false;
      
      protected var _matchesLang:int = 0;
      
      protected var _matchesNames:Object;
      
      protected var _matchesBack:DisplayObject;
      
      protected var _matchesRowsBox:Sprite;
      
      protected var _matchesNote:TextField;
      
      protected var _matchesBar:Shape;
      
      protected var _matchesCardClass:Class;
      
      protected var _matchesJoinClass:Class;
      
      protected var _matchesCards:Array = [];
      
      protected var _matchesData:Array = [];
      
      protected var _matchesVersion:int = 0;
      
      protected var _matchesJoinKey:int = 0;
      
      protected var _matchesJoinAt:int = 0;
      
      protected var _matchesSayKey:int = BottomPanel.MATCHES_SAY_NONE;
      
      protected var _matchesSayAt:int = 0;
      
      protected var _matchesMessage:TextField;
      
      protected var _matchesSayFormat:TextFormat;
      
      protected var _matchesSaySize:int = 12;
      
      protected var _matchesDemoWait:Boolean = false;
      
      protected var _matchesDemoAt:int = 0;
      
      protected var _matchesDemoKey:int = 0;
      
      protected var _matchesDemoStep:int = 0;
      
      protected var _matchesState:int = BottomPanel.MATCHES_ASKING;
      
      protected var _matchesAnswered:Boolean = false;
      
      protected var _matchesStart:int = BottomPanel.MATCHES_START;
      
      protected var _matchesCount:int = 32;
      
      protected var _matchesTickAt:int = 0;
      
      protected var _matchesOpenAt:int = 0;
      
      protected var _matchesAskedAt:int = 0;
      
      protected var _matchesHeardAt:int = 0;
      
      protected var _matchesDataAt:int = 0;
      
      protected var _matchesSecond:int = 0;
      
      protected var _matchesScroll:int = 0;
      
      protected var _matchesHeight:int = 120;
      
      public function BottomPanel()
      {
         super();
         this.debugLine.visible = false;
         BottomPanel.matchesHolder = this;
      }
      
      public static function matchesTake(rows:Array) : Boolean
      {
         var first:Object = null;
         var name:String = null;
         var place:Number = 0;
         var taken:Boolean = false;
         try
         {
            if(rows)
            {
               if(rows.length == 0)
               {
                  if(BottomPanel.matchesHolder && BottomPanel.matchesHolder.matchesWaiting())
                  {
                     taken = true;
                  }
               }
               else
               {
                  first = rows[0];
                  if(first)
                  {
                     place = Number(first.place);
                     name = String(first.nickname);
                     if(place >= BottomPanel.MATCHES_START)
                     {
                        taken = true;
                     }
                     else if(name.indexOf("M1;") == 0 || name.indexOf("M2;") == 0 || name.indexOf("J1;") == 0)
                     {
                        taken = true;
                     }
                  }
                  if(taken && BottomPanel.matchesHolder)
                  {
                     if(place >= BottomPanel.JOIN_START || name.indexOf("J1;") == 0)
                     {
                        BottomPanel.matchesHolder.matchesJoinAnswer(name);
                     }
                     else
                     {
                        BottomPanel.matchesHolder.matchesData(rows);
                     }
                  }
               }
            }
         }
         catch(e:Error)
         {
         }
         return taken;
      }
      
      public function addIvite(data:Object) : void
      {
         this._invites.push({
            "id":data.id,
            "type":data.type,
            "name":data.name,
            "text":data.text
         });
         this.barInvites.dataProvider = new DataProvider(this._invites);
         this.barInvites.validateNow();
         this.updateInvitesPos();
      }
      
      public function removeInvite(id:uint) : void
      {
         var j:int = 0;
         while(j < this._invites.length)
         {
            if(this._invites[j].id == id)
            {
               this._invites.splice(j,1);
               break;
            }
            j++;
         }
         this.barInvites.dataProvider = new DataProvider(this._invites);
         this.barInvites.validateNow();
         this.updateInvitesPos();
      }
      
      public function hasTab(id:uint) : Boolean
      {
         if(!this._tabs)
         {
            return false;
         }
         var j:int = 0;
         while(j < this._tabs.length)
         {
            if(this._tabs[j].id == id)
            {
               return true;
            }
            j++;
         }
         return false;
      }
      
      public function addTab(data:Object) : void
      {
         var l:int = 0;
         var i:uint = 0;
         var oldSelected:int = -1;
         if(this.hasTab(data.id))
         {
            return;
         }
         if(this.tabsBar.selectedIndex != -1)
         {
            oldSelected = int(this._tabs[this.tabsBar.selectedIndex].id);
         }
         this._tabs.push({
            "id":data.id,
            "channels":data.channels,
            "name":data.name,
            "icon":data.channel_to_send,
            "closable":data.closable,
            "channel_to_send":data.channel_to_send,
            "index":data.id + 1,
            "bool_save_history":data.save_history,
            "tabs_group_id":data.tabs_group_id
         });
         this._tabs.sortOn("id",Array.NUMERIC);
         this.roomChatAfterGeneral();
         this.tabsBar.dataProvider = new DataProvider(this._tabs);
         this.tabsBar.dataProvider.invalidate();
         if(oldSelected != -1)
         {
            l = int(this._tabs.length);
            i = 0;
            while(i < l)
            {
               if(this._tabs[i].id == oldSelected)
               {
                  this.tabsBar.selectedIndex = i;
                  break;
               }
               i++;
            }
         }
      }
      
      public function removeTab(id:uint) : void
      {
         var j:int = 0;
         while(j < this._tabs.length)
         {
            if(this._tabs[j].id == id)
            {
               this._tabs.splice(j,1);
               break;
            }
            j++;
         }
         this.tabsBar.dataProvider = new DataProvider(this._tabs);
      }
      
      override protected function preInitialize() : void
      {
         constraints = new Constraints(this,ConstrainMode.REFLOW);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.tabsBar.setMaxFit(true);
         this.barInvites.setMaxFit(true);
         this.tabsBar.y = 1;
         constraints.addElement("btnFriends",this.btnFriends,Constraints.LEFT);
         constraints.addElement("btnShowLogList",this.btnShowLogList,Constraints.RIGHT);
         this.btnFriends.toggle = true;
         stage.addEventListener(Event.RESIZE,this.onStageResize,false,-1);
         this.btnFriends.addEventListener(Event.RESIZE,this.onStageResize,false,-1);
         this.tabsBar.addEventListener(Event.RESIZE,this.resizeTab);
         this.barInvites.width = 400;
         addChild(this.tabsBar);
         this.barInvites.addEventListener(ButtonEvent.CLICK,this.onInviteItemClick);
         this.createMatchesTab();
      }
      
      protected function onInviteItemClick(e:Event) : void
      {
         var acceptable:Boolean = !PlayerInfo.isInSquad;
         if(!e.target || !e.target.data)
         {
            return;
         }
         e.target.data.acceptable = acceptable;
         Message.showMessage("invite",e.target.data,true);
      }
      
      protected function updateInvitesPos() : void
      {
         this.barInvites.x = this.background.width - int(this.barInvites.container.width / this.barInvites.container.scaleX) - this.btnShowLogList.width - 5;
         this.resizeTab();
      }
      
      protected function resizeTab(event:Event = null) : void
      {
         var room:Number = NaN;
         this.background.x = 0;
         this.background.width = this.btnShowLogList.x + 194;
         this.debugLine.x = this.background.width * 0.75;
         this.barInvites.setMaxSize(this.background.width / 3.2);
         room = this.barInvites.x - this.tabsBar.x;
         if(this.matchesTab)
         {
            room -= 150;
         }
         this.tabsBar.setMaxSize(room);
         this.tabsBar.invalidate(InvalidationType.RENDERERS);
         this.placeMatchesTab();
         // The rooms window follows the screen.
         this.hideMatchesTab();
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.layout();
         }
      }
      
      protected function onStageResize(event:Event = null) : void
      {
         var scale:Number = MainScene.scaleFactor;
         setActualSize(stage.stageWidth / scale,height);
         setSize(stage.stageWidth / scale,height);
         constraints.update(stage.stageWidth / scale,_height);
         invalidate(InvalidationType.SETTINGS);
         this.tabsBar.x = this.btnFriends.x + 154;
         this.resizeTab();
         this.y = MainScene.stageHeight - height + 13;
         this.placeMatchesWindow();
      }
      
      protected function matchesNameTable() : Object
      {
         var friends:Array = ["Friends","Друзі","Друзья","Freunde","Znajomi", "Prijatelji", "Přátelé", "Barátok", "Kaverit", "Сябры"];
         var levels:Object = {
            "1":["Rudnya","Рудня","Рудня","Rudnya","Rudnia", "Rudnja", "Rudňa", "Rudnya", "Rudnja", "Рудня"],
            "2":["School","Школа","Школа","Schule","Szkoła", "Škola", "Škola", "Iskola", "Koulu", "Школа"],
            "3":["Vostok Radar Station","РЛС \"Восток\"","РЛС \"Восток\"","RS Vostok","Stacja Radarowa \"Vostok\"", "Radarska stanica Vostok", "Radarová stanice Vostok", "Vosztok radarállomás", "Vostokin tutka-asema", "РЛС \"Восток\""],
            "4":["Chemical Plant","Хімзавод","Химзавод","Chemiebetrieb","Zakłady chemiczne", "Hemijska fabrika", "Chemička", "Vegyi üzem", "Kemiantehdas", "Хімзавод"],
            "5":["Tarakanovsky Fort","Тараканівський форт","Таракановский форт","Tarakanowski Fort","Fort Tarakanów", "Tarakanovski fort", "Tarakanovská pevnost", "Tarakanovi erőd", "Tarakanovin linnoitus", "Тараканаўскі форт"],
            "6":["Mamayev Kurgan","Мамаїв курган","Мамаев курган","Mamaew kurgan","Kurhan Mamaja", "Mamajev kurgan", "Mamajova mohyla", "Mamajev-kurgán", "Mamajevin kurgaani", "Мамаеў курган"],
            "7":["Vector Laboratory","Лабораторія \"Вектор\"","Лаборатория \"Вектор\"","Labor Vektor","Laboratorium \"Wektor\"", "Laboratorija Vektor", "Laboratoř Vektor", "Vektor laboratórium", "Vektor-laboratorio", "Лабараторыя \"Вектар\""],
            "8":["Cologne","Кельн","Кельн","Köln","Most w Kolonii", "Keln", "Kolín nad Rýnem", "Köln", "Köln", "Кёльн"],
            "9":["London","Лондон","Лондон","London","Londyn", "London", "Londýn", "London", "Lontoo", "Лондан"],
            "10":["Vostok Radar Station (v0.21)","РЛС \"Восток\" (v0.21)","РЛС \"Восток\" (v0.21)","RS Vostok (v0.21)","Stacja Radarowa \"Vostok\" (v0.21)", "Radarska stanica Vostok (v0.21)", "Radarová stanice Vostok (v0.21)", "Vosztok radarállomás (v0.21)", "Vostokin tutka-asema (v0.21)", "РЛС \"Восток\" (v0.21)"],
            "11":["Mamayev Kurgan (v0.21)","Мамаїв курган (v0.21)","Мамаев курган (v0.21)","Mamaew kurgan (v0.21)","Kurhan Mamaja (v0.21)", "Mamajev kurgan (v0.21)", "Mamajova mohyla (v0.21)", "Mamajev-kurgán (v0.21)", "Mamajevin kurgaani (v0.21)", "Мамаеў курган (v0.21)"],
            "12":["Chemical Plant (v0.11)","Хімзавод (v0.11)","Химзавод (v0.11)","Chemiebetrieb (v0.11)","Zakłady chemiczne (v0.11)", "Hemijska fabrika (v0.11)", "Chemička (v0.11)", "Vegyi üzem (v0.11)", "Kemiantehdas (v0.11)", "Хімзавод (v0.11)"],
            "13":["Vostok Radar Station (v0.11)","РЛС \"Восток\" (v0.11)","РЛС \"Восток\" (v0.11)","RS Vostok (v0.11)","Stacja Radarowa \"Vostok\" (v0.11)", "Radarska stanica Vostok (v0.11)", "Radarová stanice Vostok (v0.11)", "Vosztok radarállomás (v0.11)", "Vostokin tutka-asema (v0.11)", "РЛС \"Восток\" (v0.11)"]
         };
         var modes:Object = {
            "2":["Protective device","Захисний пристрій","Защитное устройство","Laden der Schutzeinrichtung","Urządzenie ochronne", "Zaštitni uređaj", "Ochranné zařízení", "Védelmi berendezés", "Suojalaite", "Ахоўная прылада"],
            "3":["Protective device","Захисний пристрій","Защитное устройство","Laden der Schutzeinrichtung","Urządzenie ochronne", "Zaštitni uređaj", "Ochranné zařízení", "Védelmi berendezés", "Suojalaite", "Ахоўная прылада"],
            "4":["Team Deathmatch","Командний бій","Командный бой","Kommandokampf","Walka drużynowa", "Timska borba", "Týmový deathmatch", "Csapatos deathmatch", "Joukkuekuolemanottelu", "Камандны бой"],
            "5":["Research","Дослідження","Исследование","Forschung","Badania", "Istraživanje", "Výzkum", "Kutatás", "Tutkimus", "Даследаванне"]
         };
         return {
            "friends":friends,
            "levels":levels,
            "modes":modes
         };
      }
      
      protected function matchesTable() : Object
      {
         if(!this._matchesNames)
         {
            this._matchesNames = this.matchesNameTable();
         }
         return this._matchesNames;
      }
      
      protected function matchesName(kind:String, id:int) : String
      {
         var name:String = null;
         var names:Array = this.matchesTable()[kind][id] as Array;
         if(names)
         {
            name = names[this._matchesLang] as String;
         }
         if(!name)
         {
            name = "#" + String(id);
         }
         return name;
      }
      
      protected function matchesText(key:int) : String
      {
         var words:Array = [["Matches","Матчі","Матчи","Matches","Rozgrywki", "Mečevi", "Zápasy", "Meccsek", "Ottelut", "Матчы"],["No matches right now","Зараз матчів немає","Сейчас матчей нет","Zurzeit keine Matches","Brak rozgrywek w tej chwili", "Trenutno nema mečeva", "Teď neběží žádné zápasy", "Most nincs meccs", "Ei otteluita juuri nyt", "Зараз матчаў няма"],["waiting","очікування","ожидание","Warten","oczekiwanie", "čekanje", "čekání", "várakozás", "odottaa", "чаканне"],["starting","старт","старт","Start","start", "početak", "spouštění", "indítás", "alkaa", "старт"],["joining…","приєднання…","вход…","Beitritt…","dołączanie…", "pridruživanje…", "připojování…", "csatlakozás…", "liitytään…", "уваход…"],["No free place in this match","У матчі немає вільних місць","В матче нет свободных мест","Kein freier Platz im Match","Brak wolnych miejsc w rozgrywce", "Nema slobodnog mesta u ovom meču", "V tomto zápase není volné místo", "Nincs szabad hely ebben a meccsben", "Ottelussa ei ole tilaa", "У матчы няма вольных месцаў"],["This match is closed","Цей матч уже закритий","Этот матч уже закрыт","Dieses Match ist geschlossen","Ta rozgrywka jest już zamknięta", "Ovaj meč je zatvoren", "Tento zápas je uzavřený", "Ez a meccs le van zárva", "Ottelu on suljettu", "Гэты матч ужо закрыты"],["You cannot join now","Зараз приєднатися не можна","Сейчас присоединиться нельзя","Beitritt gerade nicht möglich","Nie możesz teraz dołączyć", "Trenutno ne možeš da se pridružiš", "Teď se nemůžeš připojit", "Most nem csatlakozhatsz", "Et voi liittyä nyt", "Зараз далучыцца нельга"],["No answer, try again","Немає відповіді, спробуйте ще","Нет ответа, попробуйте ещё","Keine Antwort, erneut versuchen","Brak odpowiedzi, spróbuj ponownie", "Nema odgovora, pokušaj ponovo", "Žádná odpověď, zkus to znovu", "Nincs válasz, próbáld újra", "Ei vastausta, yritä uudelleen", "Няма адказу, паспрабуйце яшчэ"],["Match is already balanced","Матч уже збалансований","Матч уже сбалансирован"
         ,"Match ist bereits ausgeglichen","Rozgrywka jest już wyrównana", "Meč je već uravnotežen", "Zápas je už vyvážený", "A meccs már kiegyensúlyozott", "Ottelu on jo tasapainossa", "Матч ужо збалансаваны"],["No free place in this match","У матчі немає вільних місць","В матче нет свободных мест","Kein freier Platz im Match","Brak wolnych miejsc w rozgrywce", "Nema slobodnog mesta u ovom meču", "V tomto zápase není volné místo", "Nincs szabad hely ebben a meccsben", "Ottelussa ei ole tilaa", "У матчы няма вольных месцаў"],["Another player is joining","Інший гравець уже приєднується","Другой игрок уже присоединяется","Ein anderer Spieler tritt gerade bei","Inny gracz właśnie dołącza", "Drugi igrač se pridružuje", "Připojuje se jiný hráč", "Másik játékos csatlakozik", "Toinen pelaaja liittyy", "Іншы гулец ужо далучаецца"],["Match has not started yet","Матч ще не почався","Матч ещё не начался","Match hat noch nicht begonnen","Rozgrywka jeszcze się nie zaczęła", "Meč još nije počeo", "Zápas ještě nezačal", "A meccs még nem kezdődött el", "Ottelu ei ole vielä alkanut", "Матч яшчэ не пачаўся"],["Match ends soon","Матч скоро завершиться","Матч скоро закончится","Match endet bald","Rozgrywka wkrótce się skończy", "Meč se uskoro završava", "Zápas brzy skončí", "A meccs hamarosan véget ér", "Ottelu päättyy pian", "Матч хутка скончыцца"],["Match takes no new players","Матч не приймає нових гравців","Матч не принимает новых игроков","Match nimmt keine neuen Spieler auf","Rozgrywka nie przyjmuje nowych graczy", "Meč ne prima nove igrače", "Zápas nepřijímá další hráče", "A meccs nem fogad új játékosokat", "Otteluun ei oteta uusia pelaajia", "Матч не прымае новых гульцоў"],["You are already in a match","Ви вже в матчі","Вы уже в матче","Du bist bereits in einem Match","Jesteś już w rozgrywce", "Već si u meču", "Už jsi v zápase", "Már meccsben vagy", "Olet jo ottelussa", "Вы ўжо ў матчы"],["Leave your squad first","Спершу залиште загін","Сначала покиньте отряд"
         ,"Verlasse zuerst deine Abteilung","Najpierw opuść brygadę", "Prvo napusti odred", "Nejdřív opusť četu", "Előbb lépj ki a rajból", "Poistu ensin ryhmästä", "Спачатку пакіньце атрад"],["Update your game","Оновіть гру","Обновите игру","Aktualisiere dein Spiel","Zaktualizuj grę", "Ažuriraj igru", "Aktualizuj hru", "Frissítsd a játékot", "Päivitä peli", "Абнавіце гульню"],["You already played in this match","Ви вже грали в цьому матчі","Вы уже играли в этом матче","Du hast in diesem Match schon gespielt","Twoje miejsce w tej rozgrywce już wykorzystano", "Već si igrao u ovom meču", "V tomto zápase už jsi hrál", "Ebben a meccsben már játszottál", "Olet jo pelannut tässä ottelussa", "Вы ўжо гулялі ў гэтым матчы"],["Ends after maintenance starts","Завершиться після початку обслуговування","Закончится после начала обслуживания","Endet nach Beginn der Wartung","Skończy się po rozpoczęciu konserwacji", "Završava se nakon početka održavanja", "Skončí až po začátku údržby", "A karbantartás kezdete után ér véget", "Päättyy huollon alkamisen jälkeen", "Скончыцца пасля пачатку абслугоўвання"],["Leave the search first","Спершу скасуйте пошук","Сначала отмените поиск","Brich zuerst die Suche ab","Najpierw przerwij wyszukiwanie", "Prvo prekini pretragu", "Nejdřív zruš hledání", "Előbb állítsd le a keresést", "Lopeta ensin haku", "Спачатку скасуйце пошук"]];
         return words[key][this._matchesLang];
      }
      
      protected function matchesWhyKey(code:int) : int
      {
         if(code >= MatchJoinCode.BALANCED && code <= MatchJoinCode.QUEUED)
         {
            return BottomPanel.MATCHES_WORD_REASONS + code;
         }
         return BottomPanel.MATCHES_SAY_NONE;
      }
      
      protected function matchesWord() : String
      {
         return this.matchesText(BottomPanel.MATCHES_WORD_TITLE);
      }
      
      protected function onMatchesLocal(value:String) : void
      {
         var i:int = 0;
         var friends:Array = this.matchesTable().friends;
         this._matchesLang = Language.EN;
         i = 1;
         while(i < friends.length)
         {
            if(value == friends[i])
            {
               this._matchesLang = i;
            }
            i++;
         }
         if(this.matchesTab)
         {
            this.matchesTab.data = {
               "id":-1,
               "name":this.matchesWord()
            };
         }
         if(this._matchesWindow)
         {
            this.dropMatchesWindow();
            if(this._matchesShown)
            {
               this.showMatches(true);
            }
         }
      }
      
      protected function createMatchesTab() : void
      {
         var icon:Shape = null;
         var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
         this.matchesTab = new tabClass() as Button;
         this.matchesTab.name = "matchesTab";
         this.matchesTab.data = {
            "id":-1,
            "name":this.matchesWord()
         };
         this.matchesTab.y = 1;
         addChild(this.matchesTab);
         icon = new Shape();
         icon.graphics.lineStyle(1.6,10266277);
         icon.graphics.moveTo(1,12);
         icon.graphics.lineTo(12,1);
         icon.graphics.moveTo(12,12);
         icon.graphics.lineTo(1,1);
         icon.graphics.moveTo(2,8);
         icon.graphics.lineTo(5,11);
         icon.graphics.moveTo(8,11);
         icon.graphics.lineTo(11,8);
         icon.x = 11;
         icon.y = 8;
         this.matchesTab.addChild(icon);
         this.matchesTab.addEventListener(ButtonEvent.CLICK,this.onMatchesTabClick);
         this.matchesTab.addEventListener(Event.ENTER_FRAME,this.onMatchesTabFrame);
         LocalizationData.addCallback(this.onMatchesLocal,"btn_friends");
         this.placeMatchesTab();
         // The tab is still made, and hidden at once; the rooms window (in
         // the lobby, over this bar) is made with it.
         this.hideMatchesTab();
         this.makeRoomsWindow();
      }
      
      protected function onMatchesTabFrame(e:Event) : void
      {
         var chatIcon:DisplayObject = null;
         var back:DisplayObjectContainer = this.matchesTab["background"] as DisplayObjectContainer;
         if(back)
         {
            chatIcon = back.getChildByName("icon");
            if(chatIcon && chatIcon.visible)
            {
               chatIcon.visible = false;
            }
         }
         if(this._matchesShown)
         {
            this.matchesTick();
         }
      }
      
      protected function placeMatchesTab() : void
      {
         var used:Number = NaN;
         if(!this.matchesTab || !this.tabsBar || !this.tabsBar.container)
         {
            return;
         }
         used = this.tabsBar.container.width / this.tabsBar.container.scaleX;
         this.matchesTab.x = this.tabsBar.x + int(used);
         this.placeMatchesWindow();
         // The rooms window follows the bar.
         this.hideMatchesTab();
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.layout();
         }
      }
      
      protected function onMatchesTabClick(e:Event) : void
      {
         this.showMatches(!this._matchesShown);
      }
      
      protected function showMatches(value:Boolean) : void
      {
         var now:int = getTimer();
         this._matchesShown = value;
         if(!value)
         {
            this._matchesJoinKey = 0;
            this._matchesDemoWait = false;
            this._matchesSayKey = BottomPanel.MATCHES_SAY_NONE;
            this.matchesShowSay();
         }
         if(value && !this._matchesWindow)
         {
            this.buildMatchesWindow();
         }
         if(this._matchesWindow)
         {
            this._matchesWindow.visible = value;
            if(value && this._matchesWindow.parent)
            {
               this._matchesWindow.parent.setChildIndex(this._matchesWindow,this._matchesWindow.parent.numChildren - 1);
            }
         }
         this.matchesTab.selected = value;
         if(value && this._matchesWindow)
         {
            this._matchesScroll = 0;
            this.matchesBegin(now);
         }
         this.placeMatchesWindow();
      }
      
      protected function matchesBegin(now:int) : void
      {
         var age:int = now - this._matchesDataAt;
         if(this._matchesState != BottomPanel.MATCHES_LISTED || age < 0 || age >= 6000)
         {
            this._matchesState = BottomPanel.MATCHES_ASKING;
            this._matchesData = [];
         }
         this._matchesAnswered = false;
         this._matchesOpenAt = now;
         this._matchesTickAt = now;
         this.matchesRender();
         this.matchesAsk(now);
      }
      
      protected function placeMatchesWindow() : void
      {
         if(!this._matchesWindow)
         {
            return;
         }
         this._matchesWindow.x = int(this.x + this.matchesTab.x);
         this._matchesWindow.y = int(this.y - this._matchesHeight - 4);
      }
      
      public function matchesWaiting() : Boolean
      {
         return this._matchesShown && this._matchesWindow != null;
      }
      
      protected function matchesAsk(now:int) : void
      {
         this._matchesAskedAt = now;
         if(BottomPanel.MATCHES_DEMO)
         {
            BottomPanel.matchesTake(this.matchesDemoRows(now));
            return;
         }
         try
         {
            this.matchesRequest(this._matchesStart,this._matchesCount);
         }
         catch(e:Error)
         {
         }
      }
      
      protected function matchesRequest(start:int, count:int) : void
      {
         ExternalInterface.call("request_players_range",start,count);
      }
      
      protected function matchesCanJoin(row:Object) : Boolean
      {
         return this._matchesVersion == BottomPanel.MATCHES_M2 && row && row.join == true && int(row.key) > 0;
      }
      
      protected function onMatchesJoinClick(e:Event) : void
      {
         var i:int = 0;
         var row:Object = null;
         var now:int = 0;
         try
         {
            if(this._matchesJoinKey == 0 && this._matchesShown && this._matchesWindow)
            {
               i = 0;
               while(i < this._matchesCards.length)
               {
                  if(this._matchesCards[i].join == e.currentTarget)
                  {
                     row = this._matchesData[i];
                  }
                  i++;
               }
               if(row && this.matchesCanJoin(row))
               {
                  now = getTimer();
                  this._matchesJoinKey = int(row.key);
                  this._matchesJoinAt = now;
                  this.matchesSay(BottomPanel.MATCHES_SAY_NONE,now);
                  this.matchesRender();
                  this.matchesJoinAsk(this._matchesJoinKey,now);
               }
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function matchesJoinAsk(key:int, now:int) : void
      {
         var start:int = BottomPanel.JOIN_START;
         var count:int = 1;
         start += key;
         if(BottomPanel.MATCHES_DEMO)
         {
            this._matchesDemoWait = true;
            this._matchesDemoAt = now + 1500;
            this._matchesDemoKey = key;
            return;
         }
         try
         {
            this.matchesRequest(start,count);
         }
         catch(err:Error)
         {
         }
      }
      
      public function matchesJoinAnswer(name:String) : void
      {
         var say:int = BottomPanel.MATCHES_WORD_CANNOT_JOIN;
         var result:String = null;
         var now:int = getTimer();
         var fields:Array = name.split(";");
         if(fields.length < 3 || fields[0] != "J1")
         {
            return;
         }
         if(this._matchesJoinKey == 0 || this.matchesNumber(fields[2]) != this._matchesJoinKey)
         {
            return;
         }
         result = String(fields[1]);
         this._matchesJoinKey = 0;
         this._matchesDemoWait = false;
         if(result == "ok")
         {
            this.showMatches(false);
            if(!BottomPanel.MATCHES_DEMO)
            {
               try
               {
                  ExternalInterface.call("play_button_clicked");
               }
               catch(err:Error)
               {
               }
            }
            return;
         }
         if(result == "full")
         {
            say = BottomPanel.MATCHES_WORD_FULL;
         }
         else if(result == "gone")
         {
            say = BottomPanel.MATCHES_WORD_CLOSED;
         }
         if(fields.length >= 4 && this.matchesWhyKey(this.matchesNumber(fields[3])) >= 0)
         {
            say = this.matchesWhyKey(this.matchesNumber(fields[3]));
         }
         this.matchesSay(say,now);
         if(this._matchesShown && this._matchesWindow)
         {
            this.matchesRender();
            this.matchesAsk(now);
         }
      }
      
      protected function matchesSay(key:int, now:int) : void
      {
         this._matchesSayKey = key;
         this._matchesSayAt = now;
         this.matchesShowSay();
      }
      
      protected function matchesShowSay() : void
      {
         var size:int = this._matchesSaySize;
         var room:Number = 0;
         var text:String = "";
         var field:TextField = this._matchesMessage;
         if(!field || !this._matchesTitle)
         {
            return;
         }
         if(this._matchesSayKey >= 0)
         {
            text = this.matchesText(this._matchesSayKey);
         }
         if(field.text == text)
         {
            return;
         }
         room = BottomPanel.MATCHES_W - 24 - this._matchesTitle.textWidth - 14;
         this._matchesSayFormat.size = size;
         field.defaultTextFormat = this._matchesSayFormat;
         field.text = text;
         while(text.length > 0 && field.textWidth > room && size > 9)
         {
            size--;
            this._matchesSayFormat.size = size;
            field.setTextFormat(this._matchesSayFormat);
         }
         field.y = 7 + int((this._matchesSaySize - size) / 2);
         field.textColor = 15911311;
      }
      
      protected function matchesTick() : void
      {
         var now:int = 0;
         var gap:int = 0;
         var heard:int = 0;
         var limit:int = 0;
         var second:int = 0;
         try
         {
            if(this._matchesWindow && stage)
            {
               now = getTimer();
               gap = now - this._matchesTickAt;
               this._matchesTickAt = now;
               if(gap < 0 || gap > 3000)
               {
                  this.matchesBegin(now);
               }
               else if(now - this._matchesAskedAt >= 5000)
               {
                  this.matchesAsk(now);
               }
               heard = this._matchesOpenAt;
               limit = 6000;
               if(this._matchesAnswered)
               {
                  heard = this._matchesHeardAt;
                  limit = 12000;
               }
               if(this._matchesState != BottomPanel.MATCHES_NO_ANSWER && now - heard >= limit)
               {
                  this._matchesState = BottomPanel.MATCHES_NO_ANSWER;
                  this._matchesData = [];
                  this.matchesRender();
               }
               second = int((now - this._matchesDataAt) / 1000);
               if(second != this._matchesSecond)
               {
                  this._matchesSecond = second;
                  this.matchesClock(now);
               }
               if(this._matchesDemoWait && now - this._matchesDemoAt >= 0)
               {
                  this.matchesDemoAnswer();
               }
               if(this._matchesJoinKey != 0 && (now - this._matchesJoinAt >= 6000 || now - this._matchesJoinAt < 0))
               {
                  this._matchesJoinKey = 0;
                  this._matchesDemoWait = false;
                  this.matchesSay(BottomPanel.MATCHES_WORD_NO_ANSWER,now);
                  this.matchesRender();
               }
               if(this._matchesSayKey >= 0 && (now - this._matchesSayAt >= 8000 || now - this._matchesSayAt < 0))
               {
                  this.matchesSay(BottomPanel.MATCHES_SAY_NONE,now);
               }
            }
         }
         catch(e:Error)
         {
         }
      }
      
      public function matchesData(rows:Array) : void
      {
         var i:int = 0;
         var row:Object = null;
         var list:Array = [];
         var now:int = getTimer();
         this._matchesVersion = BottomPanel.MATCHES_M1;
         if(rows.length > 0 && rows[0] && String(rows[0].nickname).indexOf("M2;") == 0)
         {
            this._matchesVersion = BottomPanel.MATCHES_M2;
         }
         i = 0;
         while(i < rows.length)
         {
            row = this.matchesParseRow(rows[i]);
            if(row)
            {
               list.push(row);
            }
            i++;
         }
         this._matchesData = list;
         this._matchesState = BottomPanel.MATCHES_LISTED;
         this._matchesAnswered = true;
         this._matchesHeardAt = now;
         this._matchesDataAt = now;
         this._matchesSecond = 0;
         if(this._matchesShown && this._matchesWindow)
         {
            this.matchesRender();
         }
      }
      
      protected function matchesParseRow(item:Object) : Object
      {
         var key:int = -1;
         var join:int = 0;
         var matchLevel:int = -1;
         var fields:Array = null;
         var level:int = 0;
         var mode:int = 0;
         var phase:int = 0;
         var team0:int = 0;
         var team1:int = 0;
         var score0:int = 0;
         var score1:int = 0;
         var score:String = "–";
         var row:Object = null;
         try
         {
            if(item)
            {
               fields = String(item.nickname).split(";");
               if(fields.length >= 9)
               {
                  level = this.matchesNumber(fields[0]);
                  mode = this.matchesNumber(fields[1]);
                  phase = this.matchesNumber(fields[2]);
                  team0 = this.matchesNumber(fields[3]);
                  team1 = this.matchesNumber(fields[4]);
                  score0 = this.matchesNumber(fields[5]);
                  score1 = this.matchesNumber(fields[6]);
                  if(score0 >= 0 && score1 >= 0)
                  {
                     score = String(score0) + ":" + String(score1);
                  }
                  if(fields.length >= 10)
                  {
                     key = this.matchesNumber(fields[9]);
                     if(key < 1 || key > BottomPanel.MATCHES_KEY_MAX)
                     {
                        key = -1;
                     }
                  }
                  if(fields.length >= 11)
                  {
                     matchLevel = this.matchesNumber(fields[10]);
                     if(matchLevel > BottomPanel.MATCH_LEVEL_MAX)
                     {
                        matchLevel = -1;
                     }
                  }
                  join = this.matchesNumber(fields[8]);
                  if(level >= 0 && mode >= 0 && phase >= 0 && team0 >= 0 && team1 >= 0)
                  {
                     row = {
                        "level":level,
                        "mode":mode,
                        "phase":phase,
                        "team0":team0,
                        "team1":team1,
                        "score":score,
                        "played":this.matchesNumber(fields[7]),
                        "join":join == MatchJoinCode.MAY_JOIN,
                        "why":join,
                        "key":key,
                        "matchLevel":matchLevel
                     };
                  }
               }
            }
         }
         catch(e:Error)
         {
            row = null;
         }
         return row;
      }
      
      protected function matchesNumber(value:Object) : int
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
      
      protected function matchesDemoRows(now:int) : Array
      {
         var i:int = 0;
         var rows:Array = [];
         var add:int = int((now - this._matchesOpenAt) / 1000);
         var texts:Array = ["M2;11","1;4;2;5;4;38;41;" + (String(445 + add)) + ";1;101;3","2;2;2;8;8;2;1;" + (String(760 + add)) + ";2;102;12","4;3;2;10;10;0;0;" + (String(65 + add)) + ";12;103;20","3;4;1;3;3;-;-;-;1;104;0","5;4;0;1;0;-;-;-;1;105;-","broken;row","7;2;2;10;9;100;100;" + (String(3590 + add)) + ";9;106;7","6;4;2;7;6;120;98;" + (String(301 + add)) + ";4;107;5","8;3;2;2;3;1;4;" + (String(902 + add)) + ";13;108","99;9;2;4;4;12;7;" + (String(15 + add)) + ";1"];
         i = 0;
         while(i < texts.length)
         {
            rows.push({
               "nickname":texts[i],
               "place":BottomPanel.MATCHES_START + i,
               "ranking":0,
               "ranking_diff":0
            });
            i++;
         }
         return rows;
      }
      
      protected function matchesDemoAnswer() : void
      {
         var results:Array = ["full","gone","busy","","ok","full"];
         var codes:Array = [";2",";12",";9","",";1",""];
         var step:int = this._matchesDemoStep % 6;
         var result:String = results[step];
         this._matchesDemoWait = false;
         this._matchesDemoStep += 1;
         BottomPanel.matchesTake([{
            "nickname":"J1;ok;" + (String(this._matchesDemoKey + 1000)),
            "place":BottomPanel.JOIN_START,
            "ranking":0,
            "ranking_diff":0
         }]);
         if(result.length > 0)
         {
            BottomPanel.matchesTake([{
               "nickname":"J1;" + result + ";" + String(this._matchesDemoKey) + codes[step],
               "place":BottomPanel.JOIN_START,
               "ranking":0,
               "ranking_diff":0
            }]);
         }
      }
      
      protected function buildMatchesWindow() : void
      {
         var format:TextFormat = null;
         var panelClass:Class = getDefinitionByName("SrvNotificationListSC") as Class;
         var panel:MovieClip = new panelClass() as MovieClip;
         var back:DisplayObject = panel.getChildByName("background");
         var tabText:TextField = this.matchesTab["textField"] as TextField;
         var window:Sprite = new Sprite();
         this._matchesCardClass = getDefinitionByName("NotificationMatchRenderer") as Class;
         this._matchesJoinClass = getDefinitionByName("SvButton2SmallFr") as Class;
         window.name = "matchesWindow";
         back.x = 0;
         back.y = 0;
         back.width = BottomPanel.MATCHES_W;
         back.height = 120;
         window.addChild(back);
         this._matchesTitle = new TextField();
         this._matchesTitle.embedFonts = true;
         this._matchesTitle.selectable = false;
         this._matchesTitle.mouseEnabled = false;
         this._matchesTitle.defaultTextFormat = tabText.getTextFormat();
         this._matchesTitle.x = 12;
         this._matchesTitle.y = 7;
         this._matchesTitle.width = BottomPanel.MATCHES_W - 24;
         this._matchesTitle.height = 24;
         this._matchesTitle.text = this.matchesWord().toUpperCase();
         window.addChild(this._matchesTitle);
         this._matchesRowsBox = new Sprite();
         this._matchesRowsBox.y = 34;
         window.addChild(this._matchesRowsBox);
         format = tabText.getTextFormat();
         format.align = TextFormatAlign.CENTER;
         this._matchesNote = new TextField();
         this._matchesNote.embedFonts = true;
         this._matchesNote.selectable = false;
         this._matchesNote.mouseEnabled = false;
         this._matchesNote.defaultTextFormat = format;
         this._matchesNote.x = 12;
         this._matchesNote.y = 61;
         this._matchesNote.width = BottomPanel.MATCHES_W - 24;
         this._matchesNote.height = 24;
         window.addChild(this._matchesNote);
         this._matchesBar = new Shape();
         window.addChild(this._matchesBar);
         this._matchesSayFormat = tabText.getTextFormat();
         this._matchesSayFormat.align = TextFormatAlign.RIGHT;
         this._matchesSaySize = 12;
         if(this._matchesSayFormat.size != null && int(this._matchesSayFormat.size) > 9)
         {
            this._matchesSaySize = int(this._matchesSayFormat.size);
         }
         this._matchesMessage = new TextField();
         this._matchesMessage.embedFonts = true;
         this._matchesMessage.selectable = false;
         this._matchesMessage.mouseEnabled = false;
         this._matchesMessage.defaultTextFormat = this._matchesSayFormat;
         this._matchesMessage.x = 12;
         this._matchesMessage.y = 7;
         this._matchesMessage.width = BottomPanel.MATCHES_W - 24;
         this._matchesMessage.height = 24;
         window.addChild(this._matchesMessage);
         window.addEventListener(MouseEvent.MOUSE_WHEEL,this.onMatchesWheel);
         window.visible = false;
         parent.addChild(window);
         this._matchesBack = back;
         this._matchesHeight = 120;
         this._matchesCards = [];
         this._matchesWindow = window;
         this.matchesShowSay();
      }
      
      protected function dropMatchesWindow() : void
      {
         var window:Sprite = this._matchesWindow;
         this._matchesWindow = null;
         this._matchesTitle = null;
         this._matchesBack = null;
         this._matchesRowsBox = null;
         this._matchesNote = null;
         this._matchesBar = null;
         this._matchesMessage = null;
         this._matchesCards = [];
         if(window)
         {
            window.removeEventListener(MouseEvent.MOUSE_WHEEL,this.onMatchesWheel);
            if(window.parent)
            {
               window.parent.removeChild(window);
            }
         }
      }
      
      protected function onMatchesWheel(e:MouseEvent) : void
      {
         var old:int = this._matchesScroll;
         var last:int = this._matchesCards.length - 5;
         if(last <= 0)
         {
            return;
         }
         if(e.delta > 0)
         {
            this._matchesScroll = old - 1;
         }
         else
         {
            this._matchesScroll = old + 1;
         }
         if(this._matchesScroll < 0)
         {
            this._matchesScroll = 0;
         }
         if(this._matchesScroll > last)
         {
            this._matchesScroll = last;
         }
         if(this._matchesScroll != old)
         {
            this.matchesLayout();
            e.preventDefault();
            e.stopImmediatePropagation();
         }
      }
      
      protected function matchesRender() : void
      {
         var i:int = 0;
         var rec:Object = null;
         var now:int = getTimer();
         var count:int = int(this._matchesData.length);
         if(!this._matchesWindow)
         {
            return;
         }
         while(this._matchesCards.length > count)
         {
            rec = this._matchesCards.pop();
            if(rec.holder.parent)
            {
               rec.holder.parent.removeChild(rec.holder);
            }
         }
         while(this._matchesCards.length < count)
         {
            rec = this.matchesNewCard();
            if(!rec)
            {
               break;
            }
            this._matchesCards.push(rec);
         }
         i = 0;
         while(i < this._matchesCards.length)
         {
            rec = this._matchesCards[i];
            this.matchesFillCard(rec,this._matchesData[i],now);
            if(!rec.holder.parent)
            {
               this._matchesRowsBox.addChild(rec.holder);
            }
            i++;
         }
         this.matchesLayout();
      }
      
      protected function matchesRing(card:MovieClip) : DisplayObject
      {
         var i:int = 0;
         var child:DisplayObject = null;
         var ring:DisplayObject = null;
         try
         {
            i = card.numChildren - 1;
            while(i >= 0)
            {
               child = card.getChildAt(i);
               if(!ring && child && !(child is InteractiveObject))
               {
                  if(child.width > 23 && child.width < 30 && child.height > 23 && child.height < 30)
                  {
                     ring = child;
                  }
               }
               i--;
            }
         }
         catch(e:Error)
         {
            ring = null;
         }
         return ring;
      }
      
      protected function matchesNewCard() : Object
      {
         var rec:Object = null;
         var holder:Sprite = null;
         var cardBox:Sprite = null;
         var card:MovieClip = null;
         var join:Button = null;
         var map:TextField = null;
         var label:TextField = null;
         var result:TextField = null;
         var time:TextField = null;
         var status:TextField = null;
         var ring:DisplayObject = null;
         var cover:Shape = null;
         var step:int = 0;
         var angle:Number = 0;
         var format:TextFormat = null;
         var statusFormat:TextFormat = null;
         var joinLabel:String = null;
         var joinWidth:int = 0;
         var extra:int = 0;
         var back:DisplayObject = null;
         var level:TextField = null;
         var levelFormat:TextFormat = null;
         var statusSize:int = 14;
         var cardClass:Class = this._matchesCardClass;
         var joinClass:Class = this._matchesJoinClass;
         var centerX:Number = 27;
         var centerY:Number = 31.5;
         var timeY:Number = 51;
         try
         {
            holder = new Sprite();
            cardBox = new Sprite();
            cardBox.mouseEnabled = false;
            cardBox.mouseChildren = false;
            card = new cardClass() as MovieClip;
            card.x = 7;
            cardBox.addChild(card);
            holder.addChild(cardBox);
            card["rewardBar"].visible = false;
            map = card["textField"] as TextField;
            label = card["textFieldLabel"] as TextField;
            result = card["textFieldResult"] as TextField;
            time = card["textFieldTime"] as TextField;
            format = result.getTextFormat();
            statusFormat = label.getTextFormat();
            levelFormat = label.getTextFormat();
            ring = this.matchesRing(card);
            if(ring)
            {
               ring.scaleX = 1.4;
               ring.scaleY = 1.4;
               ring.x = centerX - 1.4 * 29.775;
               ring.y = centerY - 1.4 * 32.525;
            }
            else
            {
               cover = new Shape();
               cover.graphics.beginFill(197636,1);
               cover.graphics.moveTo(29.775 + 14.2,32.525);
               step = 1;
               while(step <= 48)
               {
                  angle = step * Math.PI / 24;
                  cover.graphics.lineTo(29.775 + 14.2 * Math.cos(angle),32.525 + 14.2 * Math.sin(angle));
                  step++;
               }
               cover.graphics.endFill();
               cover.graphics.lineStyle(1.4,10066329,0.5);
               cover.graphics.moveTo(centerX + 17.6,centerY);
               step = 1;
               while(step <= 48)
               {
                  angle = step * Math.PI / 24;
                  cover.graphics.lineTo(centerX + 17.6 * Math.cos(angle),centerY + 17.6 * Math.sin(angle));
                  step++;
               }
               card.addChild(cover);
               card.addChild(result);
            }
            result.wordWrap = false;
            result.x = centerX - 22.075 - 1.2;
            result.text = "";
            time.wordWrap = false;
            time.x = centerX - 22.075 - 1.2;
            time.y = timeY;
            time.text = "";
            extra = BottomPanel.MATCHES_CARD_W - 286;
            back = card["background"] as DisplayObject;
            if(back)
            {
               back.width = BottomPanel.MATCHES_CARD_W;
            }
            if(card["btnClose"])
            {
               card["btnClose"].x += extra;
            }
            map.wordWrap = false;
            map.width = BottomPanel.MATCHES_CARD_W - 59 - BottomPanel.MATCHES_LEVEL_W - 8;
            map.text = "";
            label.wordWrap = false;
            label.width = BottomPanel.MATCHES_CARD_W - 59;
            label.text = "";
            levelFormat.align = TextFormatAlign.RIGHT;
            level = new TextField();
            level.embedFonts = true;
            level.selectable = false;
            level.mouseEnabled = false;
            level.defaultTextFormat = levelFormat;
            level.x = card.x + BottomPanel.MATCHES_CARD_W - 4 - BottomPanel.MATCHES_LEVEL_W;
            level.y = map.y;
            level.width = BottomPanel.MATCHES_LEVEL_W;
            level.height = 22;
            holder.addChild(level);
            join = new joinClass() as Button;
            joinLabel = LocalizationData.getName("btn_join_team");
            join.label = joinLabel;
            joinWidth = int(joinLabel.length * 7.3 + 18);
            if(joinWidth < 100)
            {
               joinWidth = 100;
            }
            if(joinWidth > 160)
            {
               joinWidth = 160;
            }
            join.x = card.x + BottomPanel.MATCHES_CARD_W - 4 - joinWidth;
            join.y = 47;
            holder.addChild(join);
            join.width = joinWidth;
            join.enabled = false;
            join.addEventListener(ButtonEvent.CLICK,this.onMatchesJoinClick);
            status = new TextField();
            status.embedFonts = true;
            status.selectable = false;
            status.mouseEnabled = false;
            status.defaultTextFormat = statusFormat;
            status.x = card.x + 59 - 2;
            status.y = 48;
            status.width = join.x - 6 - status.x;
            status.height = 22;
            holder.addChild(status);
            statusSize = 14;
            if(statusFormat.size != null && int(statusFormat.size) > 9)
            {
               statusSize = int(statusFormat.size);
            }
            rec = {
               "holder":holder,
               "card":card,
               "map":map,
               "label":label,
               "result":result,
               "time":time,
               "status":status,
               "statusFormat":statusFormat,
               "statusSize":statusSize,
               "statusY":status.y,
               "level":level,
               "join":join,
               "format":format,
               "size":0,
               "centerY":centerY
            };
         }
         catch(e:Error)
         {
            rec = null;
         }
         return rec;
      }
      
      protected function matchesPut(field:TextField, value:String, colour:uint) : void
      {
         if(field.text != value)
         {
            field.text = value;
            field.textColor = colour;
         }
      }
      
      protected function matchesPutFit(rec:Object, value:String, colour:uint) : void
      {
         var field:TextField = rec.status;
         var format:TextFormat = rec.statusFormat;
         var base:int = int(rec.statusSize);
         var size:int = base;
         var room:Number = field.width - 6;
         var text:String = value;
         if(rec.statusText == value && rec.statusColour == colour)
         {
            return;
         }
         rec.statusText = value;
         rec.statusColour = colour;
         format.size = size;
         field.defaultTextFormat = format;
         field.text = text;
         while(text.length > 0 && field.textWidth > room && size > 10)
         {
            size--;
            format.size = size;
            field.setTextFormat(format);
         }
         while(text.length > 1 && field.textWidth > room)
         {
            text = text.substr(0,text.length - 2) + "…";
            field.text = text;
            field.setTextFormat(format);
         }
         field.y = Number(rec.statusY) + int((base - size) / 2);
         field.textColor = colour;
      }
      
      protected function matchesLevelWord() : String
      {
         var word:String = LocalizationData.getName("level");
         if(!word || word.length == 0)
         {
            return "";
         }
         return word.charAt(0).toUpperCase() + word.substr(1).toLowerCase();
      }
      
      protected function matchesFillCard(rec:Object, row:Object, now:int) : void
      {
         var size:int = 12;
         var canJoin:Boolean = false;
         var word:String = "";
         var wordColour:uint = 8356742;
         var score:String = String(row.score);
         var label:String = this.matchesName("modes",int(row.mode)) + "  ·  " + String(row.team0) + " v " + String(row.team1);
         this.matchesPut(rec.map,this.matchesName("levels",int(row.level)),13421772);
         this.matchesPut(rec.label,label,6973545);
         if(score.length == 6)
         {
            size = 10;
         }
         else if(score.length > 6)
         {
            size = 9;
         }
         if(rec.size != size)
         {
            rec.size = size;
            rec.format.size = size;
            rec.result.defaultTextFormat = rec.format;
            rec.result.y = rec.centerY - 0.636 * size - 1.5;
            rec.result.text = score;
            rec.result.setTextFormat(rec.format);
            rec.result.textColor = 10266277;
         }
         else
         {
            this.matchesPut(rec.result,score,10266277);
         }
         if(row.phase == MatchPhase.GATHERING)
         {
            word = this.matchesText(BottomPanel.MATCHES_WORD_WAITING);
         }
         else if(row.phase == MatchPhase.COUNTDOWN)
         {
            word = this.matchesText(BottomPanel.MATCHES_WORD_STARTING);
            wordColour = 16704178;
         }
         canJoin = this._matchesJoinKey == 0 && this.matchesCanJoin(row);
         if(word == "" && !canJoin && this._matchesVersion == BottomPanel.MATCHES_M2 && this.matchesWhyKey(int(row.why)) >= 0)
         {
            word = this.matchesText(this.matchesWhyKey(int(row.why)));
            wordColour = 16704178;
         }
         if(this._matchesJoinKey != 0 && int(row.key) == this._matchesJoinKey)
         {
            word = this.matchesText(BottomPanel.MATCHES_WORD_JOINING);
            wordColour = 16704178;
         }
         this.matchesPutFit(rec,word,wordColour);
         if(int(row.matchLevel) >= 0)
         {
            this.matchesPut(rec.level,this.matchesLevelWord() + " " + String(row.matchLevel),10266277);
         }
         else
         {
            this.matchesPut(rec.level,"",10266277);
         }
         this.matchesPutTime(rec,row,now);
         if(rec.join.enabled != canJoin)
         {
            rec.join.enabled = canJoin;
         }
      }
      
      protected function matchesPutTime(rec:Object, row:Object, now:int) : void
      {
         var total:int = 0;
         var minutes:int = 0;
         var seconds:int = 0;
         var text:String = "";
         if(row.phase >= MatchPhase.PLAYING && row.played >= 0)
         {
            total = int(row.played) + int((now - this._matchesDataAt) / 1000);
            minutes = int(total / 60);
            seconds = total - minutes * 60;
            text = (minutes < 10 ? "0" : "") + String(minutes) + ":" + (seconds < 10 ? "0" : "") + String(seconds);
         }
         this.matchesPut(rec.time,text,6973545);
      }
      
      protected function matchesClock(now:int) : void
      {
         var i:int = 0;
         i = 0;
         while(i < this._matchesCards.length)
         {
            if(this._matchesData[i])
            {
               this.matchesPutTime(this._matchesCards[i],this._matchesData[i],now);
            }
            i++;
         }
      }
      
      protected function matchesLayout() : void
      {
         var i:int = 0;
         var holder:DisplayObject = null;
         var thumb:int = 0;
         var thumbY:int = 0;
         var rowsHeight:int = 0;
         var full:int = 0;
         var note:String = "";
         var count:int = int(this._matchesCards.length);
         var shown:int = count;
         var last:int = count - 5;
         if(!this._matchesWindow)
         {
            return;
         }
         if(shown > 5)
         {
            shown = 5;
         }
         if(shown < 1)
         {
            shown = 1;
         }
         if(last < 0)
         {
            last = 0;
         }
         if(this._matchesScroll > last)
         {
            this._matchesScroll = last;
         }
         if(this._matchesScroll < 0)
         {
            this._matchesScroll = 0;
         }
         i = 0;
         while(i < count)
         {
            holder = this._matchesCards[i].holder;
            holder.y = (i - this._matchesScroll) * 80;
            holder.visible = i >= this._matchesScroll && i < this._matchesScroll + 5;
            i++;
         }
         rowsHeight = shown * 80;
         full = 34 + rowsHeight + 6;
         if(this._matchesHeight != full)
         {
            this._matchesHeight = full;
            this._matchesBack.height = full;
         }
         this._matchesBar.graphics.clear();
         if(last > 0)
         {
            thumb = int((rowsHeight - 1) * 5 / count);
            if(thumb < 20)
            {
               thumb = 20;
            }
            thumbY = int((rowsHeight - 1 - thumb) * this._matchesScroll / last);
            this._matchesBar.graphics.beginFill(3158321,1);
            this._matchesBar.graphics.drawRect(BottomPanel.MATCHES_W - 8,34,3,rowsHeight - 1);
            this._matchesBar.graphics.endFill();
            this._matchesBar.graphics.beginFill(10266277,1);
            this._matchesBar.graphics.drawRect(BottomPanel.MATCHES_W - 8,34 + thumbY,3,thumb);
            this._matchesBar.graphics.endFill();
         }
         if(this._matchesData.length == 0)
         {
            if(this._matchesState == BottomPanel.MATCHES_ASKING)
            {
               note = "…";
            }
            else
            {
               note = this.matchesText(BottomPanel.MATCHES_WORD_NONE);
            }
         }
         this.matchesPut(this._matchesNote,note,8356742);
         this._matchesNote.visible = this._matchesData.length == 0;
         this.placeMatchesWindow();
      }
      
      // The Matches tab (Skhid's list of running matches and their JOIN) is
      // gone: every PvP match is a room's, listed and joined in the rooms
      // window (the owner's of 2026-10-17).
      private function hideMatchesTab() : void
      {
         if(this.matchesTab != null && this.matchesTab.visible)
         {
            this.matchesTab.visible = false;
         }
      }
      
      // The rooms window, made once, with the Matches tab: in the lobby, over
      // this bar (survarium.rooms).
      private function makeRoomsWindow() : void
      {
         try
         {
            if(RoomsWindow.me == null)
            {
               RoomsWindow.me = new RoomsWindow(this);
            }
         }
         catch(err:Error)
         {
         }
      }

      // The tabs sorted by id: a room's chat goes right after the general
      // chat's (the owner's of 2026-10-20: "[General Chat] [Room Chat]").
      private function roomChatAfterGeneral() : void
      {
         var room:int = -1;
         var general:int = -1;
         for(var i:int = 0; i < this._tabs.length; i++)
         {
            if(this._tabs[i].id == ChatTab.ROOM)
            {
               room = i;
            }
            else if(this._tabs[i].id == ChatTab.GENERAL)
            {
               general = i;
            }
         }
         if(room < 0 || general < 0 || room == general + 1)
         {
            return;
         }
         var tab:Object = this._tabs.splice(room,1)[0];
         if(room < general)
         {
            general--;
         }
         this._tabs.splice(general + 1,0,tab);
      }
   }
}

