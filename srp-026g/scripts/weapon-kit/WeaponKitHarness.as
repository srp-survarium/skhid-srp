package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
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
   // - the texts: each language's title, page names, slot labels, hints and button fitting.
   // Prints "WEAPONKIT ..." lines and ends with "WEAPONKIT RESULT pass" or "... fail: ...".
   public class WeaponKitHarness extends Sprite
   {
      private static const RIFLE:int = 9001;

      // a weapon whose stock scope the window names (default_scope)
      private static const STOCK_RIFLE:int = 9002;

      private static const ARMOUR:int = 9010;

      private static const DECAL_A:int = 9020;

      private static const DECAL_B:int = 9021;

      // a shop scope (ItemDictId.SHOP_SCOPE_FIRST..LAST)
      private static const SCOPE:int = 2150;

      private static const CAMO:int = 9030;

      private static const CAMO_OTHER:int = 9031;

      // item ids
      private static const RIFLE_ID:int = 11;

      private static const ARMOUR_ID:int = 12;

      private static const STOCK_RIFLE_ID:int = 13;

      private static const DECAL_A_ID:int = 21;

      private static const DECAL_B_ID:int = 22;

      private static const SCOPE_ID:int = 31;

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
            descriptor.setItemsDictionary([item(RIFLE,Category.ASSAULT_RIFLES,"Rifle",2,1),item(STOCK_RIFLE,Category.ASSAULT_RIFLES,"Stock-scoped rifle",2,1),item(ARMOUR,Category.BODY_ARMOR,"Armour",2,1),item(DECAL_A,Category.DECALS,"Decal A",0,0),item(DECAL_B,Category.DECALS,"Decal B",0,0),item(SCOPE,Category.SCOPES,"Scope",0,0),item(CAMO,Category.CAMOUFLAGES,"Camouflage",0,0),item(CAMO_OTHER,Category.CAMOUFLAGES,"Other camouflage",0,0)]);
            // the scopes the rifles take (ItemFactory gives the movie its pairs so)
            Window.addCompatibility(RIFLE,SCOPE);
            Window.addCompatibility(STOCK_RIFLE,SCOPE);
            Window.addCompatibility(STOCK_RIFLE,domain.getDefinition("survarium.constants.ItemDictId").SCOPE_PSO_1);
            at = "the shop";
            var shop:* = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([RIFLE,STOCK_RIFLE,ARMOUR,DECAL_A,DECAL_B,SCOPE,CAMO,CAMO_OTHER].map(function(dictId:int, i:int, a:Array):Object
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
                  inventory.setupInventoryData([owned(RIFLE_ID,RIFLE),owned(ARMOUR_ID,ARMOUR),owned(STOCK_RIFLE_ID,STOCK_RIFLE),owned(DECAL_A_ID,DECAL_A),owned(DECAL_B_ID,DECAL_B),owned(SCOPE_ID,SCOPE),owned(CAMO_HIGH_ID,CAMO),owned(CAMO_LOW_ID,CAMO),owned(CAMO_OTHER_ID,CAMO_OTHER)]);
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
                  break;
               case 10:
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

      // The weapon kit's record as patch 68 gives it the movie.
      private static function kit(own:int, ownSlot:int, camoDict:int) : Object
      {
         return {id:own,param:3,kit_own:ownSlot,kit_camo:camoDict,kit_muzzle:0,kit_grip:0};
      }

      private function slots() : Array
      {
         return [window.decalSlot,window.decalSlot1,window.decalSlot2,window.camoSlot];
      }

      // The window opened on an owned item with these upgrade records (the account's), as
      // LobbyMenu.openModifierWindow opens it.
      private function open(itemId:int, upgrades:Array) : void
      {
         var data:* = inventory.getInventoryItem(itemId);
         data.upgrades = upgrades;
         if(window.visible && window.getItemId() != 0)
         {
            window.closePopup();
         }
         window.visible = true;
         window.setData(data);
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
         open(ARMOUR_ID,[]);
         measure(name,"armour");
         trace("WEAPONKIT " + name + ": " + measured + " texts, widest " + widest.what + " \"" + widest.text + "\" " + widest.used + " of " + widest.room + " px");
      }

      private function measure(language:String, page:String) : void
      {
         var c:* = window.container;
         for each(var s:* in slots())
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
         for each(s in slots())
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
