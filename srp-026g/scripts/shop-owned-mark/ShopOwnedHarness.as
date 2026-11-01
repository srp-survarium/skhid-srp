package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.utils.getQualifiedClassName;

   // The shop's owned mark (docs/notes/shop-owned-mark.md) with the lobby movie's own inventory list
   // and shop: a shop entry of an item the account owns has the check mark, the others not.
   // - owned: the rifle, the medkit (a stack of 3); not owned: the helmet, the bandages (a stack of 0);
   // - a bundle whose id is the rifle's dictionary id: no mark (a bundle's id is not an item's);
   // - the helmet bought (the account comes again, as the server sends it after a buy): marked;
   // - the rifle sold: unmarked.
   // Prints "SHOPMARK ..." lines; "SHOPMARK SHOT <name>" when the list is drawn for a screenshot
   // (held a while); ends with "SHOPMARK RESULT pass" or "SHOPMARK RESULT fail: ...".
   public class ShopOwnedHarness extends Sprite
   {
      private static const RIFLE:int = 9001;

      private static const MEDKIT:int = 9002;

      private static const HELMET:int = 9003;

      private static const BANDAGE:int = 9004;

      // ItemRendererInv's OWNED_X / OWNED_Y.
      private static const MARK_X:Number = 158;

      private static const MARK_Y:Number = 6;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var shop:*;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      public function ShopOwnedHarness()
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
               trace("SHOPMARK error " + e.error);
            }
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function C(name:String) : Class
      {
         return domain.getDefinition(name) as Class;
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
            domain.getDefinition("survarium.data.LocalizationData").setData({buy:"Buy",sell:"Sell",shop:"Shop",non_faction_items:"Any"});
            at = "the icons";
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["weapons","equipment","consumables","factionItemIcons","bundles"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("shop-owned-mark-check");
            descriptor.setItemsDictionary([item(RIFLE,Category("ASSAULT_RIFLES"),"Rifle",false),item(MEDKIT,Category("MEDKITS"),"Medkit",true),item(HELMET,Category("HELMETS"),"Helmet",false),item(BANDAGE,Category("MEDKITS"),"Bandage",true)]);
            at = "the shop";
            var Shop:Class = C("survarium.controls.GangShop");
            shop = new Shop();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([offer(RIFLE),offer(MEDKIT),offer(HELMET),offer(BANDAGE)]);
            at = "the lobby";
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

      private static function offer(dictId:int) : Object
      {
         return {dictId:dictId,faction_id:0,level:0,cost:100,condition_or_stack:10000};
      }

      private static function owned(id:int, dictId:int, value:int) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:value,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0};
      }

      // One step a few frames apart: the list draws its rows when the frame renders, as in the game.
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
                  // The shop, every seller's items, the account owning the rifle and 3 medkits
                  // (and an empty stack of bandages).
                  inventory.fillSellers([]);
                  inventory.sellersTabPage.selectedIndex = 0;
                  inventory.filterItems(null);
                  inventory.optionControl.checkbox_all_traders.selected = true;
                  var offers:Array = [];
                  shop.forEachItem(function(entry:*):Boolean
                  {
                     offers.push(entry);
                     return false;
                  });
                  inventory.setupShopItems(offers);
                  inventory.setupBundleData([{bundle_id:RIFLE,bundle_icon:0,bundle_name:"Bundle",bundle_descr:"",bundle_properties:[],currency_type:false,cost:500,bundle_items:[]}]);
                  inventory.setupInventoryData([owned(11,RIFLE,10000),owned(12,MEDKIT,3),owned(15,BANDAGE,0)]);
                  inventory.toggleShopMode(true);
                  inventory.inventoryTabPage.selectedIndex = 0;
                  break;
               case 1:
                  trace("SHOPMARK the shop's rows: " + names());
                  expect(RIFLE,true,"the owned rifle");
                  expect(MEDKIT,true,"the owned medkits (a stack)");
                  expect(HELMET,false,"the helmet not owned");
                  expect(BANDAGE,false,"the bandages, an empty stack");
                  require(bundleRow() != null && !marked(bundleRow()),"the bundle whose id is the rifle's has no mark");
                  shot("before");
                  break;
               case 2:
                  // The helmet bought: the server sends the account again.
                  inventory.setupInventoryData([owned(11,RIFLE,10000),owned(12,MEDKIT,3),owned(15,BANDAGE,0),owned(13,HELMET,10000)]);
                  break;
               case 3:
                  expect(HELMET,true,"the helmet just bought");
                  expect(RIFLE,true,"the rifle still owned");
                  shot("bought");
                  break;
               case 4:
                  // The rifle sold.
                  inventory.setupInventoryData([owned(12,MEDKIT,3),owned(15,BANDAGE,0),owned(13,HELMET,10000)]);
                  break;
               case 5:
                  expect(RIFLE,false,"the rifle just sold");
                  expect(HELMET,true,"the helmet still owned");
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

      // The list held as it is for a screenshot (check.sh takes it on this line).
      private function shot(name:String) : void
      {
         trace("SHOPMARK SHOT " + name);
         wait = 120;
      }

      private function require(ok:Boolean, why:String) : void
      {
         trace("SHOPMARK " + (ok ? "ok " : "FAIL ") + why);
         if(!ok)
         {
            failures.push(why);
         }
      }

      private function expect(dictId:int, mark:Boolean, what:String) : void
      {
         require(marked(shopRow(dictId)) == mark,what + (mark ? " has the mark" : " has no mark"));
      }

      // Whether a row's tile shows the owned mark: its Shape at the mark's corner, visible.
      private function marked(row:*) : Boolean
      {
         var tile:* = row.itemRenderer;
         for(var i:int = 0; i < tile.numChildren; i++)
         {
            var child:DisplayObject = tile.getChildAt(i);
            if(getQualifiedClassName(child) == "flash.display::Shape" && child.x == MARK_X && child.y == MARK_Y && child.visible)
            {
               return true;
            }
         }
         return false;
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
            return r.data.name + "#" + r.data.dictId + "@" + r.data.descriptor + (marked(r) ? " (owned)" : "");
         }).join(", ");
      }

      // The shop list's entry of an item, by its dictionary id.
      private function shopRow(dictId:int) : *
      {
         var Place:Object = domain.getDefinition("survarium.constants.ItemPlace");
         for each(var r:* in rows())
         {
            if(r.data.dictId == dictId && r.data.descriptor == Place.SHOP)
            {
               return r;
            }
         }
         throw new Error("no shop row for item " + dictId + ": " + names());
      }

      private function bundleRow() : *
      {
         var Place:Object = domain.getDefinition("survarium.constants.ItemPlace");
         for each(var r:* in rows())
         {
            if(r.data.descriptor == Place.BUNDLE)
            {
               return r;
            }
         }
         return null;
      }

      private function finish(result:String) : void
      {
         trace("SHOPMARK RESULT " + result);
      }
   }
}
