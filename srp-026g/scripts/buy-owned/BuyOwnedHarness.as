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

   // The shop's Buy of an item owned already (LobbyMenu.try_buy_item, docs/notes/items.md §6) with
   // the lobby movie's own root, its inventory list and shop:
   // - an owned camouflage: the box that asks first ("apply", already_in_inventory);
   // - a camouflage not owned: the buy dialog ("ask") at once;
   // - an owned helmet: the box that asks first, as before.
   // The game's boxes call the exe as they are made (ExternalInterface, which Ruffle lacks), so the
   // box each Buy opens is told by a stand-in registered for its type (BoxSeen).
   // Prints "BUYOWNED ..." lines and ends with "BUYOWNED RESULT pass" or "BUYOWNED RESULT fail: ...".
   public class BuyOwnedHarness extends Sprite
   {
      private static const CAMO:int = 9101;

      private static const OTHER_CAMO:int = 9102;

      private static const HELMET:int = 9103;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var inventory:*;

      private var shop:*;

      private var failures:Array = [];

      private var frames:int = 0;

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

      // The parts try_buy_item uses, as the root's own setup gives them in the game: the icons, the
      // items, the shop, the inventory list (as the inventory-menu check makes it), the boxes.
      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("MainScene").stage = lobby;
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["equipment","camo","factionItemIcons"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("buy-owned-check");
            descriptor.setItemsDictionary([item(CAMO,Category.CAMOUFLAGES,"Camo"),item(OTHER_CAMO,Category.CAMOUFLAGES,"Other camo"),item(HELMET,Category.HELMETS,"Helmet")]);
            lobby._itemDescriptor = descriptor;
            at = "the shop";
            shop = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([offer(CAMO),offer(OTHER_CAMO),offer(HELMET)]);
            lobby.shop_list = shop;
            at = "the inventory";
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

      // The shop's Buy of dictId (GangShop.buyFunction) and the box it opens.
      private function expect(what:String, dictId:int, want:String) : void
      {
         BoxSeen.seen.length = 0;
         var Place:Object = domain.getDefinition("survarium.constants.ItemPlace");
         try
         {
            lobby.try_buy_item(1,0,dictId,Place.SHOP,Place.INVENTORY,1,100,0);
         }
         catch(err:Error)
         {
            // the stand-in is no box of the game's: Message stops there
         }
         var got:String = BoxSeen.seen.join(",") || "none";
         trace("BUYOWNED " + what + ": " + got);
         if(got != want)
         {
            failures.push(what + ": " + got + " (want " + want + ")");
         }
      }

      // The inventory list as the inventory-menu check makes it, a few frames after it is made.
      private function tick(e:Event) : void
      {
         if(++frames < 10)
         {
            return;
         }
         removeEventListener(Event.ENTER_FRAME,this.tick);
         try
         {
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
            inventory.setupInventoryData([owned(1,CAMO),owned(2,HELMET)]);
         }
         catch(err:Error)
         {
            finish("fail: the inventory: " + err);
            return;
         }
         expect("an owned camouflage",CAMO,"apply");
         expect("a camouflage not owned",OTHER_CAMO,"ask");
         expect("an owned helmet",HELMET,"apply");
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
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
