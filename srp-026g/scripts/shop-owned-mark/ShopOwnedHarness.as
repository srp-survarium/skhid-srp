package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.FrameLabel;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextLineMetrics;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
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
   // The box (the compare box's symbol, CheckBoxShared, at its size): off at first, labelled, left of
   // Buy on its row (a Russian label too); on, the owned items' entries leave the list and the others
   // stay; with the upgrades tab chosen too; the decal bought while it is on leaves the list; off,
   // every entry is back. It looks as the compare box: the same symbol, bounds and label format in
   // every state.
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

      // The check box symbol's square (CheckBoxShared): its side and its top in the symbol.
      private static const SQUARE:Number = 21;

      private static const SQUARE_TOP:Number = 2;

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

      // The language the layout is checked in (HarnessText.LANGUAGES), one after another.
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

      // The lobby's texts in `language` as the exe gives them: srp's and the shop's stock ones from
      // the dictionaries (HarnessText), a few others the shop shows.
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
         // The game sends its texts once, before the list shows; the compare box takes its label
         // then only (InventoryList.onLocal), so a later language is given to it here.
         if(inventory != null)
         {
            inventory.checkboxCompareItems.label = data.checkbox_compare_items;
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
                  stacked("english");
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
                  // A longer language's label (Russian); the window's title as the shop gives it.
                  texts("russian");
                  inventory.toggleShopMode(true);
                  break;
               case 12:
                  stacked("russian");
                  shot("long-label");
                  break;
               case 13:
                  // Every language's texts, one after another.
                  texts(HarnessText.LANGUAGES[language]);
                  inventory.toggleShopMode(true);
                  break;
               case 14:
                  stacked(HarnessText.LANGUAGES[language]);
                  if(++language < HarnessText.LANGUAGES.length)
                  {
                     step = 13;
                  }
                  break;
               case 15:
                  // Last: it leaves the boxes on their frames' last state.
                  sameLook();
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

      // The shop's "not owned" box: the list's check box child that is not the compare box (the
      // last, should the list have another).
      private function toggle() : *
      {
         var found:* = null;
         var CheckBox:Class = C("survarium.controls.UnfocusedCheckBox");
         for(var i:int = 0; i < inventory.numChildren; i++)
         {
            var child:DisplayObject = inventory.getChildAt(i);
            if(child is CheckBox && child != inventory.checkboxCompareItems)
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

      // The shop's foot in `what`'s texts: "Hide owned" directly below "Comparing items", at its left
      // edge, the next row down; both labels whole; nothing there overlaps: the two boxes (the square as the game draws
      // it, the label's text), Buy, the sort's label and its choices, the search box, the list; the
      // boxes take no press over Buy or the sort's choices; all inside the window.
      private function stacked(what:String) : void
      {
         var compare:* = inventory.checkboxCompareItems;
         var box:* = toggle();
         var sort:* = inventory.sortTab;
         for each(var it:* in [compare,box,sort])
         {
            it.validateNow();
         }
         var gap:Number = box.y - (compare.y + compare.height);
         require(box.x == compare.x && gap >= 0 && gap < compare.height,what + ": the box directly below the compare box, at its left edge (x " + box.x + " / " + compare.x + ", gap " + gap + ")");
         var buy:Rectangle = inventory.itemControlPanel.btnSellBig.getBounds(inventory);
         var parts:Object = {};
         parts["the compare box"] = square(compare);
         parts["its label"] = ink(compare.textField);
         parts["the box"] = square(box);
         parts["its label \"" + box.label + "\""] = ink(box.textField);
         parts["Buy"] = buy;
         parts["the sort's label"] = ink(inventory.textField);
         parts["the search box"] = searchBox().getBounds(inventory);
         parts["the list"] = new Rectangle(inventory.itemsTileList.x,inventory.itemsTileList.y,inventory.itemsTileList.width,inventory.itemsTileList.height);
         var choices:Array = [];
         for(var i:int = 0; i < sort.dataProvider.length; i++)
         {
            var choice:* = sort.getButtonAt(i);
            parts["the sort's choice \"" + choice.label + "\""] = ink(choice.textField);
            choices.push(choice.getBounds(inventory));
         }
         for each(var labelled:* in [compare,box])
         {
            var field:TextField = labelled.textField;
            var room:Number = field.width / field.scaleX;
            require(field.textWidth + TEXT_GUTTER <= room + 0.5,what + ": the label \"" + labelled.label + "\" is whole in its field (" + field.textWidth + " + " + TEXT_GUTTER + " <= " + room + ")");
         }
         var window:Rectangle = inventory.background.getBounds(inventory);
         var names:Array = [];
         for(var name:String in parts)
         {
            names.push(name);
            require(window.containsRect(parts[name]),what + ": " + name + " inside the window (" + parts[name] + ")");
         }
         names.sort();
         for(i = 0; i < names.length; i++)
         {
            for(var j:int = i + 1; j < names.length; j++)
            {
               require(!parts[names[i]].intersects(parts[names[j]]),what + ": " + names[i] + " clear of " + names[j] + " (" + parts[names[i]] + ", " + parts[names[j]] + ")");
            }
         }
         for each(var pressed:* in [compare,box])
         {
            var area:Rectangle = pressed.getBounds(inventory);
            for each(var other:Rectangle in [buy].concat(choices))
            {
               require(!area.intersects(other),what + ": " + pressed.label + " takes no press over " + other + " (" + area + ")");
            }
         }
         trace("SHOPMARK " + what + ": compare " + compare.getBounds(inventory) + " " + parts["its label"] + "; box " + box.getBounds(inventory) + " " + parts["its label \"" + box.label + "\""] + "; sort " + parts["the sort's label"] + " " + sort.getBounds(inventory) + "; buy " + buy);
      }

      // A check box's square as the game draws it: the symbol's scale-9 grid keeps its left column
      // (the square) unscaled.
      private function square(box:*) : Rectangle
      {
         var at:Point = inventory.globalToLocal(box.localToGlobal(new Point(0,0)));
         return new Rectangle(at.x,at.y + SQUARE_TOP,SQUARE,SQUARE);
      }

      // A text field's text (its first line) in the list's units.
      private function ink(field:TextField) : Rectangle
      {
         var line:TextLineMetrics = field.getLineMetrics(0);
         var from:Point = inventory.globalToLocal(field.localToGlobal(new Point(line.x,2)));
         var to:Point = inventory.globalToLocal(field.localToGlobal(new Point(line.x + line.width,2 + line.height)));
         return new Rectangle(from.x,from.y,to.x - from.x,to.y - from.y);
      }

      // The shop's search box: the list's child of the friends window's name box class.
      private function searchBox() : DisplayObject
      {
         for(var i:int = 0; i < inventory.numChildren; i++)
         {
            if(getQualifiedClassName(inventory.getChildAt(i)) == "TextInputFriendName")
            {
               return inventory.getChildAt(i);
            }
         }
         return null;
      }

      // The box looks as the compare box below it: the same symbol, bounds and scale; its label as
      // shown (off, on, disabled) and in every state's frame of the symbol in the same format (font,
      // size, bold, colour), drawn at the same scale.
      private function sameLook() : void
      {
         var compare:* = inventory.checkboxCompareItems;
         var box:* = toggle();
         var a:Rectangle = compare.getBounds(inventory);
         var b:Rectangle = box.getBounds(inventory);
         require(getQualifiedClassName(box) == getQualifiedClassName(compare) && a.width == b.width && a.height == b.height && box.actualScaleX == compare.actualScaleX && box.actualScaleY == compare.actualScaleY,"the box is the compare box's symbol at its scale (" + getQualifiedClassName(box) + " " + b.width + "x" + b.height + " / " + getQualifiedClassName(compare) + " " + a.width + "x" + a.height + ")");
         var shown:Array = [["off",false,true],["on",true,true],["disabled",false,false]];
         for each(var state:Array in shown)
         {
            for each(var it:* in [compare,box])
            {
               it.selected = state[1];
               it.enabled = state[2];
               it.validateNow();
            }
            sameLabel("the label shown " + state[0] + " (" + box.currentFrameLabel + ")",compare.textField,box.textField);
         }
         for each(var frame:FrameLabel in compare.currentLabels)
         {
            compare.gotoAndStop(frame.name);
            box.gotoAndStop(frame.name);
            sameLabel("the label of the frame " + frame.name,compare.getChildByName("textField"),box.getChildByName("textField"));
         }
      }

      private function sameLabel(what:String, compare:*, box:*) : void
      {
         var a:TextFormat = compare.getTextFormat();
         var b:TextFormat = box.getTextFormat();
         var sa:Number = compare.transform.concatenatedMatrix.a;
         var sb:Number = box.transform.concatenatedMatrix.a;
         var said:String = b.font + " " + b.size + (b.bold ? " bold" : "") + " #" + uint(b.color).toString(16) + " x" + sb.toFixed(3) + " / " + a.font + " " + a.size + (a.bold ? " bold" : "") + " #" + uint(a.color).toString(16) + " x" + sa.toFixed(3);
         require(a.font == b.font && a.size == b.size && a.bold == b.bold && a.color == b.color && Math.abs(sa - sb) < 0.001 && compare.embedFonts == box.embedFonts,what + ": as the compare box's (" + said + ")");
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
