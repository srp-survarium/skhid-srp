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
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;

   // The reticle's steppers of the weapon's Attachments window (docs/notes/reticle-choice.md, "The
   // movie"), with the lobby movie's own window (SrvModifiersControlReskin, UpgradeWindow,
   // ReticlePanel) and inventory, fed as the exe feeds them: a collimator scope item's srp_reticle
   // member {pattern, size, colour, brightness, patterns: [name], sizes: n, colours: [name]} with
   // every inventory (setup_inventory_data; LobbyMenu then hands the open window its weapon again,
   // itemUpdated). check.sh says what it checks. Prints "RETICLE ..." lines and ends with
   // "RETICLE RESULT pass" or "... fail: ...".
   public class ReticleHarness extends Sprite
   {
      // a weapon that takes the scopes (both pages: decals and a camouflage)
      private static const RIFLE:int = 9001;

      private static const ARMOUR:int = 9010;

      // shop scopes (ItemDictId.SHOP_SCOPE_FIRST..LAST): a collimator with the choice, one without
      private static const DOT_SCOPE:int = 2150;

      private static const PLAIN_SCOPE:int = 2151;

      // the HK416 (SecondSights rows), its scope (the DeltaPoint, with the choice) and second sight
      private static const WIDE_RIFLE:int = 2218;

      private static const OPTIC_A:int = 2112;

      private static const OPTIC_B:int = 2116;

      private static const DEVICE:int = 2601;

      private static const GRIP:int = 2659;

      private static const RIFLE_ID:int = 11;

      private static const ARMOUR_ID:int = 12;

      private static const WIDE_ID:int = 16;

      private static const DOT_ID:int = 31;

      private static const PLAIN_ID:int = 32;

      private static const DEVICE_ID:int = 33;

      private static const GRIP_ID:int = 34;

      private static const OPTIC_A_ID:int = 44;

      private static const OPTIC_B_ID:int = 46;

      // the contract's names (data/srp_reticle)
      private static const ALL_PATTERNS:Array = ["dot","eotech_ring","eotech_xr308","ncstar_cross","ncstar_bullseye","ncstar_starburst","cobra_cross","cobra_tick","cobra_tick_dot","dppro_triangle"];

      private static const ALL_COLOURS:Array = ["red","green","blue"];

      // the steppers (ReticleChoice.STEPPERS)
      private static const PATTERN:int = 0;

      private static const SIZE:int = 1;

      private static const COLOUR:int = 2;

      private static const BRIGHTNESS:int = 3;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var window:*;

      private var shop:*;

      private var Window:Object;

      private var Category:Object;

      private var Choice:Object;

      // the account the harness plays: each weapon's upgrade records, each scope's srp_reticle
      private var kits:Object = {};

      private var members:Object = {};

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      private var language:int = 0;

      private var widest:Object;

      private var measured:int = 0;

      private var shotX:Number = 0;

      public function ReticleHarness()
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
               trace("RETICLE error " + e.error + (e.error is Error ? " " + Error(e.error).getStackTrace() : ""));
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
            Choice = domain.getDefinition("survarium.upgrade.ReticleChoice");
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
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("reticle-check");
            descriptor.setItemsDictionary([item(RIFLE,Category.ASSAULT_RIFLES,"Rifle",2,1),item(ARMOUR,Category.BODY_ARMOR,"Armour",2,1),item(DOT_SCOPE,Category.SCOPES,"Collimator",0,0),item(PLAIN_SCOPE,Category.SCOPES,"Plain scope",0,0),item(WIDE_RIFLE,Category.ASSAULT_RIFLES,"HK416",2,1),item(OPTIC_A,Category.SCOPES,"DeltaPoint",0,0),item(OPTIC_B,Category.SCOPES,"RSA-S",0,0),item(DEVICE,Category.SCOPES,"Muzzle device",0,0),item(GRIP,Category.SCOPES,"Grip",0,0)]);
            Window.addCompatibility(RIFLE,DOT_SCOPE);
            Window.addCompatibility(RIFLE,PLAIN_SCOPE);
            for each(var attachment:int in [OPTIC_A,OPTIC_B,DEVICE,GRIP])
            {
               Window.addCompatibility(WIDE_RIFLE,attachment);
            }
            at = "the shop";
            shop = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([RIFLE,ARMOUR,DOT_SCOPE,PLAIN_SCOPE,WIDE_RIFLE,OPTIC_A,OPTIC_B,DEVICE,GRIP].map(function(dictId:int, i:int, a:Array):Object
            {
               return {dictId:dictId,faction_id:0,level:0,cost:100,condition_or_stack:10000};
            }));
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
            at = "the window";
            window = new (C("SrvModifiersControlReskin"))();
            lobby.addChild(window);
            window.validateNow();
            window.setInventory(inventory);
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err + " " + err.getStackTrace());
         }
      }

      private static function item(dictId:int, category:int, name:String, decals:int, camo:int) : Object
      {
         return {dictId:dictId,category:category,name:name,level:1,icon_row:1,icon_column:1,is_stack:false,modifiers:[],item_properties:[],decal_available:decals,camo_available:camo};
      }

      // The collimator's member: three patterns, four sizes, two colours.
      private static function dotMember(pattern:int, size:int, colour:int, brightness:int) : Object
      {
         return {pattern:pattern,size:size,colour:colour,brightness:brightness,patterns:["dot","eotech_ring","eotech_xr308"],sizes:4,colours:["red","green"]};
      }

      // The DeltaPoint's: two patterns, four sizes, one colour.
      private static function opticMember(pattern:int, brightness:int) : Object
      {
         return {pattern:pattern,size:0,colour:0,brightness:brightness,patterns:["dot","dppro_triangle"],sizes:4,colours:["green"]};
      }

      // The weapon kit's record as patch 68 gives it the movie, its own the scope.
      private static function kit(own:int) : Object
      {
         return {id:own,param:3,kit_own:3,kit_camo:0,kit_muzzle:0,kit_grip:0};
      }

      private static function wide(own:int, device:int, grip:int, second:int, side:int) : Object
      {
         return {id:own,param:3,kit_own:3,kit_camo:0,kit_muzzle:device,kit_grip:grip,kit_second:second,kit_side:side};
      }

      // The account's inventory as the exe hands it to the lobby (setup_inventory_data), each scope
      // with its member, each weapon with its records; then the open window its weapon again, as
      // LobbyMenu.setup_inventory_data does (updateModifierWindowItem).
      private function account() : void
      {
         var list:Array = [];
         for each(var pair:Array in [[RIFLE_ID,RIFLE],[ARMOUR_ID,ARMOUR],[WIDE_ID,WIDE_RIFLE],[DOT_ID,DOT_SCOPE],[PLAIN_ID,PLAIN_SCOPE],[OPTIC_A_ID,OPTIC_A],[OPTIC_B_ID,OPTIC_B],[DEVICE_ID,DEVICE],[GRIP_ID,GRIP]])
         {
            var raw:Object = {id:pair[0],dictId:pair[1],faction_id:0,condition_or_stack:10000,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0,upgrades:kits[pair[0]] is Array ? kits[pair[0]] : []};
            if(members[pair[0]] != null)
            {
               raw.srp_reticle = members[pair[0]];
            }
            list.push(raw);
         }
         inventory.setupInventoryData(list);
         if(window.getItemId() != 0)
         {
            window.itemUpdated(inventory.getInventoryItem(window.getItemId()));
         }
      }

      private function text(language:String) : Object
      {
         var t:Object = {};
         for(var k:String in ReticleText.DATA[language])
         {
            t[k] = ReticleText.DATA[language][k];
         }
         return t;
      }

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
                  members[DOT_ID] = dotMember(0,0,0,6);
                  members[OPTIC_A_ID] = opticMember(0,6);
                  account();
                  checkParse();
                  require(inventory.getInventoryItem(DOT_ID).reticle != null && inventory.getInventoryItem(PLAIN_ID).reticle == null,"the inventory keeps the collimator's choice, none for the plain scope");
                  open(RIFLE_ID,[kit(PLAIN_ID)]);
                  break;
               case 1:
                  require(window.camoSlot.visible && window.camoSlot.data && window.camoSlot.data.itemId == PLAIN_ID,"the plain scope in the scope slot");
                  require(!panel().visible,"a scope without srp_reticle: no steppers");
                  open(RIFLE_ID,[]);
                  break;
               case 2:
                  require(!panel().visible,"an empty scope slot: no steppers");
                  open(RIFLE_ID,[kit(DOT_ID)]);
                  break;
               case 3:
                  expectPanel("the collimator",["dot","1/4","red","6"],[true,true,true,true],[true,true,true,true]);
                  checkPlace("the rifle",[window.camoSlot],null);
                  // each step: all four values, the stepped one on or back, wrapping
                  external().calls = [];
                  press(PATTERN,1);
                  press(PATTERN,-1);
                  press(SIZE,-1);
                  press(SIZE,1);
                  press(COLOUR,1);
                  press(COLOUR,-1);
                  press(BRIGHTNESS,1);
                  press(BRIGHTNESS,-1);
                  expectCalls("the steps from 0, 0, 0, 6",[call(DOT_ID,1,0,0,6),call(DOT_ID,2,0,0,6),call(DOT_ID,0,3,0,6),call(DOT_ID,0,1,0,6),call(DOT_ID,0,0,1,6),call(DOT_ID,0,0,1,6),call(DOT_ID,0,0,0,7),call(DOT_ID,0,0,0,5)]);
                  expectPanel("after the steps, before the account",["dot","1/4","red","6"],[true,true,true,true],[true,true,true,true]);
                  // the account brings the server's choice: the last pattern, size and colour, brightness 10
                  members[DOT_ID] = dotMember(2,3,1,10);
                  account();
                  expectPanel("the account's 2, 3, 1, 10",["eotech_xr308","4/4","green","10"],[true,true,true,true],[true,true,true,false]);
                  external().calls = [];
                  press(BRIGHTNESS,1);
                  press(PATTERN,1);
                  press(SIZE,1);
                  press(COLOUR,1);
                  press(BRIGHTNESS,-1);
                  expectCalls("the steps from 2, 3, 1, 10 (brightness 10 does not go on)",[call(DOT_ID,0,3,1,10),call(DOT_ID,2,0,1,10),call(DOT_ID,2,3,0,10),call(DOT_ID,2,3,1,9)]);
                  members[DOT_ID] = dotMember(1,1,0,0);
                  account();
                  expectPanel("the account's 1, 1, 0, 0",["eotech_ring","2/4","red","0"],[true,true,true,false],[true,true,true,true]);
                  external().calls = [];
                  press(BRIGHTNESS,-1);
                  press(BRIGHTNESS,1);
                  expectCalls("the steps from brightness 0 (it does not go below)",[call(DOT_ID,1,1,0,1)]);
                  turnPage();
                  break;
               case 4:
                  require(!window.camoSlot.visible && !panel().visible,"the Appearance page: no steppers");
                  turnPage();
                  break;
               case 5:
                  expectPanel("Attachments again",["eotech_ring","2/4","red","0"],[true,true,true,false],[true,true,true,true]);
                  // another scope put in: its own (none); the collimator back: its steppers, at once
                  doubleClick(PLAIN_ID);
                  require(window.camoSlot.data && window.camoSlot.data.itemId == PLAIN_ID && !panel().visible,"the plain scope double-clicked in: the steppers gone at once");
                  doubleClick(DOT_ID);
                  require(window.camoSlot.data && window.camoSlot.data.itemId == DOT_ID && panel().visible,"the collimator double-clicked in: the steppers at once");
                  checkPlace("the rifle after a swap",[window.camoSlot],null);
                  // the account without a scope on the rifle: none
                  kits[RIFLE_ID] = [];
                  account();
                  require(!window.camoSlot.data && !panel().visible,"the account's rifle without a scope: no steppers");
                  open(ARMOUR_ID,[]);
                  break;
               case 6:
                  require(!panel().visible,"armour's window: no steppers");
                  // the HK416: the DeltaPoint as its scope (one colour), the device, the grip, the RSA-S as
                  // its second sight
                  open(WIDE_ID,[wide(OPTIC_A_ID,DEVICE,GRIP,OPTIC_B,1)]);
                  break;
               case 7:
                  require(window.camoSlot.visible && window.secondSightSlot.visible && window.muzzleSlot.visible && window.gripSlot.visible,"the HK416's four slots");
                  expectPanel("the DeltaPoint",["dot","1/4","green","6"],[true,true,false,true],[true,true,false,true]);
                  external().calls = [];
                  press(COLOUR,1);
                  press(COLOUR,-1);
                  press(PATTERN,-1);
                  expectCalls("the DeltaPoint's steps (one colour: none)",[call(OPTIC_A_ID,1,0,0,6)]);
                  checkPlace("the HK416",[window.camoSlot,window.secondSightSlot,window.muzzleSlot,window.gripSlot],window.secondSightSlot);
                  shotX = window.container.x;
                  window.container.x = 640 - window.localToGlobal(new Point(0,0)).x;
                  trace("RETICLE shot");
                  wait = 240;
                  break;
               case 8:
                  window.container.x = shotX;
                  // every language: the texts in their fields
                  texts(ReticleText.LANGUAGES[language]);
                  if(++language < ReticleText.LANGUAGES.length)
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

      // ReticleChoice.parse: the contract's member, and none for one out of its bounds.
      private function checkParse() : void
      {
         var good:Object = dotMember(2,3,1,10);
         require(Choice.parse(good) != null,"parse takes the contract's member");
         require(Choice.parse(null) == null && Choice.parse({}) == null,"parse: no member, no choice");
         var bad:Array = [["pattern past its list",{pattern:3}],["size past its count",{size:4}],["colour past its list",{colour:2}],["brightness 11",{brightness:11}],["brightness -1",{brightness:-1}],["a fractional pattern",{pattern:1.5}],["no sizes",{sizes:0}],["nine sizes",{sizes:9}],["no colours",{colours:[]}],["a name not [a-z0-9_]",{patterns:["dot","Ring"]}],["a name of 24 bytes",{colours:["abcdefghijklmnopqrstuvwx"]}],["seventeen patterns",{patterns:["a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q"]}],["patterns not a list",{patterns:"dot"}]];
         for each(var row:Array in bad)
         {
            var member:Object = dotMember(2,3,1,10);
            for(var k:String in row[1])
            {
               member[k] = row[1][k];
            }
            require(Choice.parse(member) == null,"parse refuses " + row[0]);
         }
      }

      private function panel() : *
      {
         return window.reticlePanel;
      }

      private static function call(scope:int, pattern:int, size:int, colour:int, brightness:int) : String
      {
         return ["srp_set_reticle",scope,pattern,size,colour,brightness].join(" ");
      }

      // An arrow's press, as CLIK's button dispatches it (a disabled one too: the panel ignores it).
      private function press(stepper:int, direction:int) : void
      {
         var arrow:* = direction > 0 ? panel().nextArrows[stepper] : panel().backArrows[stepper];
         arrow.dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
      }

      // The panel shown with these values (a pattern's and a colour's by their names' texts) and these
      // arrows enabled, its title and names the language's.
      private function expectPanel(what:String, shown:Array, back:Array, next:Array) : void
      {
         var p:* = panel();
         var data:Object = ReticleText.DATA[ReticleText.LANGUAGES[0]];
         var values:Array = p.values.map(function(f:TextField, i:int, a:Array):String
         {
            return f.text;
         });
         var wanted:Array = [data["st_srp_reticle_pattern_" + shown[0]],shown[1],data["st_srp_reticle_colour_" + shown[2]],shown[3]];
         var arrows:String = [0,1,2,3].map(function(i:int, x:int, a:Array):String
         {
            return (p.backArrows[i].enabled ? "<" : "-") + (p.nextArrows[i].enabled ? ">" : "-");
         }).join(" ");
         var wantedArrows:String = [0,1,2,3].map(function(i:int, x:int, a:Array):String
         {
            return (back[i] ? "<" : "-") + (next[i] ? ">" : "-");
         }).join(" ");
         trace("RETICLE " + what + ": " + (p.visible ? "shown" : "hidden") + ", \"" + p.title.text + "\", " + p.labels.map(function(f:TextField, i:int, a:Array):String
         {
            return f.text + " " + values[i];
         }).join(", ") + "; arrows " + arrows);
         require(p.visible,what + ": the steppers shown");
         require(values.join("|") == wanted.join("|"),what + ": the values " + values.join(", ") + ", not " + wanted.join(", "));
         require(arrows == wantedArrows,what + ": the arrows " + arrows + ", not " + wantedArrows);
         require(p.title.text == data.st_srp_reticle_title && p.labels[0].text == data.st_srp_reticle_pattern && p.labels[1].text == data.st_srp_reticle_size && p.labels[2].text == data.st_srp_reticle_colour && p.labels[3].text == data.st_srp_reticle_brightness,what + ": the title and the names are the dictionary's");
      }

      private function external() : Object
      {
         return domain.getDefinition("HarnessExternal");
      }

      private function expectCalls(what:String, calls:Array) : void
      {
         var made:Array = external().calls as Array;
         trace("RETICLE " + what + ": the exe was called " + made.join("; "));
         require(made.join("; ") == calls.join("; "),what + ": the calls " + made.join("; ") + ", not " + calls.join("; "));
      }

      // A double click on an owned item of the inventory's list (InventoryList.onItemDoubleClick).
      private function doubleClick(itemId:int) : void
      {
         var ListEvent:Class = C("scaleform.clik.events.ListEvent");
         inventory.itemsTileList.dispatchEvent(new ListEvent(ListEvent.ITEM_DOUBLE_CLICK,false,true,-1,-1,-1,null,inventory.getInventoryItem(itemId)));
      }

      // The window opened on an owned item with these records, as LobbyMenu.openModifierWindow opens it.
      private function open(itemId:int, upgrades:Array) : void
      {
         var data:* = inventory.getInventoryItem(itemId);
         kits[itemId] = upgrades;
         data.upgrades = upgrades;
         if(window.visible && window.getItemId() != 0)
         {
            window.closePopup();
         }
         window.visible = true;
         window.setData(data);
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
         require(arrow != null,"the rifle has both pages' arrows");
         if(arrow != null)
         {
            arrow.dispatchEvent(new (C("scaleform.clik.events.ButtonEvent"))("buttonClick"));
         }
      }

      // The slots one under the other, the panel under the scope's label and over the next slot's
      // square (and the second sight's toggle under its label over the next), everything inside the
      // window and above Confirm, the panel on the slots' middle line.
      private function checkPlace(what:String, shown:Array, toggled:*) : void
      {
         var bounds:Rectangle = frame();
         var confirm:Rectangle = window.btnAccept.getBounds(window.container);
         var p:Rectangle = panel().getBounds(window.container);
         var middle:Number = square(window.camoSlot).x + square(window.camoSlot).width / 2;
         trace("RETICLE " + what + ": the window " + bounds + ", Confirm " + confirm + ", the steppers " + p + "; " + shown.map(function(s:*, i:int, a:Array):String
         {
            return s.name + " scale " + s.actualScaleX + " square " + square(s) + " label " + label(s);
         }).join("; "));
         require(bounds.containsRect(p) && p.bottom <= confirm.top,what + ": the steppers " + p + " inside the window " + bounds + ", above Confirm " + confirm);
         require(p.top >= label(window.camoSlot).bottom,what + ": the steppers " + p + " under the scope's label " + label(window.camoSlot));
         require(Math.abs(p.x + p.width / 2 - middle) <= 1.5,what + ": the steppers on the slots' middle line " + middle);
         for(var i:int = 0; i < shown.length; i++)
         {
            var a:Rectangle = square(shown[i]);
            var named:Rectangle = label(shown[i]);
            require(bounds.containsRect(a) && bounds.containsRect(named) && named.bottom <= confirm.top,what + ": slot " + i + " " + a + " and its label " + named + " inside the window, above Confirm");
            require(!a.intersects(p) && !named.intersects(p),what + ": slot " + i + " clear of the steppers");
            if(i > 0)
            {
               require(a.top >= label(shown[i - 1]).bottom,what + ": slot " + i + " under slot " + (i - 1) + "'s label");
            }
         }
         if(shown.length > 1)
         {
            require(p.bottom <= square(shown[1]).top,what + ": the steppers " + p + " over the next slot's square " + square(shown[1]));
         }
         if(toggled != null)
         {
            for each(var b:* in window.sideButtons)
            {
               var r:Rectangle = b.getBounds(window.container);
               require(b.visible && !r.intersects(p) && r.top >= label(toggled).bottom && bounds.containsRect(r),what + ": the side toggle " + r + " under its label, clear of the steppers");
            }
         }
      }

      private function frame() : Rectangle
      {
         var best:Rectangle = null;
         for(var i:int = 0; i < window.container.numChildren; i++)
         {
            var child:DisplayObject = window.container.getChildAt(i);
            if(child is C("scaleform.clik.core.UIComponent") || child is TextField || child == panel() || !child.visible)
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

      private function label(s:*) : Rectangle
      {
         return s.textFieldLabel.getBounds(window.container);
      }

      private function square(s:*) : Rectangle
      {
         return s.border.getBounds(window.container);
      }

      // One language: the panel's title and names, and every pattern's and colour's text, in their
      // fields.
      private function texts(name:String) : void
      {
         domain.getDefinition("survarium.data.LocalizationData").setData(text(name));
         widest = null;
         measured = 0;
         members[DOT_ID] = dotMember(0,0,0,6);
         kits[RIFLE_ID] = [kit(DOT_ID)];
         account();
         open(RIFLE_ID,[kit(DOT_ID)]);
         var p:* = panel();
         require(p.visible,name + ": the steppers shown");
         fits(name,"title",p.title);
         for(var i:int = 0; i < 4; i++)
         {
            fits(name,"name " + i,p.labels[i]);
            fits(name,"value " + i,p.values[i]);
         }
         for(var k:int = 0; k < ALL_PATTERNS.length; k++)
         {
            p.show(Choice.parse({pattern:k,size:0,colour:k % ALL_COLOURS.length,brightness:10,patterns:ALL_PATTERNS,sizes:8,colours:ALL_COLOURS}));
            fits(name,"pattern " + ALL_PATTERNS[k],p.values[PATTERN]);
            fits(name,"colour " + ALL_COLOURS[k % ALL_COLOURS.length],p.values[COLOUR]);
         }
         trace("RETICLE " + name + ": " + measured + " texts, widest " + widest.what + " \"" + widest.text + "\" " + widest.used + " of " + widest.room + " px");
      }

      private function fits(language:String, what:String, field:TextField) : void
      {
         var used:Number = field.textWidth + 4;
         require(field.text.replace(/\s/g,"") != "" && field.text.charAt(0) != "#",language + " " + what + ": \"" + field.text + "\" (a missing text)");
         require(field.numLines == 1 && used <= field.width,language + " " + what + ": \"" + field.text + "\" needs " + used.toFixed(1) + " of " + field.width.toFixed(1) + " px (" + field.getTextFormat().size + " pt)");
         measured++;
         if(widest == null || used / field.width > widest.used / widest.room)
         {
            widest = {what:what,text:field.text,used:used.toFixed(1),room:field.width.toFixed(1)};
         }
      }

      private function require(ok:Boolean, why:String) : void
      {
         if(!ok)
         {
            failures.push(why);
            trace("RETICLE FAIL " + why);
         }
      }

      private function finish(result:String) : void
      {
         trace("RETICLE RESULT " + result);
      }
   }
}
