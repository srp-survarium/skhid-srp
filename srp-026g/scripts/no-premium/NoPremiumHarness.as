package
{
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.utils.Timer;

   // The results screen without premium (the owner, 2026-11-09: "no premium state anywhere should
   // exist"), with the lobby movie's own results screen (SrvVictoryScreenReskin) and a match's report
   // as the exe gives it (LobbyMenu.fill_victory_screen; docs/notes/results.md 1.3, 1.4), its premium
   // block filled as an old server would have, to show that nothing reads it:
   // - no premium row in the bonuses and no "Activate premium" button;
   // - the totals are the rows, the quests, the items' bonus and the day's bonus, never the premium
   //   block;
   // - the spent and total rows sit right under the last bonus row shown (the day's, or the items'
   //   when there is no day's bonus): no gap where the premium row was.
   // Prints "NOPREM ..." lines (trace), "NOPREM SHOT <name>" when the screen is ready for a shot, and
   // ends with "NOPREM RESULT pass" or "NOPREM RESULT fail: ...".
   public class NoPremiumHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var results:*;

      private var failures:Array = [];

      private var steps:Array;

      public function NoPremiumHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            trace("NOPREM uncaught: " + (e.error is Error ? Error(e.error).getStackTrace() : e.error));
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
            results = make("SrvVictoryScreenReskin");
            results.registerHintRenderer(make("InventoryItemHint"));
            Object(loader.content).addChild(results);
         }
         catch(err:Error)
         {
            finish("fail: the lobby: " + err.getStackTrace());
            return;
         }
         steps = [function():void
         {
            show(report(true));
         },function():void
         {
            checkScreen("with the day's bonus",true);
            trace("NOPREM SHOT results");
         },function():void
         {
         },function():void
         {
            show(report(false));
         },function():void
         {
            checkScreen("no day's bonus",false);
            trace("NOPREM SHOT results-no-daily");
         },function():void
         {
         },function():void
         {
            finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
         }];
         var timer:Timer = new Timer(1500,steps.length);
         timer.addEventListener(TimerEvent.TIMER,function(e:TimerEvent):void
         {
            steps.shift()();
         });
         timer.start();
      }

      // A won match of one kit: SHOT1's rows (pay.rs), a quest, the items' 10 %, the premium block an
      // old server sent (half of the rows), and with `daily` the day's doubled experience and
      // reputation.
      private function report(daily:Boolean) : Object
      {
         return {
            match_id:daily ? 7 : 8,
            match_result:2,
            team1_score:5,
            team2_score:4,
            score_components:[{
               name:"Kills",
               amount:[17],
               money:[551],
               experience:[41],
               reputation:[17],
               profile_name:["Kit 1"],
               profile_play_time:["10:00"]
            },{
               name:"Help",
               amount:[4],
               money:[58],
               experience:[4],
               reputation:[4]
            },{
               name:"Activity",
               money:[463],
               experience:[143],
               reputation:[5]
            }],
            statistics_rating:{
               elo_before:118,
               elo_after:120
            },
            statistics_exp:{
               match_exp:188,
               premium_items_exp:[18],
               premium_items_bonus_exp:[10],
               daily_exp:[daily ? 188 : 0],
               daily_bonus_exp:[daily ? 100 : 0],
               premium_exp:[94]
            },
            statistics_money:{
               match_money:1072,
               quest_money:[300],
               premium_items_money:[107],
               premium_items_bonus_money:[10],
               daily_money:[0],
               daily_bonus_money:[0],
               premium_money:[536]
            },
            statistics_reputation:{
               faction_id:1,
               match_rep:26,
               quest_rep:[12],
               premium_items_rep:[2],
               premium_items_bonus_rep:[10],
               daily_rep:[daily ? 26 : 0],
               daily_bonus_rep:[daily ? 100 : 0],
               premium_rep:[13]
            },
            allow_drop_card:false,
            dropped_item:null,
            players:[]
         };
      }

      private function show(data:Object) : void
      {
         try
         {
            results.prepare();
            results.setData(data);
         }
         catch(err:Error)
         {
            trace("NOPREM setData: " + err.getStackTrace());
         }
      }

      private function shown(object:DisplayObject) : Boolean
      {
         for(var at:DisplayObject = object; at != null; at = at.parent)
         {
            if(at == results)
            {
               return true;
            }
            if(!at.visible)
            {
               return false;
            }
         }
         return false;
      }

      private function checkScreen(what:String, daily:Boolean) : void
      {
         var total:MovieClip = results.totalScore;
         var bonuses:MovieClip = total.section_2;
         var sums:MovieClip = total.section_3;
         trace("NOPREM " + what + ": bonuses y " + bonuses.y + ", quests " + bonuses.scoreQuests.y + ", items " + bonuses.scoreEquipment.y + ", daily " + bonuses.scoreDaily.y + " " + bonuses.scoreDaily.visible + ", premium " + (bonuses.scorePremium ? bonuses.scorePremium.y + " " + shown(bonuses.scorePremium) : "none") + "; sums y " + sums.y + "; totals " + sums.scoreTotal.money.text + " / " + sums.scoreTotal.exp.text + " / " + sums.scoreTotal.rep.text);
         // No money is spent after a match (no autobuy, no repair): none of its rows, the total first.
         for each(var spent:String in ["scoreWasted","wastedAmmo","wastedItems","wastedRepair"])
         {
            if(shown(sums[spent]))
            {
               failures.push(what + ": the money spent row " + spent + " is shown");
            }
            if(sums[spent].y < sums.scoreTotal.y - 1)
            {
               failures.push(what + ": the total is under the hidden row " + spent + " (" + sums.scoreTotal.y + ", " + sums[spent].y + ")");
            }
         }
         var button:DisplayObject = results.premium_button;
         if(button != null && shown(button))
         {
            failures.push(what + ": the premium button is shown");
         }
         var premium:DisplayObject = bonuses.getChildByName("scorePremium");
         if(premium != null && shown(premium))
         {
            failures.push(what + ": the premium row is shown");
         }
         // the rows (551 + 58 + 463 / 41 + 4 + 143 / 17 + 4 + 5), the quest, the items, the day
         expect(what + ": money",sums.scoreTotal.money.text,String(1072 + 300 + 107));
         expect(what + ": experience",sums.scoreTotal.exp.text,String(188 + 18 + (daily ? 188 : 0)));
         expect(what + ": reputation",sums.scoreTotal.rep.text,String(26 + 12 + 2 + (daily ? 26 : 0)));
         var last:DisplayObject = daily ? bonuses.scoreDaily : bonuses.scoreEquipment;
         var gap:Number = sums.y - (bonuses.y + last.y);
         var step:Number = bonuses.scoreDaily.y - bonuses.scoreEquipment.y;
         if(gap > step + 12)
         {
            failures.push(what + ": the sums are " + gap + " under the last bonus row (a row is " + step + ")");
         }
      }

      private function expect(what:String, got:String, want:String) : void
      {
         if(got != want)
         {
            failures.push(what + " " + got + ", not " + want);
         }
      }

      private function finish(result:String) : void
      {
         trace("NOPREM RESULT " + result);
      }
   }
}
