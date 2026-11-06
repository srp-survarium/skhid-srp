package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The modification window's item card (docs/notes/reroll-keep-old.md, "The item's stats on
   // hover") at 2560 x 1440, with the lobby movie's own root, inventory and cards. The window opened
   // as the item card's Modification opens it (Message "upgrade_item", ItemCard.upgradeItem);
   // its item's picture hovered: the card is the inventory's (LobbyMenu.itemHintRenderer, in the
   // cards' layer) at the inventory row's card's size, left of the window (right of it with no room
   // on the left), on the screen; no tooltip. The window low on the screen, and the card past the
   // stage's bottom as Shift or Alt resize it: it stays on the screen. The picture left, and the
   // window closed under the pointer: no card. An item tip
   // (SrvItemHintRednerer, another player's gear) is drawn at the lobby's scale too, not twice.
   // The root's own setup (LobbyMenu.init) stops outside the game at the stage it is not on yet,
   // after its messages' layer and the cards' listeners; its resize (onStageResize, the stage's
   // listener it never adds) is given here as it scales the layers.
   // Prints "MODCARD ..." lines and ends with "MODCARD RESULT pass" or "MODCARD RESULT fail: ...".
   public class ModificationCardHarness extends Sprite
   {
      private static const RIFLE:int = 9301;

      private static const OWNED_RIFLE:int = 7;

      // 2560 x 1440 against the 1920 x 1080 design
      private static const SCALE:Number = 1440 / 1080;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var inventory:*;

      private var cards:DisplayObjectContainer;

      private var window:*;

      private var rowCardWidth:Number;

      private var failures:Array = [];

      private var frames:int = 0;

      private var step:int = 0;

      private var wait:int = 0;

      public function ModificationCardHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
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

      private function loaded(e:Event) : void
      {
         var at:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("survarium.data.LocalizationData").setData({wear:"Wear",drop:"Drop",sell:"Sell",buy:"Buy",non_faction_items:"Any",reroll:"Change",item_modification:"Change modification"});
            var Container:Class = C("survarium.data.IconContainer");
            for each(var icons:String in ["equipment","weapons","camo","consumables","bundles","factionItemIcons","modifiers"])
            {
               new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
            }
            at = "the items";
            var Category:Object = domain.getDefinition("survarium.constants.ItemCategory");
            var descriptor:* = new (C("survarium.data.ItemDescriptor"))("modification-card-check");
            descriptor.setItemsDictionary([{dictId:RIFLE,category:Category.SNIPER_RIFLES,name:"Mosin carbine",level:4,icon_row:1,icon_column:1,is_stack:false,modifiers:[],item_properties:[]}]);
            lobby._itemDescriptor = descriptor;
            at = "the lobby's parts";
            inventory = lobby.inventory_list;
            var shop:* = lobby.shop_list;
            var card:* = lobby.item_control;
            shop.registerItemDescriptor(descriptor);
            shop.setupShopItems([]);
            inventory.registerItemDescriptor(descriptor);
            card.setInventory(inventory);
            inventory.item_control = card;
            lobby.player_profile.registerInventory(inventory);
            for(var i:int = 0; i < lobby.numChildren; i++)
            {
               lobby.getChildAt(i).visible = lobby.getChildAt(i).name == "stageLayer";
            }
            inventory.x = 960;
            inventory.y = 30;
            inventory.visible = true;
            lobby.addChild(inventory);
            domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
            domain.getDefinition("scaleform.clik.managers.DragManager").init(stage,lobby);
            at = "the scale";
            // LobbyMenu.onStageResize at 2560 x 1440: the root at the scale, its stage layer at the
            // stage's, the tooltips' container at overlayScale, the cards as the root (placeOverlay)
            var Main:Object = domain.getDefinition("MainScene");
            Main.scaleFactor = SCALE;
            Main.overlayScale = Math.max(1,SCALE);
            domain.getDefinition("survarium.controls.Message").setScaleFactor(SCALE);
            lobby.scaleX = lobby.scaleY = SCALE;
            var layer:DisplayObjectContainer = DisplayObjectContainer(lobby).getChildByName("stageLayer") as DisplayObjectContainer;
            layer.scaleX = layer.scaleY = 1 / SCALE;
            // the overlay (the stage layer's last): the corner buttons, the cards, then the
            // tooltips' container, which the root's constructor adds after its setup
            var overlay:DisplayObjectContainer = DisplayObjectContainer(layer.getChildAt(layer.numChildren - 1));
            cards = DisplayObjectContainer(overlay.getChildAt(1));
            lobby.tipContainer = new Sprite();
            overlay.addChild(lobby.tipContainer);
            domain.getDefinition("scaleform.clik.controls.ToolTip").tipContainer = lobby.tipContainer;
            lobby.tipContainer.scaleX = lobby.tipContainer.scaleY = Main.overlayScale;
            at = "the messages";
            // the messages' layer (the root's _top_container, the victory screen's top_container),
            // in the layers as the root's setup puts it after where it stops, the window registered
            var top:DisplayObjectContainer = lobby.victory_screen.top_container;
            DisplayObjectContainer(layer.getChildAt(0)).addChild(top);
            var Message:Object = domain.getDefinition("survarium.controls.Message");
            Message.registerMessage("upgrade_item",C("SrvMessageRerollItemSkinned"));
            Message.initStage(top);
            cards.scaleX = cards.scaleY = SCALE;
            addEventListener(Event.ENTER_FRAME,this.tick);
         }
         catch(err:Error)
         {
            finish("fail: " + at + ": " + err + " " + err.getStackTrace());
         }
      }

      private function expect(what:String, ok:Boolean, got:String) : void
      {
         trace("MODCARD " + what + ": " + got + (ok ? "" : " (wrong)"));
         if(!ok)
         {
            failures.push(what + ": " + got);
         }
      }

      // The card the lobby shows, in the stage's pixels; null when none is up.
      private function shown() : Rectangle
      {
         var hint:* = lobby.itemHintRenderer;
         return hint.parent == cards && hint.data != null ? DisplayObject(hint).getBounds(stage) : null;
      }

      private function onScreen(r:Rectangle) : Boolean
      {
         return r != null && r.left >= -1 && r.top >= -1 && r.right <= stage.stageWidth + 1 && r.bottom <= stage.stageHeight + 1;
      }

      private static function size(r:Rectangle) : String
      {
         return r == null ? "none" : Math.round(r.x) + "," + Math.round(r.y) + " " + Math.round(r.width) + "x" + Math.round(r.height);
      }

      private function row() : *
      {
         var list:* = inventory.itemsTileList;
         for(var i:int = 0; i < list.dataProvider.length; i++)
         {
            var r:* = list.getRendererAt(i,0);
            if(r != null && r.data != null && r.data.itemId == OWNED_RIFLE)
            {
               return r;
            }
         }
         throw new Error("no row of the rifle");
      }

      private function picture(type:String) : void
      {
         window.itemRenderer.dispatchEvent(new MouseEvent(type));
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
                  inventory.setupShopItems([]);
                  inventory.setupBundleData([]);
                  inventory.setupInventoryData([{id:OWNED_RIFLE,dictId:RIFLE,faction_id:0,condition_or_stack:10000,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0}]);
                  break;
               case 1:
                  // the inventory row's card: the size to match
                  var r:* = row();
                  var List:Class = C("scaleform.clik.events.ListEvent");
                  inventory.itemsTileList.dispatchEvent(new List(List.ITEM_ROLL_OVER,false,true,r.index,-1,-1,r,r.data));
                  var rowCard:Rectangle = shown();
                  expect("the inventory row's card",rowCard != null,size(rowCard));
                  rowCardWidth = rowCard != null ? rowCard.width : 0;
                  inventory.itemsTileList.dispatchEvent(new List(List.ITEM_ROLL_OUT,false,true,r.index,-1,-1,r,r.data));
                  // the item card's Modification (ItemCard.upgradeItem): the window, a copy of the item
                  var item:* = row().data.clone();
                  item.itemId = row().data.itemId;
                  window = domain.getDefinition("survarium.controls.Message").showMessage("upgrade_item",item,true);
                  window.x = 1000;
                  window.y = 300;
                  break;
               case 2:
                  picture(MouseEvent.ROLL_OVER);
                  break;
               case 3:
                  var card:Rectangle = shown();
                  expect("the window's card is the inventory's",card != null,size(card));
                  expect("at the inventory row's card's width",card != null && Math.abs(card.width - rowCardWidth) < 1,(card != null ? Math.round(card.width) : 0) + " (the row's " + Math.round(rowCardWidth) + ")");
                  var frame:Rectangle = DisplayObject(window.background).getBounds(stage);
                  expect("left of the window",card != null && card.right <= frame.left + 1,size(card) + " | window " + size(frame));
                  expect("on the screen",onScreen(card),size(card));
                  expect("no tooltip",lobby.tipContainer.numChildren == 0,lobby.tipContainer.numChildren + " in the tooltips' container");
                  picture(MouseEvent.ROLL_OUT);
                  expect("the picture left: no card",shown() == null,size(shown()));
                  // no room on the left
                  window.x = 20;
                  picture(MouseEvent.ROLL_OVER);
                  break;
               case 4:
                  var right:Rectangle = shown();
                  var frameLeft:Rectangle = DisplayObject(window.background).getBounds(stage);
                  expect("no room on the left: right of the window",right != null && right.left >= frameLeft.right - 1 && onScreen(right),size(right) + " | window " + size(frameLeft));
                  picture(MouseEvent.ROLL_OUT);
                  // low on the screen
                  window.x = 1000;
                  window.y = 1100;
                  picture(MouseEvent.ROLL_OVER);
                  break;
               case 5:
                  expect("the window low: the card on the screen",onScreen(shown()),size(shown()));
                  // the card past the stage's bottom and its RESIZE, as Shift or Alt change its rows
                  // (ItemInfoRenderer.detailsFrame)
                  lobby.itemHintRenderer.y = 900;
                  lobby.itemHintRenderer.dispatchEvent(new Event(Event.RESIZE));
                  break;
               case 6:
                  expect("grown: the card on the screen",onScreen(shown()),size(shown()));
                  // closed under the pointer (Enter, Esc)
                  window.parent.removeChild(window);
                  break;
               case 7:
                  expect("the window closed: no card",shown() == null,size(shown()));
                  // an item tip (another player's gear: ToolTip with SrvItemHintRednerer)
                  var tip:* = new (C("SrvItemHintRednerer"))();
                  lobby.tipContainer.addChild(tip);
                  tip.data = row().data;
                  var drawn:Number = DisplayObject(tip.itemInfo).transform.concatenatedMatrix.a;
                  var lobbyScale:Number = cards.transform.concatenatedMatrix.a;
                  expect("an item tip at the lobby's scale",Math.abs(drawn - lobbyScale) < 0.01 && Math.abs(drawn - SCALE) < 0.01,drawn.toFixed(3) + " (the cards' " + lobbyScale.toFixed(3) + ")");
                  lobby.tipContainer.removeChild(tip);
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

      private function finish(result:String) : void
      {
         trace("MODCARD RESULT " + result);
      }
   }
}
