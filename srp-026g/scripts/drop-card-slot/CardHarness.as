package
{
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The results screen's drop card slot (docs/notes/drop_cards.md, "The slot"), with the lobby
   // movie's own results screen (SrvVictoryScreenReskin) and PlayerInfo's vouchers:
   // - the slot is open on any screen whose match took no card: a new one or a report reopened,
   //   whatever the match dropped (allow_drop_card false);
   // - a click with no card asks to buy one ("try_buy_map_to_use"); the dialog's yes
   //   (cardBought) waits for it, and the card coming (updateVoucherCount) uses it at once;
   // - a click with a card uses it at once;
   // - the item the card gave is shown again when the match's report is reopened;
   // - a refusal of the buy opens the slot again, a refusal of the card greys it.
   // Ruffle has no exe: ExternalInterface.call throws, so the use is seen in the screen's state.
   // Prints "CARD ..." lines (trace) and ends with "CARD RESULT pass" or "CARD RESULT fail: ...".
   public class CardHarness extends Sprite
   {
      // the drop card's voucher type (survarium.constants.VoucherType.DROP_CARD) is read from the movie
      private static const CARD_DICT:uint = 1501;

      private static const CARD_ITEM:uint = 77;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var results:*;

      private var info:*;

      private var cardType:uint;

      private var buyAsked:int = 0;

      private var failures:Array = [];

      public function CardHarness()
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

      private function make(name:String) : *
      {
         return new (domain.getDefinition(name) as Class)();
      }

      private function loaded(e:Event) : void
      {
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            addChild(loader);
            domain.getDefinition("MainScene").layout = make("scaleform.clik.layout.Layout");
            info = domain.getDefinition("survarium.controls.PlayerInfo");
            cardType = domain.getDefinition("survarium.constants.VoucherType").DROP_CARD;
            results = make("SrvVictoryScreenReskin");
            results.addEventListener("try_buy_map_to_use",function(e:Event):void
            {
               buyAsked++;
            });
            Object(loader.content).addChild(results);
         }
         catch(err:Error)
         {
            finish("fail: the lobby: " + err);
            return;
         }
         buyThenUse();
         useOwned();
         reopenShowsTheItem();
         refusals();
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      private function cards(count:uint) : void
      {
         info.setVoucherCount(CARD_DICT,count,cardType,CARD_ITEM);
      }

      // A match's results as the lobby gives them (LobbyMenu.fill_victory_screen): this match
      // dropped nothing, so the client's allow_drop_card is false.
      private function report(matchId:uint) : Object
      {
         return {
            match_id:matchId,
            score_components:[],
            statistics_rating:{},
            statistics_exp:{},
            statistics_money:{},
            statistics_reputation:{faction_id:1},
            team1_score:0,
            team2_score:0,
            allow_drop_card:false,
            dropped_item:null,
            players:[]
         };
      }

      private function show(data:Object) : void
      {
         try
         {
            results.setData(data);
         }
         catch(err:Error)
         {
            // the rest of the screen outside the game; the slot is set before
            trace("CARD setData: " + err.getStackTrace());
         }
      }

      private function click() : void
      {
         try
         {
            results.searchIcon.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
         }
         catch(err:Error)
         {
            // use_drop_card: no exe
         }
      }

      private function quietly(f:Function) : void
      {
         try
         {
            f();
         }
         catch(err:Error)
         {
         }
      }

      private function open() : Boolean
      {
         return results.searchIcon.visible && results.treasureItem.alpha == 1 && results.treasureItem.itemSlot.data == null;
      }

      private function asked(matchId:uint) : Boolean
      {
         var pending:Object = results.cardPending;
         return pending != null && pending.step == "asked" && pending.match == matchId;
      }

      private function expect(what:String, ok:Boolean) : void
      {
         if(!ok)
            failures.push(what);
      }

      private function item() : *
      {
         var data:* = make("survarium.data.ItemData");
         data.dictId = 46;
         return data;
      }

      private function buyThenUse() : void
      {
         cards(0);
         show(report(101));
         expect("no card: the slot of a match that dropped nothing is not open",open());
         click();
         expect("no card: the click did not ask to buy one (" + buyAsked + ")",buyAsked == 1);
         expect("no card: the click used a card",results.cardPending == null);
         results.cardBought({});
         expect("bought: the slot still open while the card comes",!results.searchIcon.visible);
         click();
         expect("bought: a second click asked again",buyAsked == 1);
         cards(1);
         quietly(results.updateVoucherCount);
         expect("bought: the card that came was not used on the match",asked(101));
         results.setDropItem(item());
         trace("CARD no card: the click buys one, and it is used as soon as it comes");
      }

      private function useOwned() : void
      {
         cards(2);
         show(report(102));
         expect("a card: the slot not open",open());
         click();
         expect("a card: the click did not use it",asked(102));
         expect("a card: the click asked to buy",buyAsked == 1);
         trace("CARD a card: the click uses it");
      }

      // The card's answer (LobbyMenu.fill_victory_screen_drop_item): the item in the slot and on the
      // screen's report; the report reopened (LobbyMenu.showFullReaport) shows it.
      private function reopenShowsTheItem() : void
      {
         cards(1);
         var data:Object = report(103);
         show(data);
         click();
         var got:* = item();
         results.setDropItem(got);
         results.getData().treasureItem = got;
         show(report(104));
         expect("another match: the slot not open",open());
         show(data);
         expect("reopened: the card's item not in the slot",results.treasureItem.itemSlot.data == got);
         expect("reopened: the search icon shown over the item",!results.searchIcon.visible);
         show(report(104));
         expect("reopened another: the slot kept the item",results.treasureItem.itemSlot.data == null && open());
         trace("CARD the card's item is shown again when its match's report is reopened");
      }

      private function refusals() : void
      {
         cards(0);
         show(report(105));
         click();
         results.cardBought({});
         results.messageOver("ok");
         expect("buy refused: the slot not open again",open());
         cards(1);
         click();
         results.messageOver("ok");
         expect("card refused: the slot not grey",results.treasureItem.alpha == 0.5 && !results.searchIcon.visible);
         show(report(105));
         expect("card refused, reopened: the slot not grey",results.treasureItem.alpha == 0.5 && !results.searchIcon.visible);
         trace("CARD a refused buy opens the slot again; a refused card greys it, reopened too");
      }

      private function finish(result:String) : void
      {
         trace("CARD RESULT " + result);
      }
   }
}
