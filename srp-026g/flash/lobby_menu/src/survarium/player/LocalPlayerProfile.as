package survarium.player
{
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.*;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.motion.Tween;
   import scaleform.clik.utils.Padding;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.PlayerProfileBase;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IAmmoShare;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.IItemFactory;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   import survarium.constants.AmmoView;
   import survarium.constants.MoneyType;
   import survarium.constants.SuperCategory;
   import survarium.constants.UiSound;
   import survarium.rooms.RoomsText;
   
   public class LocalPlayerProfile extends PlayerProfileBase implements ILocalPlayerProfile
   {
      
      public var labelText:TextField;
      
      public var btnShowAmmo:Button;
      
      public var playerWeight:MovieClip;
      
      public var glassSprite:MovieClip;
      
      public var checkbox_autobuy:Button;
      
      public var checkbox_autorepair:Button;
      
      public var ammoContainer:MovieClip;
      
      public var buyPremium:MovieClip;
      
      private var _weightC:Number = 0;
      
      // autobuy_changed's state: which of the auto buy (ammo) and auto repair boxes are ticked
      private static const AUTOBUY_NONE:uint = 0;
      
      private static const AUTOBUY_AMMO:uint = 1;
      
      private static const AUTOBUY_REPAIR:uint = 2;
      
      private static const AUTOBUY_BOTH:uint = 3;
      
      private var _weightM:Number = 0;
      
      private var _relocateFunctionRef:Function;
      
      private var _ammo_share:IAmmoShare;
      
      private var _tween:Tween;
      
      private var _playerPaperDoll:ILocalPaperDoll;
      
      private var _playerInventory:IInventoryItemFactory;
      
      protected var _unresolvedItems:Array = new Array();
      
      protected var _convertedItems:Array;
      // Kit presets (presets.md, the owner's of 2026-10-23): five sets of the
      // three kits, one active. The panel's title is the switcher: it names
      // the active preset ("Preset 2 · Lvl. 15"), and a press on it opens the
      // five under it; a press on one chooses it. The active one is the kits:
      // choosing another puts it on, and every change of the kits is the
      // active one's (the server's). srp's wire: the ranking request at
      // PRESETS_FIRST and on (lobby.md 2.16.4), answered with the list,
      // `P1;L;<done>;<preset>;<empty>;<active>` and a row
      // `<preset>;<saved>;<level>` each (level -1: none of its kits can play).
      public static var me:LocalPlayerProfile;
      
      private static const PRESETS_FIRST:int = 2090000000;
      
      private static const PRESETS_LAST:int = PRESETS_FIRST + 99;
      
      private static const PRESET_CHOOSE:int = PRESETS_FIRST + 10;
      
      private static const PRESET_COUNT:int = 5;
      
      // What the list's header says was done: chosen, refused.
      private static const PRESET_CHOSEN:int = 1;
      
      private static const PRESET_REFUSED:int = 2;
      
      // The title bar (the timeline's header, 570 x 31 at 5, 5) and the
      // caret drawn after the title's text.
      private static const TITLE_X:Number = 5;
      
      private static const TITLE_Y:Number = 5;
      
      private static const TITLE_W:Number = 570;
      
      private static const TITLE_H:Number = 31;
      
      private static const CARET_W:Number = 10;
      
      private static const CARET_H:Number = 6;
      
      private static const CARET_GAP:Number = 8;
      
      // A choice shown before the server's answer, at most this long (ms).
      private static const PRESET_PENDING_MS:int = 5000;
      
      // The words, in the lobby's five languages (RoomsText.language). The
      // level's word is the kit tabs' own ("Lvl."), this one if it is missing.
      private static const PRESET_WORDS:Object = {
         "preset":["Preset","Пресет","Пресет","Preset","Zestaw"],
         "level":["Lvl.","Рів.","Ур.","Stufe","Poz."],
         "empty":["empty","порожній","пустой","leer","pusty"],
         "unfit":["can't play","не для бою","не для боя","nicht spielbar","niegrywalny"],
         "gone":["{n} slots left empty: those items are gone.","Порожніх слотів: {n}, цих предметів немає.","Пустых слотов: {n}, этих предметов нет.","{n} Plätze leer: diese Gegenstände fehlen.","Pustych miejsc: {n}, tych przedmiotów brak."],
         "refused":["The preset could not be chosen.","Не вдалося обрати пресет.","Не удалось выбрать пресет.","Das Preset ließ sich nicht wählen.","Nie udało się wybrać zestawu."]
      };
      
      // The title bar's press area and its hover / open look (below the
      // title's text), the caret, and the open list (null: closed).
      private var _presetTitle:Sprite;
      
      private var _presetGlow:Shape;
      
      private var _presetCaret:Shape;
      
      private var _presetList:ScrollingList;
      
      private var _presetHover:Boolean = false;
      
      // The title's text field as the timeline placed it: its x and centre.
      private var _titleX:Number = 0;
      
      private var _titleCentre:Number = 0;
      
      // {known, saved, level} a preset, preset 1 first; the active one (0
      // the first), as the server last said; whether it said it yet.
      private var _presets:Array = [];
      
      private var _presetAt:int = 0;
      
      private var _presetsSeen:Boolean = false;
      
      // A preset chosen and not answered yet (-1: none): the title names it.
      private var _presetPending:int = -1;
      
      private var _presetPendingAt:int = 0;
      
      private var _presetsAskedAt:int = -100000;
      
      // The kits locked (as the paper doll and the ammo button): no preset
      // chosen.
      private var _presetsLocked:Boolean = false;
      
      public function LocalPlayerProfile()
      {
         super();
         me = this;
         var rendererPremium:Class = getDefinitionByName("ProfileBuyPremium") as Class;
         buyPremium = new rendererPremium();
         _playerPaperDoll = profileItems as ILocalPaperDoll;
         addChild(this.buyPremium);
         this.buyPremium.visible = false;
         this.buyPremium.x = 17;
         this.buyPremium.y = 113;
         var rendererBack:Class = getDefinitionByName("BackBlack") as Class;
         var bb:MovieClip = new rendererBack();
         bb.y = -10;
         bb.x = -10;
         this.buyPremium.addChildAt(bb,0);
         bb.height = this.height - this.buyPremium.y - bb.y - 4;
         bb.width = this.width - this.buyPremium.x - bb.x - 4;
      }
      
      override public function get height() : Number
      {
         return 902;
      }
      
      override public function get width() : Number
      {
         return 580;
      }
      
      public function set _relocateFunction(value:Function) : void
      {
         trace("LocalPlayerProfile::set",value);
         _relocateFunctionRef = value;
      }
      
      public function get _relocateFunction() : Function
      {
         trace("LocalPlayerProfile::get",this._relocateFunctionRef);
         return this._relocateFunctionRef;
      }
      
      public function get paperDoll() : ILocalPaperDoll
      {
         return this._playerPaperDoll;
      }
      
      public function get btnSetAmmo() : Button
      {
         return this.ammoContainer.btn_apply;
      }
      
      public function setAmmoShare(value:IAmmoShare) : void
      {
         _ammo_share = value;
         this._ammo_share.registerWeightTextField(this.playerWeight.textFieldValue);
      }
      
      public function lockTutorial(value:Boolean) : void
      {
      }
      
      public function getRelativeData(value:Object) : Object
      {
         if(value.superCategory == SuperCategory.AMMO)
         {
            return null;
         }
         var equippedItems:DataProvider = profileItems.dataProvider as DataProvider;
         if(equippedItems)
         {
            var l:uint = equippedItems.length;
            for(var j:uint = 0; j < l; j++)
            {
               var itemData:ItemData = equippedItems[j];
               if(itemData.category == value.category || itemData.isWeapon && value.isWeapon && itemData.oneHanded == value.oneHanded)
               {
                  if(itemData.itemId == value.itemId)
                  {
                     return null;
                  }
                  return itemData;
               }
            }
         }
         return null;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         profileItems.enabled = value;
         this.btnShowAmmo.enabled = value;
         this.lockPresets(!value);
         if(this.btnShowAmmo.selected && value == false)
         {
            this.onAmmoButton();
         }
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function get_ammo_share() : IAmmoShare
      {
         return this._ammo_share;
      }
      
      override public function registerInventory(link:IItemFactory) : void
      {
         _playerInventory = link as IInventoryItemFactory;
         _inventory = link;
         this._playerPaperDoll.registerInventory(this._playerInventory);
      }
      
      public function registerRelocateFunction(link:Function) : void
      {
         trace("PP:registerRelocateFunction");
         _relocateFunction = link;
      }
      
      public function updateWeight(cur:Number, max:Number) : void
      {
         _weightC = cur;
         _weightM = max;
         this.playerWeight.textFieldValue.styleSheet = MainScene.textStyleSheet;
         var style:String = this._weightC < this._weightM ? "highlight" : "red";
         this.playerWeight.textFieldValue.text = Util.styledChar(cur.toFixed(2),style) + " / " + Util.getNumberString(max);
         this.playerWeight.progress_indicator.maximum = this._weightM;
         this.playerWeight.progress_indicator.value = this._weightC;
         this._playerPaperDoll.ammoControl.update(null,"player_profile");
      }
      
      public function enableSecondWeapon(value:Boolean) : void
      {
         this._playerPaperDoll.enableSecondWeapon(value);
      }
      
      public function allowSecondaryTwoHandedWeapon(value:Boolean) : void
      {
         this._playerPaperDoll.allowSecondaryTwoHandedWeapon(value);
      }
      
      public function lock(state:Boolean, arg:uint) : void
      {
         if(state)
         {
            this._playerPaperDoll.lockSlot(state,arg);
            profileItems.enabled = false;
         }
         else
         {
            this._playerPaperDoll.lockSlot(state,0);
            profileItems.enabled = true;
         }
         this._ammo_share.enabled = !state;
         this.btnShowAmmo.enabled = !state;
         this.lockPresets(state);
      }
      
      public function onDrag(e:DragEvent) : void
      {
         if(e == null || e.dragData == null)
         {
            return;
         }
         if(e.type == "dragStart")
         {
            this._playerPaperDoll.selectAgeements(e.dragData.category,e.dragData.dictId,e.dragData.oneHanded,false,e.dragData.itemId);
         }
         else
         {
            this._playerPaperDoll.deselectAgeements();
         }
      }
      
      public function fillAutoBuyItems(value:Array) : void
      {
         this._playerPaperDoll.ammoControl.fillItems(value);
      }
      
      public function fillEquippedItems() : void
      {
         var equippedItems:DataProvider = profileItems.dataProvider as DataProvider;
         trace("fillEquippedItems",equippedItems);
         if(equippedItems)
         {
            var l:uint = equippedItems.length;
            for(var j:uint = 0; j < l; j++)
            {
               var itemData:ItemData = equippedItems[j];
               var item:ItemData = this._playerInventory.getInventoryItem(itemData.itemId);
               if(!item)
               {
                  trace("failedToCreateItem");
               }
               else
               {
                  trace("fillItem");
                  itemData.name = itemData.name;
                  var new_value:int = item.isStack ? (itemData.value) : (item.value);
                  var itemUpdated:Boolean = false;
                  if(itemData.value != new_value)
                  {
                     itemData.value = new_value;
                     itemUpdated = true;
                  }
                  if(itemData.modifiers != item.modifiers)
                  {
                     itemData.modifiers = item.modifiers;
                     itemData.modifiersDescription = item.modifiersDescription;
                     itemUpdated = true;
                  }
                  if(itemData.rank != item.rank)
                  {
                     itemData.rank = item.rank;
                     itemUpdated = true;
                  }
                  if(itemData.broken != item.broken)
                  {
                     itemData.broken = item.broken;
                     itemUpdated = true;
                  }
                  if(itemUpdated)
                  {
                     this._playerPaperDoll.onItemUpdated(itemData.slotId);
                  }
                  if(item && !item.isStack)
                  {
                     item.equipped = true;
                  }
               }
            }
            trace("invalidateOnEquipped");
            equippedItems.invalidate();
         }
      }
      
      public function refillItem(value:uint) : void
      {
         trace("pp:refillItem");
         var currentItems:DataProvider = profileItems.dataProvider as DataProvider;
         var l:uint = currentItems.length;
         for(var i:uint = 0; i < l; i++)
         {
            var item:ItemData = currentItems[i];
            if(item && item.itemId == value)
            {
               var currentItemDescr:ItemData = this._playerInventory.getInventoryItem(value);
               var newItem:ItemData = currentItemDescr.clone();
               newItem.value = newItem.isStack ? item.value : currentItemDescr.value;
               newItem.slotId = item.slotId;
               currentItems[i] = newItem;
               currentItems.invalidate();
               this._playerPaperDoll.onItemUpdated(item.slotId);
               currentItemDescr.equipped = true;
               return;
            }
         }
      }
      
      public function setPlayerSpeed(value:uint) : void
      {
      }
      
      public function handleUnresolvedItems() : void
      {
         var l:uint = this._unresolvedItems.length;
         if(!l)
         {
            return;
         }
         for(var i:uint = 0; i < l; i++)
         {
            var profileItemData:Object = this._unresolvedItems[i];
            var currentItemDescr:ItemData = this._playerInventory.getInventoryItem(profileItemData.itemId);
            if(currentItemDescr)
            {
               var item:ItemData = this.createItem(profileItemData,currentItemDescr);
               this._convertedItems.push(item);
               profileItems.dataProvider = new DataProvider(this._convertedItems);
            }
         }
         for(var j:uint = 0; j < this._convertedItems.length; j++)
         {
            item = this._playerInventory.getInventoryItem(this._convertedItems[j].itemId);
            if(item && !item.isStack)
            {
               item.equipped = true;
            }
         }
      }
      
      override protected function handleUnresolvedItem(value:Object) : void
      {
         this._unresolvedItems.push(value);
      }
      
      override protected function getItemData(value:Object) : ItemData
      {
         return this._playerInventory.getInventoryItem(value.itemId);
      }
      
      override public function fillProfileItems(data:Object) : void
      {
         trace("fillProfileItems",data.items.length);
         var oldData:DataProvider = profileItems.dataProvider as DataProvider;
         if(oldData)
         {
            for(var j:uint = 0; j < oldData.length; j++)
            {
               var item:ItemData = this._playerInventory.getInventoryItem(oldData[j].itemId);
               if(item && !item.isStack)
               {
                  item.equipped = false;
               }
            }
         }
         this._unresolvedItems.length = 0;
         super.fillProfileItems(data);
         // The presets' levels follow the inventory: asked again.
         this.askPresets();
         trace("PP:invalidateItems");
         this._playerInventory.invalidateList();
      }
      
      override protected function createItem(profileItem:Object, inventoryData:ItemData) : ItemData
      {
         if(!inventoryData.isStack)
         {
            inventoryData.equipped = true;
         }
         var itemData:ItemData = super.createItem(profileItem,inventoryData);
         itemData.descriptor = itemData.slotId;
         return itemData;
      }
      
      public function swapQuickSlots(value_1:int, value_2:int) : void
      {
         this._ammo_share.swapElements(value_1,value_2);
      }
      
      protected function onLocal(value:String = "") : void
      {
         this.playerWeight.textFieldLabel.text = LocalizationData.getName("equipment_weight");
         this.ammoContainer.tx_total_label.text = LocalizationData.getName("equipment_cost");
         this.btnShowAmmo.label = LocalizationData.getName("show_ammo");
         this.showPresetTitle();
         this.checkbox_autobuy.tipData = {name:LocalizationData.getName("auto_button_hint")};
         this.checkbox_autorepair.tipData = {name:LocalizationData.getName("auto_repair_hint")};
      }
      
      override protected function init(e:Event) : void
      {
         super.init(e);
         this.btnShowAmmo.toggle = true;
         trace("PlayerProfileInit");
         LocalizationData.addCallback(this.onLocal,"equipment_weight");
         LocalizationData.addButton(this.ammoContainer.btn_cancel,"cancel");
         LocalizationData.addButton(this.ammoContainer.btn_apply,"refill_ammo_btn");
         LocalizationData.addButton(this.checkbox_autobuy,"autobuy_button_label");
         LocalizationData.addButton(this.checkbox_autorepair,"auto_repair_button_label");
         this.checkbox_autobuy.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.checkbox_autorepair.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.ammoContainer.labelCostP.moneyType = MoneyType.PREMIUM;
         this.checkbox_autorepair.visible = true;
         this.checkbox_autorepair.enabled = true;
         this.checkbox_autorepair.addEventListener(ButtonEvent.CLICK,this.onAutoRepair);
         this.checkbox_autobuy.addEventListener(ButtonEvent.CLICK,this.onAutoRepair);
         trace("PlayerProfileInit2",this._ammo_share,this.ammoContainer,this.ammoContainer.labelCostR,this.ammoContainer.labelCostP,this.btnShowAmmo);
         this._ammo_share.autoBuyCheckBox = this.checkbox_autobuy;
         this.ammoContainer.addChildAt(this._ammo_share as DisplayObject,4);
         this._ammo_share.x = 10;
         this._ammo_share.y = 10;
         this._ammo_share.acceptButton = this.ammoContainer.btn_apply;
         this._ammo_share.cancelButton = this.ammoContainer.btn_cancel;
         this.ammoContainer.labelCostR.moveIcon = false;
         this.ammoContainer.labelCostP.moveIcon = false;
         this._ammo_share.updateFunction = this.onTotalUpdate;
         this.btnShowAmmo.addEventListener(ButtonEvent.CLICK,this.onAmmoButton);
         this.ammoContainer.shift = -575;
         this.ammoContainer.addEventListener(Event.CLOSE,this.onAmmoButtonSwitch);
         this.makePresets();
      }
      
      private function onAutoRepair(e:Event) : void
      {
         trace("PP::updateAutoRepair",this.checkbox_autorepair.selected,this.getAutoBuyState());
         ExternalInterface.call("autobuy_changed",this.getAutoBuyState());
      }
      
      private function getAutoBuyState() : uint
      {
         if(this.checkbox_autorepair.selected == false && this.checkbox_autobuy.selected == false)
         {
            return AUTOBUY_NONE;
         }
         if(this.checkbox_autorepair.selected == true && this.checkbox_autobuy.selected == true)
         {
            return AUTOBUY_BOTH;
         }
         if(this.checkbox_autorepair.selected == true)
         {
            return AUTOBUY_REPAIR;
         }
         return AUTOBUY_AMMO;
      }
      
      public function setAutoRepair(value:Boolean) : void
      {
         this.checkbox_autorepair.selected = value;
      }
      
      private function onAmmoButtonSwitch(e:Event) : void
      {
         if(this.btnShowAmmo.selected == true)
         {
            this.btnShowAmmo.selected = false;
            this.onAmmoButton();
         }
      }
      
      private function onAmmoButton(e:Event = null) : void
      {
         trace("onAmmoButton",this.btnShowAmmo.selected,this.ammoContainer);
         if(this.btnShowAmmo.selected)
         {
            ExternalInterface.call("sound_play",UiSound.MENU_SLIDE_OUT);
            this.btnShowAmmo.label = LocalizationData.getName("hide_ammo");
            if(this._tween)
            {
               this._tween.reset();
               this._tween.paused = true;
            }
            stage.focus = this.ammoContainer;
            trace("ammo_container_showed");
            ExternalInterface.call("ammo_container_showed");
            _tween = new Tween(250,this.ammoContainer,{shift:0},{ease:Strong.easeOut});
         }
         else
         {
            ExternalInterface.call("sound_play",UiSound.MENU_SLIDE_IN);
            this.btnShowAmmo.label = LocalizationData.getName("show_ammo");
            if(this._tween)
            {
               this._tween.reset();
               this._tween.paused = true;
            }
            stage.focus = this;
            if(this.ammoContainer.btn_apply.enabled)
            {
               this._ammo_share.setAmmo();
            }
            _tween = new Tween(250,this.ammoContainer,{shift:-575},{ease:Strong.easeOut});
         }
      }
      
      private function onTotalUpdate(eRegular:Number, ePremium:Number) : void
      {
         trace("onTotalUpdate",eRegular,ePremium);
         this.ammoContainer.labelCostR.cost = eRegular;
         this.ammoContainer.labelCostP.visible = ePremium != 0;
         this.ammoContainer.plusSprite.visible = this.ammoContainer.labelCostP.visible;
         this.ammoContainer.labelCostP.cost = ePremium;
      }
      
      private function changeIndexByFilter(e:IndexEvent) : void
      {
         if(e.index == 0)
         {
            this._playerPaperDoll.ammoControl.reflow(null,"playerProfile",AmmoView.ALL);
         }
         else
         {
            this._playerPaperDoll.ammoControl.reflow(null,"playerProfile",-1);
         }
      }
      
      // PlayerRanking.fillPlayers, first: rows of the presets' range are
      // the list (true: taken).
      public static function presetsTake(rows:Array) : Boolean
      {
         if(rows == null || rows.length == 0 || rows[0] == null)
         {
            return false;
         }
         var place:Number = Number(rows[0].place);
         if(place < PRESETS_FIRST || place > PRESETS_LAST)
         {
            return false;
         }
         if(me != null)
         {
            try
            {
               me.takePresets(rows);
            }
            catch(e:Error)
            {
            }
         }
         return true;
      }
      
      // LobbyMenu.has_active_window / close_active_window: Esc closes the
      // presets' list first (true: it was open).
      public static function presetListOpen() : Boolean
      {
         return me != null && me._presetList != null;
      }
      
      public static function closePresetListByEsc() : Boolean
      {
         if(!presetListOpen())
         {
            return false;
         }
         me.closePresetList();
         return true;
      }
      
      private static function presetWord(key:String) : String
      {
         return String(PRESET_WORDS[key][RoomsText.language()]);
      }
      
      // The title bar's press area over the panel, the title's text (which
      // takes no press) above it, and the caret.
      private function makePresets() : void
      {
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            this._presets.push({
               "known":false,
               "saved":false,
               "level":-1
            });
            i++;
         }
         this._titleX = this.labelText.x;
         this._titleCentre = this.labelText.x + this.labelText.width / 2;
         this.labelText.mouseEnabled = false;
         this._presetTitle = new Sprite();
         this._presetTitle.x = TITLE_X;
         this._presetTitle.y = TITLE_Y;
         // Alpha 0 is not hit-tested in Scaleform: 0.01.
         this._presetTitle.graphics.beginFill(0,0.01);
         this._presetTitle.graphics.drawRect(0,0,TITLE_W,TITLE_H);
         this._presetTitle.graphics.endFill();
         this._presetTitle.mouseChildren = false;
         this._presetGlow = new Shape();
         this._presetTitle.addChild(this._presetGlow);
         this._presetCaret = new Shape();
         this._presetTitle.addChild(this._presetCaret);
         // On top of the panel, the text above it.
         addChild(this._presetTitle);
         setChildIndex(this.labelText,numChildren - 1);
         this._presetTitle.addEventListener(MouseEvent.ROLL_OVER,this.onPresetOver);
         this._presetTitle.addEventListener(MouseEvent.ROLL_OUT,this.onPresetOut);
         // On the panel, not the bar: a press Scaleform gives the panel
         // itself (scaleform-movie-pitfalls) is found by its point.
         addEventListener(MouseEvent.MOUSE_DOWN,this.onPanelPress);
         addEventListener(Event.REMOVED_FROM_STAGE,this.onPresetsGone);
         this.showPresetTitle();
      }
      
      private function presetLabel(i:int) : String
      {
         var preset:Object = this._presets[i];
         var label:String = presetWord("preset") + " " + (i + 1);
         if(!preset.known)
         {
            return label;
         }
         if(!preset.saved)
         {
            return label + " · " + presetWord("empty");
         }
         if(preset.level < 0)
         {
            return label + " · " + presetWord("unfit");
         }
         var level:String = LocalizationData.getName("clan_players_level_label");
         if(level == null || level == "" || level == "clan_players_level_label")
         {
            level = presetWord("level");
         }
         return label + " · " + level + " " + preset.level;
      }
      
      // The preset the title names: the one chosen while unanswered.
      private function presetShown() : int
      {
         return this._presetPending >= 0 ? this._presetPending : this._presetAt;
      }
      
      // The title: the panel's name until the server's first list, then the
      // active preset with a caret, the two centred in the bar together.
      private function showPresetTitle() : void
      {
         if(this._presetTitle == null)
         {
            this.labelText.text = LocalizationData.getName("paperdoll_label");
            return;
         }
         var ready:Boolean = this._presetsSeen;
         this.labelText.text = ready ? this.presetLabel(this.presetShown()) : LocalizationData.getName("paperdoll_label");
         var shift:Number = ready ? (CARET_GAP + CARET_W) / 2 : 0;
         this.labelText.x = this._titleX - shift;
         this._presetCaret.visible = ready;
         this._presetCaret.x = this._titleCentre - shift + this.labelText.textWidth / 2 + CARET_GAP - TITLE_X;
         this._presetCaret.y = TITLE_H / 2 - CARET_H / 2 + 1;
         this._presetTitle.buttonMode = ready && !this._presetsLocked;
         this.drawPresetTitle();
      }
      
      // The lobby's list rows' look (OptionsListRenderer): over, white at
      // 1/32; open, as a selected row, with its 1 px outline at 10 %.
      private function drawPresetTitle() : void
      {
         if(this._presetTitle == null)
         {
            return;
         }
         var open:Boolean = this._presetList != null;
         var live:Boolean = this._presetsSeen && !this._presetsLocked;
         var g:Graphics = this._presetGlow.graphics;
         g.clear();
         if(live && (open || this._presetHover))
         {
            g.beginFill(16777215,0.03125);
            g.drawRect(0,0,TITLE_W,TITLE_H);
            g.endFill();
         }
         if(live && open)
         {
            g.beginFill(16777215,0.1);
            g.drawRect(0,0,TITLE_W,1);
            g.drawRect(0,TITLE_H - 1,TITLE_W,1);
            g.drawRect(0,1,1,TITLE_H - 2);
            g.drawRect(TITLE_W - 1,1,1,TITLE_H - 2);
            g.endFill();
         }
         // The caret: down, up while the list is open; the title's colour.
         var c:Graphics = this._presetCaret.graphics;
         c.clear();
         c.beginFill(this.labelText.textColor,live ? 1 : 0.35);
         if(open)
         {
            c.moveTo(0,CARET_H);
            c.lineTo(CARET_W,CARET_H);
            c.lineTo(CARET_W / 2,0);
            c.lineTo(0,CARET_H);
         }
         else
         {
            c.moveTo(0,0);
            c.lineTo(CARET_W,0);
            c.lineTo(CARET_W / 2,CARET_H);
            c.lineTo(0,0);
         }
         c.endFill();
      }
      
      private function onPresetOver(e:MouseEvent) : void
      {
         this._presetHover = true;
         this.drawPresetTitle();
      }
      
      private function onPresetOut(e:MouseEvent) : void
      {
         this._presetHover = false;
         this.drawPresetTitle();
      }
      
      // A press on the title opens or closes the list. A press on a row the
      // list missed (given to the panel) chooses that row.
      private function onPanelPress(e:MouseEvent) : void
      {
         if(this._presetTitle == null || e.target != this && e.target != this._presetTitle)
         {
            return;
         }
         if(this._presetTitle.hitTestPoint(e.stageX,e.stageY,false))
         {
            if(this._presetList != null)
            {
               this.closePresetList();
            }
            else
            {
               this.openPresetList();
            }
            return;
         }
         if(e.target != this || this._presetList == null)
         {
            return;
         }
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            var row:DisplayObject = this._presetList.getRendererAt(i) as DisplayObject;
            if(row != null && row.visible && row.hitTestPoint(e.stageX,e.stageY,false))
            {
               this.choosePreset(i);
               return;
            }
            i++;
         }
      }
      
      // The lobby's list (the game's dropdowns' list and rows) under the
      // title, its width, five rows, the active one selected.
      private function openPresetList() : void
      {
         if(this._presetList != null || !this._presetsSeen || this._presetsLocked || stage == null)
         {
            return;
         }
         var listClass:Class = getDefinitionByName("OptionScrollingList") as Class;
         var list:ScrollingList = new listClass() as ScrollingList;
         list.itemRenderer = getDefinitionByName("OptionsListRenderer") as Class;
         list.width = TITLE_W;
         list.padding = new Padding(0,0,0,0);
         list.margin = 0;
         list.wrapping = "normal";
         list.thumbOffset = {
            "top":0,
            "bottom":0
         };
         list.rowCount = PRESET_COUNT;
         list.dataProvider = new DataProvider(this.presetRows());
         list.selectedIndex = this.presetShown();
         list.x = TITLE_X;
         list.y = TITLE_Y + TITLE_H;
         list.addEventListener(ListEvent.ITEM_CLICK,this.onPresetRow);
         this._presetList = list;
         addChild(list);
         stage.addEventListener(MouseEvent.MOUSE_DOWN,this.onStagePress);
         addEventListener(Event.ENTER_FRAME,this.watchPresetList);
         this.drawPresetTitle();
      }
      
      private function presetRows() : Array
      {
         var rows:Array = [];
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            rows.push({"label":this.presetLabel(i)});
            i++;
         }
         return rows;
      }
      
      private function closePresetList() : void
      {
         if(this._presetList == null)
         {
            return;
         }
         var list:ScrollingList = this._presetList;
         this._presetList = null;
         list.removeEventListener(ListEvent.ITEM_CLICK,this.onPresetRow);
         if(list.parent != null)
         {
            list.parent.removeChild(list);
         }
         if(stage != null)
         {
            stage.removeEventListener(MouseEvent.MOUSE_DOWN,this.onStagePress);
         }
         removeEventListener(Event.ENTER_FRAME,this.watchPresetList);
         this.drawPresetTitle();
      }
      
      // A press outside the title and the list closes it, no choice.
      private function onStagePress(e:MouseEvent) : void
      {
         if(this._presetList == null)
         {
            return;
         }
         if(this._presetList.hitTestPoint(e.stageX,e.stageY,false) || this._presetTitle.hitTestPoint(e.stageX,e.stageY,false))
         {
            return;
         }
         this.closePresetList();
      }
      
      // The panel hidden (closed, another window) closes the list.
      private function watchPresetList(e:Event) : void
      {
         var at:DisplayObject = this;
         while(at != null)
         {
            if(!at.visible)
            {
               this.closePresetList();
               return;
            }
            at = at.parent;
         }
         if(stage == null)
         {
            this.closePresetList();
         }
      }
      
      private function onPresetsGone(e:Event) : void
      {
         this._presetHover = false;
         this.closePresetList();
      }
      
      private function onPresetRow(e:ListEvent) : void
      {
         e.stopPropagation();
         this.choosePreset(e.index);
      }
      
      // A preset picked: it becomes the active one (its kits put on). The
      // title names it at once; the server's answer settles it.
      private function choosePreset(at:int) : void
      {
         this.closePresetList();
         if(this._presetsLocked || at < 0 || at >= PRESET_COUNT || at == this.presetShown())
         {
            return;
         }
         this._presetPending = at;
         this._presetPendingAt = getTimer();
         this.showPresetTitle();
         this.sendPresets(PRESET_CHOOSE + at + 1);
      }
      
      private function lockPresets(locked:Boolean) : void
      {
         this._presetsLocked = locked;
         if(locked)
         {
            this.closePresetList();
         }
         if(this._presetTitle != null)
         {
            this.showPresetTitle();
         }
      }
      
      // The list now, at most every 2 s (an account pushed asks for it).
      private function askPresets() : void
      {
         if(getTimer() - this._presetsAskedAt < 2000)
         {
            return;
         }
         this._presetsAskedAt = getTimer();
         this.sendPresets(PRESETS_FIRST);
      }
      
      private function sendPresets(start:int) : void
      {
         try
         {
            ExternalInterface.call("request_players_range",start,1);
         }
         catch(e:Error)
         {
         }
      }
      
      // `P1;L;<done>;<preset>;<empty>;<active>`, then `<preset>;<saved>;<level>`
      // each.
      private function takePresets(rows:Array) : void
      {
         var head:Array = String(rows[0].nickname).split(";");
         if(head[0] != "P1" || head[1] != "L")
         {
            return;
         }
         var active:int = int(head[5]) - 1;
         if(active >= 0 && active < PRESET_COUNT)
         {
            this._presetAt = active;
         }
         var i:int = 1;
         while(i < rows.length && i <= PRESET_COUNT)
         {
            var f:Array = String(rows[i].nickname).split(";");
            var at:int = int(f[0]) - 1;
            if(at >= 0 && at < PRESET_COUNT && f.length >= 3)
            {
               this._presets[at] = {
                  "known":true,
                  "saved":f[1] == "1",
                  "level":int(f[2])
               };
            }
            i++;
         }
         this._presetsSeen = true;
         var done:int = int(head[2]);
         // A choice's answer (or one never coming) ends the choice shown.
         if(done == PRESET_CHOSEN || done == PRESET_REFUSED || getTimer() - this._presetPendingAt > PRESET_PENDING_MS)
         {
            this._presetPending = -1;
         }
         this.showPresetTitle();
         if(this._presetList != null)
         {
            this._presetList.dataProvider = new DataProvider(this.presetRows());
            this._presetList.selectedIndex = this.presetShown();
         }
         var empty:int = int(head[4]);
         if(done == PRESET_CHOSEN && empty > 0)
         {
            Message.showMessage("ok",presetWord("gone").split("{n}").join(String(empty)));
         }
         else if(done == PRESET_REFUSED)
         {
            Message.showMessage("ok",presetWord("refused"));
         }
      }
   }
}
