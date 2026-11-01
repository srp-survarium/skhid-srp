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

   // The buy dialog's starting count (docs/notes/items.md §8), with the lobby movie's own root
   // (try_buy_item, which every buy reaches) and its own buy dialog (MessageAsk): each buy is made
   // with the arguments its caller gives (the card's Buy, the inventory's and the kit's menu Buy,
   // a drop on a pocket), then the dialog's slider and total are read.
   // - a medkit, a grenade, a trap with money for 30 or more: 30, the total 30 x the price;
   // - a medkit with money for 12: 12; with none: the slider at 0 and Buy off, as before;
   // - a medkit dropped on a pocket: 30 too (12 with money for 12): the pocket takes its size, the
   //   rest stays in the inventory (LobbyMenu.relocate_to_paperdoll; not checked here);
   // - a booster (fills a whole pocket), ammo (one clip), a helmet (no slider): as before.
   // Prints "BUY30 ..." lines, "BUY30 SHOT dialog-30" while the medkit's dialog at 30 is shown for
   // a screenshot, and ends with "BUY30 RESULT pass" or "BUY30 RESULT fail: ...".
   public class ConsumablesBuyHarness extends Sprite
   {
      private static const MEDKIT:int = 67;

      private static const GRENADE:int = 5;

      private static const TRAP:int = 68;

      private static const BOOSTER:int = 340;

      private static const AMMO:int = 20;

      private static const HELMET:int = 9103;

      private static const MEDKIT_COST:int = 20;

      private static const AMMO_CLIP:int = 20;

      private static const POCKET_SIZE:int = 5;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var boxes:Sprite;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      private var cases:Array;

      private var current:Object;

      private var inventoryReady:Boolean = false;

      public function ConsumablesBuyHarness()
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

      private function constant(className:String, name:String) : int
      {
         return domain.getDefinition("survarium.constants." + className)[name];
      }

      // The parts a buy uses, as the root's own setup gives them in the game: the icons, the items,
      // the shop, the inventory list, the boxes with the game's buy dialog.
      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("MainScene").stage = lobby;
            domain.getDefinition("survarium.data.LocalizationData").setData({buy:"Buy",cost:"Cost",ok:"OK",cancel:"Cancel",item_piece_postfix:"pcs"});
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["equipment","weapons","camo","consumables","bundles","factionItemIcons"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("consumables-buy-check");
            descriptor.setItemsDictionary([
               item(MEDKIT,"MEDKITS","Medikit",true,false),
               item(GRENADE,"GRENADES","RGD-5",true,false),
               item(TRAP,"BOOBY_TRAPS","Trap",true,false),
               item(BOOSTER,"MEDKITS","Tornado Energy Drink",true,true),
               item(AMMO,"PISTOL_AMMO","9x18",true,false,AMMO_CLIP),
               item(HELMET,"HELMETS","Helmet",false,false)]);
            lobby._itemDescriptor = descriptor;
            at = "the shop";
            var shop:* = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([offer(MEDKIT,MEDKIT_COST),offer(GRENADE,30),offer(TRAP,20),offer(BOOSTER,1500),offer(AMMO,60),offer(HELMET,500)]);
            lobby.shop_list = shop;
            at = "the inventory";
            domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
            var profile:* = new (C("SrvLocalPlayerProfile"))();
            profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
            lobby.player_profile = profile;
            lobby.addChild(profile);
            var card:* = new (C("SrvSelectedItemControl"))();
            lobby.item_control = card;
            var inventory:* = new (C("SrvInventoryReskin"))();
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
            boxes = new Sprite();
            lobby.addChild(boxes);
            Message.initStage(boxes);
            Message.registerMessage("ask",C("survarium.messages.MessageAsk"));
            domain.getDefinition("survarium.controls.PlayerInfo").pocketSize = POCKET_SIZE;
            cases = this.allCases();
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err);
         }
      }

      private function item(dictId:int, category:String, name:String, stack:Boolean, wholePocket:Boolean, clip:int = 0) : Object
      {
         var value:Object = {dictId:dictId,category:constant("ItemCategory",category),name:name,level:1,icon_row:1,icon_column:1,is_stack:stack,fills_whole_pocket:wholePocket,weight:0.2,modifiers:[],item_properties:[]};
         if(clip)
         {
            value.clip_size = clip;
         }
         return value;
      }

      private static function offer(dictId:int, cost:int) : Object
      {
         return {dictId:dictId,faction_id:1,level:0,cost:cost,condition_or_stack:1};
      }

      // The buys and what each dialog must start at: the slider's count (null: no slider), the
      // total shown (null: not checked).
      private function allCases() : Array
      {
         var shop:int = constant("ItemPlace","SHOP");
         var inventory:int = constant("ItemPlace","INVENTORY");
         var pocket:int = constant("ProfileSlot","QUICK_1");
         var none:int = constant("Faction","NONE");
         var plenty:uint = 100000;
         return [
            // the card's Buy (ItemCard: the entry's count, no seller)
            {what:"the card's Buy, a medkit, money for 30 and more",money:plenty,dictId:MEDKIT,target:inventory,count:1,price:MEDKIT_COST,seller:none,count_want:30,total_want:30 * MEDKIT_COST,shot:"dialog-30"},
            // the inventory's and the kit's menu Buy (InventoryList, LobbyMenu: count 1; their seller,
            // Faction.ANY, is looked up by GangShop.getItemSeller, which Ruffle cannot run: a seller here)
            {what:"the menu's Buy, a medkit, money for 30 and more",money:plenty,dictId:MEDKIT,target:inventory,count:1,price:MEDKIT_COST,seller:1,count_want:30,total_want:30 * MEDKIT_COST},
            {what:"the menu's Buy, a medkit, money for 12",money:12 * MEDKIT_COST + MEDKIT_COST - 1,dictId:MEDKIT,target:inventory,count:1,price:MEDKIT_COST,seller:1,count_want:12,total_want:12 * MEDKIT_COST},
            {what:"the menu's Buy, a medkit, no money",money:0,dictId:MEDKIT,target:inventory,count:1,price:MEDKIT_COST,seller:1,count_want:0,total_want:0},
            {what:"the menu's Buy, a grenade",money:plenty,dictId:GRENADE,target:inventory,count:1,price:30,seller:1,count_want:30,total_want:900},
            {what:"the menu's Buy, a trap",money:plenty,dictId:TRAP,target:inventory,count:1,price:20,seller:1,count_want:30,total_want:600},
            // a shop stack dropped on a pocket (ItemPaperDollSlot: count -1)
            {what:"a medkit dropped on a pocket",money:plenty,dictId:MEDKIT,target:pocket,count:-1,price:MEDKIT_COST,seller:1,count_want:30,total_want:30 * MEDKIT_COST},
            {what:"a medkit dropped on a pocket, money for 12",money:12 * MEDKIT_COST,dictId:MEDKIT,target:pocket,count:-1,price:MEDKIT_COST,seller:1,count_want:12,total_want:12 * MEDKIT_COST},
            // as before: one step
            {what:"the menu's Buy, a booster",money:plenty,dictId:BOOSTER,target:inventory,count:1,price:1500,seller:1,count_want:1,total_want:1500},
            {what:"the menu's Buy, ammo",money:plenty,dictId:AMMO,target:inventory,count:1,price:60,seller:1,count_want:AMMO_CLIP,total_want:60},
            {what:"the menu's Buy, a helmet",money:plenty,dictId:HELMET,target:inventory,count:1,price:500,seller:1,count_want:null,total_want:null}];
      }

      // The game's buy dialog on top of the boxes.
      private function dialog() : *
      {
         var Ask:Class = C("survarium.messages.MessageAsk");
         for(var i:int = boxes.numChildren - 1; i >= 0; i--)
         {
            if(boxes.getChildAt(i) is Ask && boxes.getChildAt(i).visible)
            {
               return boxes.getChildAt(i);
            }
         }
         return null;
      }

      private function buy(c:Object) : void
      {
         var PlayerInfo:Object = domain.getDefinition("survarium.controls.PlayerInfo");
         PlayerInfo.genericMoney = c.money;
         PlayerInfo.premiumMoney = 0;
         lobby.try_buy_item(1,0,c.dictId,constant("ItemPlace","SHOP"),c.target,c.count,c.price,c.seller);
      }

      private function check(c:Object) : void
      {
         var ask:* = dialog();
         if(ask == null)
         {
            failures.push(c.what + ": no buy dialog");
            trace("BUY30 " + c.what + ": no buy dialog");
            return;
         }
         // set up now if a slow frame has not drawn it yet (UIComponent.validateNow runs configUI once)
         ask.validateNow();
         // Ruffle leaves the caret after the stepper's text, which NumericStepperSrv.updateLabel takes
         // for a selection and keeps the text, and refuses the stepper's null side label (Scaleform
         // takes it): the count is drawn again with neither
         ask.slider.textField.setSelection(0,0);
         ask.slider.label = "";
         ask.slider.validateNow();
         var count:* = ask.slider.visible ? ask.slider.value : null;
         var label:String = ask.slider.textField.text;
         // the total as the dialog shows it (Util.getMoneyString: digits, maybe grouped)
         var total:String = ask._CountPrice.textField.text;
         trace("BUY30 " + c.what + ": count " + count + " (shown " + label + "), total " + total + ", Buy " + (ask.btnAccept.enabled ? "on" : "off"));
         if(count !== c.count_want)
         {
            failures.push(c.what + ": count " + count + " (want " + c.count_want + ")");
         }
         if(count !== null && label != String(count))
         {
            failures.push(c.what + ": shown " + label + " (count " + count + ")");
         }
         if(c.total_want !== null && int(total.replace(/\D/g,"")) != c.total_want)
         {
            failures.push(c.what + ": total " + total + " (want " + c.total_want + ")");
         }
         if(c.count_want === 0 && ask.btnAccept.enabled)
         {
            failures.push(c.what + ": Buy on with no money");
         }
      }

      // A buy, then its dialog read a few frames later (the dialog sets itself up as it is drawn).
      private function tick(e:Event) : void
      {
         if(++frames < 10 || --wait > 0)
         {
            return;
         }
         wait = 4;
         try
         {
            if(!inventoryReady)
            {
               // an empty inventory (the buy counts what is owned), as the inventory-menu check
               // fills it once the list is drawn
               var inventory:* = lobby.inventory_list;
               inventory.fillSellers([{faction_id:1,name:"Seller"}]);
               inventory.sellersTabPage.selectedIndex = 0;
               inventory.toggleShopMode(false);
               inventory.filterItems(null);
               inventory.optionControl.checkbox_all_traders.selected = true;
               var offers:Array = [];
               lobby.shop_list.forEachItem(function(entry:*):Boolean
               {
                  offers.push(entry);
                  return false;
               });
               inventory.setupShopItems(offers);
               inventory.setupBundleData([]);
               inventory.setupInventoryData([]);
               inventoryReady = true;
               return;
            }
            if(current != null)
            {
               check(current);
               if(current.shot)
               {
                  trace("BUY30 SHOT " + current.shot);
                  wait = 90;
               }
               current = null;
               return;
            }
            if(cases.length == 0)
            {
               removeEventListener(Event.ENTER_FRAME,this.tick);
               finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
               return;
            }
            current = cases.shift();
            buy(current);
         }
         catch(err:Error)
         {
            removeEventListener(Event.ENTER_FRAME,this.tick);
            finish("fail: " + (current ? current.what : "the inventory") + ": " + err);
         }
      }

      private function finish(result:String) : void
      {
         trace("BUY30 RESULT " + result);
      }
   }
}
