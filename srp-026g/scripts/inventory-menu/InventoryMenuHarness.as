package
{
   import flash.display.BitmapData;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The right-click menu's Buy (InventoryList.showContextMenu, docs/notes/shop-character-menu.md) with
   // the lobby movie's own inventory list, item card, character menu and shop: Buy buys the item
   // the menu was opened on, whatever the list has selected.
   // - a row of the inventory list;
   // - the kit's medkit and rifle in the inventory (the character menu selects the item's row);
   // - the kit's rifle in the shop, whose list holds the shop's entries only, none selected (the
   //   character menu shows there too: shop-character-menu.md);
   // - a shop entry locked at its seller, the item sold unlocked by another seller (the menu lists
   //   Buy for an item the shop sells at any seller).
   // Each buy is caught at GangShop.buyFunction (LobbyMenu.try_buy_item in the game, which opens
   // the buy dialog). Prints "INVMENU ..." lines and ends with "INVMENU RESULT pass" or
   // "INVMENU RESULT fail: ...".
   public class InventoryMenuHarness extends Sprite
   {
      private static const RIFLE:int = 9001;

      private static const MEDKIT:int = 9002;

      private static const HELMET:int = 9003;

      // The rifle's second seller, whose entry the account has not unlocked.
      private static const LOCKED_SELLER:int = 1;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var card:*;

      private var shop:*;

      private var bought:Array = [];

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      public function InventoryMenuHarness()
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
               trace("INVMENU error " + e.error);
            }
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

      private function Category(name:String) : int
      {
         return domain.getDefinition("survarium.constants.ItemCategory")[name];
      }

      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            domain.getDefinition("MainScene").stage = this;
            domain.getDefinition("survarium.data.LocalizationData").setData({buy:"Buy",sell:"Sell",wear:"Wear",drop:"Drop",fix:"Repair",non_faction_items:"Any"});
            at = "the icons";
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["weapons","equipment","consumables","factionItemIcons"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("inventory-menu-check");
            descriptor.setItemsDictionary([item(RIFLE,Category("ASSAULT_RIFLES"),"Rifle",false),item(MEDKIT,Category("MEDKITS"),"Medkit",true),item(HELMET,Category("HELMETS"),"Helmet",false)]);
            at = "the shop";
            var Shop:Class = C("survarium.controls.GangShop");
            shop = new Shop();
            shop.registerItemDescriptor(descriptor);
            // Each item at seller 0 (always unlocked), the rifle also at a seller's level 5.
            shop.setupShopItems([offer(RIFLE,0,0),offer(MEDKIT,0,0),offer(HELMET,0,0),offer(RIFLE,LOCKED_SELLER,5)]);
            Shop.buyFunction = this.buy;
            at = "the lobby";
            // The lobby's root as the list and the card find it (parent["player_profile"]).
            lobby = new MovieClip();
            addChild(lobby);
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
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err);
         }
      }

      private static function item(dictId:int, category:int, name:String, stack:Boolean) : Object
      {
         return {dictId:dictId,category:category,name:name,level:1,icon_row:1,icon_column:1,is_stack:stack,modifiers:[],item_properties:[]};
      }

      private static function offer(dictId:int, seller:int, level:int) : Object
      {
         return {dictId:dictId,faction_id:seller,level:level,cost:100,condition_or_stack:10000};
      }

      private static function owned(id:int, dictId:int, value:int) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:value,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0};
      }

      private function buy(... args) : Boolean
      {
         trace("INVMENU buy: dict " + args[2] + ", from slot " + args[3] + ", price " + args[6]);
         bought.push(args[2]);
         return true;
      }

      // One step a few frames apart: the list applies a selection and draws its rows when the
      // frame renders, as in the game.
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
                  // The inventory tab, every seller's items: the rifle, a medkit, a helmet.
                  inventory.fillSellers([{faction_id:LOCKED_SELLER,name:"Seller"}]);
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
                  inventory.setupInventoryData([owned(11,RIFLE,10000),owned(12,MEDKIT,3),owned(13,HELMET,10000)]);
                  break;
               case 1:
                  // The kit: the rifle in hand, the helmet on, the medkit in the first pocket.
                  lobby.player_profile.paperDoll.setSlotsRestrictions([{slot_id:Slot("HEAD"),category_id:Category("HELMETS")},{slot_id:Slot("WEAPON_1"),category_id:Category("ASSAULT_RIFLES")},{slot_id:Slot("QUICK_1"),category_id:Category("MEDKITS")}]);
                  lobby.player_profile.fillProfileItems({profileId:1,items:[{itemId:11,slotId:Slot("WEAPON_1"),condition_or_stack:10000},{itemId:13,slotId:Slot("HEAD"),condition_or_stack:10000},{itemId:12,slotId:Slot("QUICK_1"),condition_or_stack:3}]});
                  trace("INVMENU the inventory's rows: " + names());
                  break;
               case 2:
                  rightClick(row(13));
                  break;
               case 3:
                  choose(HELMET,"the helmet's row");
                  kitClick(Slot("QUICK_1"));
                  break;
               case 4:
                  choose(MEDKIT,"the kit's medkit");
                  kitClick(Slot("WEAPON_1"));
                  break;
               case 5:
                  choose(RIFLE,"the kit's rifle");
                  // The shop: its list holds the shop's entries, none selected.
                  inventory.toggleShopMode(true);
                  inventory.itemsTileList.selectedIndex = -1;
                  break;
               case 6:
                  // The kit's rifle there (the character menu: LocalPaperDoll puts it on the card and
                  // opens the list's menu on its slot); the shop's list has no row for it.
                  kitClick(Slot("WEAPON_1"));
                  break;
               case 7:
                  choose(RIFLE,"the kit's rifle in the shop");
                  // The locked seller's items only: the rifle's entry there is locked, the rifle
                  // sold unlocked at seller 0.
                  inventory.optionControl.checkbox_all_traders.selected = false;
                  inventory.sellersTabPage.selectedIndex = 1;
                  break;
               case 8:
                  var entry:* = shopRow(RIFLE);
                  require(entry.data.canBuy == false,"the rifle's entry is locked at its seller");
                  rightClick(entry);
                  break;
               case 9:
                  choose(RIFLE,"the rifle's entry locked at its seller");
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

      private function require(ok:Boolean, why:String) : void
      {
         if(!ok)
         {
            failures.push(why);
            trace("INVMENU FAIL " + why);
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

      private function names() : String
      {
         return rows().map(function(r:*, i:int, a:Array):String
         {
            return r.data.name + "#" + r.data.itemId;
         }).join(", ");
      }

      // The list's row of an owned item, by its id.
      private function row(itemId:int) : *
      {
         for each(var r:* in rows())
         {
            if(r.data.itemId == itemId)
            {
               return r;
            }
         }
         throw new Error("no row for item " + itemId);
      }

      // The shop list's row of an item, by its dictionary id.
      private function shopRow(dictId:int) : *
      {
         for each(var r:* in rows())
         {
            if(r.data.dictId == dictId)
            {
               return r;
            }
         }
         throw new Error("no shop row for item " + dictId);
      }

      // A right click on a row: its press and release as Scaleform sends them to it
      // (InventoryItem: its ButtonEvent, the list selects the row), then the list's menu for it
      // (InventoryList.onMouseItemClick, which the list's CLICK reaches in the game).
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

      // A right click on a kit slot, as Scaleform sends it to the slot (LocalPaperDoll: it
      // selects the item in the list, puts it on the card and opens the list's menu for the slot).
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

      // Chooses Buy in the open right-click menu: it must buy `dictId`.
      private function choose(dictId:int, what:String) : void
      {
         var Menu:Object = domain.getDefinition("survarium.controls.SrvContextMenu");
         var shown:* = Menu.visisble ? Menu.getInstance().contextList.list.dataProvider : null;
         var options:Array = [];
         var chosen:Object = null;
         for(var i:int = 0; shown != null && i < shown.length; i++)
         {
            options.push(shown[i].name);
            if(shown[i].name == "Buy")
            {
               chosen = shown[i];
            }
         }
         trace("INVMENU the menu of " + what + ": " + options.join(", ") + "; the list selects " + selected() + ", the card shows " + cardName());
         if(chosen == null)
         {
            require(false,what + ": no Buy");
            return;
         }
         bought.length = 0;
         Menu.hide();
         try
         {
            chosen.click_function();
         }
         catch(err:Error)
         {
            trace("INVMENU Buy threw " + err);
         }
         require(bought.length == 1 && bought[0] == dictId,what + ": Buy bought [" + bought.join(",") + "], not " + dictId);
      }

      private function selected() : String
      {
         var list:* = inventory.itemsTileList;
         var at:* = list.selectedIndex >= 0 && list.selectedIndex < list.dataProvider.length ? list.dataProvider[list.selectedIndex] : null;
         return at == null ? "nothing" : at.name + "#" + at.itemId;
      }

      private function cardName() : String
      {
         var shown:* = card.getItemData();
         return shown == null ? "nothing" : shown.name + "#" + shown.itemId;
      }

      private function finish(result:String) : void
      {
         trace("INVMENU RESULT " + result);
      }
   }
}
