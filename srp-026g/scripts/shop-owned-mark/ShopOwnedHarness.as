package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.text.TextLineMetrics;
   import flash.utils.getQualifiedClassName;

   // The shop's owned mark and its "Hide owned" check box (docs/notes/shop-owned-mark.md) with the
   // lobby movie's own inventory list and shop.
   // The mark: a shop entry of an item the account owns has it, the others not.
   // - owned: the rifle, the medkit (a stack of 3), a scope in the stash, a scope worn on the rifle
   //   (the stash lists it once, attached to the rifle), a camouflage; not owned: the helmet, the
   //   bandages (a stack of 0), a decal;
   // - a bundle whose id is the rifle's dictionary id: no mark (a bundle's id is not an item's);
   // - the helmet bought (the account comes again, as the server sends it after a buy): marked;
   // - the rifle sold: unmarked; the scope it wore stays owned, marked.
   // The box (the compare box's symbol, CheckBoxShared): off at first, labelled; on, the owned items'
   // entries leave the list and the others stay; with the upgrades tab chosen too; the decal bought
   // while it is on leaves the list; off, every entry is back. The last row in every language (the
   // dictionaries' texts, HarnessText): [Comparing items] [Hide owned] Sort [By level] [By price],
   // margin to margin, evenly apart, nothing cut, touching or outside the window.
   // Prints "SHOPMARK ..." lines; "SHOPMARK SHOT <name>" when the list is drawn for a screenshot
   // (held a while); ends with "SHOPMARK RESULT pass" or "SHOPMARK RESULT fail: ...".
   public class ShopOwnedHarness extends Sprite
   {
      private static const RIFLE:int = 9001;

      private static const MEDKIT:int = 9002;

      private static const HELMET:int = 9003;

      private static const BANDAGE:int = 9004;

      // Shop scopes (ItemDictId.SHOP_SCOPE_FIRST..): one in the stash, one worn on the rifle.
      private static const SCOPE:int = 2101;

      private static const SCOPE_WORN:int = 2102;

      private static const CAMO:int = 9005;

      private static const DECAL:int = 9006;

      // The rifle's item id, and the worn scope's.
      private static const RIFLE_ITEM:int = 11;

      private static const SCOPE_WORN_ITEM:int = 16;

      // ItemRendererInv's OWNED_X / OWNED_Y.
      private static const MARK_X:Number = 158;

      private static const MARK_Y:Number = 6;

      // The last row: its smallest gap between two parts (the stock row's closest, the sort's label to
      // its first choice, 12.5), and how far its gaps (and its end from the margin) may differ, by the
      // whole units the row is placed in.
      private static const ROW_GAP:Number = 12;

      private static const ROW_EVEN:Number = 2;

      // A text field's gutters: its text is cut beyond its width less them.
      private static const TEXT_GUTTER:Number = 4;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var shop:*;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      // The language the last row is checked in (HarnessText.LANGUAGES), one after another.
      private var language:int = 0;

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
            at = "the icons";
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["weapons","equipment","consumables","factionItemIcons","bundles","camo","decals","vouchers"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("shop-owned-mark-check");
            descriptor.setItemsDictionary([item(RIFLE,Category("ASSAULT_RIFLES"),"Rifle",false),item(MEDKIT,Category("MEDKITS"),"Medkit",true),item(HELMET,Category("HELMETS"),"Helmet",false),item(BANDAGE,Category("MEDKITS"),"Bandage",true),item(SCOPE,Category("SCOPES"),"Scope",false),item(SCOPE_WORN,Category("SCOPES"),"Worn scope",false),item(CAMO,Category("CAMOUFLAGES"),"Camo",false),item(DECAL,Category("DECALS"),"Decal",false)]);
            at = "the shop";
            var Shop:Class = C("survarium.controls.GangShop");
            shop = new Shop();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([offer(RIFLE),offer(MEDKIT),offer(HELMET),offer(BANDAGE),offer(SCOPE),offer(SCOPE_WORN),offer(CAMO),offer(DECAL)]);
            at = "the lobby";
            lobby = new MovieClip();
            // The whole window on the 720-high stage (the screenshots show the Buy row too).
            lobby.scaleX = lobby.scaleY = 0.78;
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
            // The texts come after the movie is built, as the game sends them (set_localization_data).
            texts("english");
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err);
         }
      }

      // The lobby's texts in `language` as the exe gives them (set_localization_data): srp's and the
      // shop's stock ones from the dictionaries (HarnessText), a few others the shop shows. The game
      // gives them once, before the list shows; the compare box's label (InventoryList.onLocal) and
      // the sort's choices (OptionFilterControl.onLocal) take them then only, so a later language is
      // given to them here as those do.
      private function texts(language:String) : void
      {
         var data:Object = {sell:"Sell",shop:"Shop",non_faction_items:"Any"};
         var key:String;
         for(key in HarnessText.DATA[language])
         {
            data[key] = HarnessText.DATA[language][key];
         }
         for(key in HarnessText.STOCK[language])
         {
            data[key] = HarnessText.STOCK[language][key];
         }
         domain.getDefinition("survarium.data.LocalizationData").setData(data);
         if(inventory != null)
         {
            inventory.checkboxCompareItems.label = data.checkbox_compare_items;
            inventory.sortTab.dataProvider = new (C("scaleform.clik.data.DataProvider"))([data.sort_level,data.sort_cost]);
            inventory.sortTab.selectedIndex = 0;
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

      private static function owned(id:int, dictId:int, value:int, attachedTo:int = 0, upgrades:Array = null) : Object
      {
         return {id:id,dictId:dictId,faction_id:0,condition_or_stack:value,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:attachedTo,upgrades:(upgrades != null ? upgrades : [])};
      }

      // The account's stash at the start: the rifle wearing a scope (its record in slot 3, the scope
      // attached to it), the medkits, an empty stack of bandages, a loose scope and a camouflage.
      private static function stash() : Array
      {
         return [owned(RIFLE_ITEM,RIFLE,10000,0,[{id:SCOPE_WORN_ITEM,dict_id:SCOPE_WORN,value:0,param:3}]),owned(12,MEDKIT,3),owned(15,BANDAGE,0),owned(SCOPE_WORN_ITEM,SCOPE_WORN,10000,RIFLE_ITEM),owned(17,SCOPE,10000),owned(18,CAMO,10000)];
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
                  inventory.setupInventoryData(stash());
                  inventory.toggleShopMode(true);
                  inventory.inventoryTabPage.selectedIndex = 0;
                  break;
               case 1:
                  // Ruffle draws the code-made CLIK parts only at their next change (no RENDER event,
                  // which Scaleform sends): drawn now for the screenshot.
                  toggle().validateNow();
                  trace("SHOPMARK the shop's rows: " + names());
                  expect(RIFLE,true,"the owned rifle");
                  expect(MEDKIT,true,"the owned medkits (a stack)");
                  expect(SCOPE,true,"the scope in the stash");
                  expect(SCOPE_WORN,true,"the scope worn on the rifle");
                  expect(CAMO,true,"the owned camouflage");
                  expect(HELMET,false,"the helmet not owned");
                  expect(BANDAGE,false,"the bandages, an empty stack");
                  expect(DECAL,false,"the decal not owned");
                  require(bundleRow() != null && !marked(bundleRow()),"the bundle whose id is the rifle's has no mark");
                  require(toggle() != null && toggle().visible && !toggle().selected && toggle().label == "Hide owned","the shop shows the hide-owned box, off, labelled");
                  // The tick: the compare box's symbol (Autobuy's, CheckBoxSharedPD, is pixelated).
                  require(getQualifiedClassName(toggle()) == getQualifiedClassName(inventory.checkboxCompareItems),"the box is the compare box's symbol (" + getQualifiedClassName(toggle()) + ")");
                  lastRow("english");
                  shot("before");
                  break;
               case 2:
                  // The helmet bought: the server sends the account again.
                  inventory.setupInventoryData(stash().concat([owned(13,HELMET,10000)]));
                  break;
               case 3:
                  expect(HELMET,true,"the helmet just bought");
                  expect(RIFLE,true,"the rifle still owned");
                  shot("bought");
                  break;
               case 4:
                  // The rifle sold: the scope it wore stays in the stash, on nothing.
                  inventory.setupInventoryData([owned(12,MEDKIT,3),owned(15,BANDAGE,0),owned(SCOPE_WORN_ITEM,SCOPE_WORN,10000),owned(17,SCOPE,10000),owned(18,CAMO,10000),owned(13,HELMET,10000)]);
                  break;
               case 5:
                  expect(RIFLE,false,"the rifle just sold");
                  expect(HELMET,true,"the helmet still owned");
                  expect(SCOPE_WORN,true,"the scope the sold rifle wore");
                  // The toggle on.
                  press();
                  break;
               case 6:
                  trace("SHOPMARK not owned only: " + names());
                  listed([RIFLE,BANDAGE,DECAL],[HELMET,MEDKIT,SCOPE,SCOPE_WORN,CAMO],"the toggle on");
                  require(bundleRow() != null,"the toggle on: the bundle stays");
                  for each(var r:* in rows())
                  {
                     require(!marked(r),"the toggle on: no entry left has the mark (" + r.data.name + ")");
                  }
                  shot("not-owned");
                  break;
               case 7:
                  // The upgrades tab too.
                  inventory.inventoryTabPage.selectedIndex = domain.getDefinition("survarium.constants.SuperCategory").UPGRADES;
                  break;
               case 8:
                  trace("SHOPMARK not owned only, upgrades: " + names());
                  listed([DECAL],[SCOPE,SCOPE_WORN,CAMO,RIFLE,BANDAGE],"the toggle on, the upgrades tab");
                  shot("upgrades");
                  break;
               case 9:
                  // The decal bought while the toggle is on.
                  inventory.setupInventoryData([owned(12,MEDKIT,3),owned(15,BANDAGE,0),owned(SCOPE_WORN_ITEM,SCOPE_WORN,10000),owned(17,SCOPE,10000),owned(18,CAMO,10000),owned(13,HELMET,10000),owned(19,DECAL,10000)]);
                  break;
               case 10:
                  trace("SHOPMARK the decal bought: " + names());
                  listed([],[DECAL,SCOPE,SCOPE_WORN,CAMO],"the decal bought with the toggle on");
                  // The toggle off.
                  press();
                  break;
               case 11:
                  trace("SHOPMARK the toggle off, upgrades: " + names());
                  listed([DECAL,SCOPE,SCOPE_WORN,CAMO],[],"the toggle off, the upgrades tab");
                  expect(DECAL,true,"the decal just bought");
                  expect(SCOPE,true,"the scope in the stash, the upgrades tab");
                  break;
               case 12:
                  // Every language's texts, one after another; the window's title as the shop gives it.
                  texts(HarnessText.LANGUAGES[language]);
                  inventory.toggleShopMode(true);
                  break;
               case 13:
                  lastRow(HarnessText.LANGUAGES[language]);
                  shot("row-" + HarnessText.LANGUAGES[language]);
                  if(++language < HarnessText.LANGUAGES.length)
                  {
                     step = 12;
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
         var row:* = shopRow(dictId);
         require(row != null && marked(row) == mark,what + (mark ? " has the mark" : " has no mark"));
      }

      // The shop's entries the list holds: `present` all of them, `absent` none.
      private function listed(present:Array, absent:Array, what:String) : void
      {
         var held:Array = entries();
         for each(var dictId:int in present)
         {
            require(held.indexOf(dictId) != -1,what + ": " + dictId + " listed");
         }
         for each(dictId in absent)
         {
            require(held.indexOf(dictId) == -1,what + ": " + dictId + " not listed");
         }
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

      // The shop's "not owned" box: the list's child of the compare box's class that is not the
      // compare box (the last, should the list have another).
      private function toggle() : *
      {
         var found:* = null;
         for(var i:int = 0; i < inventory.numChildren; i++)
         {
            var child:DisplayObject = inventory.getChildAt(i);
            if(getQualifiedClassName(child) == "CheckBoxShared" && child != inventory.checkboxCompareItems)
            {
               found = child;
            }
         }
         return found;
      }

      // A press on the box as its release ends one (scaleform.clik.controls.Button: a toggle turns
      // over, then CLICK).
      private function press() : void
      {
         var box:* = toggle();
         box.selected = !box.selected;
         box.dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
      }

      // The last row in `what`'s texts, left to right: [Comparing items] [Hide owned] Sort [By level]
      // [By price], each from its square or circle to its label's end (the sort: its label); the
      // box on the compare box's row; from the compare box's margin to the same margin at the
      // window's right, the gaps even and at least ROW_GAP; every label whole in its field; the
      // sort's choices above the box and the box above the compare box (each takes its presses).
      private function lastRow(what:String) : void
      {
         var compare:* = inventory.checkboxCompareItems;
         var box:* = toggle();
         var sort:* = inventory.sortTab;
         for each(var it:* in [compare,box,sort])
         {
            it.validateNow();
         }
         var level:* = sort.getButtonAt(0);
         var price:* = sort.getButtonAt(1);
         var parts:Array = [["\"" + compare.label + "\"",leftOf(compare),textRight(compare.textField)],["\"" + box.label + "\"",leftOf(box),textRight(box.textField)],["\"" + inventory.textField.text + "\"",textLeft(inventory.textField),textRight(inventory.textField)],["\"" + level.label + "\"",leftOf(level),textRight(level.textField)],["\"" + price.label + "\"",leftOf(price),textRight(price.textField)]];
         var said:String = "";
         var gaps:Array = [];
         for(var i:int = 0; i < parts.length; i++)
         {
            said += parts[i][0] + " " + parts[i][1].toFixed(1) + "-" + parts[i][2].toFixed(1) + "  ";
            if(i > 0)
            {
               gaps.push(parts[i][1] - parts[i - 1][2]);
            }
         }
         gaps.sort(Array.NUMERIC);
         var margin:Number = compare.x;
         var end:Number = inventory.background.width - margin;
         var right:Number = parts[parts.length - 1][2];
         trace("SHOPMARK " + what + ": " + said + "gaps " + gaps.join(",") + ", ends " + right.toFixed(1) + " / " + end);
         require(box.y == compare.y && parts[1][1] > parts[0][2],what + ": the box on the compare box's row, at its right");
         require(gaps[0] >= ROW_GAP && gaps[gaps.length - 1] - gaps[0] <= ROW_EVEN,what + ": the row's gaps even and at least " + ROW_GAP + " (" + gaps.join(",") + ")");
         require(Math.abs(right - end) <= ROW_EVEN,what + ": the row ends at the window's margin (" + right + " / " + end + ")");
         for each(var labelled:* in [compare,box,level,price])
         {
            whole(what,labelled.label,labelled.textField);
         }
         whole(what,inventory.textField.text,inventory.textField);
         require(inventory.getChildIndex(sort) > inventory.getChildIndex(box) && inventory.getChildIndex(box) > inventory.getChildIndex(compare),what + ": the sort's choices above the box, the box above the compare box");
      }

      // A label whole in its field: its text no wider than the field less its gutters.
      private function whole(what:String, text:String, field:TextField) : void
      {
         var room:Number = field.width / field.scaleX - TEXT_GUTTER;
         require(field.textWidth <= room + 0.5,what + ": \"" + text + "\" whole in its field (" + field.textWidth + " <= " + room + ")");
      }

      // A text's ends (its first line), a control's left edge (its square or circle), in the list's
      // units.
      private function textLeft(field:TextField) : Number
      {
         var line:TextLineMetrics = field.getLineMetrics(0);
         return inventory.globalToLocal(field.localToGlobal(new Point(line.x,0))).x;
      }

      private function textRight(field:TextField) : Number
      {
         var line:TextLineMetrics = field.getLineMetrics(0);
         return inventory.globalToLocal(field.localToGlobal(new Point(line.x + line.width,0))).x;
      }

      private function leftOf(control:DisplayObject) : Number
      {
         return inventory.globalToLocal(control.localToGlobal(new Point(0,0))).x;
      }

      private function place(name:String) : int
      {
         return domain.getDefinition("survarium.constants.ItemPlace")[name];
      }

      // The dictionary ids of the shop's entries the list holds (drawn or not).
      private function entries() : Array
      {
         var list:* = inventory.itemsTileList;
         var out:Array = [];
         for(var i:int = 0; i < list.dataProvider.length; i++)
         {
            if(list.dataProvider[i].descriptor == place("SHOP"))
            {
               out.push(int(list.dataProvider[i].dictId));
            }
         }
         return out;
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

      // The shop list's drawn entry of an item, by its dictionary id; null when it has none.
      private function shopRow(dictId:int) : *
      {
         for each(var r:* in rows())
         {
            if(r.data.dictId == dictId && r.data.descriptor == place("SHOP"))
            {
               return r;
            }
         }
         trace("SHOPMARK no drawn shop row for " + dictId + ": " + names());
         return null;
      }

      private function bundleRow() : *
      {
         for each(var r:* in rows())
         {
            if(r.data.descriptor == place("BUNDLE"))
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
