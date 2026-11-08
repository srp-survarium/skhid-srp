package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.Font;
   import flash.text.TextField;
   import flash.text.TextFormat;

   // srp's replay buttons in the lobby movie, in every language (HarnessText):
   // - a History match row (NotificationListScrollContent) has Download and Watch at its top right,
   //   side by side, each label fitting its button, the map name left room;
   // - Watch asks; the server's answers (`R1;<status>;<match>;<n>`, PlayerRanking's rows) set the
   //   row's chip under Watch: watching/ready Ready, preparing Preparing, incompatible Old version,
   //   not_recorded Not recorded, gone Expired; an answer that says nothing of the replay (busy)
   //   and an answer for another match leave it; the chip fits the row;
   // - the results screen (SrvVictoryScreenReskin) shows Watch replay and Watch my deaths for a
   //   match with an id, left of the close button, labels fitting; none without an id; Watch my
   //   deaths asks through the History's prompt (its answer sets the History row's chip).
   // Ruffle has no exe (ExternalInterface throws): the asks are seen through the answers taken.
   // Prints "REPLAYBUTTONS ..." lines and ends with "REPLAYBUTTONS RESULT pass" or "... fail: ...".
   public class ReplayButtonsHarness extends Sprite
   {
      private static const MATCH:uint = 0x1CE45CBE;

      private static const OTHER:uint = 0x69F8C4BC;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var words:*;

      private var Prompt:*;

      private var Click:*;

      private var font:Font;

      private var failures:Array = [];

      public function ReplayButtonsHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe)
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
            words = domain.getDefinition("survarium.data.LocalizationData");
            Prompt = domain.getDefinition("survarium.controls.NotificationListScrollContent");
            Click = domain.getDefinition("scaleform.clik.events.ButtonEvent");
            font = make("Fonts_CB_Bold") as Font;
            addChild(loader);
            domain.getDefinition("MainScene").layout = make("scaleform.clik.layout.Layout");
            for each(var language:String in HarnessText.LANGUAGES)
            {
               words.setData(HarnessText.DATA[language]);
               history(language);
               results(language);
            }
         }
         catch(err:Error)
         {
            failures.push("the lobby: " + err.getStackTrace());
         }
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      private function expect(what:String, ok:Boolean) : void
      {
         if(!ok)
            failures.push(what);
      }

      private function find(box:DisplayObjectContainer, name:String) : DisplayObject
      {
         for(var i:int = 0; i < box.numChildren; i++)
         {
            var child:DisplayObject = box.getChildAt(i);
            if(child.name == name)
               return child;
            if(child is DisplayObjectContainer)
            {
               var deeper:DisplayObject = find(child as DisplayObjectContainer,name);
               if(deeper != null)
                  return deeper;
            }
         }
         return null;
      }

      // How wide `text` is in the movie's bold Calibri at `size`.
      private function measure(text:String, size:Number) : Number
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.defaultTextFormat = new TextFormat(font.fontName,size,0xFFFFFF,true);
         field.text = text;
         return field.textWidth;
      }

      private function fits(what:String, button:*) : void
      {
         var size:Number = Number(button.textField.getTextFormat().size);
         var want:Number = measure(button.label,size) + 8;
         expect(what + " '" + button.label + "' needs " + int(want) + " px, has " + int(button.width),want <= button.width);
      }

      private function answer(status:String, match:uint, n:int = 0) : void
      {
         var hex:String = match.toString(16);
         while(hex.length < 8)
            hex = "0" + hex;
         Prompt.replayTake([{"nickname":"R1;" + status + ";" + hex + ";" + n,"place":0,"ranking":0,"ranking_diff":0}]);
      }

      private function chip(row:DisplayObjectContainer) : String
      {
         var box:DisplayObjectContainer = find(row,"srpReplayChip") as DisplayObjectContainer;
         return box == null ? null : TextField(box.getChildAt(0)).text;
      }

      // The History with one row of MATCH; its row.
      private function historyRow(list:*) : DisplayObjectContainer
      {
         var types:* = domain.getDefinition("survarium.constants.NotificationType");
         list.registerRenderer(types.MATCH,domain.getDefinition("NotificationMatchRenderer"));
         Object(loader.content).addChild(list);
         list.addNotification({"notificationType":types.MATCH,"label":"","match_id":MATCH,"map_name":"Vostok Radar Station",
            "time":"19:05","result":"100:98","match_result":1,"reward":null});
         var watch:DisplayObject = find(list,"srpReplayWatch");
         return watch == null ? null : watch.parent;
      }

      private function history(language:String) : void
      {
         var list:* = make("SrvNotificationListSC");
         var row:DisplayObjectContainer = historyRow(list);
         if(row == null)
         {
            failures.push(language + ": no Watch on the History row");
            return;
         }
         var watch:* = find(row,"srpReplayWatch");
         var download:* = find(row,"srpReplay");
         expect(language + ": no Download on the History row",download != null);
         if(download == null)
            return;
         fits(language + ": History Watch",watch);
         fits(language + ": History Download",download);
         expect(language + ": Download overlaps Watch",download.x + download.width <= watch.x);
         var map:TextField = row["textFieldLabel"] as TextField;
         expect(language + ": the map name is left " + int(map.width) + " px",map != null && map.width >= 60 && map.x + map.width <= download.x);
         var back:DisplayObject = row["background"] as DisplayObject;
         expect(language + ": a chip before any answer",chip(row) == null);
         var cases:Array = [["watching","st_srp_replay_chip_ready"],["preparing","st_srp_replay_chip_preparing"],
            ["incompatible","st_srp_replay_chip_old"],["not_recorded","st_srp_replay_chip_not_recorded"],
            ["gone","st_srp_replay_chip_gone"],["ready","st_srp_replay_chip_ready"]];
         for each(var c:Array in cases)
         {
            watch.dispatchEvent(new Click(Click.CLICK));
            answer(c[0],MATCH);
            var want:String = words.getName(c[1]);
            expect(language + ": " + c[0] + " gave the chip '" + chip(row) + "', not '" + want + "'",chip(row) == want);
            var box:DisplayObject = find(row,"srpReplayChip");
            if(box != null)
               expect(language + ": the chip '" + want + "' leaves the row",box.x >= 0 && box.x + box.width <= back.width
                  && box.y >= watch.y + watch.height && box.y + box.height <= back.height);
            answer("failed",MATCH);
         }
         watch.dispatchEvent(new Click(Click.CLICK));
         answer("busy",MATCH);
         expect(language + ": busy changed the chip",chip(row) == words.getName("st_srp_replay_chip_ready"));
         watch.dispatchEvent(new Click(Click.CLICK));
         answer("gone",OTHER);
         expect(language + ": another match's answer changed the chip",chip(row) == words.getName("st_srp_replay_chip_ready"));
         answer("failed",MATCH);
         list.parent.removeChild(list);
         trace("REPLAYBUTTONS " + language + ": History row: Download, Watch and its chips");
      }

      private function report(match:uint) : Object
      {
         return {
            match_id:match,
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

      private function show(screen:*, match:uint) : void
      {
         try
         {
            screen.setData(report(match));
         }
         catch(err:Error)
         {
            // the rest of the screen outside the game; the buttons are placed before
         }
      }

      private function results(language:String) : void
      {
         var list:* = make("SrvNotificationListSC");
         var row:DisplayObjectContainer = historyRow(list);
         var screen:* = make("SrvVictoryScreenReskin");
         Object(loader.content).addChild(screen);
         var buttons:Array = [find(screen,"srpWatchReplay"),find(screen,"srpWatchDeaths")];
         show(screen,0);
         expect(language + ": results buttons without a match id",!buttons[0].visible && !buttons[1].visible);
         show(screen,MATCH);
         var close:* = screen.btn_close;
         expect(language + ": results buttons hidden",buttons[0].visible && buttons[1].visible);
         fits(language + ": results Watch replay",buttons[0]);
         fits(language + ": results Watch my deaths",buttons[1]);
         expect(language + ": results buttons overlap",buttons[0].x + buttons[0].width <= buttons[1].x);
         expect(language + ": results buttons reach the close button",buttons[1].x + buttons[1].width <= close.x);
         buttons[1].dispatchEvent(new Click(Click.CLICK));
         answer("watching",MATCH,4000);
         expect(language + ": Watch my deaths did not ask (no chip on the History row)",row != null
            && chip(row) == words.getName("st_srp_replay_chip_ready"));
         screen.parent.removeChild(screen);
         list.parent.removeChild(list);
         trace("REPLAYBUTTONS " + language + ": results: Watch replay, Watch my deaths");
      }

      private function finish(result:String) : void
      {
         trace("REPLAYBUTTONS RESULT " + result);
      }
   }
}
