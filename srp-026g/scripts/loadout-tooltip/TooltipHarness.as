package
{
   import flash.display.BitmapData;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The kit's item card (LobbyMenu.showPdItemHint) after a weapon is swapped, with the lobby movie's
   // own root, profile, inventory and cards. The kit: a helmet and a rifle. Another rifle of the
   // inventory dropped on the weapon slot (ItemPaperDollSlot.handleDropEvent: the slot shows a copy
   // of the inventory's item until the server answers), the pointer then on the weapon slot, and the
   // server's new kit: the card is the new rifle, not the helmet; the weapon slot's right-click menu
   // is the rifle's; the helmet slot's card is the helmet. A list row's card (LobbyMenu.showItemHint)
   // stays its row's item when the kit changes.
   // The root's own setup (LobbyMenu.init) wires the cards; outside the game it stops at the stage
   // it is not on yet, after the cards' listeners: the rest is given here as the game gives it.
   // Prints "LOADOUT ..." lines, "LOADOUT SHOT <name>" when a capture would show the card, and ends
   // with "LOADOUT RESULT pass" or "LOADOUT RESULT fail: ...".
   public class TooltipHarness extends Sprite
   {
      private static const HELMET:int = 9201;

      private static const RIFLE_OLD:int = 9202;

      private static const RIFLE_NEW:int = 9203;

      // the owned items' ids
      private static const OWNED_HELMET:int = 2;

      private static const OWNED_RIFLE_OLD:int = 4;

      private static const OWNED_RIFLE_NEW:int = 5;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var inventory:*;

      private var profile:*;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      public function TooltipHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function C(name:String) : Class
      {
         return domain.getDefinition(name) as Class;
      }

      private function Slot(name:String) : int
      {
         return domain.getDefinition("survarium.constants.ProfileSlot")[name];
      }

      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("MainScene").scaleFactor = 1;
            domain.getDefinition("survarium.data.LocalizationData").setData({wear:"Wear",drop:"Drop",sell:"Sell",buy:"Buy",non_faction_items:"Any"});
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["equipment","weapons","camo","consumables","bundles","factionItemIcons"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("loadout-tooltip-check");
            descriptor.setItemsDictionary([item(HELMET,Category.HELMETS,"Heavy armored helmet"),item(RIFLE_OLD,Category.ASSAULT_RIFLES,"AK-74M"),item(RIFLE_NEW,Category.ASSAULT_RIFLES,"VSK-94 Guardian")]);
            lobby._itemDescriptor = descriptor;
            at = "the lobby's parts";
            // the root's own: its cards' listeners are on this profile's paper doll
            profile = lobby.player_profile;
            inventory = lobby.inventory_list;
            var shop:* = lobby.shop_list;
            var card:* = lobby.item_control;
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([]);
            inventory.registerItemDescriptor(descriptor);
            card.setInventory(inventory);
            inventory.item_control = card;
            profile.registerInventory(inventory);
            profile.paperDoll.ammoControl = new (C("survarium.ammo.AmmoShareControll"))();
            profile.paperDoll.ammoControl.registerShop(shop);
            profile.paperDoll.ammoControl.registerInventory(inventory);
            profile.registerRelocateFunction(function(moves:Array):void
            {
               trace("LOADOUT relocate " + moves.length);
            });
            // the kit beside the inventory, the cards' layer over them; the root's other windows hidden
            for(var i:int = 0; i < lobby.numChildren; i++)
            {
               lobby.getChildAt(i).visible = lobby.getChildAt(i).name == "stageLayer";
            }
            profile.x = 10;
            profile.y = 30;
            profile.visible = true;
            lobby.addChild(profile);
            inventory.x = 660;
            inventory.y = 30;
            inventory.visible = true;
            lobby.addChild(inventory);
            domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
            domain.getDefinition("survarium.controls.SrvContextMenu").setPatternt(new (C("ContextPattern"))());
            domain.getDefinition("scaleform.clik.managers.DragManager").init(stage,lobby);
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err + " " + err.getStackTrace());
         }
      }

      private static function item(dictId:int, category:int, name:String) : Object
      {
         return {dictId:dictId,category:category,name:name,level:14,icon_row:1,icon_column:1,is_stack:false,modifiers:[],item_properties:[]};
      }

      private static function owned(id:int, dictId:int) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:10000,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0};
      }

      private function kit(rifle:int) : void
      {
         profile.fillProfileItems({profileId:1,items:[{itemId:OWNED_HELMET,slotId:Slot("HEAD"),condition_or_stack:10000},{itemId:rifle,slotId:Slot("WEAPON_1"),condition_or_stack:10000}]});
      }

      private function slot(name:String) : *
      {
         return profile.paperDoll.getSlotAt(Slot(name));
      }

      // The pointer onto / off a kit slot, as Scaleform sends it.
      private function over(name:String) : void
      {
         slot(name).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      }

      private function out(name:String) : void
      {
         slot(name).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      }

      // The item the card shows ("none" when no card is up).
      private function shown() : String
      {
         var hint:* = lobby.itemHintRenderer;
         return hint.parent != null && hint.data != null ? hint.data.name : "none";
      }

      private function expect(what:String, got:String, want:String) : void
      {
         trace("LOADOUT " + what + ": " + got);
         if(got != want)
         {
            failures.push(what + ": " + got + " (want " + want + ")");
         }
      }

      // The comparison card's item ("none" when it is not up).
      private function compared() : String
      {
         var hint:* = lobby.itemHintRendererSec;
         return hint.parent != null && hint.data != null ? hint.data.name : "none";
      }

      // The pointer onto / off an inventory row, as the list sends it.
      private function rowOver(itemId:int) : void
      {
         var r:* = row(itemId);
         var List:Class = C("scaleform.clik.events.ListEvent");
         inventory.itemsTileList.dispatchEvent(new List(List.ITEM_ROLL_OVER,false,true,r.index,-1,-1,r,r.data));
      }

      private function rowOut(itemId:int) : void
      {
         var r:* = row(itemId);
         var List:Class = C("scaleform.clik.events.ListEvent");
         inventory.itemsTileList.dispatchEvent(new List(List.ITEM_ROLL_OUT,false,true,r.index,-1,-1,r,r.data));
      }

      // The inventory list's row of an owned item.
      private function row(itemId:int) : *
      {
         var list:* = inventory.itemsTileList;
         for(var i:int = 0; i < list.dataProvider.length; i++)
         {
            var r:* = list.getRendererAt(i,0);
            if(r != null && r.data != null && r.data.itemId == itemId)
            {
               return r;
            }
         }
         throw new Error("no row of item " + itemId);
      }

      // One step a few frames apart: the lists draw as the frame renders, the cards fade in.
      private function tick(e:Event) : void
      {
         if(++frames < 10 || --wait > 0)
         {
            return;
         }
         wait = 20;
         try
         {
            switch(step++)
            {
               case 0:
                  var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
                  profile.paperDoll.setSlotsRestrictions([{slot_id:Slot("HEAD"),category_id:Category.HELMETS},{slot_id:Slot("WEAPON_1"),category_id:Category.ASSAULT_RIFLES},{slot_id:Slot("WEAPON_2"),category_id:Category.ASSAULT_RIFLES}]);
                  inventory.fillSellers([{faction_id:1,name:"Seller"}]);
                  inventory.sellersTabPage.selectedIndex = 0;
                  inventory.toggleShopMode(false);
                  inventory.filterItems(null);
                  inventory.optionControl.checkbox_all_traders.selected = true;
                  inventory.setupShopItems([]);
                  inventory.setupBundleData([]);
                  inventory.setupInventoryData([owned(OWNED_HELMET,HELMET),owned(OWNED_RIFLE_OLD,RIFLE_OLD),owned(OWNED_RIFLE_NEW,RIFLE_NEW)]);
                  break;
               case 1:
                  kit(OWNED_RIFLE_OLD);
                  break;
               case 2:
                  over("WEAPON_1");
                  expect("the weapon slot's card before the swap",shown(),"AK-74M");
                  out("WEAPON_1");
                  over("HEAD");
                  expect("the helmet slot's card before the swap",shown(),"Heavy armored helmet");
                  out("HEAD");
                  // the new rifle dragged from its row onto the weapon slot and dropped
                  var source:* = row(OWNED_RIFLE_NEW);
                  var target:* = slot("WEAPON_1");
                  profile.paperDoll.selectAgeements(source.data.category,source.data.dictId,source.data.oneHanded,false,source.data.itemId);
                  var Drag:Class = C("scaleform.clik.events.DragEvent");
                  var dropped:Boolean = target.handleDropEvent(new Drag(Drag.DROP,source.data,source,target,null));
                  profile.paperDoll.deselectAgeements();
                  trace("LOADOUT dropped " + dropped + ", the slot shows " + target.data.name);
                  // the pointer is on the weapon slot as the drop ends
                  over("WEAPON_1");
                  expect("the weapon slot's card after the drop",shown(),"VSK-94 Guardian");
                  break;
               case 3:
                  // the server's answer: the new kit
                  kit(OWNED_RIFLE_NEW);
                  break;
               case 4:
                  expect("the weapon slot's card once the kit is the server's",shown(),"VSK-94 Guardian");
                  trace("LOADOUT SHOT weapon-after-swap");
                  wait = 60;
                  break;
               case 5:
                  // a right click on the weapon slot: the menu is its item's
                  var Ex:Class = C("scaleform.gfx.MouseEventEx");
                  var click:* = new Ex("click");
                  click.buttonIdx = 1;
                  slot("WEAPON_1").dispatchEvent(click);
                  var Menu:Object = domain.getDefinition("survarium.controls.SrvContextMenu");
                  expect("the weapon slot's right-click menu",Menu.visisble ? slot("WEAPON_1").data.name : "none","VSK-94 Guardian");
                  if(Menu.visisble)
                  {
                     Menu.hide();
                  }
                  out("WEAPON_1");
                  over("HEAD");
                  break;
               case 6:
                  expect("the helmet slot's card after the swap",shown(),"Heavy armored helmet");
                  trace("LOADOUT SHOT helmet-after-swap");
                  wait = 60;
                  break;
               case 7:
                  out("HEAD");
                  // a list row's card, compared with the kit's rifle; then the kit comes again
                  rowOver(OWNED_RIFLE_OLD);
                  expect("the old rifle's row card",shown() + " | " + compared(),"AK-74M | VSK-94 Guardian");
                  kit(OWNED_RIFLE_NEW);
                  break;
               case 8:
                  expect("the old rifle's row card once the kit comes again",shown() + " | " + compared(),"AK-74M | VSK-94 Guardian");
                  rowOut(OWNED_RIFLE_OLD);
                  // the same with Compare items off: the row's card alone
                  C("survarium.controls.InventoryList").AllOW_COMPARE = false;
                  rowOver(OWNED_RIFLE_OLD);
                  expect("the old rifle's row card, no comparison",shown() + " | " + compared(),"AK-74M | none");
                  kit(OWNED_RIFLE_NEW);
                  break;
               case 9:
                  expect("the old rifle's row card, no comparison, once the kit comes again",shown() + " | " + compared(),"AK-74M | none");
                  trace("LOADOUT SHOT row-after-kit");
                  wait = 60;
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
            throw err;
         }
      }

      private function finish(result:String) : void
      {
         trace("LOADOUT RESULT " + result);
      }
   }
}
