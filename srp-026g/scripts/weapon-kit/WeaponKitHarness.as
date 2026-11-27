package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;

   // The weapon's upgrade window with the weapon kit (docs/notes/weapon-kit.md, weapon-camo.md),
   // with the lobby movie's own window (SrvModifiersControlReskin, UpgradeWindow), inventory list
   // and profile, fed as the exe feeds them (an item's upgrades as patch 68 gives them):
   // - layout: a weapon's two decal slots and its camouflage on the Appearance page, apart and
   //   inside the window; armour's slots where the movie has them, before and after a weapon;
   // - the kit: decals as their records, the kit's own item in its slot, the camouflage it names
   //   as the inventory's lowest item of that dict id; a kit whose own is the camouflage; armour's
   //   records as they come;
   // - the preview: a drop on a slot shown at once (the slot's handleDropEvent, the window's
   //   onItemDropped): a decal into the second slot, a camouflage beside the kit's scope;
   // - a double click on an inventory item (InventoryList.onItemDoubleClick, UpgradeWindow.attach):
   //   a camouflage into the camouflage slot, a decal into the first free decal slot, over the first
   //   when both are full, a scope into the scope slot, the item already held left as it is, nothing
   //   the page does not show; armour's camouflage and decal; with the window closed, nothing;
   // - the second sight (SecondSights, the movie's table: the HK416 2218 with the optics 2112 and
   //   2116): a wide kit (kit_second, kit_side) shows its optic under the scope (the scope's own
   //   optic's dict id: the other item of it) and its side on the Left/Right toggle; all four slots
   //   and the toggle inside the window; a side's press sends srp_set_upgrade_value(host, optic,
   //   side) (HarnessExternal's calls) and shows at once, the account's update what the server holds;
   //   a kit whose own is the second sight; the slot chosen: what it takes bright, the rest dim, a
   //   double click into it, the side left; a weapon without rows has no second-sight slot;
   // - real clicks (check.sh's xdotool on Ruffle's window) on the Appearance page as the right-click's
   //   entry opens it: the list shows the slot's own kind, the camouflage slot's the camouflages
   //   whether empty or filled (a filled slot's middle is its change button), after a decal slot's
   //   click and a decal edit's start and cancel too; a decal's change button starts its edit
   //   (start_decal_edit), a camouflage's never;
   // - the texts: each language's title, page names, slot labels, hints, buttons and the toggle fitting.
   // Prints "WEAPONKIT ..." lines and ends with "WEAPONKIT RESULT pass" or "... fail: ...".
   public class WeaponKitHarness extends Sprite
   {
      private static const RIFLE:int = 9001;

      // a weapon whose stock scope the window names (default_scope)
      private static const STOCK_RIFLE:int = 9002;

      private static const ARMOUR:int = 9010;

      private static const DECAL_A:int = 9020;

      private static const DECAL_B:int = 9021;

      private static const DECAL_C:int = 9022;

      // a shop scope (ItemDictId.SHOP_SCOPE_FIRST..LAST)
      private static const SCOPE:int = 2150;

      private static const CAMO:int = 9030;

      // a weapon that takes the scope, a muzzle device and a grip; one that takes the grip alone
      private static const MODULE_RIFLE:int = 9003;

      private static const GRIP_RIFLE:int = 9004;

      // a muzzle device and a grip (ModuleKinds, of scripts/weapon-modules/ids.tsv)
      private static const DEVICE:int = 2601;

      private static const GRIP:int = 2659;

      private static const CAMO_OTHER:int = 9031;

      // a weapon with SecondSights rows (the HK416), its two optics (shop scopes: its scopes too)
      private static const WIDE_RIFLE:int = 2218;

      private static const OPTIC_A:int = 2112;

      private static const OPTIC_B:int = 2116;

      private static const WIDE_RIFLE_ID:int = 16;

      // two items of OPTIC_A: the scope (the kit's own) and the second sight
      private static const OPTIC_A_LOW_ID:int = 44;

      private static const OPTIC_A_HIGH_ID:int = 45;

      private static const OPTIC_B_ID:int = 46;

      // item ids
      private static const RIFLE_ID:int = 11;

      private static const ARMOUR_ID:int = 12;

      private static const STOCK_RIFLE_ID:int = 13;

      private static const DECAL_A_ID:int = 21;

      private static const DECAL_B_ID:int = 22;

      private static const DECAL_C_ID:int = 23;

      private static const SCOPE_ID:int = 31;

      private static const DEVICE_ID:int = 32;

      private static const GRIP_ID:int = 33;

      private static const MODULE_RIFLE_ID:int = 14;

      private static const GRIP_RIFLE_ID:int = 15;

      // the scope slot's place on the Attachments page (the top one)
      private var topPlace:Array;

      // two items of the camouflage's dict id: the kit's camouflage is the lowest, 40
      private static const CAMO_HIGH_ID:int = 50;

      private static const CAMO_LOW_ID:int = 40;

      private static const CAMO_OTHER_ID:int = 60;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var window:*;

      private var shop:*;

      private var Window:Object;

      private var Category:Object;

      private var Slot:Object;

      // each slot's place as the movie has it, before any item: [x, y, scaleX, scaleY]
      private var moviePlaces:Object = {};

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      private var language:int = 0;

      private var widest:Object;

      private var measured:int = 0;

      private var shotX:Number = 0;

      private var shotHintX:Number = 0;

      public function WeaponKitHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): only its classes are used
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
            if(frames > 0)
            {
               trace("WEAPONKIT error " + e.error + (e.error is Error ? " " + Error(e.error).getStackTrace() : ""));
            }
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function C(name:String) : Class
      {
         return domain.getDefinition(name) as Class;
      }

      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            Window = domain.getDefinition("survarium.upgrade.UpgradeWindow");
            Category = domain.getDefinition("survarium.constants.ItemCategory");
            Slot = domain.getDefinition("survarium.constants.CosmeticSlot");
            var Scene:Object = domain.getDefinition("MainScene");
            Scene.stage = this;
            Scene.scaleFactor = 1;
            domain.getDefinition("survarium.data.LocalizationData").setData(text("english"));
            at = "the icons";
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["weapons","equipment","consumables","factionItemIcons","bundles","decals","camo","camo_big","vouchers","modifiers"])
            {
               new Container(icons,8,8,new BitmapData(8,8,true,0xff808080),false,1,0,0,false);
            }
            at = "the items";
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("weapon-kit-check");
            descriptor.setItemsDictionary([item(RIFLE,Category.ASSAULT_RIFLES,"Rifle",2,1),item(STOCK_RIFLE,Category.ASSAULT_RIFLES,"Stock-scoped rifle",2,1),item(ARMOUR,Category.BODY_ARMOR,"Armour",2,1),item(DECAL_A,Category.DECALS,"Decal A",0,0),item(DECAL_B,Category.DECALS,"Decal B",0,0),item(DECAL_C,Category.DECALS,"Decal C",0,0),item(SCOPE,Category.SCOPES,"Scope",0,0),item(CAMO,Category.CAMOUFLAGES,"Camouflage",0,0),item(CAMO_OTHER,Category.CAMOUFLAGES,"Other camouflage",0,0),item(MODULE_RIFLE,Category.ASSAULT_RIFLES,"Module rifle",2,1),item(GRIP_RIFLE,Category.ASSAULT_RIFLES,"Grip rifle",2,1),item(DEVICE,Category.SCOPES,"Muzzle device",0,0),item(GRIP,Category.SCOPES,"Grip",0,0),item(WIDE_RIFLE,Category.ASSAULT_RIFLES,"HK416",2,1),item(OPTIC_A,Category.SCOPES,"DeltaPoint",0,0),item(OPTIC_B,Category.SCOPES,"RSA-S",0,0)]);
            // the scopes the rifles take (ItemFactory gives the movie its pairs so)
            Window.addCompatibility(RIFLE,SCOPE);
            Window.addCompatibility(STOCK_RIFLE,SCOPE);
            Window.addCompatibility(STOCK_RIFLE,domain.getDefinition("survarium.constants.ItemDictId").SCOPE_PSO_1);
            Window.addCompatibility(MODULE_RIFLE,SCOPE);
            Window.addCompatibility(MODULE_RIFLE,DEVICE);
            Window.addCompatibility(MODULE_RIFLE,GRIP);
            Window.addCompatibility(GRIP_RIFLE,GRIP);
            // the HK416: both optics and the scope as its scope, a device and a grip; the optics as its
            // second sight too (SecondSights)
            for each(var attachment:int in [OPTIC_A,OPTIC_B,SCOPE,DEVICE,GRIP])
            {
               Window.addCompatibility(WIDE_RIFLE,attachment);
            }
            at = "the shop";
            var shop:* = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([RIFLE,STOCK_RIFLE,ARMOUR,DECAL_A,DECAL_B,DECAL_C,SCOPE,CAMO,CAMO_OTHER,MODULE_RIFLE,GRIP_RIFLE,DEVICE,GRIP,WIDE_RIFLE,OPTIC_A,OPTIC_B].map(function(dictId:int, i:int, a:Array):Object
            {
               return {dictId:dictId,faction_id:0,level:0,cost:100,condition_or_stack:10000};
            }));
            this.shop = shop;
            at = "the lobby";
            // The lobby's root as the list, the card and the window find it (parent["inventory_list"]).
            lobby = new MovieClip();
            addChild(lobby);
            domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
            var profile:* = new (C("SrvLocalPlayerProfile"))();
            profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
            lobby.player_profile = profile;
            lobby.addChild(profile);
            var card:* = new (C("SrvSelectedItemControl"))();
            lobby.item_control = card;
            inventory = new (C("SrvInventoryReskin"))();
            inventory.name = "inventory_list";
            lobby.inventory_list = inventory;
            lobby.addChild(inventory);
            lobby.addChild(card);
            card.setInventory(inventory);
            inventory.registerItemDescriptor(descriptor);
            profile.registerInventory(inventory);
            profile.paperDoll.ammoControl = new (C("survarium.ammo.AmmoShareControll"))();
            profile.paperDoll.ammoControl.registerShop(shop);
            profile.paperDoll.ammoControl.registerInventory(inventory);
            inventory.item_control = card;
            at = "the window";
            // as LobbyMenu makes it: the movie's window symbol, on the lobby, given the inventory
            window = new (C("SrvModifiersControlReskin"))();
            lobby.addChild(window);
            window.validateNow();
            window.setInventory(inventory);
            for each(var s:* in slots())
            {
               moviePlaces[s.name] = [s.x,s.y,s.actualScaleX,s.actualScaleY];
            }
            trace("WEAPONKIT the movie's slots: " + slots().map(function(s:*, i:int, a:Array):String
            {
               return s.name + " index " + s.index + " at " + moviePlaces[s.name].join(",");
            }).join("; "));
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err + " " + err.getStackTrace());
         }
      }

      // decal_available as the exe gives the movie a config's `true`: its flags & 2, 2 (items.md)
      private static function item(dictId:int, category:int, name:String, decals:int, camo:int) : Object
      {
         return {dictId:dictId,category:category,name:name,level:1,icon_row:1,icon_column:1,is_stack:false,modifiers:[],item_properties:[],decal_available:decals,camo_available:camo};
      }

      private static function owned(id:int, dictId:int) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:10000,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0,upgrades:[]};
      }

      private function text(language:String) : Object
      {
         var t:Object = {};
         for(var k:String in WeaponKitText.DATA[language])
         {
            t[k] = WeaponKitText.DATA[language][k];
         }
         return t;
      }

      // One step a few frames apart: the slots draw (and their frames tween) as the frames render.
      private function tick(e:Event) : void
      {
         if(clickWait >= 0 && ++clickWait > 1800)
         {
            clickWait = -1;
            require(false,"check.sh's click " + clickIndex + " never came");
         }
         if(++frames < 10 || --wait > 0 || clickWait >= 0)
         {
            return;
         }
         wait = 20;
         try
         {
            switch(step++)
            {
               case 0:
                  inventory.fillSellers([{faction_id:1,name:"Seller"}]);
                  inventory.sellersTabPage.selectedIndex = 0;
                  inventory.toggleShopMode(false);
                  inventory.filterItems(null);
                  inventory.optionControl.checkbox_all_traders.selected = true;
                  var offers:Array = [];
                  shop.forEachItem(function(entry:*):Boolean
                  {
                     offers.push(entry);
                     return false;
                  });
                  inventory.setupShopItems(offers);
                  inventory.setupBundleData([]);
                  inventory.setupInventoryData([owned(RIFLE_ID,RIFLE),owned(ARMOUR_ID,ARMOUR),owned(STOCK_RIFLE_ID,STOCK_RIFLE),owned(DECAL_A_ID,DECAL_A),owned(DECAL_B_ID,DECAL_B),owned(DECAL_C_ID,DECAL_C),owned(SCOPE_ID,SCOPE),owned(CAMO_HIGH_ID,CAMO),owned(CAMO_LOW_ID,CAMO),owned(CAMO_OTHER_ID,CAMO_OTHER),owned(MODULE_RIFLE_ID,MODULE_RIFLE),owned(GRIP_RIFLE_ID,GRIP_RIFLE),owned(DEVICE_ID,DEVICE),owned(GRIP_ID,GRIP),owned(WIDE_RIFLE_ID,WIDE_RIFLE),owned(OPTIC_A_HIGH_ID,OPTIC_A),owned(OPTIC_A_LOW_ID,OPTIC_A),owned(OPTIC_B_ID,OPTIC_B)]);
                  require(inventory.getInventoryItemOf(CAMO).itemId == CAMO_LOW_ID,"getInventoryItemOf gives the lowest item of a dict id (" + inventory.getInventoryItemOf(CAMO).itemId + ")");
                  require(inventory.getInventoryItemOf(4242) == null,"getInventoryItemOf gives null for a dict id not owned");
                  // armour first, before any weapon: the movie's window
                  open(ARMOUR_ID,[{id:DECAL_A_ID,param:0},{id:DECAL_B_ID,param:1},{id:CAMO_HIGH_ID,param:3}]);
                  break;
               case 1:
                  checkArmour("armour before a weapon");
                  // the kit: two decals and the kit (its own the scope, naming the camouflage)
                  open(RIFLE_ID,[{id:DECAL_A_ID,param:0},{id:DECAL_B_ID,param:1},kit(SCOPE_ID,3,CAMO)]);
                  break;
               case 2:
                  require(window.container.textField.text == WeaponKitText.DATA.english.scope_window_title,"a scoped weapon opens on Attachments (the title says " + window.container.textField.text + ")");
                  expect("the kit (scope, camouflage named)",[DECAL_A_ID,DECAL_B_ID,CAMO_LOW_ID,SCOPE_ID]);
                  require(window.camoSlot.visible && !window.decalSlot.visible && !window.decalSlot1.visible && !window.decalSlot2.visible,"Attachments shows the scope slot only");
                  turnPage();
                  break;
               case 3:
                  require(window.container.textField.text == WeaponKitText.DATA.english.btn_upgrade_label,"the arrow turns to Appearance (the title says " + window.container.textField.text + ")");
                  require(window.decalSlot.visible && window.decalSlot1.visible && window.decalSlot2.visible && !window.camoSlot.visible,"Appearance shows two decal slots and the camouflage, not the scope");
                  expect("the kit on Appearance",[DECAL_A_ID,DECAL_B_ID,CAMO_LOW_ID,SCOPE_ID]);
                  checkWeaponLayout();
                  // a kit whose own is the camouflage, nothing named
                  open(RIFLE_ID,[kit(CAMO_HIGH_ID,2,0)]);
                  break;
               case 4:
                  expect("a kit whose own is the camouflage",[0,0,CAMO_HIGH_ID,0]);
                  // armour again, after a weapon: the movie's places back, its records as they come
                  open(ARMOUR_ID,[{id:DECAL_A_ID,param:0},{id:DECAL_B_ID,param:1},{id:CAMO_HIGH_ID,param:3}]);
                  break;
               case 5:
                  checkArmour("armour after a weapon");
                  // the preview: decal A only, decal B dropped into the second decal slot
                  open(RIFLE_ID,[{id:DECAL_A_ID,param:0}]);
                  break;
               case 6:
                  turnPage();
                  break;
               case 7:
                  expect("decal A alone",[DECAL_A_ID,0,0,0]);
                  drop(window.decalSlot1,DECAL_B_ID);
                  expect("decal B dropped into the second decal slot",[DECAL_A_ID,DECAL_B_ID,0,0]);
                  // a camouflage dropped beside the kit's scope
                  open(RIFLE_ID,[kit(SCOPE_ID,3,0)]);
                  break;
               case 8:
                  expect("the kit (scope, nothing named)",[0,0,0,SCOPE_ID]);
                  turnPage();
                  break;
               case 9:
                  drop(window.decalSlot2,CAMO_OTHER_ID);
                  expect("a camouflage dropped beside the kit's scope",[0,0,CAMO_OTHER_ID,SCOPE_ID]);
                  // double clicks: an empty weapon on Appearance
                  open(RIFLE_ID,[]);
                  turnPage();
                  break;
               case 10:
                  doubleClick(CAMO_OTHER_ID,true);
                  expect("a camouflage double-clicked: the camouflage slot",[0,0,CAMO_OTHER_ID,0]);
                  doubleClick(DECAL_A_ID,true);
                  expect("a decal double-clicked: the first decal slot",[DECAL_A_ID,0,CAMO_OTHER_ID,0]);
                  doubleClick(DECAL_A_ID,false);
                  expect("the decal held double-clicked again: left as it is",[DECAL_A_ID,0,CAMO_OTHER_ID,0]);
                  doubleClick(DECAL_B_ID,true);
                  expect("a second decal double-clicked: the second decal slot",[DECAL_A_ID,DECAL_B_ID,CAMO_OTHER_ID,0]);
                  doubleClick(DECAL_C_ID,true);
                  expect("a decal double-clicked with both full: over the first",[DECAL_C_ID,DECAL_B_ID,CAMO_OTHER_ID,0]);
                  doubleClick(CAMO_LOW_ID,true);
                  expect("another camouflage double-clicked: over the camouflage",[DECAL_C_ID,DECAL_B_ID,CAMO_LOW_ID,0]);
                  doubleClick(SCOPE_ID,false);
                  expect("a scope double-clicked on Appearance: nothing",[DECAL_C_ID,DECAL_B_ID,CAMO_LOW_ID,0]);
                  // the first decal slot free, the second full
                  open(RIFLE_ID,[{id:DECAL_A_ID,param:1}]);
                  turnPage();
                  break;
               case 11:
                  doubleClick(DECAL_B_ID,true);
                  expect("a decal double-clicked, the second slot full: the first",[DECAL_B_ID,DECAL_A_ID,0,0]);
                  // Attachments: the scope
                  open(RIFLE_ID,[]);
                  break;
               case 12:
                  doubleClick(CAMO_OTHER_ID,false);
                  expect("a camouflage double-clicked on Attachments: nothing",[0,0,0,0]);
                  doubleClick(SCOPE_ID,true);
                  expect("a scope double-clicked on Attachments: the scope slot",[0,0,0,SCOPE_ID]);
                  // armour: its camouflage and decals
                  open(ARMOUR_ID,[{id:DECAL_A_ID,param:0}]);
                  break;
               case 13:
                  doubleClick(CAMO_OTHER_ID,true);
                  expect("armour: a camouflage double-clicked",[DECAL_A_ID,0,0,CAMO_OTHER_ID]);
                  doubleClick(DECAL_B_ID,true);
                  expect("armour: a decal double-clicked, the first full: the second",[DECAL_A_ID,DECAL_B_ID,0,CAMO_OTHER_ID]);
                  doubleClick(DECAL_C_ID,true);
                  expect("armour: a decal double-clicked with both full: over the first",[DECAL_C_ID,DECAL_B_ID,0,CAMO_OTHER_ID]);
                  // the window closed: nothing
                  window.closePopup();
                  window.visible = false;
                  doubleClick(CAMO_OTHER_ID,false);
                  doubleClick(DECAL_A_ID,false);
                  break;
               case 14:
                  // a weapon that takes a muzzle device and a grip, its kit naming both
                  open(MODULE_RIFLE_ID,[{id:SCOPE_ID,param:3,kit_own:3,kit_camo:0,kit_muzzle:DEVICE,kit_grip:GRIP}]);
                  break;
               case 15:
                  require(window.container.textField.text == WeaponKitText.DATA.english.scope_window_title,"a weapon with modules opens on Attachments");
                  require(window.camoSlot.visible && window.muzzleSlot.visible && window.gripSlot.visible,"Attachments shows the scope, muzzle device and grip slots");
                  require(!window.secondSightSlot.visible && !sideButtonsShown(),"a weapon without SecondSights rows: no second-sight slot, no side toggle");
                  require(!window.decalSlot.visible && !window.decalSlot1.visible && !window.decalSlot2.visible,"Attachments shows no decal or camouflage slot");
                  expectAttachments("the kit naming a device and a grip",[SCOPE_ID,DEVICE_ID,GRIP_ID]);
                  checkAttachmentLayout();
                  topPlace = [window.camoSlot.x,window.camoSlot.y];
                  open(MODULE_RIFLE_ID,[]);
                  break;
               case 16:
                  doubleClick(DEVICE_ID,true);
                  doubleClick(GRIP_ID,true);
                  doubleClick(SCOPE_ID,true);
                  expectAttachments("a device, a grip and a scope double-clicked: each its own slot",[SCOPE_ID,DEVICE_ID,GRIP_ID]);
                  var Drag:Class = C("scaleform.clik.events.DragEvent");
                  window.onDrag(new Drag("dragStart",inventory.getInventoryItem(GRIP_ID),null,null,null));
                  require(window.gripSlot.highlighted && !window.muzzleSlot.highlighted && !window.camoSlot.highlighted,"a grip's drag lights up the grip's slot alone");
                  window.onDrag(new Drag("dragEnd",inventory.getInventoryItem(GRIP_ID),null,null,null));
                  // the server refused the grip (a pair the weapon cannot hold): the account's update
                  // holds the device alone, and reaches the open window as the game hands it the
                  // inventory (LobbyMenu.update_inventory_item: UpgradeWindow.itemUpdated)
                  var data:* = inventory.getInventoryItem(MODULE_RIFLE_ID);
                  data.upgrades = [{id:SCOPE_ID,param:3,kit_own:3,kit_camo:0,kit_muzzle:DEVICE,kit_grip:0}];
                  window.itemUpdated(data);
                  expectAttachments("the account's update without the grip",[SCOPE_ID,DEVICE_ID,0]);
                  open(GRIP_RIFLE_ID,[]);
                  break;
               case 17:
                  require(window.gripSlot.visible && !window.muzzleSlot.visible && !window.camoSlot.visible,"a weapon that takes grips alone: the grip slot alone");
                  require(window.gripSlot.x == topPlace[0] && window.gripSlot.y == topPlace[1],"its grip slot at the top place (" + window.gripSlot.x + "," + window.gripSlot.y + ", not " + topPlace.join(",") + ")");
                  require(Window.greyed(inventory.getInventoryItem(DEVICE_ID)) && !Window.greyed(inventory.getInventoryItem(GRIP_ID)),"the inventory greys the device, not the grip");
                  break;
               case 18:
                  // the HK416's wide kit: the scope (the low item of OPTIC_A) its own, naming the device,
                  // the grip and OPTIC_A as its second sight, on the right
                  open(WIDE_RIFLE_ID,[wide(OPTIC_A_LOW_ID,3,DEVICE,GRIP,OPTIC_A,1)]);
                  break;
               case 19:
                  require(window.container.textField.text == WeaponKitText.DATA.english.scope_window_title,"the HK416 opens on Attachments");
                  require(window.camoSlot.visible && window.secondSightSlot.visible && window.muzzleSlot.visible && window.gripSlot.visible,"the HK416's Attachments shows the scope, second sight, muzzle device and grip slots");
                  expectFour("the wide kit",[OPTIC_A_LOW_ID,OPTIC_A_HIGH_ID,DEVICE_ID,GRIP_ID]);
                  expectSide("the wide kit",1,3);
                  checkFourLayout();
                  require(Window.secondSightSide(inventory.getInventoryItem(WIDE_RIFLE_ID)) == 1 && Window.attachedIn(inventory.getInventoryItem(WIDE_RIFLE_ID),Slot.SECOND_SIGHT).itemId == OPTIC_A_HIGH_ID,"the weapon's card reads its second sight and side (attachedIn, secondSightSide)");
                  require(window.infoContainer.infoDescr.text.indexOf(WeaponKitText.DATA.english.st_srp_second_sight_hint) != -1,"the hint names the second sight's switch");
                  // the window right of the list for the picture (its place is the lobby's, outside the game
                  // off the screen); put back after
                  shotX = window.container.x;
                  shotHintX = window.infoContainer.x;
                  window.container.x = 640 - window.localToGlobal(new Point(0,0)).x;
                  window.infoContainer.x = 860;
                  trace("WEAPONKIT the window at " + window.x + "," + window.y + " visible " + window.visible + ", its container at " + window.container.x + "," + window.container.y + " visible " + window.container.visible + ", on the stage at " + window.container.localToGlobal(new Point(0,0)));
                  trace("WEAPONKIT shot second");
                  wait = 240;
                  break;
               case 20:
                  window.container.x = shotX;
                  window.infoContainer.x = shotHintX;
                  // the toggle: Left sends the place's value 0, the same side again nothing, Right 1
                  external().calls = [];
                  press(0);
                  expectSide("Left pressed",0,3);
                  press(0);
                  press(1);
                  expectSide("Right pressed",1,3);
                  expectCalls("the toggle",["srp_set_upgrade_value " + WIDE_RIFLE_ID + " " + OPTIC_A_HIGH_ID + " 0","srp_set_upgrade_value " + WIDE_RIFLE_ID + " " + OPTIC_A_HIGH_ID + " 1"]);
                  // the account's update holds the left (the server refused the right): the toggle says so
                  var held:* = inventory.getInventoryItem(WIDE_RIFLE_ID);
                  held.upgrades = [wide(OPTIC_A_LOW_ID,3,DEVICE,GRIP,OPTIC_A,0)];
                  window.itemUpdated(held);
                  expectSide("the account's update on the left",0,3);
                  expectFour("the account's update",[OPTIC_A_LOW_ID,OPTIC_A_HIGH_ID,DEVICE_ID,GRIP_ID]);
                  // a kit whose own is the second sight (nothing else on the weapon), on the right
                  open(WIDE_RIFLE_ID,[wide(OPTIC_B_ID,6,0,0,0,1)]);
                  break;
               case 21:
                  expectFour("a kit whose own is the second sight",[0,OPTIC_B_ID,0,0]);
                  expectSide("a kit whose own is the second sight",1,3);
                  // an empty HK416: the second sight's slot chosen
                  open(WIDE_RIFLE_ID,[]);
                  break;
               case 22:
                  require(!sideButtonsShown(),"an empty second-sight slot: no side toggle");
                  external().calls = [];
                  window.secondSightSlot.dispatchEvent(new MouseEvent(MouseEvent.CLICK,false));
                  require(!Window.greyed(inventory.getInventoryItem(OPTIC_B_ID)) && Window.greyed(inventory.getInventoryItem(SCOPE_ID)) && Window.greyed(inventory.getInventoryItem(DEVICE_ID)),"the second sight's slot chosen: its optics bright, the scope it does not take and the device dim");
                  require(Window.listsItem(inventory.getInventoryItem(SCOPE_ID)) && Window.listsItem(inventory.getInventoryItem(OPTIC_B_ID)),"the HK416's scope and optics listed");
                  doubleClick(OPTIC_B_ID,true);
                  expectFour("an optic double-clicked with the second sight's slot chosen",[0,OPTIC_B_ID,0,0]);
                  expectSide("an optic put in the empty slot",0,3);
                  doubleClick(SCOPE_ID,true);
                  expectFour("a scope the second sight does not take, double-clicked: the scope slot",[SCOPE_ID,OPTIC_B_ID,0,0]);
                  expectCalls("the double clicks",["relocate_upgrade_item " + WIDE_RIFLE_ID + " " + OPTIC_B_ID + " " + Slot.SECOND_SIGHT,"relocate_upgrade_item " + WIDE_RIFLE_ID + " " + SCOPE_ID + " " + Slot.CAMOUFLAGE]);
                  var Drag:Class = C("scaleform.clik.events.DragEvent");
                  window.onDrag(new Drag("dragStart",inventory.getInventoryItem(OPTIC_A_LOW_ID),null,null,null));
                  require(window.camoSlot.highlighted && window.secondSightSlot.highlighted && !window.muzzleSlot.highlighted,"an optic's drag lights up the scope's and the second sight's slots");
                  window.onDrag(new Drag("dragEnd",inventory.getInventoryItem(OPTIC_A_LOW_ID),null,null,null));
                  window.onDrag(new Drag("dragStart",inventory.getInventoryItem(SCOPE_ID),null,null,null));
                  require(window.camoSlot.highlighted && !window.secondSightSlot.highlighted,"a scope the second sight does not take lights up the scope's slot alone");
                  window.onDrag(new Drag("dragEnd",inventory.getInventoryItem(SCOPE_ID),null,null,null));
                  // the window on another weapon: none chosen, as before
                  open(MODULE_RIFLE_ID,[]);
                  require(!window.secondSightSlot.visible && !Window.greyed(inventory.getInventoryItem(SCOPE_ID)),"another weapon's window: no second-sight slot, its scope bright");
                  break;
               case 23:
                  // real clicks on the Appearance page (check.sh's xdotool), the window right of the list
                  // and its hint beside it as for the picture; first where the Ruffle window puts the stage
                  shotX = window.container.x;
                  shotHintX = window.infoContainer.x;
                  clicks = appearanceClicks();
                  stage.addEventListener(MouseEvent.MOUSE_DOWN,this.mouseDown,true);
                  stage.addEventListener(MouseEvent.MOUSE_DOWN,this.mouseDown);
                  stage.addEventListener(MouseEvent.CLICK,this.mouseClick,true);
                  trace("WEAPONKIT calibrate");
                  clickWait = 0;
                  break;
               case 24:
                  if(clickIndex > 0)
                  {
                     clicks[clickIndex - 1].after();
                  }
                  if(clickIndex < clicks.length)
                  {
                     var next:Object = clicks[clickIndex];
                     if(next.before != null)
                     {
                        next.before();
                     }
                     window.container.x = 640 - window.localToGlobal(new Point(0,0)).x;
                     window.infoContainer.x = 860;
                     external().calls = [];
                     var at:Rectangle = next.target().getBounds(stage);
                     trace("WEAPONKIT click " + clickIndex + " at " + int(at.x + at.width / 2) + "," + int(at.y + at.height / 2) + ": " + next.what);
                     clickIndex++;
                     clickWait = 0;
                     step--;
                  }
                  else
                  {
                     stage.removeEventListener(MouseEvent.MOUSE_DOWN,this.mouseDown,true);
                     stage.removeEventListener(MouseEvent.MOUSE_DOWN,this.mouseDown);
                     stage.removeEventListener(MouseEvent.CLICK,this.mouseClick,true);
                     window.container.x = shotX;
                     window.infoContainer.x = shotHintX;
                  }
                  break;
               case 25:
                  // every language: each window's texts in their fields
                  texts(WeaponKitText.LANGUAGES[language]);
                  if(++language < WeaponKitText.LANGUAGES.length)
                  {
                     step--;
                  }
                  break;
               default:
                  removeEventListener(Event.ENTER_FRAME,this.tick);
                  finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
            }
         }
         catch(err:Error)
         {
            removeEventListener(Event.ENTER_FRAME,this.tick);
            finish("fail: step " + (step - 1) + ": " + err + " " + err.getStackTrace());
         }
      }

      // A double click on an owned item of the inventory's list, as its tile list dispatches it
      // (CoreList: itemDoubleClick with the row's item); `taken` whether the window took it (the
      // game sent relocate_upgrade_item).
      private function doubleClick(itemId:int, taken:Boolean) : void
      {
         var ListEvent:Class = C("scaleform.clik.events.ListEvent");
         var item:* = inventory.getInventoryItem(itemId);
         var before:String = slots().concat(attachmentSlots(),[window.secondSightSlot]).map(function(s:*, i:int, a:Array):String
         {
            return s.data ? String(s.data.itemId) : "0";
         }).join(",");
         inventory.itemsTileList.dispatchEvent(new ListEvent(ListEvent.ITEM_DOUBLE_CLICK,false,true,-1,-1,-1,null,item));
         var after:String = slots().concat(attachmentSlots(),[window.secondSightSlot]).map(function(s:*, i:int, a:Array):String
         {
            return s.data ? String(s.data.itemId) : "0";
         }).join(",");
         var changed:Boolean = before != after;
         trace("WEAPONKIT double click on " + item.name + " (" + itemId + "): " + (changed ? "taken" : "nothing"));
         require(changed == taken,"a double click on " + item.name + (taken ? " goes into a slot" : " changes no slot"));
      }

      // A wide kit's record as patch 68 gives it the movie: its own item and slot, the dict ids it names
      // (device, grip, second sight) and the second sight's side.
      private static function wide(own:int, ownSlot:int, device:int, grip:int, second:int, side:int) : Object
      {
         return {id:own,param:ownSlot,kit_own:ownSlot,kit_camo:0,kit_muzzle:device,kit_grip:grip,kit_second:second,kit_side:side};
      }

      private function external() : Object
      {
         return domain.getDefinition("HarnessExternal");
      }

      private function expectCalls(what:String, calls:Array) : void
      {
         var made:Array = external().calls as Array;
         trace("WEAPONKIT " + what + ": the exe was called " + made.join("; "));
         require(made.join("; ") == calls.join("; "),what + ": the calls " + made.join("; ") + ", not " + calls.join("; "));
      }

      // A side button's press, as CLIK's button dispatches it.
      private function press(side:int) : void
      {
         window.sideButtons[side].dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
      }

      private function sideButtonsShown() : Boolean
      {
         return window.sideButtons[0].visible || window.sideButtons[1].visible;
      }

      // The toggle shown, `side` chosen (the other not), each side enabled by the mask (1 left, 2 right).
      private function expectSide(what:String, side:int, sides:int) : void
      {
         var b:Array = window.sideButtons;
         trace("WEAPONKIT " + what + ": the toggle " + b.map(function(x:*, i:int, a:Array):String
         {
            return x.name + (x.visible ? "" : " hidden") + (x.chosen ? " chosen" : "") + (x.enabled ? "" : " disabled") + " " + x.face;
         }).join(", "));
         require(b[0].visible && b[1].visible,what + ": the toggle shown");
         require(b[side].chosen && !b[1 - side].chosen,what + ": side " + side + " chosen");
         require(b[0].enabled == ((sides & 1) != 0) && b[1].enabled == ((sides & 2) != 0),what + ": the sides " + sides + " enabled");
      }

      private function fourSlots() : Array
      {
         return [window.camoSlot,window.secondSightSlot,window.muzzleSlot,window.gripSlot];
      }

      // The item ids of the Attachments page's four slots (scope, second sight, muzzle device, grip).
      private function expectFour(what:String, ids:Array) : void
      {
         var shown:Array = fourSlots().map(function(s:*, i:int, a:Array):int
         {
            return s.data ? int(s.data.itemId) : 0;
         });
         trace("WEAPONKIT " + what + ": scope, second sight, muzzle, grip hold " + shown.join(", "));
         require(shown.join(",") == ids.join(","),what + ": " + shown.join(",") + ", not " + ids.join(","));
      }

      // All four slots one under the other in the window, each label over the next square, the toggle
      // under the second sight's label and over the device's square, all of it above the Confirm button.
      private function checkFourLayout() : void
      {
         var bounds:Rectangle = frame();
         var shown:Array = fourSlots();
         var confirm:Rectangle = window.btnAccept.getBounds(window.container);
         trace("WEAPONKIT the window's frame " + bounds + ", its Confirm button " + confirm + "; the HK416's Attachments: " + shown.map(function(s:*, i:int, a:Array):String
         {
            return s.name + " at " + s.x + "," + s.y + " scale " + s.actualScaleX + " square " + square(s) + " label " + label(s);
         }).join("; ") + "; the toggle " + window.sideButtons.map(function(b:*, i:int, a:Array):String
         {
            return b.name + " " + b.getBounds(window.container);
         }).join(", "));
         for(var i:int = 0; i < shown.length; i++)
         {
            var a:Rectangle = square(shown[i]);
            var named:Rectangle = label(shown[i]);
            require(bounds != null && bounds.containsRect(a) && bounds.containsRect(named) && named.bottom <= confirm.top,"the HK416's slot " + i + " " + a + " and its label " + named + " inside the window " + bounds + ", above Confirm " + confirm);
            if(i > 0)
            {
               var above:Rectangle = label(shown[i - 1]);
               require(a.top >= above.bottom && Math.abs(a.x + a.width / 2 - (square(shown[i - 1]).x + square(shown[i - 1]).width / 2)) < 0.01,"the HK416's slot " + i + " " + a + " under slot " + (i - 1) + "'s label " + above + ", on its middle line");
            }
         }
         var sight:Rectangle = label(window.secondSightSlot);
         var device:Rectangle = square(window.muzzleSlot);
         var left:Rectangle = window.sideButtons[0].getBounds(window.container);
         var right:Rectangle = window.sideButtons[1].getBounds(window.container);
         var middle:Number = square(window.secondSightSlot).x + square(window.secondSightSlot).width / 2;
         require(left.top >= sight.bottom && left.bottom <= device.top && right.top == left.top,"the toggle " + left + " " + right + " between the second sight's label " + sight + " and the device's square " + device);
         require(left.right <= right.left && Math.abs((left.left + right.right) / 2 - middle) <= 1,"the toggle's buttons side by side on the slots' middle line " + middle + ": " + left + " " + right);
         require(bounds.containsRect(left) && bounds.containsRect(right),"the toggle inside the window");
      }

      // The weapon kit's record as patch 68 gives it the movie.
      private static function kit(own:int, ownSlot:int, camoDict:int) : Object
      {
         return {id:own,param:3,kit_own:ownSlot,kit_camo:camoDict,kit_muzzle:0,kit_grip:0};
      }

      private function slots() : Array
      {
         return [window.decalSlot,window.decalSlot1,window.decalSlot2,window.camoSlot];
      }

      private function attachmentSlots() : Array
      {
         return [window.camoSlot,window.muzzleSlot,window.gripSlot];
      }

      // The item ids of the Attachments page's slots (scope, muzzle device, grip), 0 empty.
      private function expectAttachments(what:String, ids:Array) : void
      {
         var shown:Array = attachmentSlots().map(function(s:*, i:int, a:Array):int
         {
            return s.data ? int(s.data.itemId) : 0;
         });
         trace("WEAPONKIT " + what + ": scope, muzzle, grip hold " + shown.join(", "));
         require(shown.join(",") == ids.join(","),what + ": " + shown.join(",") + ", not " + ids.join(","));
      }

      // The Attachments page with all three: the scope's, the device's and the grip's slots one
      // under the other in that order, inside the window, none over another.
      private function checkAttachmentLayout() : void
      {
         var bounds:Rectangle = frame();
         var shown:Array = attachmentSlots();
         trace("WEAPONKIT the window's frame " + bounds + "; Attachments: " + shown.map(function(s:*, i:int, a:Array):String
         {
            return s.name + " at " + s.x + "," + s.y + " square " + square(s);
         }).join("; "));
         for(var i:int = 0; i < shown.length; i++)
         {
            var a:Rectangle = square(shown[i]);
            require(bounds != null && bounds.containsRect(a),"Attachments slot " + i + " " + a + " inside the window " + bounds);
            if(i > 0)
            {
               var above:Rectangle = square(shown[i - 1]);
               require(a.top >= above.bottom && shown[i].x == shown[i - 1].x,"Attachments slot " + i + " " + a + " under slot " + (i - 1) + " " + above);
            }
         }
      }

      // The window opened on an owned item with these upgrade records (the account's), as
      // LobbyMenu.openModifierWindow opens it, on a page (UpgradeWindow.PAGE_*: its right-click's entry)
      // or -1 the window's own.
      private function open(itemId:int, upgrades:Array, page:int = -1) : void
      {
         var data:* = inventory.getInventoryItem(itemId);
         data.upgrades = upgrades;
         if(window.visible && window.getItemId() != 0)
         {
            window.closePopup();
         }
         window.visible = true;
         window.setData(data,true,page);
      }

      // The item ids of the slots (decalSlot, decalSlot1, decalSlot2, camoSlot), 0 empty.
      private function expect(what:String, ids:Array) : void
      {
         var shown:Array = slots().map(function(s:*, i:int, a:Array):int
         {
            return s.data ? int(s.data.itemId) : 0;
         });
         trace("WEAPONKIT " + what + ": slots 0, 1, 2, 3 hold " + shown.join(", "));
         require(shown.join(",") == ids.join(","),what + ": " + shown.join(",") + ", not " + ids.join(","));
      }

      private function turnPage() : void
      {
         var arrow:* = null;
         for(var i:int = 0; i < window.container.numChildren; i++)
         {
            var child:* = window.container.getChildAt(i);
            if(child is C("survarium.upgrade.PageArrow") && child.visible)
            {
               arrow = child;
            }
         }
         require(arrow != null,"a weapon with both pages has its arrows");
         if(arrow != null)
         {
            arrow.dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
         }
      }

      // A drag of an owned item onto a slot as the DragManager makes it: the window's onDrag at its
      // start (the slots it fits light up), the slot's handleDropEvent, the drag's end.
      private function drop(slot:*, itemId:int) : void
      {
         var Drag:Class = C("scaleform.clik.events.DragEvent");
         var dragged:* = inventory.getInventoryItem(itemId);
         window.onDrag(new Drag("dragStart",dragged,null,null,null));
         require(slot.highlighted,slot.name + " lights up for " + dragged.name);
         var taken:Boolean = slot.handleDropEvent(new Drag(Drag.DROP,dragged,null,slot,null));
         window.onDrag(new Drag("dragEnd",dragged,null,slot,null));
         require(taken,slot.name + " takes " + dragged.name);
         require(Boolean(slot.data) && slot.data.itemId == itemId,slot.name + " shows " + dragged.name + " at once");
      }

      // Armour's window: its slots where the movie has them, its decals and camouflage.
      private function checkArmour(what:String) : void
      {
         for each(var s:* in slots())
         {
            var place:Array = [s.x,s.y,s.actualScaleX,s.actualScaleY];
            require(place.join(",") == moviePlaces[s.name].join(","),what + ": " + s.name + " at " + place.join(",") + ", the movie's " + moviePlaces[s.name].join(","));
         }
         require(window.decalSlot.visible && window.decalSlot1.visible && !window.decalSlot2.visible && window.camoSlot.visible,what + ": two decal slots and the camouflage shown");
         expect(what,[DECAL_A_ID,DECAL_B_ID,0,CAMO_HIGH_ID]);
         require(!arrowsShown(),what + ": no page arrows");
      }

      private function arrowsShown() : Boolean
      {
         for(var i:int = 0; i < window.container.numChildren; i++)
         {
            var child:* = window.container.getChildAt(i);
            if(child is C("survarium.upgrade.PageArrow") && child.visible)
            {
               return true;
            }
         }
         return false;
      }

      // The window's frame: the container's art behind the slots (its largest drawn child).
      private function frame() : Rectangle
      {
         var best:Rectangle = null;
         for(var i:int = 0; i < window.container.numChildren; i++)
         {
            var child:DisplayObject = window.container.getChildAt(i);
            if(child is C("scaleform.clik.core.UIComponent") || child is TextField || !child.visible)
            {
               continue;
            }
            var r:Rectangle = child.getBounds(window.container);
            if(best == null || r.width * r.height > best.width * best.height)
            {
               best = r;
            }
         }
         return best;
      }

      // A slot's label (under its square) in the window's coordinates.
      private function label(s:*) : Rectangle
      {
         return s.textFieldLabel.getBounds(window.container);
      }

      // A slot's square (its frame, the border) in the window's coordinates.
      private function square(s:*) : Rectangle
      {
         return s.border.getBounds(window.container);
      }

      // The weapon's Appearance page: both decal slots and the camouflage at distinct places inside
      // the window, none over another.
      private function checkWeaponLayout() : void
      {
         var bounds:Rectangle = frame();
         var shown:Array = [window.decalSlot,window.decalSlot1,window.decalSlot2];
         trace("WEAPONKIT the window's frame " + bounds + "; Appearance: " + shown.map(function(s:*, i:int, a:Array):String
         {
            return s.name + " at " + s.x + "," + s.y + " square " + square(s);
         }).join("; "));
         for(var i:int = 0; i < shown.length; i++)
         {
            var a:Rectangle = square(shown[i]);
            require(bounds != null && bounds.containsRect(a),shown[i].name + " " + a + " inside the window " + bounds);
            for(var j:int = i + 1; j < shown.length; j++)
            {
               var b:Rectangle = square(shown[j]);
               require(!(shown[i].x == shown[j].x && shown[i].y == shown[j].y),shown[i].name + " and " + shown[j].name + " at distinct places");
               require(!a.intersects(b),shown[i].name + " " + a + " over " + shown[j].name + " " + b);
            }
         }
      }

      // One language: the weapon's two pages (empty slots: their labels), a stock-scoped weapon's
      // Attachments, armour's window; each text in its field.
      private function texts(name:String) : void
      {
         var Loc:Object = domain.getDefinition("survarium.data.LocalizationData");
         Loc.setData(text(name));
         widest = null;
         measured = 0;
         open(RIFLE_ID,[]);
         measure(name,"Attachments");
         turnPage();
         measure(name,"Appearance");
         open(STOCK_RIFLE_ID,[]);
         measure(name,"stock scope");
         open(MODULE_RIFLE_ID,[]);
         measure(name,"Attachments with a muzzle device and a grip");
         open(WIDE_RIFLE_ID,[wide(OPTIC_B_ID,6,0,0,0,0)]);
         measure(name,"Attachments with a second sight");
         open(ARMOUR_ID,[]);
         measure(name,"armour");
         trace("WEAPONKIT " + name + ": " + measured + " texts, widest " + widest.what + " \"" + widest.text + "\" " + widest.used + " of " + widest.room + " px");
      }

      private function measure(language:String, page:String) : void
      {
         var c:* = window.container;
         var all:Array = slots().concat([window.secondSightSlot,window.muzzleSlot,window.gripSlot]);
         for each(var s:* in all)
         {
            s.validateNow();
         }
         window.btnAccept.validateNow();
         // the title: inside its field, between the arrows when they are shown
         var title:TextField = c.textField;
         var left:Number = title.x + (title.width - title.textWidth) / 2;
         var right:Number = left + title.textWidth;
         var room:Number = title.width;
         var used:Number = title.textWidth + 4;
         if(arrowsShown())
         {
            var arrows:Array = [];
            for(var i:int = 0; i < c.numChildren; i++)
            {
               if(c.getChildAt(i) is C("survarium.upgrade.PageArrow"))
               {
                  arrows.push(c.getChildAt(i).getBounds(c));
               }
            }
            arrows.sortOn("x",Array.NUMERIC);
            var gap:Number = arrows[1].left - arrows[0].right;
            require(left >= arrows[0].right && right <= arrows[1].left,language + " " + page + ": the title \"" + title.text + "\" (" + title.textWidth + " px) between the arrows (" + gap + " px)");
            room = Math.min(room,gap);
         }
         require(title.numLines == 1,language + " " + page + ": the title \"" + title.text + "\" on one line");
         note(language,page + " title (" + title.defaultTextFormat.size + " pt)",title,used,room);
         for each(s in all)
         {
            if(s.visible && !s.data)
            {
               // the scope slot's labels (camoSlot on a weapon) are the stock movie's texts, wider
               // than the field in most languages before this change too: said, not failed
               fits(language,page + " " + s.name + " label",s.textFieldLabel,s.name == "camoSlot");
            }
         }
         if(window.infoContainer.visible)
         {
            fits(language,page + " hint",window.infoContainer.infoDescr);
         }
         for each(var b:* in window.sideButtons)
         {
            if(b.visible)
            {
               fits(language,page + " " + b.name,b.caption);
            }
         }
         if(window.btnAccept.textField)
         {
            fits(language,page + " button",window.btnAccept.textField);
         }
      }

      // A field's text in it: one line within its width, or its lines within its height.
      private function fits(language:String, what:String, field:TextField, known:Boolean = false) : void
      {
         if(field.text.replace(/\s/g,"") == "")
         {
            require(false,language + " " + what + ": empty");
            return;
         }
         if(field.multiline || field.wordWrap)
         {
            note(language,what + " (" + field.numLines + " lines)",field,field.textHeight + 4,field.height);
            require(field.maxScrollV == 1,language + " " + what + ": \"" + field.text + "\" scrolls (" + field.numLines + " lines)");
         }
         else
         {
            note(language,what,field,field.textWidth + 4,field.width,known);
            require(field.numLines == 1,language + " " + what + ": \"" + field.text + "\" on one line");
         }
      }

      private function note(language:String, what:String, field:TextField, used:Number, room:Number, known:Boolean = false) : void
      {
         if(known && used > room)
         {
            trace("WEAPONKIT KNOWN (stock, not this change) " + language + " " + what + ": \"" + field.text + "\" needs " + used.toFixed(1) + " of " + room.toFixed(1) + " px");
         }
         require(known || used <= room,language + " " + what + ": \"" + field.text + "\" needs " + used.toFixed(1) + " of " + room.toFixed(1) + " px (" + field.getTextFormat().size + " pt, autoSize " + field.autoSize + ")");
         measured++;
         if(widest == null || used / room > widest.used / widest.room)
         {
            widest = {what:what,text:field.text.replace(/[\r\n]+/g," / "),used:used.toFixed(1),room:room.toFixed(1)};
         }
      }

      // The real clicks of case 24, one at a time: what is clicked (its middle), what runs before it
      // (the window opened, a decal slot's click) and what it checks after.
      private var clicks:Array;

      private var clickIndex:int = 0;

      // the frames waited for check.sh's click, -1 none awaited
      private var clickWait:int = -1;

      // A press: the click awaited has come (on the stage itself too, outside the capture phase).
      private function mouseDown(e:MouseEvent) : void
      {
         if(e.eventPhase == 2 && e.target != stage || clickWait < 0)
         {
            return;
         }
         trace("WEAPONKIT mouse down at " + int(e.stageX) + "," + int(e.stageY) + " on " + path(e.target as DisplayObject));
         clickWait = -1;
         wait = 20;
      }

      private function mouseClick(e:MouseEvent) : void
      {
         trace("WEAPONKIT clicked " + path(e.target as DisplayObject));
      }

      private static function path(o:DisplayObject) : String
      {
         var names:Array = [];
         for(; o != null; o = o.parent)
         {
            names.push(o.name);
         }
         return names.join("<");
      }

      // The weapon's Appearance page (its right-click's Appearance) clicked: the list after each click
      // the slot's own kind (a camouflage slot's the camouflages, empty or filled, after a decal
      // slot's click and a decal edit too), a decal's change button its edit, a camouflage's never.
      private function appearanceClicks() : Array
      {
         var filled:Array = [{id:DECAL_A_ID,param:0},{id:CAMO_LOW_ID,param:2}];
         var decalsListed:Function = function():void
         {
            window.decalSlot1.border.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true));
            requireListed("an empty decal slot's click",Category.DECALS);
         };
         return [{what:"the empty camouflage slot",target:function():DisplayObject
         {
            return window.decalSlot2.border;
         },before:function():void
         {
            open(RIFLE_ID,[],Window.PAGE_APPEARANCE);
            require(window.decalSlot2.visible && !window.camoSlot.visible,"the Appearance entry opens on Appearance");
         },after:function():void
         {
            requireListed("the empty camouflage slot clicked",Category.CAMOUFLAGES);
         }},{what:"the empty second decal slot",target:function():DisplayObject
         {
            return window.decalSlot1.border;
         },before:function():void
         {
            open(RIFLE_ID,filled,Window.PAGE_APPEARANCE);
         },after:function():void
         {
            requireListed("the empty decal slot clicked",Category.DECALS);
         }},{what:"the filled camouflage slot's middle, after the decal slot",target:function():DisplayObject
         {
            return window.decalSlot2.border;
         },after:function():void
         {
            requireListed("the filled camouflage slot clicked after a decal slot",Category.CAMOUFLAGES);
            requireNoEdit("the filled camouflage slot clicked");
         }},{what:"the filled camouflage slot's name, after a decal slot",target:function():DisplayObject
         {
            return window.decalSlot2.textField;
         },before:decalsListed,after:function():void
         {
            requireListed("the filled camouflage slot's name clicked after a decal slot",Category.CAMOUFLAGES);
            requireNoEdit("the filled camouflage slot's name clicked");
         }},{what:"the decal's change button",target:function():DisplayObject
         {
            return window.decalSlot.btnChange;
         },after:function():void
         {
            requireCalled("the decal's change button","start_decal_edit " + RIFLE_ID + " " + DECAL_A_ID);
            require(window.decalControl.visible,"the decal's change button: the editor shown");
            window.cancelDecalPosition.dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
            requireCalled("the decal edit's cancel","cancel_decal_edit " + RIFLE_ID + " " + DECAL_A_ID);
            require(!window.decalControl.visible,"the decal edit's cancel: the editor hidden");
         }},{what:"the filled camouflage slot's middle, after a decal edit and a decal slot",target:function():DisplayObject
         {
            return window.decalSlot2.border;
         },before:decalsListed,after:function():void
         {
            requireListed("the filled camouflage slot clicked after a decal edit",Category.CAMOUFLAGES);
            requireNoEdit("the filled camouflage slot clicked after a decal edit");
         }}];
      }

      // The inventory's list: the one filter `category` chosen, nothing but its items listed.
      private function requireListed(what:String, category:int) : void
      {
         var chosen:Array = inventory.optionControl.filterList.getSelectedIndexes().concat();
         var listed:Object = {};
         var others:int = 0;
         var dp:* = inventory.itemsTileList.dataProvider;
         for(var i:int = 0; i < dp.length; i++)
         {
            listed[dp[i].category] = int(listed[dp[i].category]) + 1;
            if(int(dp[i].category) != category)
            {
               others++;
            }
         }
         var counts:Array = [];
         for(var c:String in listed)
         {
            counts.push(c + ": " + listed[c]);
         }
         trace("WEAPONKIT " + what + ": the filters " + chosen.join(",") + " chosen, listed by category " + counts.join(", ") + "; the exe was called " + (external().calls as Array).join("; "));
         require(chosen.join(",") == String(category) && others == 0 && dp.length > 0,what + ": the list shows category " + chosen.join(",") + " (" + counts.join(", ") + "), not " + category);
      }

      private function requireNoEdit(what:String) : void
      {
         var made:String = (external().calls as Array).join("; ");
         require(!window.decalControl.visible && made.indexOf("start_decal_edit") == -1,what + ": no decal edit (" + made + ")");
      }

      private function requireCalled(what:String, call:String) : void
      {
         var made:Array = external().calls as Array;
         trace("WEAPONKIT " + what + ": the exe was called " + made.join("; "));
         require(made.indexOf(call) != -1,what + ": " + call + " not sent (" + made.join("; ") + ")");
      }

      private function require(ok:Boolean, why:String) : void
      {
         if(!ok)
         {
            failures.push(why);
            trace("WEAPONKIT FAIL " + why);
         }
      }

      private function finish(result:String) : void
      {
         trace("WEAPONKIT RESULT " + result);
      }
   }
}
