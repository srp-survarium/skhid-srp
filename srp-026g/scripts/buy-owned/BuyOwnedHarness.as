package
{
   import flash.display.BitmapData;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // Every way the lobby buys, with the lobby movie's own root (its try_buy_item, which they all
   // reach: GangShop.buyFunction, MainScene.buyFunction), inventory list, item card and shop
   // (docs/notes/items.md §6). An item owned already, not a stack, is asked about first (the box
   // "apply", already_in_inventory), but for Buy from the right-click menu of an item the player
   // owns; anything else opens the buy dialog ("ask") at once:
   // - not asked: the right-click menu's Buy on an owned camouflage's row, on an owned scope's
   //   row (the inventory tab lists the owned row in place of the shop's entry) and on the kit's
   //   helmet;
   // - asked: the item card's Buy on the owned scope (the list's double click too) and on the
   //   shop's entry of the owned camouflage; the right-click menu's Buy on the shop tab's entry of
   //   the owned scope;
   // - (not here: the craft and attachment windows' menus' Buy, InventoryList.buyFromCraftMenu. It
   //   makes the card's call with the item's own place, INVENTORY for an owned one, but with no
   //   seller, so try_buy_item asks GangShop.getItemSeller, which reads ItemData.seller: Scaleform
   //   gives undefined, Ruffle throws);
   // - a shop helmet dropped on the kit's head slot, a helmet owned (ItemPaperDollSlot's call);
   // - the menu's Buy on a camouflage not owned, and on a bundle whose id is an owned item's.
   // The game's boxes call the exe as they are made (ExternalInterface, which Ruffle lacks), so the
   // box each Buy opens is told by a stand-in registered for its type (BoxSeen).
   // Prints "BUYOWNED ..." lines and ends with "BUYOWNED RESULT pass" or "BUYOWNED RESULT fail: ...".
   public class BuyOwnedHarness extends Sprite
   {
      private static const CAMO:int = 9101;

      private static const OTHER_CAMO:int = 9102;

      private static const HELMET:int = 9103;

      private static const SCOPE:int = 9104;

      // owned items' ids
      private static const OWNED_CAMO:int = 1;

      private static const OWNED_HELMET:int = 2;

      private static const OWNED_SCOPE:int = 3;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var inventory:*;

      private var card:*;

      private var shop:*;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      public function BuyOwnedHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): its parts are given below
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

      // The parts the buys use, as the root's own setup gives them in the game: the icons, the
      // items, the shop, the inventory list and card (as the inventory-menu check makes them), the
      // boxes, the buy function.
      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("MainScene").stage = lobby;
            domain.getDefinition("survarium.data.LocalizationData").setData({buy:"Buy",sell:"Sell",wear:"Wear",drop:"Drop",fix:"Repair",non_faction_items:"Any"});
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["equipment","camo","consumables","bundles","factionItemIcons"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("buy-owned-check");
            descriptor.setItemsDictionary([item(CAMO,Category.CAMOUFLAGES,"Camo"),item(OTHER_CAMO,Category.CAMOUFLAGES,"Other camo"),item(HELMET,Category.HELMETS,"Helmet"),item(SCOPE,Category.SCOPES,"Scope")]);
            lobby._itemDescriptor = descriptor;
            at = "the shop";
            var Shop:Class = C("survarium.controls.GangShop");
            shop = new Shop();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([offer(CAMO),offer(OTHER_CAMO),offer(HELMET),offer(SCOPE)]);
            lobby.shop_list = shop;
            Shop.buyFunction = lobby.try_buy_item;
            domain.getDefinition("MainScene").buyFunction = lobby.try_buy_item;
            at = "the inventory";
            domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
            var profile:* = new (C("SrvLocalPlayerProfile"))();
            profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
            lobby.player_profile = profile;
            lobby.addChild(profile);
            card = new (C("SrvSelectedItemControl"))();
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
            at = "the boxes";
            var Message:* = C("survarium.controls.Message");
            var boxes:Sprite = new Sprite();
            lobby.addChild(boxes);
            Message.initStage(boxes);
            Message.registerMessage("apply",BoxSeenApply);
            Message.registerMessage("ask",BoxSeenAsk);
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err);
         }
      }

      private static function item(dictId:int, category:int, name:String) : Object
      {
         return {dictId:dictId,category:category,name:name,level:1,icon_row:1,icon_column:1,is_stack:false,modifiers:[],item_properties:[]};
      }

      private static function offer(dictId:int) : Object
      {
         return {dictId:dictId,faction_id:0,level:0,cost:100,condition_or_stack:10000};
      }

      private static function owned(id:int, dictId:int) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:10000,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0};
      }

      // A buy and the box it opens: "apply" (asked first), "ask" (the buy dialog) or "none".
      private function expect(what:String, want:String, buy:Function) : void
      {
         BoxSeen.seen.length = 0;
         var threw:String = "none";
         try
         {
            buy();
         }
         catch(err:Error)
         {
            // after a box: the stand-in is no box of the game's, Message stops there
            threw = String(err);
         }
         var got:String = BoxSeen.seen.join(",") || threw;
         trace("BUYOWNED " + what + ": " + got);
         if(got != want)
         {
            failures.push(what + ": " + got + " (want " + want + ")");
         }
      }

      private function rows() : Array
      {
         var list:* = inventory.itemsTileList;
         var out:Array = [];
         for(var i:int = 0; i < list.dataProvider.length; i++)
         {
            var r:* = list.getRendererAt(i,0);
            if(r != null && r.data != null)
            {
               out.push(r);
            }
         }
         return out;
      }

      // The list's row whose item matches.
      private function row(match:Function, what:String) : *
      {
         for each(var r:* in rows())
         {
            if(match(r.data))
            {
               return r;
            }
         }
         throw new Error("no row of " + what + ": " + rows().map(function(r:*, i:int, a:Array):String
         {
            return r.data.name + "#" + r.data.itemId + "@" + r.data.descriptor;
         }).join(", "));
      }

      private function ownedRow(itemId:int) : *
      {
         return row(function(d:*):Boolean
         {
            return d.itemId == itemId && d.descriptor == ItemPlace("INVENTORY");
         },"item " + itemId);
      }

      private function shopRow(dictId:int) : *
      {
         return row(function(d:*):Boolean
         {
            return d.dictId == dictId && d.descriptor == ItemPlace("SHOP");
         },"shop entry " + dictId);
      }

      private function shopEntry(dictId:int) : *
      {
         var found:* = null;
         shop.forEachItem(function(entry:*):Boolean
         {
            found = entry.dictId == dictId ? entry : found;
            return found != null;
         });
         return found;
      }

      private function ItemPlace(name:String) : int
      {
         return domain.getDefinition("survarium.constants.ItemPlace")[name];
      }

      // A right click on a row, as the inventory-menu check gives it.
      private function rightClick(renderer:*) : void
      {
         var Ex:Class = C("scaleform.gfx.MouseEventEx");
         for each(var type:String in ["mouseDown","click"])
         {
            var e:* = new Ex(type);
            e.buttonIdx = 1;
            renderer.dispatchEvent(e);
         }
         inventory.showContextMenu(renderer);
      }

      private function Slot(name:String) : int
      {
         return domain.getDefinition("survarium.constants.ProfileSlot")[name];
      }

      // A right click on a kit slot, as Scaleform sends it (LocalPaperDoll opens the list's menu for
      // the slot's item), as the inventory-menu check gives it.
      private function kitClick(slotId:int) : void
      {
         var slot:* = lobby.player_profile.paperDoll.getSlotAt(slotId);
         if(slot == null || slot.data == null)
         {
            throw new Error("no kit item in slot " + slotId);
         }
         var e:* = new (C("scaleform.gfx.MouseEventEx"))("click");
         e.buttonIdx = 1;
         slot.dispatchEvent(e);
      }

      // Buy in the open right-click menu.
      private function chooseBuy() : void
      {
         var Menu:Object = domain.getDefinition("survarium.controls.SrvContextMenu");
         var shown:* = Menu.visisble ? Menu.getInstance().contextList.list.dataProvider : null;
         for(var i:int = 0; shown != null && i < shown.length; i++)
         {
            if(shown[i].name == "Buy")
            {
               Menu.hide();
               shown[i].click_function();
               return;
            }
         }
         Menu.hide();
         throw new Error("no Buy in the menu");
      }

      // The item card's Buy on an item (the list's double click and the menu's Buy press it too).
      private function cardBuy(value:*) : void
      {
         inventory.setPrimaryItem(value);
         card.onBtnClick1();
      }

      // One step a few frames apart: the list draws its rows as the frame renders.
      private function tick(e:Event) : void
      {
         if(++frames < 10 || --wait > 0)
         {
            return;
         }
         wait = 5;
         try
         {
            switch(step++)
            {
               case 0:
                  // the inventory tab, every seller's items: the owned camouflage, helmet and
                  // scope; the shop's entries of what is not owned; a bundle whose id is the helmet's
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
                  inventory.setupBundleData([{bundle_id:HELMET,bundle_icon:0,bundle_name:"Bundle",bundle_descr:"",bundle_properties:[],currency_type:false,cost:500,bundle_items:[]}]);
                  inventory.setupInventoryData([owned(OWNED_CAMO,CAMO),owned(OWNED_HELMET,HELMET),owned(OWNED_SCOPE,SCOPE)]);
                  break;
               case 1:
                  // the kit: the owned helmet on its head
                  lobby.player_profile.paperDoll.setSlotsRestrictions([{slot_id:Slot("HEAD"),category_id:domain.getDefinition("survarium.constants.ItemCategory").HELMETS}]);
                  lobby.player_profile.fillProfileItems({profileId:1,items:[{itemId:OWNED_HELMET,slotId:Slot("HEAD"),condition_or_stack:10000}]});
                  break;
               case 2:
                  expect("the menu's Buy, the owned camouflage's row","ask",function():void
                  {
                     rightClick(ownedRow(OWNED_CAMO));
                     chooseBuy();
                  });
                  expect("the menu's Buy, the owned scope's row","ask",function():void
                  {
                     rightClick(ownedRow(OWNED_SCOPE));
                     chooseBuy();
                  });
                  expect("the card's Buy, the owned scope","apply",function():void
                  {
                     cardBuy(ownedRow(OWNED_SCOPE).data);
                  });
                  expect("the card's Buy, the shop's entry of the owned camouflage","apply",function():void
                  {
                     cardBuy(shopEntry(CAMO));
                  });
                  expect("a shop helmet dropped on the head slot, a helmet owned","apply",function():void
                  {
                     var entry:* = shopEntry(HELMET);
                     domain.getDefinition("MainScene").buyFunction(1,entry.itemId,HELMET,entry.descriptor,Slot("HEAD"),entry.count,entry.cost,entry.factionId);
                  });
                  // last: the kit's menu shows its item's group of the list
                  expect("the menu's Buy, the kit's helmet","ask",function():void
                  {
                     kitClick(Slot("HEAD"));
                     chooseBuy();
                  });
                  // the shop tab, every group: the shop's entries (the owned items' too), the bundle
                  inventory.toggleShopMode(true);
                  inventory.inventoryTabPage.selectedIndex = 0;
                  break;
               case 3:
                  trace("BUYOWNED the shop tab's rows: " + rows().map(function(r:*, i:int, a:Array):String
                  {
                     return r.data.name + "@" + r.data.descriptor;
                  }).join(", "));
                  expect("the menu's Buy, the shop's entry of the owned scope","apply",function():void
                  {
                     rightClick(shopRow(SCOPE));
                     chooseBuy();
                  });
                  expect("the menu's Buy, a camouflage not owned","ask",function():void
                  {
                     rightClick(shopRow(OTHER_CAMO));
                     chooseBuy();
                  });
                  expect("the menu's Buy, a bundle whose id is the owned helmet's","ask",function():void
                  {
                     rightClick(row(function(d:*):Boolean
                     {
                        return d.descriptor == ItemPlace("BUNDLE");
                     },"the bundle"));
                     chooseBuy();
                  });
                  break;
               default:
                  removeEventListener(Event.ENTER_FRAME,this.tick);
                  finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
            }
         }
         catch(err:Error)
         {
            removeEventListener(Event.ENTER_FRAME,this.tick);
            finish("fail: step " + (step - 1) + ": " + err);
         }
      }

      private function finish(result:String) : void
      {
         trace("BUYOWNED RESULT " + result);
      }
   }
}

// The box types Message.showMessage was asked for, in order.
class BoxSeen
{
   public static var seen:Array = [];
}

class BoxSeenApply
{
   public function BoxSeenApply()
   {
      BoxSeen.seen.push("apply");
   }
}

class BoxSeenAsk
{
   public function BoxSeenAsk()
   {
      BoxSeen.seen.push("ask");
   }
}
