package survarium.player
{
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.*;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.motion.Tween;
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
      
      // Kit presets (presets.md, the owner's of 2026-10-23): five snapshots of
      // the three kits, kept by the server, chosen in a dropdown left of the
      // title and saved or applied with the two buttons right of it. srp's
      // wire: the ranking request at PRESETS_FIRST and on (lobby.md 2.16.4),
      // answered with the list, `P1;L;<done>;<preset>;<empty>` and a row
      // `<preset>;<saved>;<level>` each (level -1: none of its kits can play).
      public static var me:LocalPlayerProfile;
      
      private static const PRESETS_FIRST:int = 2090000000;
      
      private static const PRESETS_LAST:int = PRESETS_FIRST + 99;
      
      private static const PRESET_SAVE:int = PRESETS_FIRST + 10;
      
      private static const PRESET_APPLY:int = PRESETS_FIRST + 20;
      
      private static const PRESET_COUNT:int = 5;
      
      // What the list's header says was done: applied, refused.
      private static const PRESET_APPLIED:int = 2;
      
      private static const PRESET_REFUSED:int = 3;
      
      // The words, in the lobby's five languages (RoomsText.language).
      private static const PRESET_WORDS:Object = {
         "preset":["Preset","Пресет","Пресет","Preset","Zestaw"],
         "level":["level","рівень","уровень","Stufe","poziom"],
         "empty":["empty","порожній","пустой","leer","pusty"],
         "unfit":["can't play","не для бою","не для боя","nicht spielbar","niegrywalny"],
         "save":["Save","Зберегти","Сохранить","Speichern","Zapisz"],
         "apply":["Apply","Застосувати","Применить","Anwenden","Zastosuj"],
         "gone":["{n} slots left empty: those items are gone.","Порожніх слотів: {n}, цих предметів немає.","Пустых слотов: {n}, этих предметов нет.","{n} Plätze leer: diese Gegenstände fehlen.","Pustych miejsc: {n}, tych przedmiotów brak."],
         "refused":["The preset could not be applied.","Не вдалося застосувати пресет.","Не удалось применить пресет.","Das Preset ließ sich nicht anwenden.","Nie udało się zastosować zestawu."]
      };
      
      private var _presetMenu:DropdownMenu;
      
      private var _presetSave:*;
      
      private var _presetApply:*;
      
      // {known, saved, level} a preset, preset 1 first; the one chosen.
      private var _presets:Array = [];
      
      private var _presetAt:int = 0;
      
      private var _presetLabels:String = "";
      
      private var _presetsAskedAt:int = -100000;
      
      // The kits locked (as the paper doll and the ammo button): no preset
      // saved or applied.
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
         this.labelText.text = LocalizationData.getName("paperdoll_label");
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
      
      private static function presetWord(key:String) : String
      {
         return String(PRESET_WORDS[key][RoomsText.language()]);
      }
      
      // The dropdown (the game's, as Player vs Bots' menus) left of the title,
      // Save and Apply right of it: the title is centred in the header.
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
         var menuClass:Class = getDefinitionByName("SrvDropDownMenuP") as Class;
         this._presetMenu = new menuClass() as DropdownMenu;
         this._presetMenu.dropdown = "OptionScrollingList";
         this._presetMenu.itemRenderer = "OptionsListRenderer";
         this._presetMenu.scrollBar = "";
         this._presetMenu.menuDirection = "down";
         this._presetMenu.menuMargin = 0;
         this._presetMenu.thumbOffsetTop = 0;
         this._presetMenu.thumbOffsetBottom = 0;
         this._presetMenu.menuRowsFixed = false;
         this._presetMenu.menuRowCount = PRESET_COUNT;
         this._presetMenu.x = 10;
         this._presetMenu.y = 4;
         this._presetMenu.width = 200;
         this._presetMenu.menuWidth = 220;
         this._presetMenu.addEventListener(ListEvent.INDEX_CHANGE,this.onPresetChosen);
         addChild(this._presetMenu);
         var buttonClass:Class = getDefinitionByName("SrvButtonSkinned") as Class;
         this._presetSave = new buttonClass();
         this._presetApply = new buttonClass();
         this.placePresetButton(this._presetSave,presetWord("save"),376,this.onPresetSave);
         this.placePresetButton(this._presetApply,presetWord("apply"),476,this.onPresetApply);
         this.fillPresetMenu();
      }
      
      private function placePresetButton(button:*, label:String, x:Number, handler:Function) : void
      {
         button.label = label;
         button.x = x;
         button.y = 2;
         addChild(button);
         // Its natural height (32): the game's button keeps its label there.
         button.setSize(92,32);
         button.addEventListener(ButtonEvent.CLICK,handler);
      }
      
      private function fillPresetMenu() : void
      {
         var rows:Array = [];
         var labels:String = "";
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            var preset:Object = this._presets[i];
            var label:String = presetWord("preset") + " " + (i + 1);
            if(preset.known)
            {
               label += ": " + (!preset.saved ? presetWord("empty") : (preset.level >= 0 ? presetWord("level") + " " + preset.level : presetWord("unfit")));
            }
            rows.push({"label":label});
            labels += label + "|";
            i++;
         }
         if(labels != this._presetLabels)
         {
            this._presetLabels = labels;
            this._presetMenu.dataProvider = new DataProvider(rows);
         }
         this._presetMenu.selectedIndex = this._presetAt;
         this._presetMenu.invalidateSelectedIndex();
         this.lockPresets(this._presetsLocked);
      }
      
      private function lockPresets(locked:Boolean) : void
      {
         this._presetsLocked = locked;
         if(this._presetMenu == null)
         {
            return;
         }
         this._presetMenu.enabled = !locked;
         this._presetSave.enabled = !locked;
         this._presetApply.enabled = !locked && Boolean(this._presets[this._presetAt].saved);
      }
      
      // The list now, at most every 2 s unless `now` (an account pushed is
      // asked for once).
      private function askPresets(now:Boolean = false) : void
      {
         if(!now && getTimer() - this._presetsAskedAt < 2000)
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
      
      // `P1;L;<done>;<preset>;<empty>`, then `<preset>;<saved>;<level>` each.
      private function takePresets(rows:Array) : void
      {
         var head:Array = String(rows[0].nickname).split(";");
         if(head[0] != "P1" || head[1] != "L")
         {
            return;
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
         this.fillPresetMenu();
         var done:int = int(head[2]);
         var empty:int = int(head[4]);
         if(done == PRESET_APPLIED && empty > 0)
         {
            Message.showMessage("ok",presetWord("gone").split("{n}").join(String(empty)));
         }
         else if(done == PRESET_REFUSED)
         {
            Message.showMessage("ok",presetWord("refused"));
         }
      }
      
      private function onPresetChosen(e:ListEvent) : void
      {
         e.stopPropagation();
         var at:int = this._presetMenu.selectedIndex;
         if(at >= 0 && at < PRESET_COUNT && at != this._presetAt)
         {
            this._presetAt = at;
            this.lockPresets(this._presetsLocked);
         }
      }
      
      private function onPresetSave(e:Event) : void
      {
         if(this._presetsLocked)
         {
            return;
         }
         this.sendPresets(PRESET_SAVE + this._presetAt + 1);
      }
      
      private function onPresetApply(e:Event) : void
      {
         if(!this._presetsLocked && this._presets[this._presetAt].saved)
         {
            this.sendPresets(PRESET_APPLY + this._presetAt + 1);
         }
      }
   }
}
