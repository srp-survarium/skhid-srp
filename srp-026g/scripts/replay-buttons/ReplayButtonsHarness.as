package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.geom.Rectangle;
   import flash.system.LoaderContext;
   import flash.text.Font;
   import flash.text.TextField;
   import flash.text.TextFormat;

   // srp's replay buttons in the lobby movie, in every language (HarnessText):
   // - a History match row (NotificationListScrollContent) has Watch at its top right (no
   //   Download), its label fitting, the map name left room; no number strip above the History;
   // - Watch asks; the server's answers (`R1;<status>;<match>;<n>`, PlayerRanking's rows) set the
   //   row's chip under Watch: watching/ready Ready, preparing Preparing, incompatible Old version,
   //   not_recorded Not recorded, gone Expired; an answer that says nothing of the replay (busy)
   //   and an answer for another match leave it; the chip fits the row; the row's number;
   // - the results screen (SrvVictoryScreenReskin) shows Watch replay and Watch my deaths for a
   //   match with an id, left of the close button, labels fitting; none without an id; Watch my
   //   deaths asks through the History's prompt (its answer sets the History row's chip); the
   //   match's number after them;
   // - the Watch window (survarium.watch.WatchWindow) fed WatchFake's rows: Live (friends' first,
   //   Watch only where one may, the roster and its players, a friend's Spectate), Recent (each
   //   replay state's chip and Watch, an answer's chip, Friends only, back on its tab after a
   //   replay), My matches (results, My deaths), a player's matches (another's rows left), the
   //   number box; every card's texts apart and its buttons' labels fitting, the window in the stage.
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
            domain.getDefinition("MainScene").stage = this;
            for each(var language:String in HarnessText.LANGUAGES)
            {
               words.setData(HarnessText.DATA[language]);
               history(language);
               results(language);
               watch(language);
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
         expect(language + ": a Download on the History row",find(row,"srpReplay") == null);
         expect(language + ": the number strip above the History",find(list,"srpReplayNumberBox") == null);
         fits(language + ": History Watch",watch);
         var map:TextField = row["textFieldLabel"] as TextField;
         expect(language + ": the map name is left " + int(map.width) + " px",map != null && map.width >= 60 && map.x + map.width <= watch.x);
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
         var number:TextField = find(row,"srpReplayNumberText") as TextField;
         var chipBox:DisplayObject = find(row,"srpReplayChip");
         expect(language + ": the row's number is '" + (number ? number.text : null) + "'",number != null && number.text == "#1ce45cbe");
         expect(language + ": the row's number overlaps the chip",number != null && (chipBox == null || number.x + number.width <= chipBox.x));
         for(var i:int = 0; number != null && i < row.numChildren; i++)
         {
            var other:TextField = row.getChildAt(i) as TextField;
            if(other == null || other == number || !other.visible || other.text == "")
               continue;
            var left:Number = other.x;
            var right:Number = other.x + Math.min(other.width,other.textWidth + 4);
            var apart:Boolean = right <= number.x || left >= number.x + number.width
               || other.y + other.height - 2 <= number.y || other.y >= number.y + number.height;
            expect(language + ": the row's number overlaps '" + other.text + "'",apart);
         }
         list.parent.removeChild(list);
         trace("REPLAYBUTTONS " + language + ": History row: Watch, its chips and number");
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
         var number:TextField = find(screen,"srpMatchNumber") as TextField;
         expect(language + ": the results' number is '" + (number ? number.text : null) + "'",number != null && number.visible && number.text == "#1ce45cbe");
         expect(language + ": the results' number reaches the close button",number != null && buttons[1].x + buttons[1].width <= number.x
            && number.x + number.width <= close.x);
         buttons[1].dispatchEvent(new Click(Click.CLICK));
         answer("watching",MATCH,4000);
         expect(language + ": Watch my deaths did not ask (no chip on the History row)",row != null
            && chip(row) == words.getName("st_srp_replay_chip_ready"));
         screen.parent.removeChild(screen);
         list.parent.removeChild(list);
         trace("REPLAYBUTTONS " + language + ": results: Watch replay, Watch my deaths");
      }

      // ---- the Watch window (survarium.watch.WatchWindow), fed WatchFake's rows ----

      // The window, made by the bottom bar as in the lobby (or made here when the bar did not).
      private function watchWindow() : *
      {
         var W:* = domain.getDefinition("survarium.watch.WatchWindow");
         if(W.me == null)
         {
            var panel:* = make("SrvBottomPanel");
            panel.y = stage.stageHeight - panel.height + 13;
            Object(loader.content).addChild(panel);
            if(W.me == null)
               W.me = new W(panel);
            Object(loader.content).addChild(W.me);
            W.me.layout();
         }
         return W.me;
      }

      private function take(names:Array) : Boolean
      {
         return domain.getDefinition("survarium.watch.WatchWindow").take(WatchFake.rows(names));
      }

      private function pickTab(win:*, tab:String) : void
      {
         var at:DisplayObject = find(win,"srpWatchTab_" + tab);
         if(at == null)
            failures.push("no tab " + tab);
         else
            at.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
      }

      private function click(box:DisplayObjectContainer, name:String) : void
      {
         var b:* = find(box,name);
         if(b != null)
            b.dispatchEvent(new Click(Click.CLICK));
      }

      private function textOf(box:DisplayObjectContainer, name:String) : String
      {
         var field:TextField = find(box,name) as TextField;
         return field == null ? null : field.text;
      }

      private function chipText(card:DisplayObjectContainer) : String
      {
         var box:DisplayObjectContainer = find(card,"srpWatchChip") as DisplayObjectContainer;
         return box == null ? null : TextField(box.getChildAt(0)).text;
      }

      // A card's texts: none past the chip and buttons, none over the next one on its line; its
      // buttons' labels fit; nothing past the card.
      private function cardFits(what:String, card:DisplayObjectContainer, w:Number) : void
      {
         var stop:Number = w;
         var fields:Array = [];
         for(var i:int = 0; i < card.numChildren; i++)
         {
            var part:DisplayObject = card.getChildAt(i);
            if(part.name == "srpWatchChip" || part.name == "srpWatchWatch" || part.name == "srpWatchPlayers" || part.name == "srpWatchDeaths")
            {
               stop = Math.min(stop,part.x);
               expect(what + ": " + part.name + " leaves the card",part.x >= 0 && part.x + part.width <= w + 1);
               if(part.name != "srpWatchChip")
                  fits(what + " " + part.name,part);
            }
            else if(part is TextField && TextField(part).text != "")
               fields.push(part);
         }
         fields.sortOn("x",Array.NUMERIC);
         for(var j:int = 0; j < fields.length; j++)
         {
            var f:TextField = fields[j];
            var right:Number = f.x + Math.min(f.width,f.textWidth + 4);
            expect(what + ": '" + f.text + "' reaches the chip or buttons",right <= stop);
            for(var k:int = j + 1; k < fields.length; k++)
               if(Math.abs(fields[k].y - f.y) < 8)
               {
                  expect(what + ": '" + f.text + "' overlaps '" + fields[k].text + "'",right <= fields[k].x);
                  break;
               }
         }
      }

      private function cards(win:*) : Array
      {
         var list:Array = [];
         for(var i:int = 0; find(win,"srpWatchCard" + i) != null; i++)
            list.push(find(win,"srpWatchCard" + i));
         return list;
      }

      // The window's place: in the stage, over the bottom bar, and the toolbar's texts apart.
      private function watchFrame(language:String, win:*) : void
      {
         var r:Rectangle = win.getBounds(stage);
         expect(language + ": the Watch window leaves the stage " + r,r.x >= 0 && r.y >= 0 && r.right <= stage.stageWidth && r.bottom <= stage.stageHeight);
         var box:DisplayObject = find(win,"srpWatchNumberBox");
         var hint:TextField = find(win,"srpWatchNumberHint") as TextField;
         fits(language + ": the number box's Watch",find(win,"srpWatchNumberWatch"));
         expect(language + ": the number box overlaps its hint or Watch",hint != null && hint.x + hint.textWidth <= box.x
            && box.x + box.width <= find(win,"srpWatchNumberWatch").x);
      }

      private function watchLive(language:String, win:*, row:DisplayObjectContainer) : void
      {
         var W:* = domain.getDefinition("survarium.watch.WatchWindow");
         W.open("live");
         expect(language + ": the window did not open on Live",win.shown && win.tab == "live");
         expect(language + ": L0 rows not taken",take(WatchFake.LIVE));
         var list:Array = cards(win);
         expect(language + ": Live is not the 5 good matches",list.length == 5);
         if(list.length != 5)
            return;
         expect(language + ": the friends' match is not first",textOf(list[0],"srpWatchNumber") == null
            && String(textOf(list[0],"srpWatchFriends")).indexOf("Petro") >= 0 && find(win,"srpWatchSectionFriends") != null
            && find(win,"srpWatchSectionOthers") != null);
         var head:TextField = find(win,"srpWatchTab_live") ? TextField(DisplayObjectContainer(find(win,"srpWatchTab_live")).getChildAt(0)) : null;
         expect(language + ": the Live tab does not count 5",head != null && head.text.indexOf("(5)") > 0);
         var mays:Array = [true,true,false,false,true];
         for(var i:int = 0; i < 5; i++)
         {
            var watch:* = find(list[i],"srpWatchWatch");
            expect(language + ": live match " + i + " Watch " + (watch && watch.enabled ? "on" : "off"),watch != null && watch.enabled == mays[i]);
            cardFits(language + ": live card " + i,list[i],list[i].width);
         }
         watchFrame(language,win);
         click(list[0],"srpWatchWatch");
         answer("watching",MATCH,0);
         expect(language + ": Live's Watch did not ask",chip(row) == words.getName("st_srp_replay_chip_ready"));
         click(list[0],"srpWatchPlayers");
         expect(language + ": L2 rows not taken",take(WatchFake.ROSTER));
         var ada:* = find(win,"srpWatchPlayer4");
         var bob:* = find(win,"srpWatchPlayer10");
         expect(language + ": the roster is not its 6 players in their teams",ada != null && bob != null && find(win,"srpWatchPlayer1") == null
            && bob.x > ada.x && String(bob.label).indexOf("Bob;the;Builder") == 0);
         if(ada != null)
         {
            fits(language + ": the roster's player",ada);
            ada.dispatchEvent(new Click(Click.CLICK));
            answer("preparing",MATCH,1);
            expect(language + ": the roster's player did not ask",chip(row) == words.getName("st_srp_replay_chip_preparing"));
            answer("failed",MATCH);
         }
         click(win,"srpWatchBack");
         expect(language + ": Back did not list the matches again",find(win,"srpWatchCard0") != null && find(win,"srpWatchPlayer4") == null);
         // a friend's Spectate: that friend's match's roster
         W.spectate("PETRO");
         take(WatchFake.ROSTER);
         expect(language + ": Spectate did not open Petro's match's roster",find(win,"srpWatchBack") != null && find(win,"srpWatchPlayer3") != null);
         click(win,"srpWatchBack");
         take(["L0;0"]);
         expect(language + ": an empty Live says '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_matches_none"));
         take(WatchFake.LIVE);
      }

      private function watchRecent(language:String, win:*, row:DisplayObjectContainer) : void
      {
         pickTab(win,"recent");
         expect(language + ": M0 rows not taken",take(WatchFake.RECENT));
         var list:Array = cards(win);
         expect(language + ": Recent is not the 5 good matches",list.length == 5);
         if(list.length != 5)
            return;
         var chips:Array = ["st_srp_replay_chip_not_recorded",null,"st_srp_replay_chip_ready","st_srp_replay_chip_gone","st_srp_replay_chip_old"];
         var mays:Array = [false,true,true,false,false];
         for(var i:int = 0; i < 5; i++)
         {
            var want:String = chips[i] == null ? null : words.getName(chips[i]);
            expect(language + ": recent " + i + " chip '" + chipText(list[i]) + "', not '" + want + "'",chipText(list[i]) == want);
            expect(language + ": recent " + i + " Watch",Object(find(list[i],"srpWatchWatch")).enabled == mays[i]);
            cardFits(language + ": recent card " + i,list[i],list[i].width);
         }
         expect(language + ": the friend not on recent 1",String(textOf(list[1],"srpWatchFriends")).indexOf("Petro") >= 0);
         expect(language + ": recent 1's number",textOf(list[1],"srpWatchNumber") == "#69f8c4bc");
         click(list[1],"srpWatchWatch");
         answer("preparing",OTHER,1);
         expect(language + ": the answer did not set recent 1's chip",chipText(cards(win)[1]) == words.getName("st_srp_replay_chip_preparing"));
         answer("watching",OTHER,0);
         // back on Recent after the replay
         var W:* = domain.getDefinition("survarium.watch.WatchWindow");
         W.away();
         win.show(false);
         W.back();
         expect(language + ": not back on Recent after the replay",win.shown && win.tab == "recent");
         var tick:DisplayObject = find(win,"srpWatchFriendsOnly");
         expect(language + ": no Friends only",tick != null);
         if(tick != null)
         {
            tick.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            take(["M0;0"]);
            expect(language + ": Friends only, none: '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_none_friends"));
            find(win,"srpWatchFriendsOnly").dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            take(WatchFake.RECENT);
         }
      }

      private function watchPlayed(language:String, win:*, row:DisplayObjectContainer) : void
      {
         var W:* = domain.getDefinition("survarium.watch.WatchWindow");
         pickTab(win,"mine");
         expect(language + ": P1 rows not taken",take(WatchFake.played("PocketTest1")));
         var list:Array = cards(win);
         expect(language + ": My matches is not the 3 good rows",list.length == 3);
         if(list.length != 3)
            return;
         var results:Array = ["victory","defeat","st_srp_player_matches_draw"];
         for(var i:int = 0; i < 3; i++)
         {
            expect(language + ": my match " + i + " result",textOf(list[i],"srpWatchResult") == words.getName(results[i]));
            expect(language + ": my match " + i + " has no My deaths",find(list[i],"srpWatchDeaths") != null);
            cardFits(language + ": my card " + i,list[i],list[i].width);
         }
         click(list[0],"srpWatchDeaths");
         answer("watching",MATCH,0);
         expect(language + ": My deaths did not ask",chip(row) == words.getName("st_srp_replay_chip_ready"));
         var ada:String = "Ada_" + language;
         W.open("player",ada);
         expect(language + ": the player's tab",win.tab == "player" && find(win,"srpWatchTab_player") != null);
         take(WatchFake.played("Bob"));
         expect(language + ": another player's rows shown",find(win,"srpWatchCard0") == null);
         take(WatchFake.played(ada));
         list = cards(win);
         expect(language + ": Ada's matches are not the 3 good rows",list.length == 3 && find(list[0],"srpWatchDeaths") == null);
         if(list.length == 3)
         {
            cardFits(language + ": Ada's card",list[0],list[0].width);
            answer("failed",MATCH);
            click(list[0],"srpWatchWatch");
            answer("preparing",MATCH,1);
            expect(language + ": Ada's Watch did not ask",chip(row) == words.getName("st_srp_replay_chip_preparing"));
            answer("failed",MATCH);
         }
      }

      private function watchNumber(language:String, win:*, row:DisplayObjectContainer) : void
      {
         var box:TextField = find(win,"srpWatchNumberBox") as TextField;
         for each(var typed:String in ["#1CE45CBE","1ce45cbe"," #1ce45cbe "])
         {
            answer("gone",OTHER);
            box.text = typed;
            click(win,"srpWatchNumberWatch");
            answer("preparing",MATCH,2);
            expect(language + ": '" + typed + "' was not asked for",chip(row) == words.getName("st_srp_replay_chip_preparing"));
            answer("failed",MATCH);
            box = find(win,"srpWatchNumberBox") as TextField;
         }
         for each(var bad:String in ["","1ce45cbe0","zz","#"])
         {
            box.text = bad;
            click(win,"srpWatchNumberWatch");
            answer("gone",MATCH);
            expect(language + ": '" + bad + "' was asked for",chip(row) == words.getName("st_srp_replay_chip_preparing"));
            box = find(win,"srpWatchNumberBox") as TextField;
         }
      }

      private function watch(language:String) : void
      {
         var list:* = make("SrvNotificationListSC");
         var row:DisplayObjectContainer = historyRow(list);
         var win:* = watchWindow();
         watchLive(language,win,row);
         watchRecent(language,win,row);
         watchPlayed(language,win,row);
         watchNumber(language,win,row);
         win.show(false);
         expect(language + ": rows not taken with the window closed",take(["L0;0"]) && take(["M0;0"]));
         list.parent.removeChild(list);
         trace("REPLAYBUTTONS " + language + ": Watch window: Live, roster, Spectate, Recent, My matches, a player's, the number box");
      }

      private function finish(result:String) : void
      {
         trace("REPLAYBUTTONS RESULT " + result);
      }
   }
}
