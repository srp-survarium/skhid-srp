package survarium.player
{
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.*;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.getDefinitionByName;
   import flash.utils.Timer;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.motion.Tween;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.Transient;
   import survarium.controls.UnfocusedCheckBox;
   import survarium.controls.PlayerProfileBase;
   import survarium.controls.PlayerInfo;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IAmmoShare;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.IItemFactory;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   import survarium.constants.AmmoView;
   import survarium.constants.ProfileSlot;
   import survarium.item.ModificationRolls;
   import survarium.constants.MoneyType;
   import survarium.constants.SuperCategory;
   import survarium.constants.UiSound;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsWindow;
   
   public class LocalPlayerProfile extends PlayerProfileBase implements ILocalPlayerProfile
   {
      
      public var labelText:TextField;
      
      public var btnShowAmmo:Button;
      
      public var playerWeight:MovieClip;
      
      public var glassSprite:MovieClip;
      
      public var checkbox_autobuy:Button;
      
      // The auto-repair box: wear is gone, so it did nothing (the owner's
      // of 2026-10-23). The original movie's graphics place it beside
      // Autobuy; init hides it and moves Autobuy into its place.
      public var checkbox_autorepair:Button;
      
      public var ammoContainer:MovieClip;
      
      public var buyPremium:MovieClip;
      
      private var _weightIndicator:WeightIndicator;

      private var _weightC:Number = 0;
      
      // autobuy_changed's state: whether the autobuy (ammo) box is ticked
      // (bit 1 was the auto-repair box, gone).
      private static const AUTOBUY_NONE:uint = 0;
      
      private static const AUTOBUY_AMMO:uint = 1;
      
      private var _weightM:Number = 0;
      
      // The exe's weight effects, before the kit's items are added to the speed.
      private var _weightEffects:WeightEffects;
      
      private var _relocateFunctionRef:Function;
      
      private var _ammo_share:IAmmoShare;
      
      private var _tween:Tween;
      
      private var _playerPaperDoll:ILocalPaperDoll;
      
      private var _playerInventory:IInventoryItemFactory;
      
      protected var _unresolvedItems:Array = new Array();
      
      protected var _convertedItems:Array;
      // Kit presets (presets.md, the owner's of 2026-10-23): five sets of the
      // three kits, one active. The panel's title is the switcher: it names
      // the active preset ("Loadout 2 · Lvl. 15": players see "Loadout"),
      // and a press on it opens the
      // five under it; a press on one chooses it. The active one is the kits:
      // choosing another puts it on, and every change of the kits is the
      // active one's (the server's). srp's wire: the ranking request at
      // PRESETS_FIRST and on (lobby.md 2.16.4), answered with the list,
      // `P2;L;<done>;<preset>;<active>;<blocks>;<minutes>`, a row `<preset>;<saved>;<level>`
      // each (level -1: none of its kits can play), then the items the
      // presets use, `U;<item id>:<presets>,...` (bit n - 1: preset n), which
      // a sale asks about (MessageAskSell).
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
      
      private static const CARET_W:Number = 12;
      
      private static const CARET_H:Number = 7;
      
      private static const CARET_GAP:Number = 10;
      
      // The title's text (the owner's of 2026-10-23: bigger): 24 px, the
      // panel's stock title 22; still one line in the bar. Its field is
      // the bar's width less the caret's room each side, so the longest
      // title of the supported languages ("Loadout 3 · nicht spielbar") fits,
      // and set 1 px higher so the larger text stays centred in the bar.
      private static const TITLE_SIZE:int = 24;
      
      private static const TITLE_TEXT_W:Number = TITLE_W - 2 * (CARET_GAP + CARET_W + 12);
      
      private static const TITLE_TEXT_Y:Number = 6;
      
      private static const TITLE_TEXT_H:Number = 34;
      
      // The open list under the title: five rows of ROW_H, their text
      // ROW_SIZE px (the stock option rows': 24 and 14), in the look of the
      // lobby's lists: the game's dropdown backdrop (near black, a faint
      // frame), a line between rows, the active row as the rooms list's
      // selected room (gold-tinted, a gold bar at its left), the name left
      // and the level or state right. Hover and press lighten a row, and the
      // title, alike.
      private static const ROW_H:Number = 36;
      
      private static const ROW_SIZE:int = 19;
      
      private static const ROW_PAD:Number = 16;
      
      private static const HOVER_ALPHA:Number = 0.06;
      
      private static const PRESS_ALPHA:Number = 0.11;
      
      // A choice shown before the server's answer, at most this long (ms).
      private static const PRESET_PENDING_MS:int = 5000;
      
      // The title bar's press area and its hover / open look (below the
      // title's text), the caret, and the open list (null: closed) with its
      // rows, the row under the pointer and the one pressed (-1: none).
      private var _presetTitle:Sprite;
      
      private var _presetGlow:Shape;
      
      private var _presetCaret:Shape;
      
      private var _presetList:Sprite;
      
      private var _presetRows:Array = [];
      
      private var _presetRowOver:int = -1;
      
      private var _presetRowDown:int = -1;
      
      private var _presetHover:Boolean = false;
      
      // The title's font (the timeline's), for the rows.
      private var _presetFont:String = null;
      
      private var _presetBold:Object = null;
      
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
      
      // Item id -> the presets that use it (bit n - 1: preset n), as the
      // server last said.
      private var _presetUses:Object = {};
      
      // The U rows the uses came from, to tell a change.
      private var _presetUsesRows:String = "";
      
      // Asks again every 2 s until the first list comes: the first asks go
      // out while the movie loads, before the game takes them
      // ("ExternalInterface.call - handler is not installed"), and are lost;
      // the title is the panel's name until a list comes.
      private var _presetsRetry:Timer;
      
      // The ask waiting for the end of the 2 s window (null: none).
      private var _presetsLater:Timer;
      
      // Why no room may be entered now (the list's `<blocks>`, the server's
      // rule for the rooms' entries; presets.md's table of the reasons):
      // Play and Join greyed by it, its reasons in their tip. 0 (none) until
      // a list says. The minutes to a planned maintenance (-1: none), and
      // the list asked again every minute while one is planned.
      private var _blocks:int = 0;
      
      // Whether the reasons are known: a list came, or none in
      // REASONS_FALLBACK_MS (then the server's doors alone decide). Until
      // then Play and the rooms' entries are greyed, with no tip (the
      // owner's of 2026-10-23: a quick click at the start went through).
      private static const REASONS_FALLBACK_MS:int = 10000;
      
      private var _reasonsKnown:Boolean = false;
      
      private var _reasonsFallback:Timer;
      
      private var _maintenanceMinutes:int = -1;
      
      private var _maintenanceWatch:Timer;
      
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
      
      public function updateWeight(cur:Number, max:Number, effects:WeightEffects = null) : void
      {
         _weightC = cur;
         _weightM = max;
         _weightEffects = effects;
         if(_weightIndicator == null) _weightIndicator = new WeightIndicator(playerWeight);
         _weightIndicator.update(cur,max,withKitItems(effects));
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
         // A bonus rerolled or repaired changes the speed without a weight callback.
         if(_weightIndicator != null) _weightIndicator.update(_weightC,_weightM,withKitItems(_weightEffects));
      }
      
      // What each of the kit's items adds (KitItems): the armour's and the pockets'
      // movement (prop 105) and carry limit (prop 106) from their modifications
      // (built-in, rolled, defects), one entry an item by its displayed name, and the
      // first weapon's own movement parameter (the weapon in hand).
      private function withKitItems(effects:WeightEffects) : WeightEffects
      {
         if(effects == null) return effects;
         var items:DataProvider = profileItems.dataProvider as DataProvider;
         var kit:KitItems = new KitItems();
         for(var i:uint = 0; items != null && i < items.length; i++)
         {
            var item:ItemData = items[i] as ItemData;
            if(item == null) continue;
            var armour:Boolean = item.slotId >= ProfileSlot.HEAD && item.slotId <= ProfileSlot.FOOT;
            var pocket:Boolean = item.slotId >= ProfileSlot.QUICK_1 && item.slotId <= ProfileSlot.QUICK_6;
            if(armour || pocket)
            {
               // A pocket's booster has the effect's own icon (the shop's consumables
               // filter icon is a thin stroke that suits none of them); armour its group's.
               var group:Object = armour ? item.category : null;
               if(propOf(item,105) != 0) kit.speed.push({name:item.name,value:propOf(item,105),category:group});
               if(propOf(item,106) != 0) kit.limit.push({name:item.name,value:propOf(item,106),category:group});
               if(propOf(item,116) != 0) kit.stamina.push({name:item.name,value:propOf(item,116),category:group});
            }
            else if(item.slotId == ProfileSlot.WEAPON_1 && propOf(item,105) != 0)
               kit.weapon = {name:item.name,value:propOf(item,105),category:item.category,killIcon:killIconOf(item)};
         }
         return effects.withItems(kit);
      }
      
      // The sum of a property over the item's modifications (built-in, rolled, defects):
      // 105 movement speed, 106 carry limit (kg).
      // A weapon's kill feed icon (ui_desc/combat_log_icon, property 161: weapon-kill-icon-ui-1), or -1.
      private static function killIconOf(item:ItemData) : int
      {
         for each(var prop:Object in item.properties)
            if(prop.prop_id == 161 && isFinite(ModificationRolls.value(prop))) return int(ModificationRolls.value(prop));
         return -1;
      }
      
      private static function propOf(item:ItemData, propId:uint) : Number
      {
         var sum:Number = 0;
         for each(var modifier:Object in item.modifiers)
            for each(var prop:Object in modifier.props)
               if(prop.prop_id == propId && isFinite(ModificationRolls.value(prop))) sum += ModificationRolls.value(prop);
         return sum;
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
         if(_weightIndicator != null) _weightIndicator.refresh();
         this.ammoContainer.tx_total_label.text = LocalizationData.getName("equipment_cost");
         this.btnShowAmmo.label = LocalizationData.getName("show_ammo");
         this.showPresetTitle();
         this.checkbox_autobuy.tipData = {name:LocalizationData.getName("auto_button_hint")};
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
         this.checkbox_autobuy.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.ammoContainer.labelCostP.moneyType = MoneyType.PREMIUM;
         // The auto-repair box is gone (see its declaration): Autobuy takes
         // its place at the row's start, no gap left.
         this.checkbox_autorepair.visible = false;
         this.checkbox_autobuy.x = this.checkbox_autorepair.x;
         // Its label as long as its text; the row has room up to the speed and weight.
         (this.checkbox_autobuy as UnfocusedCheckBox).fitLabel();
         this.checkbox_autobuy.addEventListener(ButtonEvent.CLICK,this.onAutoBuy);
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
      
      private function onAutoBuy(e:Event) : void
      {
         ExternalInterface.call("autobuy_changed",this.checkbox_autobuy.selected ? AUTOBUY_AMMO : AUTOBUY_NONE);
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
      
      // The reasons (presets.md's table): bits of the list's `<blocks>`,
      // each with its text (the maintenance's: %1$s its minutes).
      private static const BLOCK_MAINTENANCE:int = 32;
      
      private static const REASONS:Array = [
         {"bit":1,"key":"st_srp_loadout_block_locked"},
         {"bit":2,"key":"st_srp_loadout_block_shared_slot"},
         {"bit":4,"key":"st_srp_loadout_block_pocket_twice"},
         {"bit":8,"key":"st_srp_loadout_block_ammo"},
         {"bit":16,"key":"st_srp_loadout_block_weapon"},
         {"bit":BLOCK_MAINTENANCE,"key":"st_srp_loadout_block_maintenance"}
      ];
      
      // A squad member that is not its leader (the owner's of 2026-10-31,
      // squads.md): its Play greyed all the time, and it goes into no room on
      // its own (Join, Queue, a friend's room, a PIN, a new room); its
      // leader's take it. The server refuses them too (code 17).
      public static function squadSoldier() : Boolean
      {
         return PlayerInfo.isInSquad && !PlayerInfo.commander;
      }
      
      // StatusPanel.showRoomsPlay: Play greyed while any reason holds, or
      // while the reasons are not known yet.
      public static function mayPlay() : Boolean
      {
         return !squadSoldier() && (me == null || me._reasonsKnown && me._blocks == 0);
      }
      
      // Making a room: greyed under the maintenance rule only (a kit that
      // can not play is warned in the room), and until the reasons are known.
      public static function mayCreate() : Boolean
      {
         return !squadSoldier() && (me == null || me._reasonsKnown && (me._blocks & BLOCK_MAINTENANCE) == 0);
      }
      
      // Every reason that holds, a line each (the greyed buttons' tip).
      public static function reasons() : String
      {
         var lines:Array = [];
         if(squadSoldier())
         {
            lines.push(noStop(LocalizationData.getName("st_srp_rooms_code_17")));
         }
         if(me == null)
         {
            return lines.join("\n");
         }
         for each(var reason:Object in REASONS)
         {
            if((me._blocks & int(reason.bit)) == 0)
            {
               continue;
            }
            lines.push(noStop(LocalizationData.format(String(reason.key),Math.max(0,me._maintenanceMinutes))));
         }
         return lines.join("\n");
      }
      
      // A line each, no full stop at its end (the owner's of 2026-10-23).
      private static function noStop(text:String) : String
      {
         while(text.length > 0 && (text.charAt(text.length - 1) == "." || text.charAt(text.length - 1) == " "))
         {
            text = text.substr(0,text.length - 1);
         }
         return text;
      }
      
      // A tip over a greyed button (a disabled one takes no hover): the
      // game's tooltip (as Autobuy's), nested a Sprite deeper (a control
      // straight in a content Sprite may lose its mouse events to it). The
      // caller removes it with the holder.
      public static function reasonTip(parent:DisplayObjectContainer, x:Number, y:Number, w:Number, h:Number) : Sprite
      {
         var holder:Sprite = new Sprite();
         holder.x = x;
         holder.y = y;
         // Not known yet (or none): greyed, no tip.
         if(reasons() == "")
         {
            parent.addChild(holder);
            return holder;
         }
         var tip:ReasonsTip = new ReasonsTip();
         tip.graphics.beginFill(0,0.01);
         tip.graphics.drawRect(0,0,w,h);
         tip.graphics.endFill();
         holder.addChild(tip);
         parent.addChild(holder);
         tip.validateNow();
         tip.tooltipRendererName = "SrvProfileTipRendererStyled";
         tip.tipData = {"name":reasons()};
         return holder;
      }
      
      // The rooms' Join (a room of `maxLevel`; 0: any, -1: not known): no
      // reason holds, or only the kit's and the join popup can offer another
      // loadout that can play (the server's `presets::offer`): greyed
      // otherwise.
      public static function mayEnter(maxLevel:int) : Boolean
      {
         if(squadSoldier())
         {
            return false;
         }
         if(mayPlay())
         {
            return true;
         }
         if(!me._reasonsKnown || (me._blocks & BLOCK_MAINTENANCE) != 0)
         {
            return false;
         }
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            var preset:Object = me._presets[i];
            if(i != me._presetAt && preset.known && preset.level >= 0 && (maxLevel <= 0 || preset.level <= maxLevel))
            {
               return true;
            }
            i++;
         }
         return false;
      }
      
      // MessageAskSell: the question a sale of `itemId` asks first, "" when
      // no preset uses it (the owner's of 2026-10-23).
      public static function sellWarning(itemId:uint) : String
      {
         if(me == null)
         {
            return "";
         }
         var mask:int = int(me._presetUses[String(itemId)]);
         var numbers:Array = [];
         var n:int = 1;
         while(n <= PRESET_COUNT)
         {
            if((mask & 1 << n - 1) != 0)
            {
               numbers.push(n);
            }
            n++;
         }
         if(numbers.length == 0)
         {
            return "";
         }
         if(numbers.length == 1)
         {
            return LocalizationData.format("st_srp_loadout_sell_one",numbers[0]);
         }
         var items:Array = [];
         for each(var number:int in numbers)
         {
            items.push(LocalizationData.format("st_srp_loadout_sell_item",number));
         }
         var last:String = String(items.pop());
         return LocalizationData.format("st_srp_loadout_sell_many",items.join(", ") + LocalizationData.getName("st_srp_loadout_sell_and") + last);
      }
      
      // The roman numerals of the five loadouts (ItemRendererInv's badge).
      private static const ROMAN:Array = ["I","II","III","IV","V"];
      
      // ItemRendererInv: the loadouts that use `itemId` (the list's U rows,
      // as the sale's question; the active one's too), in roman numerals:
      // "II", "I IV", a run of three or more joined ("I-V", "II-IV V").
      // "" when none does.
      public static function loadoutMark(itemId:uint) : String
      {
         if(me == null)
         {
            return "";
         }
         var mask:int = int(me._presetUses[String(itemId)]);
         var parts:Array = [];
         var n:int = 0;
         while(n < PRESET_COUNT)
         {
            if((mask & 1 << n) == 0)
            {
               n++;
               continue;
            }
            var last:int = n;
            while(last + 1 < PRESET_COUNT && (mask & 1 << last + 1) != 0)
            {
               last++;
            }
            if(last - n >= 2)
            {
               parts.push(ROMAN[n] + "-" + ROMAN[last]);
            }
            else
            {
               var k:int = n;
               while(k <= last)
               {
                  parts.push(ROMAN[k]);
                  k++;
               }
            }
            n = last + 1;
         }
         return parts.join(" ");
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
         this._titleCentre = this.labelText.x + this.labelText.width / 2;
         var format:TextFormat = this.labelText.length > 0 ? this.labelText.getTextFormat() : this.labelText.defaultTextFormat;
         this._presetFont = format.font;
         this._presetBold = format.bold;
         format.size = TITLE_SIZE;
         this.labelText.defaultTextFormat = format;
         this.labelText.setTextFormat(format);
         this.labelText.width = TITLE_TEXT_W;
         this.labelText.height = TITLE_TEXT_H;
         this.labelText.y = TITLE_TEXT_Y;
         this._titleX = this._titleCentre - TITLE_TEXT_W / 2;
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
         addEventListener(ComponentEvent.HIDE,this.onPresetsGone);
         this.showPresetTitle();
         this._presetsRetry = new Timer(2000,0);
         this._presetsRetry.addEventListener(TimerEvent.TIMER,this.onPresetsRetry);
         this._presetsRetry.start();
         this.askPresets();
      }
      
      // RoomsWindow.playEnabled: the game's first word on Play, the lobby
      // live; the fallback counts from here (the asks while the movie loads
      // are lost), and an ask goes now.
      public static function lobbyLive() : void
      {
         if(me == null || me._reasonsFallback != null)
         {
            return;
         }
         me._reasonsFallback = new Timer(REASONS_FALLBACK_MS,1);
         me._reasonsFallback.addEventListener(TimerEvent.TIMER_COMPLETE,me.onReasonsFallback);
         me._reasonsFallback.start();
         me.askPresets();
      }
      
      // StatusPanel: Play greyed from its start until the reasons are known.
      public static function reasonsKnown() : Boolean
      {
         return me != null && me._reasonsKnown;
      }
      
      // No list in REASONS_FALLBACK_MS: the buttons lit (none greyed for
      // ever); the server's doors still refuse what may not go in.
      private function onReasonsFallback(e:TimerEvent) : void
      {
         if(this._reasonsKnown)
         {
            return;
         }
         this._reasonsKnown = true;
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.kitsChanged();
         }
      }
      
      private function onPresetsRetry(e:TimerEvent) : void
      {
         if(this._presetsSeen)
         {
            this._presetsRetry.stop();
            return;
         }
         this._presetsAskedAt = getTimer();
         this.sendPresets(PRESETS_FIRST);
      }
      
      // "Loadout 2 · Lvl. 15": the name, then the detail.
      private function presetLabel(i:int) : String
      {
         var detail:String = this.presetDetail(i);
         return this.presetName(i) + (detail != "" ? " · " + detail : "");
      }
      
      // Players see "Loadout", never "preset" (the owner's of 2026-10-23;
      // presets.md has the table).
      private function presetName(i:int) : String
      {
         return LocalizationData.getName("st_srp_loadout_title") + " " + (i + 1);
      }
      
      // The level, or why there is none ("": not known yet).
      private function presetDetail(i:int) : String
      {
         var preset:Object = this._presets[i];
         if(!preset.known)
         {
            return "";
         }
         if(!preset.saved)
         {
            return LocalizationData.getName("st_srp_loadout_empty");
         }
         if(preset.level < 0)
         {
            return LocalizationData.getName("st_srp_loadout_unfit");
         }
         // The kit tabs' own word ("Lvl.").
         return LocalizationData.getName("clan_players_level_label") + " " + preset.level;
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
         this._presetCaret.y = Math.round(TITLE_H / 2 - CARET_H / 2 + 2);
         this._presetTitle.buttonMode = ready && !this._presetsLocked;
         this.drawPresetTitle();
      }
      
      // Over, lightened as a row of the list; open, with a 1 px outline at
      // 10 % (the lobby's selected option row).
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
            g.beginFill(16777215,HOVER_ALPHA);
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
      
      // A press on the title opens or closes the list.
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
         }
      }
      
      // The row under a stage point chosen (a press given to the list itself:
      // scaleform-movie-pitfalls; one given to the transient layer's holder comes
      // to the list too, Transient.onHolderPress).
      private function choosePresetAt(stageX:Number, stageY:Number) : void
      {
         var i:int = 0;
         while(i < this._presetRows.length)
         {
            var row:DisplayObject = this._presetRows[i] as DisplayObject;
            if(row.hitTestPoint(stageX,stageY,false))
            {
               this.choosePreset(i);
               return;
            }
            i++;
         }
      }
      
      // The list under the title, its width, five rows, the active one
      // marked (see ROW_H).
      private function openPresetList() : void
      {
         if(this._presetList != null || !this._presetsSeen || this._presetsLocked || stage == null)
         {
            return;
         }
         var list:Sprite = new Sprite();
         list.x = TITLE_X;
         list.y = TITLE_Y + TITLE_H;
         var g:Graphics = list.graphics;
         g.beginFill(2039840,0.6);
         g.drawRect(0,0,TITLE_W,PRESET_COUNT * ROW_H + 2);
         g.endFill();
         g.beginFill(197636,0.97);
         g.drawRect(1,1,TITLE_W - 2,PRESET_COUNT * ROW_H);
         g.endFill();
         this._presetRows = [];
         this._presetRowOver = -1;
         this._presetRowDown = -1;
         var i:int = 0;
         while(i < PRESET_COUNT)
         {
            var row:Sprite = new Sprite();
            row.x = 1;
            row.y = 1 + i * ROW_H;
            row.name = String(i);
            row.buttonMode = true;
            row.mouseChildren = false;
            row.addChild(this.rowText());
            row.addChild(this.rowText());
            row.addEventListener(MouseEvent.ROLL_OVER,this.onPresetRowOver);
            row.addEventListener(MouseEvent.ROLL_OUT,this.onPresetRowOut);
            row.addEventListener(MouseEvent.MOUSE_DOWN,this.onPresetRowDown);
            row.addEventListener(MouseEvent.MOUSE_UP,this.onPresetRowUp);
            row.addEventListener(MouseEvent.CLICK,this.onPresetRow);
            list.addChild(row);
            this._presetRows.push(row);
            i++;
         }
         list.addEventListener(MouseEvent.MOUSE_DOWN,this.onPresetListPress);
         this._presetList = list;
         this.fillPresetRows();
         addChild(list);
         this.drawPresetTitle();
         // a transient window (Transient, docs/notes/transient-windows.md): a press outside the
         // list and the title, Esc or a window opening closes it with no change
         Transient.open(list,this._presetTitle,this.closePresetList);
      }
      
      private function rowText() : TextField
      {
         var field:TextField = new TextField();
         field.defaultTextFormat = new TextFormat(this._presetFont,ROW_SIZE,16777215,this._presetBold);
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.y = Math.round((ROW_H - ROW_SIZE * 1.22 - 4) / 2);
         field.height = ROW_SIZE * 1.22 + 6;
         return field;
      }
      
      // The rows' texts and looks, as the presets now are.
      private function fillPresetRows() : void
      {
         var i:int = 0;
         while(i < this._presetRows.length)
         {
            var row:Sprite = this._presetRows[i] as Sprite;
            var active:Boolean = i == this.presetShown();
            var nameField:TextField = row.getChildAt(0) as TextField;
            var detailField:TextField = row.getChildAt(1) as TextField;
            nameField.text = this.presetName(i);
            nameField.textColor = active ? 16777215 : RoomsLook.LIGHT;
            nameField.x = ROW_PAD;
            nameField.width = nameField.textWidth + 8;
            detailField.text = this.presetDetail(i);
            detailField.textColor = active ? RoomsLook.LIGHT : RoomsLook.GREY;
            detailField.width = detailField.textWidth + 8;
            detailField.x = TITLE_W - 2 - ROW_PAD - detailField.textWidth - 4;
            this.drawPresetRow(i);
            i++;
         }
      }
      
      private function drawPresetRow(i:int) : void
      {
         var row:Sprite = this._presetRows[i] as Sprite;
         var w:Number = TITLE_W - 2;
         var g:Graphics = row.graphics;
         g.clear();
         // Alpha 0 is not hit-tested in Scaleform: 0.01.
         g.beginFill(0,0.01);
         g.drawRect(0,0,w,ROW_H);
         g.endFill();
         if(i == this.presetShown())
         {
            g.beginFill(3943440,0.6);
            g.drawRect(0,0,w,ROW_H);
            g.endFill();
            g.beginFill(RoomsLook.GOLD,1);
            g.drawRect(0,0,3,ROW_H);
            g.endFill();
         }
         if(i == this._presetRowDown || i == this._presetRowOver)
         {
            g.beginFill(16777215,i == this._presetRowDown ? PRESS_ALPHA : HOVER_ALPHA);
            g.drawRect(0,0,w,ROW_H);
            g.endFill();
         }
         if(i < PRESET_COUNT - 1)
         {
            g.beginFill(1976368,1);
            g.drawRect(0,ROW_H - 1,w,1);
            g.endFill();
         }
      }
      
      private function presetRowOf(e:MouseEvent) : int
      {
         var row:Sprite = e.currentTarget as Sprite;
         return row != null ? int(row.name) : -1;
      }
      
      private function onPresetRowOver(e:MouseEvent) : void
      {
         this._presetRowOver = this.presetRowOf(e);
         this.redrawPresetRows();
      }
      
      private function onPresetRowOut(e:MouseEvent) : void
      {
         var at:int = this.presetRowOf(e);
         if(this._presetRowOver == at)
         {
            this._presetRowOver = -1;
         }
         if(this._presetRowDown == at)
         {
            this._presetRowDown = -1;
         }
         this.redrawPresetRows();
      }
      
      private function onPresetRowDown(e:MouseEvent) : void
      {
         this._presetRowDown = this.presetRowOf(e);
         this.redrawPresetRows();
      }
      
      private function onPresetRowUp(e:MouseEvent) : void
      {
         this._presetRowDown = -1;
         this.redrawPresetRows();
      }
      
      private function redrawPresetRows() : void
      {
         var i:int = 0;
         while(i < this._presetRows.length)
         {
            this.drawPresetRow(i);
            i++;
         }
      }
      
      // A press Scaleform gives the list itself, not the row under it.
      private function onPresetListPress(e:MouseEvent) : void
      {
         if(e.target == this._presetList)
         {
            this.choosePresetAt(e.stageX,e.stageY);
         }
      }
      
      private function closePresetList() : void
      {
         if(this._presetList == null)
         {
            return;
         }
         var list:Sprite = this._presetList;
         this._presetList = null;
         Transient.closed(list);
         for each(var row:Sprite in this._presetRows)
         {
            row.removeEventListener(MouseEvent.ROLL_OVER,this.onPresetRowOver);
            row.removeEventListener(MouseEvent.ROLL_OUT,this.onPresetRowOut);
            row.removeEventListener(MouseEvent.MOUSE_DOWN,this.onPresetRowDown);
            row.removeEventListener(MouseEvent.MOUSE_UP,this.onPresetRowUp);
            row.removeEventListener(MouseEvent.CLICK,this.onPresetRow);
         }
         this._presetRows = [];
         this._presetRowOver = -1;
         this._presetRowDown = -1;
         list.removeEventListener(MouseEvent.MOUSE_DOWN,this.onPresetListPress);
         if(list.parent != null)
         {
            list.parent.removeChild(list);
         }
         this.drawPresetTitle();
      }
      
      // The panel hidden (another window) or gone closes the list.
      private function onPresetsGone(e:Event) : void
      {
         this._presetHover = false;
         this.closePresetList();
      }
      
      private function onPresetRow(e:MouseEvent) : void
      {
         e.stopPropagation();
         this.choosePreset(this.presetRowOf(e));
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
      // An ask within the 2 s is not dropped: one ask goes at the window's
      // end (one waiting at most), so the list follows the last change.
      private function askPresets() : void
      {
         var since:int = getTimer() - this._presetsAskedAt;
         if(since < 2000)
         {
            if(this._presetsLater == null)
            {
               this._presetsLater = new Timer(2000 - since,1);
               this._presetsLater.addEventListener(TimerEvent.TIMER_COMPLETE,this.onPresetsLater);
               this._presetsLater.start();
            }
            return;
         }
         this._presetsAskedAt = getTimer();
         this.sendPresets(PRESETS_FIRST);
      }
      
      // A maintenance planned: the list again every minute (its minutes, and
      // its reason from 30 on).
      private function watchMaintenance() : void
      {
         if(this._maintenanceMinutes < 0)
         {
            if(this._maintenanceWatch != null)
            {
               this._maintenanceWatch.stop();
            }
            return;
         }
         if(this._maintenanceWatch == null)
         {
            this._maintenanceWatch = new Timer(60000,0);
            this._maintenanceWatch.addEventListener(TimerEvent.TIMER,this.onMaintenanceWatch);
         }
         if(!this._maintenanceWatch.running)
         {
            this._maintenanceWatch.start();
         }
      }
      
      private function onMaintenanceWatch(e:TimerEvent) : void
      {
         this.askPresets();
      }
      
      private function onPresetsLater(e:TimerEvent) : void
      {
         this._presetsLater.removeEventListener(TimerEvent.TIMER_COMPLETE,this.onPresetsLater);
         this._presetsLater = null;
         this.askPresets();
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
      
      // `P2;L;<done>;<preset>;<active>;<blocks>;<minutes>`, then `<preset>;<saved>;<level>`
      // each, then `U;<item id>:<presets>,...` rows.
      private function takePresets(rows:Array) : void
      {
         var head:Array = String(rows[0].nickname).split(";");
         if(head[0] != "P2" || head[1] != "L")
         {
            return;
         }
         var active:int = int(head[4]) - 1;
         this._blocks = int(head[5]);
         this._reasonsKnown = true;
         this._maintenanceMinutes = head.length > 6 ? int(head[6]) : -1;
         this.watchMaintenance();
         if(active >= 0 && active < PRESET_COUNT)
         {
            this._presetAt = active;
         }
         var uses:Object = {};
         var usesRows:String = "";
         var i:int = 1;
         while(i < rows.length)
         {
            var name:String = String(rows[i].nickname);
            if(name.indexOf("U;") == 0)
            {
               usesRows += name + "\n";
               var entries:Array = name.substr(2).split(",");
               var k:int = 0;
               while(k < entries.length)
               {
                  var pair:Array = String(entries[k]).split(":");
                  if(pair.length == 2)
                  {
                     uses[String(uint(pair[0]))] = int(pair[1]);
                  }
                  k++;
               }
               i++;
               continue;
            }
            var f:Array = name.split(";");
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
         this._presetUses = uses;
         // The inventory's loadout badges follow (ItemRendererInv): drawn
         // again when the uses changed.
         if(usesRows != this._presetUsesRows)
         {
            this._presetUsesRows = usesRows;
            if(this._playerInventory != null)
            {
               this._playerInventory.invalidateList();
            }
         }
         this._presetsSeen = true;
         if(this._presetsRetry != null)
         {
            this._presetsRetry.stop();
         }
         var done:int = int(head[2]);
         // A choice's answer (or one never coming) ends the choice shown.
         if(done == PRESET_CHOSEN || done == PRESET_REFUSED || getTimer() - this._presetPendingAt > PRESET_PENDING_MS)
         {
            this._presetPending = -1;
         }
         this.showPresetTitle();
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.kitsChanged();
         }
         if(this._presetList != null)
         {
            this.fillPresetRows();
         }
         // A slot whose item is gone is just empty: no word of it (the
         // owner's of 2026-10-23).
         if(done == PRESET_REFUSED)
         {
            Message.showMessage("ok",LocalizationData.getName("st_srp_loadout_refused"));
         }
      }
   }
}
