package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;

   // The item lists' search (docs/notes/item-search.md), with the lobby movie's own inventory list,
   // fed as the exe feeds it (its items' sellers, built-in and rolled modifications, the card's
   // property names, the seller tabs), typed into its own box:
   // - the "+" words: levels (any of them), factions by the seller tabs' names (any), bonuses by the
   //   card's names or English words (each), with the name's words; a defect is no bonus; a "+" word
   //   naming nothing lists nothing; "+" alone is nothing typed;
   // - every language: the hint on one line inside the box; its examples and the tip's (bonus,
   //   faction, kind) finding what they name, by the language's own names; the English words too;
   // - the tip: on the box while it is empty, its lines the dictionary's, gone once something is
   //   typed, back when the box is emptied.
   // Prints "ITEMSEARCH ..." lines and ends with "ITEMSEARCH RESULT pass" or "... fail: ...".
   public class ItemSearchHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:MovieClip;

      private var inventory:*;

      private var box:*;

      // ToolTip's container (the lobby's tipContainer)
      private var tipLayer:Sprite;

      private var shop:*;

      // the shop's entries as the exe gives them
      private var offers:Array;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      private var language:int = 0;

      private static const RIFLE:int = 9101;

      private static const RIG_A:int = 9102;

      private static const RIG_B:int = 9103;

      private static const VEST:int = 9104;

      private static const MOSIN:int = 9105;

      private static const HELMET:int = 9106;

      private static const BOOTS:int = 9107;

      // each fixture property's direction as the game has it (ui_properties: 1 more is better)
      private static const DIRECTION:Object = {"3":1,"6":1,"9":1,"10":1,"15":0,"20":1,"102":0,"105":1,"115":0,"117":1};

      // [dict id, name, category, level, seller, built-in modifications, rolled modifications]
      private var ITEMS:Array;

      public function ItemSearchHarness()
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
               trace("ITEMSEARCH error " + e.error + (e.error is Error ? " " + Error(e.error).getStackTrace() : ""));
            }
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function C(name:String) : Class
      {
         return domain.getDefinition(name) as Class;
      }

      private static function mod(prop:int, value:Number, state:int = 0) : Object
      {
         return {state:state,icon:0,props:[{prop_id:prop,prop_value:value}]};
      }

      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
            var Scene:Object = domain.getDefinition("MainScene");
            Scene.stage = this;
            Scene.scaleFactor = 1;
            domain.getDefinition("survarium.data.LocalizationData").setData(ItemSearchText.DATA.english.ui);
            at = "the icons";
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["weapons","equipment","consumables","factionItemIcons","bundles","decals","camo","camo_big","vouchers","modifiers"])
            {
               new Container(icons,8,8,new BitmapData(8,8,true,0xff808080),false,1,0,0,false);
            }
            at = "the items";
            ITEMS = [[RIFLE,"AK-74M",Category.ASSAULT_RIFLES,10,3,[],[mod(115,-0.1)]],
               [RIG_A,"Rig A",Category.RIGS,10,1,[mod(20,2),mod(117,1)],[]],
               [RIG_B,"Rig B",Category.RIGS,11,2,[mod(20,1),mod(117,1),mod(15,2)],[]],
               [VEST,"Vest",Category.BODY_ARMOR,12,3,[mod(9,1)],[mod(6,-5,1)]],
               [MOSIN,"Mosin",Category.ASSAULT_RIFLES,11,0,[mod(105,-0.05)],[mod(102,-0.1)]],
               [HELMET,"Hood",Category.HELMETS,10,4,[mod(10,1)],[mod(6,2)]],
               [BOOTS,"Shoes",Category.BOOTS,12,0,[],[]]];
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("item-search-check");
            descriptor.setItemsDictionary(ITEMS.map(function(row:Array, i:int, a:Array):Object
            {
               return {dictId:row[0],name:row[1],category:row[2],level:row[3],sellers:[{faction_id:row[4]}],modifiers:row[5],icon_row:1,icon_column:1,is_stack:false,item_properties:[]};
            }));
            props("english");
            at = "the shop";
            shop = new (C("survarium.controls.GangShop"))();
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems(ITEMS.map(function(row:Array, i:int, a:Array):Object
            {
               return {dictId:row[0],faction_id:row[4],level:0,cost:100,condition_or_stack:10000};
            }));
            at = "the lobby";
            // The lobby's root as the list and the card find it (parent["inventory_list"]).
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
            finish("fail: " + at + ": " + err + " " + err.getStackTrace());
         }
      }

      // The card's property names in a language, as set_item_props_data gives them.
      private function props(language:String) : void
      {
         var list:Array = [];
         for(var id:String in ItemSearchText.DATA[language].props)
         {
            list.push({prop_id:int(id),prop_name:ItemSearchText.DATA[language].props[id],modifier_type:1,postfix:"",direction:DIRECTION[id]});
         }
         new (C("survarium.data.PropertiesDescriptor"))("properties").setPropertiesDictionary(list);
      }

      // The seller tabs in a language, as the exe gives them (GangShop.fillSellers); once they are
      // there, their names in another language.
      private function sellers(language:String) : void
      {
         var names:Object = ItemSearchText.DATA[language].factions;
         var tabs:* = inventory.sellersTabPage.dataProvider;
         if(tabs != null && tabs.length > 1)
         {
            for(var i:int = 0; i < tabs.length; i++)
            {
               if(names[String(tabs[i].faction_id)] != null)
               {
                  tabs[i].name = names[String(tabs[i].faction_id)];
               }
            }
            return;
         }
         var list:Array = [];
         for(var id:int = 1; id <= 4; id++)
         {
            list.push({faction_id:id,name:names[String(id)]});
         }
         inventory.fillSellers(list);
      }

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
                  sellers("english");
                  inventory.sellersTabPage.selectedIndex = 0;
                  inventory.toggleShopMode(false);
                  inventory.filterItems(null);
                  inventory.optionControl.checkbox_all_traders.selected = true;
                  offers = [];
                  shop.forEachItem(function(entry:*):Boolean
                  {
                     offers.push(entry);
                     return false;
                  });
                  inventory.setupShopItems(offers);
                  inventory.setupBundleData([]);
                  inventory.setupInventoryData(ITEMS.map(function(row:Array, i:int, a:Array):Object
                  {
                     return {id:i + 1,dictId:row[0],faction_id:row[4],condition_or_stack:10000,modifiers:row[6],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0,upgrades:[]};
                  }));
                  break;
               case 1:
                  box = find(inventory,C("TextInputFriendName"));
                  if(box == null)
                  {
                     finish("fail: the list has no search box");
                     removeEventListener(Event.ENTER_FRAME,this.tick);
                     return;
                  }
                  trace("ITEMSEARCH the box is " + box.name + " in " + box.parent.name);
                  var all:Array = ["AK-74M","Rig A","Rig B","Vest","Mosin","Hood","Shoes"];
                  expect("",all,false);
                  require(tipShown(),"the tip is on the empty box");
                  checkTipRows(ItemSearchText.DATA.english.ui);
                  type("a");
                  require(!tipShown(),"no tip once something is typed");
                  type("");
                  require(tipShown(),"the tip back when the box is emptied by typing");
                  type("rig");
                  inventory.clearSearch();
                  require(tipShown() && box.text == "","the tip back when the search is cleared (Esc)");
                  expect("+",all,false);
                  expect("+ammo",["Rig A","Rig B"],false);
                  expect("+ammo +pocket",["Rig A","Rig B"],false);
                  expect("+pocket +ammo +black",["Rig B"],false);
                  expect("+pocket +scavenger",["Rig A"],false);
                  expect("+pocket +scav",["Rig A"],false);
                  expect("+10",["AK-74M","Rig A","Hood"],false);
                  expect("+10 +11",["AK-74M","Rig A","Rig B","Mosin","Hood"],false);
                  expect("+10+11 rig",["Rig A","Rig B"],false);
                  expect("ak+10",["AK-74M"],false);
                  expect("ak +10 +ammo",[],false);
                  expect("rig +10 +ammo",["Rig A"],false);
                  expect("+army",["AK-74M","Vest"],false);
                  expect("+army +scav",["AK-74M","Rig A","Vest"],false);
                  expect("+independent",["Mosin","Shoes"],false);
                  expect("+army +recoil",["AK-74M"],false);
                  expect("+army +11",[],false);
                  // armour: the Armor bonus (a rolled one on the hood; the vest's is a defect), not a kind
                  expect("+armor",["Hood"],false);
                  expect("+armour",["Hood"],false);
                  expect("+artefact",["Vest"],false);
                  expect("+artifact",["Vest"],false);
                  expect("+slot",["Hood"],false);
                  expect("+aim",["Mosin"],false);
                  expect("+xyzzy",[],false);
                  expect("+xyzzy ak",[],false);
                  expect("+amo",[],false);
                  expect("mosin",["Mosin"],false);
                  expect("mosni",["Mosin"],false);
                  // kinds: "+" words only (owner 2026-11-06), any of them, with the rest by "and"
                  expect("helmet",[],false);
                  expect("+helmet",["Hood"],false);
                  expect("+hel",["Hood"],false);
                  expect("+helmets",["Hood"],false);
                  expect("+helmet +boots",["Hood","Shoes"],false);
                  expect("+helmet +boots +12",["Shoes"],false);
                  expect("+boots +army",[],false);
                  expect("+vest",["Vest"],false);
                  expect("+rig +scavenger",["Rig A"],false);
                  expect("+gear",["Rig A","Rig B","Vest","Hood","Shoes"],false);
                  // a plain number is a part of a name, never a level (owner 2026-11-06)
                  expect("10",[],false);
                  expect("74",["AK-74M"],false);
                  expect("+11 74",[],false);
                  // the pointer over the empty box: the tip after the game's delay (300 ms)
                  type("");
                  tipLayer = new Sprite();
                  addChild(tipLayer);
                  domain.getDefinition("scaleform.clik.controls.ToolTip").tipContainer = tipLayer;
                  tip().dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OVER,true));
                  wait = 40;
                  break;
               case 2:
                  checkTipLayout();
                  // the shop: its items with their built-in modifications only
                  inventory.toggleShopMode(true);
                  inventory.filterItems(null);
                  expect("",null,false,"shop");
                  expect("+ammo +pocket",["Rig A","Rig B"],false,"shop");
                  expect("+recoil",[],false,"shop");
                  expect("+artefact +army",["Vest"],false,"shop");
                  expect("+armor",[],false,"shop");
                  expect("+rig",["Rig A","Rig B"],false,"shop");
                  inventory.toggleShopMode(false);
                  inventory.filterItems(null);
                  // Shift+= over the list, the box without the keys, as the exe may send it: no
                  // character (charCode 0); the box takes the keys on the next render with a "+"
                  type("");
                  stage.focus = null;
                  stage.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN,true,false,0,187,0,false,false,true));
                  wait = 20;
                  break;
               case 3:
                  require(box.text == "+","Shift+= without its character over the list types \"+\" (the box reads \"" + box.text + "\")");
                  type("");
                  stage.focus = null;
                  break;
               case 4:
                  // every language: the card's names and the seller tabs in it
                  var name:String = ItemSearchText.LANGUAGES[language];
                  var t:Object = ItemSearchText.DATA[name].ui;
                  props(name);
                  sellers(name);
                  // the shop's data again: the tip's lists made with this language's seller tabs
                  inventory.setupShopItems(offers);
                  type("");
                  checkFactionList(name);
                  fits(name,t.st_srp_inventory_search,": " + t.st_srp_inventory_search_examples);
                  // the hint's examples (owner 2026-11-06): exactly the pocket, the slot, the ammo and
                  // a level, each finding what it names
                  var hint:Array = plusWords(t.st_srp_inventory_search_examples);
                  require(hint.length == 4 && hint[3] == "+10" && String(t.st_srp_inventory_search_examples).split(",").length == 4,name + ": the hint is four + words: " + t.st_srp_inventory_search_examples);
                  expect(hint[0],["Rig A","Rig B"],false,name + " hint");
                  expect(hint[1],["Hood"],false,name + " hint");
                  expect(hint[2],["Rig A","Rig B"],false,name + " hint");
                  expect(hint[3],["AK-74M","Rig A","Hood"],false,name + " hint");
                  // the tip's examples
                  var bonus:Array = plusWords(t.st_srp_inventory_search_tip_bonus_example);
                  for each(var w:String in bonus)
                  {
                     expect(w,null,false,name + " tip");
                  }
                  var factions:Array = plusWords(t.st_srp_inventory_search_tip_faction_example);
                  expect(factions[0],["AK-74M","Vest"],false,name + " tip");
                  expect(factions.join(" "),["AK-74M","Vest","Rig B"],false,name + " tip");
                  var kinds:Array = plusWords(t.st_srp_inventory_search_tip_kind_example);
                  expect(kinds[0],["Hood"],false,name + " tip");
                  expect(kinds[1],["Shoes"],false,name + " tip");
                  expect(kinds.join(" "),["Hood","Shoes"],false,name + " tip");
                  // the Armor bonus by its card name: a bonus, not the body armour kind
                  expect("+" + String(ItemSearchText.DATA[name].props["6"]).split(" ")[0],["Hood"],false,name + " armour bonus");
                  // the English words in every language
                  expect("+ammo +pocket",["Rig A","Rig B"],false,name);
                  expect("+artefact",["Vest"],false,name);
                  expect("+helmet",["Hood"],false,name);
                  if(++language < ItemSearchText.LANGUAGES.length)
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

      // the "+" words of a text, in order
      private static function plusWords(text:String) : Array
      {
         return text.replace(/[,:]/g," ").split(/\s+/).filter(function(w:String, i:int, a:Array):Boolean
         {
            return w.charAt(0) == "+" && w.length > 1;
         });
      }

      // typed into the box: its text and the change it sends
      private function type(text:String) : void
      {
         box.text = text;
         box.dispatchEvent(new Event(Event.CHANGE));
      }

      // The list's names after typing the text; `first`: the names given lead it (the rest any);
      // names null: something listed.
      private function expect(text:String, names:Array, first:Boolean, what:String = "english") : void
      {
         type(text);
         var dp:* = inventory.itemsTileList.dataProvider;
         var shown:Array = [];
         for(var i:int = 0; dp != null && i < dp.length; i++)
         {
            shown.push(dp[i].name);
         }
         var ok:Boolean = names == null ? shown.length > 0 : first ? shown.slice(0,names.length).join("|") == names.join("|") : shown.slice().sort().join("|") == names.slice().sort().join("|");
         trace("ITEMSEARCH " + what + " \"" + text + "\": " + shown.join(", "));
         require(ok,what + " \"" + text + "\" lists " + shown.join(", ") + ", not " + (names == null ? "something" : names.join(", ")));
      }

      // The first child (at any depth) of a type; with `all`, every one put in it.
      private static function find(c:*, type:Class, all:Array = null) : *
      {
         for(var i:int = 0; i < c.numChildren; i++)
         {
            var child:* = c.getChildAt(i);
            if(child is type)
            {
               if(all == null)
               {
                  return child;
               }
               all.push(child);
            }
            if(child is flash.display.DisplayObjectContainer)
            {
               var found:* = find(child,type,all);
               if(found != null)
               {
                  return found;
               }
            }
         }
         return null;
      }

      // The tip's rows (tipData.rows): the dictionary's [example, meaning] in their order, with the
      // list of every bonus word under the bonus row and of every kind's under the kind row (each a
      // wrapped right cell): only what the items have.
      private function checkTipRows(ui:Object) : void
      {
         var rows:Array = tip().tipData.rows as Array;
         var key:String = "st_srp_inventory_search_tip_";
         var want:Array = [];
         for each(var k:String in ["bonus","bonus_list","level","faction","faction_list","kind","kind_list"])
         {
            want.push(k.indexOf("_list") != -1 ? " / (list)" : ui[key + k + "_example"] + " / " + ui[key + k]);
         }
         var got:Array = rows == null ? [] : rows.map(function(r:Array, i:int, a:Array):String
         {
            return r[0] + " / " + (r[2] === true ? "(list)" : r[1]);
         });
         require(got.join("|") == want.join("|"),"the tip's rows: " + got.join(" | ") + ", not " + want.join(" | "));
         if(rows == null || rows.length != 7)
         {
            return;
         }
         var bonuses:Array = String(rows[1][1]).split(" ");
         var kinds:Array = String(rows[6][1]).split(" ");
         trace("ITEMSEARCH tip bonus words (" + bonuses.length + "): " + rows[1][1]);
         trace("ITEMSEARCH tip kind words (" + kinds.length + "): " + rows[6][1]);
         // the bonuses the items have, by type (owner 2026-11-06): the gear's (the rigs' ammo and
         // pocket, the hood's slot and armor, the vest's artifact), then the weapons' (the Mosin's
         // aim, the AK's recoil), each group in SEARCH_BONUSES' order; the vest's armour is a
         // defect, the rig's +2 kg and the Mosin's movement -5 % penalties, and no item has xp or
         // use: not listed, still matched
         require(rows[1][1] == "+ammo +slot +pocket +artifact +armor +aim +recoil","the tip lists the items' bonuses by type: " + rows[1][1]);
         // the kinds some item is, in SEARCH_KINDS' order (no mask, gloves, pants or items)
         require(rows[6][1] == "+gear +helmet +body +rig +boots","the tip lists the items' kinds: " + rows[6][1]);
         expect("+weight",[],false,"a penalty (the rig's +2 kg) is no bonus");
         expect("+movement",[],false,"a penalty (the Mosin's built-in -5 %) is no bonus");
         expect("+xp",[],false,"a bonus no item has");
         expect("+mask",[],false,"a kind no item is");
         // the factions selling an item, in the seller tabs' order, a word each naming it alone
         require(rows[4][1] == "+independent +scavengers +black +renaissance +fringe","the tip lists the items' factions: " + rows[4][1]);
         // every listed kind word names its kind: the inventory's of that kind
         expect("+helmet +boots",["Hood","Shoes"],false,"tip kinds");
      }

      // The tip's faction list in a language (its seller tabs' names): every faction an item has,
      // in the tabs' order, each word listing that faction's items, the independent traders too.
      private function checkFactionList(language:String) : void
      {
         var rows:Array = tip().tipData.rows as Array;
         var words:Array = rows != null && rows.length > 4 ? String(rows[4][1]).split(" ") : [];
         trace("ITEMSEARCH " + language + " tip faction words: " + words.join(" "));
         // the tabs' order: the independent traders, then the factions 1 to 4
         var sold:Array = [["Mosin","Shoes"],["Rig A"],["Rig B"],["AK-74M","Vest"],["Hood"]];
         require(words.length == sold.length,language + ": the tip lists " + sold.length + " factions (" + words.join(" ") + ")");
         for(var i:int = 0; i < words.length && i < sold.length; i++)
         {
            expect(words[i],sold[i],false,language + " tip faction");
         }
      }

      // The tip shown (in tipLayer, ToolTip's container): the game's renderer, just above the box,
      // their left edges together; its rows two columns, the right one's at one x; each list
      // (a row [left, right, true]) under its row's meaning, dim (0x696969), no label, closer to
      // its row than the groups are to each other (the same gaps each); the padding the same on
      // every side.
      private function checkTipLayout() : void
      {
         require(tipLayer.numChildren == 1,"the tip shows over the empty box (" + tipLayer.numChildren + " shown)");
         if(tipLayer.numChildren != 1)
         {
            return;
         }
         var renderer:* = tipLayer.getChildAt(0);
         var shape:Rectangle = renderer.getBounds(tipLayer);
         var at:Rectangle = box.getBounds(tipLayer);
         trace("ITEMSEARCH tip at " + shape + ", the box at " + at);
         require(Math.abs(shape.left - at.left) <= 1,"the tip's left edge (" + shape.left + ") on the box's (" + at.left + ")");
         require(shape.bottom <= at.top,"the tip (to " + shape.bottom + ") above the box (from " + at.top + ")");
         require(shape.top >= 0,"the tip (from " + shape.top + ") on the screen");
         var rows:Array = tip().tipData.rows as Array;
         var cells:Array = [];
         find(renderer,TextField,cells);
         cells = cells.filter(function(c:TextField, i:int, a:Array):Boolean
         {
            return c.visible;
         });
         require(cells.length == rows.length * 2,"the tip has " + rows.length + " rows of 2 cells (" + cells.length + " cells)");
         if(cells.length != rows.length * 2)
         {
            return;
         }
         // each cell's text in the layer: [left, top, right, bottom] (a field's text starts 2 in)
         var text:Array = cells.map(function(c:TextField, i:int, a:Array):Array
         {
            var p:Point = tipLayer.globalToLocal(c.localToGlobal(new Point(2,2)));
            var q:Point = tipLayer.globalToLocal(c.localToGlobal(new Point(2 + c.textWidth,2 + c.textHeight)));
            return [p.x,p.y,q.x,q.y];
         });
         var lefts:Array = [];
         var rights:Array = [];
         for(var i:int = 0; i < cells.length; i += 2)
         {
            lefts.push(text[i]);
            rights.push(text[i + 1]);
         }
         var widest:Number = 0;
         var inner:Array = [];
         var between:Array = [];
         for(i = 0; i < rows.length; i++)
         {
            var list:Boolean = rows[i][2] === true;
            var colour:uint = uint(cells[i * 2 + 1].getTextFormat(0,1).color) & 0xFFFFFF;
            require(Math.abs(rights[i][0] - rights[0][0]) <= 0.5,"the meanings and lists start at one x (" + rights[i][0] + ", " + rights[0][0] + ")");
            if(list)
            {
               require(cells[i * 2].text == "","a list has no label (\"" + cells[i * 2].text + "\")");
               require(colour == 0x696969,"a list is dim (" + colour.toString(16) + ")");
            }
            else
            {
               require(Math.abs(lefts[i][0] - lefts[0][0]) <= 0.5,"the examples start at one x (" + lefts[i][0] + ", " + lefts[0][0] + ")");
               require(Math.abs(rights[i][1] - lefts[i][1]) <= 0.5,"row " + i + "'s cells on one line");
               require(colour != 0x696969,"a meaning is not dim");
               widest = Math.max(widest,lefts[i][2]);
            }
            if(i > 0)
            {
               (list ? inner : between).push(rights[i][1] - Math.max(lefts[i - 1][3],rights[i - 1][3]));
            }
         }
         trace("ITEMSEARCH tip gaps inside a group: " + inner.join(", ") + "; between the groups: " + between.join(", "));
         require(inner.length == 3 && between.length == 3,"three groups with a list each (" + inner.length + " lists, " + between.length + " gaps between groups)");
         for each(var g:Number in inner)
         {
            require(Math.abs(g - inner[0]) <= 0.5,"the same gap inside every group (" + inner.join(", ") + ")");
         }
         for each(g in between)
         {
            require(Math.abs(g - between[0]) <= 0.5 && g > inner[0] + 4,"the same gap between the groups, larger than inside one (" + between.join(", ") + " against " + inner[0] + ")");
         }
         require(rights[0][0] > widest + 8,"the meanings (from " + rights[0][0] + ") clear of the widest example (to " + widest + ")");
         var right:Number = 0;
         for each(var r:Array in rights)
         {
            right = Math.max(right,r[2]);
         }
         var last:int = rows.length - 1;
         var bottom:Number = Math.max(lefts[last][3],rights[last][3]);
         var pads:Array = [lefts[0][0] - shape.left,lefts[0][1] - shape.top,shape.right - right,shape.bottom - bottom];
         trace("ITEMSEARCH tip " + shape.width.toFixed(0) + " x " + shape.height.toFixed(0) + ", padding left, top, right, bottom: " + pads.map(function(p:Number, i:int, a:Array):String
         {
            return p.toFixed(1);
         }).join(", "));
         for each(var pad:Number in pads)
         {
            require(Math.abs(pad - pads[0]) <= 1,"the tip's padding even on every side (" + pads.join(", ") + ")");
         }
      }

      private function tip() : *
      {
         for(var i:int = 0; i < inventory.numChildren; i++)
         {
            var c:* = inventory.getChildAt(i);
            if(c is Sprite && c.numChildren == 1 && c.getChildAt(0) is C("survarium.player.ReasonsTip"))
            {
               return c.getChildAt(0);
            }
         }
         return null;
      }

      private function tipShown() : Boolean
      {
         return tip() != null;
      }

      // The hint as the empty box shows it: one line, inside the box before the magnifier.
      // Its two parts (owner 2026-11-06): the word at the hint's grey, the examples dimmer (62 %).
      private function fits(language:String, word:String, more:String) : void
      {
         type("");
         box.defaultText = word;
         box.defaultTextMore = more;
         box.validateNow();
         var hint:String = word + more;
         var field:TextField = box.textField;
         var grey:uint = uint(field.getTextFormat(0,1).color);
         var dim:uint = uint(field.getTextFormat(field.length - 1,field.length).color);
         require(grey == 0xAAAAAA && dim == 0x696969,language + ": the hint's word " + grey.toString(16) + ", its examples " + dim.toString(16) + " (aaaaaa, 696969)");
         // in the field's own units (textWidth's): the box is scaled to its size, its field counter-scaled
         var room:Number = field.width / field.scaleX - Number(field.defaultTextFormat.rightMargin);
         var used:Number = field.textWidth + 4;
         trace("ITEMSEARCH " + language + " hint \"" + field.text + "\" " + used.toFixed(1) + " of " + room.toFixed(1) + " px (" + field.getTextFormat().size + " pt)");
         require(field.text == hint,language + ": the empty box shows the hint (" + field.text + ")");
         require(field.numLines == 1 && used <= room,language + ": the hint \"" + hint + "\" needs " + used.toFixed(1) + " of " + room.toFixed(1) + " px");
         // the box (a child of the list: the lobby scales them together, every resolution alike)
         // inside the list's own width
         var list:Number = inventory.background.width;
         require(box.x >= 0 && box.x + box.width <= list,"the box (" + box.x + " to " + (box.x + box.width) + ") inside the list (" + list + ")");
         trace("ITEMSEARCH the box " + box.x + " to " + (box.x + box.width) + " in the list (" + list + "), the grid " + inventory.itemsTileList.x + " to " + (inventory.itemsTileList.x + inventory.itemsTileList.width));
      }

      private function require(ok:Boolean, why:String) : void
      {
         if(!ok)
         {
            failures.push(why);
            trace("ITEMSEARCH FAIL " + why);
         }
      }

      private function finish(result:String) : void
      {
         trace("ITEMSEARCH RESULT " + result);
      }
   }
}
