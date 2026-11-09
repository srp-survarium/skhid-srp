package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.display.StageAlign;
   import flash.display.StageScaleMode;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import flash.utils.getQualifiedClassName;
   import harness.extern.ExternalInterface;

   // srp's Watch (research/watch-redesign.html) in the lobby movie, in every language (HarnessText),
   // the lobby's parts as WatchLobby lays them out at the stage's size (check.sh: 1280x720, 1920x1080):
   // - the top menu: Watch between Rooms and Ranking, every right tab's label whole, none past the
   //   bar's width nor over another; its badge counts the live matches (none for 0); its tab opens
   //   Watch, Rooms' tab closes it (and Watch closes Rooms); no Watch tab on the bottom bar;
   // - the window: tabs Live (its count) and Replays; chips Everyone, Friends on Live, Mine (the
   //   default), Friends, Everyone on Replays, each the list it names (Live's Friends: the matches
   //   with a friend); the search box: a match's code (# optional) watches it, a name lists that
   //   player's matches (find_players' answer, then the P1 rows; another's rows left; a name not
   //   found says so; the asker's own is Mine); the player pop-up's Matches (WatchWindow.openPlayer)
   //   searches the name; every label and hint whole, the chips clear of the box;
   // - the cards: one Watch, or why not (not kept, old version; live: counting down, full, closed,
   //   over; an answer's word), the texts apart and clear of it; the code selectable (no copy icon);
   //   updated in place keyed by match: a refresh under a press keeps the card and its Watch, and
   //   the click still asks; a match gone from the list loses its card;
   // - Live's Watch asks at once from the place of the friend in the match (or anyone's), no
   //   roster; a friend's Spectate watches the friend's match from the friend's place, waits while
   //   it counts down, says so when its 60 s end or the match can't be watched;
   // - one ask-and-poll loop (ReplayAsks) on the window's second: a watch polls 2200000000, a save
   //   2210000000, both from the same tick; after a watch asked from the window, its view again once
   //   it played (the load, `watching`) and not after a refusal, nor for an ask under another's;
   //   Mine not asked at once back in the lobby (BACK_ASK_MS: the history row is not written yet);
   // - a lobby window opening (MainScene.activeWindow) or the rooms window closes Watch;
   // - Save replay (ReplayAsks): a save icon right of the code on the cards that may be watched
   //   (none on live cards or ones that cannot be watched); a press asks `3D 00 FFFFFF01 <match>`
   //   and polls 2210000000; `R2;preparing` keeps it saving, `R2;ready` hands the link to the exe once
   //   (video_tutorial_link_clicked, no argument) and shows it saved; a refusal, or no answer, says
   //   why and it may be saved again; the saving and saved words fit the card;
   // - the results screen: a red Watch replay, then Save replay (Save replay, Saving..., Saved),
   //   left of the close button, their labels whole, the match's number left of them; why not in
   //   their place once known; no Watch my deaths;
   // - a History row: Watch, or why not, and the match's number under it, inside the row.
   // Ruffle has no exe: its calls land in the stand-in (scripts/exe-stand-in, ruffle_movie.py --exe),
   // and the server's answers are told to the movie as the exe would. The movie has no test hooks.
   // Prints "REPLAYBUTTONS ..." lines and ends with "REPLAYBUTTONS RESULT pass" or "... fail: ...".
   public class ReplayButtonsHarness extends Sprite
   {
      private static const MATCH:uint = WatchFake.MATCH;

      private static const OTHER:uint = WatchFake.OTHER;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:WatchLobby;

      private var words:*;

      private var Prompt:*;

      private var Click:*;

      private var Asks:*;

      private var Message:*;


      private var failures:Array = [];

      public function ReplayButtonsHarness()
      {
         stage.align = StageAlign.TOP_LEFT;
         stage.scaleMode = StageScaleMode.NO_SCALE;
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
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain(ApplicationDomain.currentDomain)));
      }

      private function loaded(e:Event) : void
      {
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            words = domain.getDefinition("survarium.data.LocalizationData");
            Prompt = domain.getDefinition("survarium.controls.NotificationListScrollContent");
            Click = domain.getDefinition("scaleform.clik.events.ButtonEvent");
            Asks = domain.getDefinition("survarium.watch.ReplayAsks");
            Message = domain.getDefinition("survarium.controls.Message");
            lobby = new WatchLobby(domain,this,WatchLobby.texts("english"));
            for each(var language:String in HarnessText.LANGUAGES)
            {
               words.setData(WatchLobby.texts(language));
               lobby.top.tabPage.validateNow();
               fresh();
               topMenu(language);
               replays(language);
               search(language);
               live(language);
               copying(language);
               pressing(language);
               loop(language);
               windows(language);
               saving(language);
               history(language);
               results(language);
               lobby.win.show(false);
            }
         }
         catch(err:Error)
         {
            failures.push("the lobby: " + err.getStackTrace());
         }
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // ---- helpers ----

      private function expect(what:String, ok:Boolean) : void
      {
         if(!ok)
            failures.push(what);
      }

      private function find(box:DisplayObjectContainer, name:String) : DisplayObject
      {
         return WatchLobby.find(box,name);
      }

      private function textOf(box:DisplayObjectContainer, name:String) : String
      {
         var field:TextField = box == null ? null : find(box,name) as TextField;
         return field == null || !field.visible ? null : field.text;
      }

      private function fits(what:String, button:*) : void
      {
         // The button's own field, in the game's font: a field made here does not see the movie's
         // fonts under Ruffle and measures a wider fallback font. A button draws its label when it
         // validates, so it is made to.
         button.validateNow();
         var want:Number = button.textField.textWidth + 8;
         expect(what + " shows its label ('" + button.textField.text + "', '" + button.label + "')",button.textField.text == button.label);
         expect(what + " '" + button.label + "' needs " + int(want) + " px, has " + int(button.width),want <= button.width);
      }

      // A field shows its whole text (RoomsLook.fitName cuts with dots).
      private function whole(what:String, field:TextField, text:String) : void
      {
         expect(what + ": '" + (field ? field.text : null) + "' is not '" + text + "' whole",field != null && field.text == text
            && field.textWidth <= field.width - 3);
      }

      private function click(box:DisplayObjectContainer, name:String) : void
      {
         var b:* = find(box,name);
         if(b == null)
            failures.push("no " + name + " to click");
         else
            b.dispatchEvent(new Click(Click.CLICK));
      }

      private function press(box:DisplayObjectContainer, name:String) : void
      {
         var b:* = find(box,name);
         if(b == null)
            failures.push("no " + name + " to press");
         else
            b.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
      }

      private function hex(match:uint) : String
      {
         var text:String = match.toString(16);
         while(text.length < 8)
            text = "0" + text;
         return text;
      }

      private function get win() : *
      {
         return lobby.win;
      }

      private function get W() : *
      {
         return lobby.W;
      }

      private function cards() : Array
      {
         var list:Array = [];
         for(var i:int = 0; find(win,"srpWatchCard" + i) != null; i++)
            list.push(find(win,"srpWatchCard" + i));
         return list;
      }

      // The prompt idle again and nothing known of the harness's matches' replays.
      private function fresh() : void
      {
         for each(var m:uint in [MATCH,OTHER,0x11,0x13,0x14,0x15,0x21,0x22])
         {
            lobby.answer("failed",m);
            Prompt.noteReplay(hex(m),null);
         }
      }

      // The last watch asked for: [match, player].
      private function asked() : Array
      {
         for(var i:int = ExternalInterface.calls.length - 1; i >= 0; i--)
         {
            var c:Array = ExternalInterface.calls[i];
            if(c[0] == "victory_screen_request_player_data" && c[1] >>> 24 == 0xC0)
            {
               return [c[2],c[1]];
            }
         }
         return [0,0];
      }

      // The ask's second (the window's timer's work).
      private function second() : void
      {
         win.dispatchEvent(new TimerEvent(TimerEvent.TIMER));
      }

      // The watch asked for nothing more: its next ask is seen.
      private function idle() : void
      {
         lobby.answer("failed",asked()[0]);
      }

      // The tab and the chip the window shows, as a player sees them chosen (white, a tab or chip's
      // first child is its label); "player" while none is (a searched player's list).
      private function shownTab() : String
      {
         for each(var t:String in ["live","replays"])
         {
            var tab:DisplayObjectContainer = find(win,"srpWatchTab_" + t) as DisplayObjectContainer;
            if(tab != null && TextField(tab.getChildAt(0)).textColor == 0xFFFFFF)
               return t;
         }
         return null;
      }

      private function shownChip() : String
      {
         for each(var id:String in ["everyone","friends","mine"])
         {
            var chip:DisplayObjectContainer = find(win,"srpWatchChip_" + id) as DisplayObjectContainer;
            if(chip != null && TextField(chip.getChildAt(0)).textColor == 0xFFFFFF)
               return id;
         }
         return "player";
      }

      // What the game's message boxes say now, their boxes closed.
      private function told() : Array
      {
         var said:Array = [];
         for(var guard:int = 0; guard < 8; guard++)
         {
            var box:* = null;
            for(var i:int = 0; i < lobby.holder.numChildren; i++)
            {
               var child:* = lobby.holder.getChildAt(i);
               if(String(child.name).indexOf("message_") == 0)
                  box = child;
            }
            if(box == null)
               break;
            said.push(String(box.text));
            Message.close(uint(box.getMessageId()),true);
            if(box.parent != null)
               box.parent.removeChild(box);
         }
         return said;
      }

      // A card's texts: none past its Watch or reason, none over the next one on its line; Watch's
      // label and the reason whole; nothing past the card.
      private function cardFits(what:String, card:DisplayObjectContainer, w:Number) : void
      {
         var stop:Number = w;
         var fields:Array = [];
         for(var i:int = 0; i < card.numChildren; i++)
         {
            var part:DisplayObject = card.getChildAt(i);
            if(part.name == "srpWatchWatch" || part.name == "srpWatchReason")
            {
               stop = Math.min(stop,part.x);
               expect(what + ": " + part.name + " leaves the card",part.x >= 0 && part.x + part.width <= w + 1 && part.y >= 0
                  && part.y + part.height <= card.height + 1);
               if(part.name == "srpWatchWatch")
                  fits(what + " Watch",part);
               else
               {
                  var why:TextField = part as TextField;
                  expect(what + ": the reason '" + why.text + "' takes " + why.numLines + " lines",why.numLines <= 2 && why.textHeight <= why.height);
               }
            }
            else if(part is TextField && TextField(part).text != "")
               fields.push(part);
         }
         fields.sortOn("x",Array.NUMERIC);
         for(var j:int = 0; j < fields.length; j++)
         {
            var f:TextField = fields[j];
            var right:Number = f.x + Math.min(f.width,f.textWidth + 4);
            expect(what + ": '" + f.text + "' reaches the Watch or reason",right <= stop);
            for(var k:int = j + 1; k < fields.length; k++)
               if(Math.abs(fields[k].y - f.y) < 8)
               {
                  expect(what + ": '" + f.text + "' overlaps '" + fields[k].text + "'",right <= fields[k].x);
                  break;
               }
         }
      }

      private function allCardsFit(what:String) : void
      {
         var list:Array = cards();
         for(var i:int = 0; i < list.length; i++)
            cardFits(what + " card " + i,list[i],list[i].width);
      }

      // ---- the top menu ----

      private function topMenu(language:String) : void
      {
         var top:* = lobby.top;
         expect(language + ": Watch is not between Rooms and Ranking",top.WATCH_INDEX == top.MAP_INDEX + 1 && top.RANK_INDEX == top.WATCH_INDEX + 1);
         var tabs:Array = [];
         for each(var at:int in [top.CLAN_INDEX,top.MAP_INDEX,top.WATCH_INDEX,top.RANK_INDEX])
            tabs.push(top.getRenderer(at));
         for(var i:int = 0; i < tabs.length; i++)
         {
            var tab:* = tabs[i];
            if(tab == null)
            {
               failures.push(language + ": top tab " + i + " not drawn");
               continue;
            }
            var label:TextField = tab.textField as TextField;
            expect(language + ": the top tab '" + (label ? label.text : null) + "' is cut (" + int(label ? label.textWidth : 0) + " of " + int(tab.width) + " px)",
               label != null && label.textWidth <= tab.width - 6);
            expect(language + ": top tab " + i + " past the bar",tab.x + tab.width <= top.tabPage.width);
            if(i > 0)
               expect(language + ": top tab " + i + " overlaps the one before",tab.x >= tabs[i - 1].x + tabs[i - 1].width);
         }
         var watch:* = tabs[2];
         var watchLabel:TextField = watch ? watch.textField as TextField : null;
         expect(language + ": the Watch tab says '" + (watchLabel ? watchLabel.text : null) + "'",watchLabel != null
            && watchLabel.text.toUpperCase() == String(words.getName("st_srp_watch_title")).toUpperCase());
         expect(language + ": a Watch tab on the bottom bar",find(lobby.panel,"matchesTab") == null);
         // the badge: the live matches
         // (the movie's own root, which fails outside the game, makes a top menu of its own)
         W.topMenu = top;
         lobby.take(WatchFake.LIVE);
         var badge:DisplayObjectContainer = find(top,"srpWatchBadge") as DisplayObjectContainer;
         expect(language + ": the badge does not say 6",badge != null && badge.visible && TextField(badge.getChildAt(0)).text == "6");
         lobby.take(["L0;0"]);
         expect(language + ": a badge for no live match",badge != null && !badge.visible);
         lobby.take(WatchFake.LIVE);
         // the tab opens Watch, Rooms' closes it; Watch opened closes Rooms
         var Rooms:* = domain.getDefinition("survarium.rooms.RoomsWindow");
         top.selectActiveWindow(top.WATCH_INDEX);
         expect(language + ": the Watch tab did not open Watch",win.visible && top.getActiveWindow() == top.WATCH_INDEX);
         top.selectActiveWindow(top.MAP_INDEX);
         expect(language + ": Rooms did not close Watch",!win.visible && Rooms.me.shown);
         W.open("live");
         expect(language + ": Live is not on Everyone at first",language != HarnessText.LANGUAGES[0] || shownChip() == "everyone");
         expect(language + ": Watch did not close Rooms",win.visible && !Rooms.me.shown && top.getActiveWindow() == top.WATCH_INDEX);
         var r:Rectangle = win.getBounds(stage);
         expect(language + ": the Watch window leaves the stage " + r,r.x >= 0 && r.y >= 0 && r.right <= stage.stageWidth && r.bottom <= stage.stageHeight);
         win.show(false);
         expect(language + ": closing Watch left its tab chosen",top.getActiveWindow() == -1);
         trace("REPLAYBUTTONS " + language + ": top menu: Watch between Rooms and Ranking, its badge, opening and closing");
      }

      // ---- the window ----

      private function toolbar(language:String, chips:Array) : void
      {
         var box:TextField = find(win,"srpWatchSearch") as TextField;
         var hint:TextField = find(win,"srpWatchSearchHint") as TextField;
         whole(language + ": the search hint",hint,words.getName("st_srp_watch_search_hint"));
         var right:Number = 0;
         for each(var c:Array in chips)
         {
            var chip:DisplayObjectContainer = find(win,"srpWatchChip_" + c[0]) as DisplayObjectContainer;
            if(chip == null)
            {
               failures.push(language + ": no chip " + c[0]);
               continue;
            }
            var label:TextField = chip.getChildAt(0) as TextField;
            expect(language + ": the chip '" + label.text + "' is cut",label.text == words.getName(c[1]) && label.x + label.textWidth <= chip.width - 6);
            right = Math.max(right,chip.x + chip.width);
         }
         var delay:TextField = find(win,"srpWatchDelay") as TextField;
         if(delay != null)
            right = Math.max(right,delay.x + delay.textWidth + 4);
         expect(language + ": the chips reach the search box",box != null && right < box.x);
         for each(var t:String in ["live","replays"])
         {
            var tab:DisplayObjectContainer = find(win,"srpWatchTab_" + t) as DisplayObjectContainer;
            var tabLabel:TextField = tab ? tab.getChildAt(0) as TextField : null;
            expect(language + ": the tab '" + (tabLabel ? tabLabel.text : null) + "' is cut",tabLabel != null && tabLabel.text.indexOf("…") < 0);
         }
      }

      private function replays(language:String) : void
      {
         W.open("live");
         press(win,"srpWatchTab_replays");
         expect(language + ": Replays is not on Mine",shownTab() == "replays" && shownChip() == "mine");
         toolbar(language,[["mine","st_srp_watch_chip_mine"],["friends","st_srp_watch_chip_friends"],["everyone","st_srp_watch_chip_everyone"]]);
         expect(language + ": P1 rows not taken",lobby.take(WatchFake.played("PocketTest1")));
         var list:Array = cards();
         expect(language + ": Mine is not the 4 good rows",list.length == 4);
         if(list.length == 4)
         {
            var results:Array = ["defeat","victory","st_srp_player_matches_draw","defeat"];
            var reasons:Array = [null,null,"st_srp_watch_reason_old","st_srp_watch_reason_gone"];
            for(var i:int = 0; i < 4; i++)
            {
               expect(language + ": my match " + i + " result",textOf(list[i],"srpWatchResult") == words.getName(results[i]));
               var why:String = reasons[i] == null ? null : words.getName(reasons[i]);
               expect(language + ": my match " + i + " says '" + textOf(list[i],"srpWatchReason") + "', not '" + why + "'",textOf(list[i],"srpWatchReason") == why
                  && (find(list[i],"srpWatchWatch") != null) == (why == null));
               expect(language + ": a Watch my deaths",find(list[i],"srpWatchDeaths") == null);
            }
            expect(language + ": the map's name is not the pool's",textOf(list[0],"srpWatchMap") == words.getName("st_srp_map_rudnya"));
            allCardsFit(language + ": Mine");
            // Watch: the player's own place's view
            idle();
            click(list[0],"srpWatchWatch");
            expect(language + ": Mine's Watch asked " + asked(),asked()[0] == MATCH && asked()[1] == 0xC0040000);
            lobby.answer("incompatible",MATCH);
            expect(language + ": an answer's old version did not reach the card",textOf(cards()[0],"srpWatchReason") == words.getName("st_srp_watch_reason_old"));
            Prompt.noteReplay(hex(MATCH),null);
         }
         // Friends: recent with a friend; Everyone: recent
         press(win,"srpWatchChip_friends");
         lobby.take(WatchFake.FRIENDS);
         list = cards();
         expect(language + ": Friends is not the friend's match",shownChip() == "friends" && list.length == 1
            && String(textOf(list[0],"srpWatchFriends")).indexOf("Petro") >= 0);
         lobby.take(["M0;0"]);
         expect(language + ": an empty Friends says '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_none_friends"));
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.RECENT);
         list = cards();
         expect(language + ": Everyone is not the 5 good matches",shownChip() == "everyone" && list.length == 5);
         if(list.length == 5)
         {
            var states:Array = [null,null,"st_srp_watch_reason_gone","st_srp_watch_reason_gone","st_srp_watch_reason_old"];
            for(var j:int = 0; j < 5; j++)
            {
               var want:String = states[j] == null ? null : words.getName(states[j]);
               expect(language + ": recent " + j + " says '" + textOf(list[j],"srpWatchReason") + "', not '" + want + "'",textOf(list[j],"srpWatchReason") == want);
            }
            expect(language + ": a map the pool lacks is not a retired map",textOf(list[2],"srpWatchMap") == words.getName("st_srp_watch_map_retired"));
            allCardsFit(language + ": Everyone");
            idle();
            click(list[1],"srpWatchWatch");
            expect(language + ": Everyone's Watch asked " + asked(),asked()[0] == OTHER && asked()[1] == 0xC0FF0000);
            // the game loads the replay, then the lobby says `watching`; back on Everyone after it
            W.away();
            lobby.answer("watching",OTHER);
            win.show(false);
            W.back();
            expect(language + ": not back on Replays, Everyone after the replay",win.visible && shownTab() == "replays" && shownChip() == "everyone");
         }
         trace("REPLAYBUTTONS " + language + ": Replays: Mine, Friends, Everyone, the reasons, Watch, back after a replay");
      }

      private function typeIn(text:String) : void
      {
         var box:TextField = find(win,"srpWatchSearch") as TextField;
         box.text = text;
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN,true,false,0,Keyboard.ENTER));
      }

      private function search(language:String) : void
      {
         // a match's code watches it, with # or without
         for each(var code:String in ["#1CE45CBE","1ce45cbe"])
         {
            idle();
            typeIn(code);
            expect(language + ": the code '" + code + "' asked " + asked(),asked()[0] == MATCH && asked()[1] == 0xC0FF0000);
         }
         idle();
         // a name: the player found, then its matches
         var Info:* = domain.getDefinition("survarium.controls.PlayerInfo");
         Info.nickname = "PocketTest1";
         typeIn("ada");
         expect(language + ": a name did not search",shownTab() == "replays" && shownChip() == "player" && textOf(win,"srpWatchEmpty") == "…");
         expect(language + ": the search took find_players' answer",W.found([{"id":6,"name":"Adam_K"},{"id":5,"name":"Ada"}]));
         expect(language + ": the search box does not say Ada",(find(win,"srpWatchSearch") as TextField).text == "Ada");
         lobby.take(WatchFake.played("Bob"));
         expect(language + ": another player's rows shown",cards().length == 0);
         lobby.take(WatchFake.played("Ada"));
         var list:Array = cards();
         expect(language + ": Ada's matches are not the 4 good rows",list.length == 4);
         for each(var id:String in ["mine","friends","everyone"])
         {
            var chip:DisplayObjectContainer = find(win,"srpWatchChip_" + id) as DisplayObjectContainer;
            expect(language + ": the chip " + id + " is chosen while searching",chip != null && TextField(chip.getChildAt(0)).textColor != 0xFFFFFF);
         }
         if(list.length == 4)
         {
            idle();
            click(list[1],"srpWatchWatch");
            expect(language + ": Ada's Watch asked " + asked(),asked()[0] == OTHER && asked()[1] == 0xC0020000);
         }
         expect(language + ": a players' answer nobody asked for taken",!W.found([]));
         // a name nobody has
         typeIn("Nobody_Here");
         W.found([]);
         expect(language + ": a name not found says '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_no_player").split("%1$s").join("Nobody_Here"));
         // the asker's own name: Mine
         typeIn("pockettest1");
         expect(language + ": the own name is not Mine",shownChip() == "mine");
         // the player pop-up's Matches
         win.show(false);
         W.openPlayer("Ada");
         expect(language + ": the pop-up's Matches did not search Ada",win.visible && shownTab() == "replays" && shownChip() == "player"
            && (find(win,"srpWatchSearch") as TextField).text == "Ada");
         W.found([{"id":5,"name":"Ada"}]);
         lobby.take(WatchFake.played("Ada"));
         expect(language + ": the pop-up's search lists no matches",cards().length == 4);
         press(win,"srpWatchChip_mine");
         expect(language + ": a chip did not end the search",shownChip() == "mine" && (find(win,"srpWatchSearch") as TextField).text == "");
         Info.nickname = "";
         trace("REPLAYBUTTONS " + language + ": search: codes, a name, none found, the own name, the pop-up's Matches");
      }

      private function live(language:String) : void
      {
         press(win,"srpWatchTab_live");
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.LIVE);
         expect(language + ": Live is not on Everyone",shownTab() == "live" && shownChip() == "everyone");
         toolbar(language,[["everyone","st_srp_watch_chip_everyone"],["friends","st_srp_watch_chip_friends"]]);
         var head:TextField = find(win,"srpWatchTab_live") ? TextField(DisplayObjectContainer(find(win,"srpWatchTab_live")).getChildAt(0)) : null;
         expect(language + ": the Live tab does not count 6",head != null && head.text.indexOf("6") > 0 && find(win,"srpWatchLiveDot") != null);
         var list:Array = cards();
         expect(language + ": Live is not the 6 good matches",list.length == 6);
         if(list.length == 6)
         {
            expect(language + ": the friends' match is not first",String(textOf(list[0],"srpWatchFriends")).indexOf("Petro") >= 0);
            var reasons:Array = [null,null,"st_srp_watch_reason_starts","st_srp_watch_reason_full","st_srp_watch_reason_private","st_srp_watch_reason_over"];
            for(var i:int = 0; i < 6; i++)
            {
               var want:String = reasons[i] == null ? null : words.getName(reasons[i]);
               expect(language + ": live " + i + " says '" + textOf(list[i],"srpWatchReason") + "', not '" + want + "'",textOf(list[i],"srpWatchReason") == want
                  && (find(list[i],"srpWatchWatch") != null) == (want == null));
            }
            expect(language + ": a roster's Players",find(win,"srpWatchPlayers") == null);
            allCardsFit(language + ": Live");
            // Watch at once: the friend's place, or anyone's
            idle();
            click(list[0],"srpWatchWatch");
            expect(language + ": Live's Watch did not follow Petro: " + asked(),asked()[0] == MATCH && asked()[1] == 0xC0030000);
            idle();
            click(list[1],"srpWatchWatch");
            expect(language + ": Live's Watch is not anyone's: " + asked(),asked()[0] == OTHER && asked()[1] == 0xC0FF0000);
            idle();
         }
         press(win,"srpWatchChip_friends");
         expect(language + ": Live's Friends is not the friends' match",cards().length == 1);
         lobby.take(["L0;0"]);
         expect(language + ": an empty Live's Friends says '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_none_live_friends"));
         // a friend's Spectate: watched from the friend's place
         win.show(false);
         idle();
         W.spectate("OLHA_KOVALENKO");
         lobby.take(WatchFake.LIVE);
         expect(language + ": Spectate did not watch Olha's match from her place: " + asked(),asked()[0] == MATCH && asked()[1] == 0xC0090000);
         idle();
         trace("REPLAYBUTTONS " + language + ": Live: chips, the reasons, Watch at once following a friend, Spectate");
      }

      private function copying(language:String) : void
      {
         press(win,"srpWatchTab_replays");
         press(win,"srpWatchChip_mine");
         lobby.take(WatchFake.played("PocketTest1"));
         expect(language + ": a copy icon",find(win,"srpWatchCopy") == null);
         // the code selectable; typing leaves it as it was
         var code:TextField = find(cards()[1],"srpWatchNumber") as TextField;
         expect(language + ": the code is not selectable: '" + (code ? code.text : null) + "'",code != null && code.text == "#69f8c4bc" && code.selectable && code.mouseEnabled);
         if(code != null)
         {
            code.text = "edited";
            code.dispatchEvent(new Event(Event.CHANGE));
            expect(language + ": typing changed the code to '" + code.text + "'",code.text == "#69f8c4bc");
         }
         cardFits(language + ": the card with its code",cards()[1],cards()[1].width);
         trace("REPLAYBUTTONS " + language + ": the code selectable, no copy icon");
      }

      // A refresh of the list under a press: the same card and Watch; the click still asks. A match
      // no longer in the list loses its card; one still in it stays the same object in a new order.
      private function pressing(language:String) : void
      {
         press(win,"srpWatchTab_replays");
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.RECENT);
         var before:Array = cards();
         var watch:* = find(before[1],"srpWatchWatch");
         expect(language + ": no Watch to press",watch != null);
         if(watch == null)
            return;
         watch.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true));
         lobby.take(WatchFake.RECENT);
         expect(language + ": a refresh made the card anew",cards()[1] === before[1] && find(cards()[1],"srpWatchWatch") === watch && watch.stage != null);
         idle();
         watch.dispatchEvent(new Click(Click.CLICK));
         expect(language + ": the click under a refresh did not ask: " + asked(),asked()[0] == OTHER && asked()[1] == 0xC0FF0000);
         idle();
         // the friend's match is card 1 of RECENT, and alone in FRIENDS: the same object, moved up
         lobby.take(WatchFake.FRIENDS);
         var after:Array = cards();
         expect(language + ": the kept match is not the same card " + after.length,after.length == 1 && after[0] === before[1] && after[0].y == 0 && find(after[0],"srpWatchWatch") === watch);
         expect(language + ": a match no longer listed kept its card",before[0].parent == null && before[2].parent == null);
         trace("REPLAYBUTTONS " + language + ": cards in place, keyed by match, through a press and a new order");
      }

      // ---- the one ask-and-poll loop ----

      private function polls(start:uint) : int
      {
         var n:int = 0;
         for each(var c:Array in ExternalInterface.calls)
         {
            if(c[0] == "request_players_range" && c[1] == start)
               n++;
         }
         return n;
      }

      // The one loop: a watch and a save poll from the same second, each its own start; a watch
      // that the window asked comes back to its view only once it played; a Spectate that gives up
      // says so.
      private function loop(language:String) : void
      {
         var first:Boolean = language == HarnessText.LANGUAGES[0];
         press(win,"srpWatchTab_live");
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.LIVE);
         ExternalInterface.calls.length = 0;
         // a watch from the window; a poll on the window's second
         click(cards()[1],"srpWatchWatch");
         expect(language + ": the watch did not ask: " + asked(),asked()[0] == OTHER);
         second();
         expect(language + ": the watch did not poll 2200000000",polls(2200000000) == 1);
         // refused: no view to come back to, whatever loads next
         lobby.answer("gone",OTHER);
         Prompt.noteReplay(hex(OTHER),null);
         second();
         expect(language + ": polled after a refusal",polls(2200000000) == 1);
         win.show(false);
         W.away();
         ExternalInterface.calls.length = 0;
         W.back();
         expect(language + ": a refused watch brought Watch back",!win.visible);
         expect(language + ": Mine asked at once back in the lobby, before the history row",polls(2230000001) == 0);
         // a watch under another's is ignored, and leaves no view to come back to
         W.open("live");
         lobby.take(WatchFake.LIVE);
         click(cards()[1],"srpWatchWatch");
         click(cards()[0],"srpWatchWatch");
         lobby.answer("gone",OTHER);
         Prompt.noteReplay(hex(OTHER),null);
         win.show(false);
         W.away();
         W.back();
         expect(language + ": an ignored watch left a view to come back to",!win.visible);
         // no answer at all (31 polls): the same, and the ask ended
         W.open("live");
         lobby.take(WatchFake.LIVE);
         click(cards()[1],"srpWatchWatch");
         win.show(false);
         ExternalInterface.calls.length = 0;
         for(var p:int = 0; p < 31; p++)
            second();
         expect(language + ": polled " + polls(2200000000) + " times",polls(2200000000) == 30);
         W.away();
         W.back();
         expect(language + ": a watch with no answer brought Watch back",!win.visible);
         // the second ask goes after the first ended
         W.open("live");
         lobby.take(WatchFake.LIVE);
         click(cards()[1],"srpWatchWatch");
         // a save under the watch: both polled by one second
         ExternalInterface.calls.length = 0;
         press(win,"srpWatchTab_replays");
         press(win,"srpWatchChip_mine");
         lobby.take(WatchFake.played("PocketTest1"));
         if(first)
         {
            var saveCard:* = cards()[1];
            click(saveCard,"srpWatchSave");
            ExternalInterface.calls.length = 0;
            second();
            expect(language + ": one second did not poll both: " + polls(2200000000) + ", " + polls(2210000000),polls(2200000000) == 1 && polls(2210000000) == 1);
            saveAnswer("gone",OTHER);
            told();
         }
         lobby.answer("watching",OTHER);
         // the watch that played: on coming back the view it was asked from (Live)
         win.show(false);
         W.back();
         expect(language + ": not back on Live after the watch",win.visible && shownTab() == "live");
         W.back();
         win.show(false);
         // a friend's Spectate never listed: after its 60 s it says so, once
         W.spectate("Vlad");
         lobby.take(["L0;0"]);
         for(p = 0; p < 59; p++)
            second();
         expect(language + ": gave up too soon",told().length == 0);
         second();
         var said:Array = told();
         var lost:String = words.getName("st_srp_watch_spectate_lost").split("%1$s").join("Vlad");
         expect(language + ": a Spectate that gave up says " + said,said.length == 1 && said[0] == lost);
         second();
         expect(language + ": it said so again",told().length == 0);
         win.show(false);
         // found, but the match can't be watched: it says why
         W.spectate("Petro");
         lobby.take(["L0;1","L1;00000006;64;5;2;4;4;0;1;300;0;2","F1;00000006;3;0;Petro"]);
         said = told();
         expect(language + ": a closed match's Spectate says " + said,said.length == 1 && said[0] == words.getName("st_srp_watch_reason_private"));
         win.show(false);
         // counting down: waits, then watches once it plays
         ExternalInterface.calls.length = 0;
         W.spectate("Petro");
         lobby.take(["L0;1","L1;00000006;64;5;1;4;4;0;1;0;0;1","F1;00000006;3;0;Petro"]);
         expect(language + ": Spectate didn't wait out a countdown: " + told(),asked()[0] == 0);
         lobby.take(["L0;1","L1;00000006;64;5;2;4;4;0;1;20;0;1","F1;00000006;3;0;Petro"]);
         expect(language + ": Spectate did not watch once it played: " + asked(),asked()[0] == 6 && asked()[1] == 0xC0030000);
         idle();
         win.show(false);
         trace("REPLAYBUTTONS " + language + ": one loop: watch and save polled by one second, back only after a watch played, Spectate gives up with a word");
      }

      // A lobby window or the rooms window opening closes Watch.
      private function windows(language:String) : void
      {
         var Scene:* = domain.getDefinition("MainScene");
         var Rooms:* = domain.getDefinition("survarium.rooms.RoomsWindow");
         W.open("live");
         expect(language + ": Watch did not open",win.visible);
         Scene.activeWindow = lobby.holder["shop_list"];
         expect(language + ": a lobby window opening did not close Watch",!win.visible);
         Scene.activeWindow = null;
         W.open("live");
         Rooms.me.show(true);
         expect(language + ": the rooms window did not close Watch",!win.visible && Rooms.me.shown);
         Rooms.me.show(false);
         W.open("live");
         trace("REPLAYBUTTONS " + language + ": a lobby window or the rooms window closes Watch");
      }

      // ---- Save replay ----

      private function saveCalls(name:String) : Array
      {
         return ExternalInterface.calls.filter(function(c:Array, i:int, all:Array):Boolean
         {
            return c[0] == name;
         });
      }

      private function saveAnswer(status:String, match:uint, n:int = 0) : void
      {
         expect("an R2 row not taken",lobby.take(["R2;" + status + ";" + hex(match) + ";" + n]));
      }

      private function saveState(card:DisplayObjectContainer) : String
      {
         return textOf(card,"srpWatchSaveState");
      }

      private function saving(language:String) : void
      {
         var first:Boolean = language == HarnessText.LANGUAGES[0];
         var savingWord:String = words.getName("st_srp_replay_saving");
         var savedWord:String = words.getName("st_srp_replay_saved");
         press(win,"srpWatchTab_live");
         lobby.take(WatchFake.LIVE);
         for each(var c:DisplayObjectContainer in cards())
            expect(language + ": a save icon on a live card",find(c,"srpWatchSave") == null);
         press(win,"srpWatchTab_replays");
         press(win,"srpWatchChip_mine");
         lobby.take(WatchFake.played("PocketTest1"));
         var list:Array = cards();
         for(var i:int = 0; i < list.length; i++)
            expect(language + ": my match " + i + "'s save icon",(find(list[i],"srpWatchSave") != null) == (i < 2)
               && (i >= 2 || find(list[i],"srpWatchSave").x > find(list[i],"srpWatchNumber").x));
         told();
         ExternalInterface.calls.length = 0;
         if(first)
         {
            // saved: asked, polled, the link handed to the exe once
            expect(language + ": a save before any press",saveState(list[0]) == null);
            click(list[0],"srpWatchSave");
            var asks:Array = saveCalls("victory_screen_request_player_data");
            expect(language + ": the save asked " + asks,asks.length == 1 && asks[0][1] == 0xFFFFFF01 && asks[0][2] == MATCH);
            expect(language + ": the card does not say saving",saveState(cards()[0]) == savingWord);
            cardFits(language + ": the saving card",cards()[0],cards()[0].width);
            click(cards()[1],"srpWatchSave");
            expect(language + ": a second save while one is under way",saveCalls("victory_screen_request_player_data").length == 1);
            second();
            expect(language + ": the save did not poll",polls(2210000000) == 1);
            saveAnswer("preparing",MATCH,2);
            saveAnswer("ready",OTHER,1000);
            expect(language + ": preparing or another match's ready ended the save",Asks.stateOf(MATCH) == "saving" && saveCalls("video_tutorial_link_clicked").length == 0);
            saveAnswer("ready",MATCH,4404019);
            saveAnswer("ready",MATCH,4404019);
            var links:Array = saveCalls("video_tutorial_link_clicked");
            expect(language + ": the link not handed to the exe once with no argument " + links,links.length == 1 && links[0].length == 1);
            expect(language + ": the card does not say saved",saveState(cards()[0]) == savedWord && told().length == 0);
            var before:int = polls(2210000000);
            second();
            expect(language + ": polled after the save",polls(2210000000) == before);
            var askedSoFar:int = saveCalls("victory_screen_request_player_data").length;
            click(cards()[0],"srpWatchSave");
            expect(language + ": a saved replay asked again",saveCalls("victory_screen_request_player_data").length == askedSoFar);
         }
         expect(language + ": the saved card says '" + saveState(cards()[0]) + "'",saveState(cards()[0]) == savedWord);
         cardFits(language + ": the saved card",cards()[0],cards()[0].width);
         // refused: why, and it may be saved again; no answer: the same
         click(cards()[1],"srpWatchSave");
         expect(language + ": the other card does not say saving",saveState(cards()[1]) == savingWord);
         cardFits(language + ": the saving card",cards()[1],cards()[1].width);
         saveAnswer("gone",OTHER);
         var said:Array = told();
         expect(language + ": a refusal told " + said,said.length == 1 && said[0] == words.getName("st_srp_replay_gone") && saveState(cards()[1]) == null);
         click(cards()[1],"srpWatchSave");
         for(var p:int = 0; p <= 30; p++)
            second();
         said = told();
         expect(language + ": no answer told " + said,said.length == 1 && said[0] == words.getName("st_srp_replay_no_answer") && Asks.stateOf(OTHER) == "save");
         trace("REPLAYBUTTONS " + language + ": Save: the icon, asked, polled, the link once, saved, refused, no answer");
      }

      // ---- History and the results screen ----

      private function history(language:String) : void
      {
         var list:* = lobby.make("SrvNotificationListSC");
         list.registerRenderer(domain.getDefinition("survarium.constants.NotificationType").MATCH,domain.getDefinition("NotificationMatchRenderer"));
         lobby.history.parent.addChild(list);
         lobby.history.parent.removeChild(lobby.history);
         lobby.history = list;
         list.x = 1920 - 440;
         list.y = 300;
         fresh();
         var row:DisplayObjectContainer = lobby.historyRow(MATCH,"Rudnya","87:64",0);
         if(row == null)
         {
            failures.push(language + ": no Watch on the History row");
            return;
         }
         var watch:* = find(row,"srpReplayWatch");
         fits(language + ": History Watch",watch);
         var back:DisplayObject = row["background"] as DisplayObject;
         var map:TextField = row["textFieldLabel"] as TextField;
         expect(language + ": the map name is left " + int(map.width) + " px",map != null && map.width >= 60 && map.x + map.width <= watch.x);
         var number:TextField = find(row,"srpReplayNumberText") as TextField;
         expect(language + ": the row's number is '" + (number ? number.text : null) + "'",number != null && number.text == "#1ce45cbe"
            && number.y >= watch.y + watch.height);
         idle();
         watch.dispatchEvent(new Click(Click.CLICK));
         expect(language + ": History's Watch did not ask",asked()[0] == MATCH && asked()[1] == 0xC0FF0000);
         // the server's word: gone, then the rows of the asker's matches
         for each(var c:Array in [["gone","st_srp_watch_reason_gone"],["incompatible","st_srp_watch_reason_old"]])
         {
            idle();
            watch.dispatchEvent(new Click(Click.CLICK));
            lobby.answer(c[0],MATCH);
            var why:TextField = find(row,"srpReplayReason") as TextField;
            expect(language + ": " + c[0] + " shows '" + (why && why.visible ? why.text : null) + "'",why != null && why.visible && why.text == words.getName(c[1]) && !watch.visible);
            if(why != null)
               expect(language + ": the reason '" + why.text + "' leaves the row",why.x >= 0 && why.x + why.width <= back.width && why.numLines <= 2
                  && number.y >= why.y + why.textHeight && number.y + number.height <= back.height);
            Prompt.noteReplay(hex(MATCH),null);
         }
         expect(language + ": Watch did not come back once the replay may be watched",watch.visible);
         lobby.take(WatchFake.played("PocketTest1"));
         var old:DisplayObjectContainer = lobby.historyRow(0x21,"Chemical Plant","100:64",1);
         expect(language + ": the row of an old version's match does not say so",old != null && textOf(old,"srpReplayReason") == words.getName("st_srp_watch_reason_old"));
         trace("REPLAYBUTTONS " + language + ": History row: Watch or why not, its number");
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
         var screen:* = lobby.make("SrvVictoryScreenReskin");
         lobby.holder.addChild(screen);
         var button:* = find(screen,"srpWatchReplay");
         var save:* = find(screen,"srpSaveReplay");
         var number:TextField = find(screen,"srpMatchNumber") as TextField;
         show(screen,0);
         expect(language + ": Watch or Save replay without a match id",!button.visible && !save.visible);
         fresh();
         show(screen,MATCH);
         number = find(screen,"srpMatchNumber") as TextField;
         var close:* = screen.btn_close;
         expect(language + ": Watch replay hidden",button.visible);
         expect(language + ": Watch replay is not red: " + getQualifiedClassName(button),getQualifiedClassName(button) == "SrvButtonSkinnedRed");
         fits(language + ": results Watch replay",button);
         expect(language + ": Save replay hidden",save.visible);
         expect(language + ": Save replay is red",getQualifiedClassName(save) != "SrvButtonSkinnedRed");
         expect(language + ": Save replay is not just left of the close button",save.x + save.width <= close.x && close.x - (save.x + save.width) <= 20);
         expect(language + ": Watch replay is not just left of Save replay",button.x + button.width <= save.x && save.x - (button.x + button.width) <= 20);
         // MATCH is saved (saving); OTHER may be: Save replay, Saving..., back
         expect(language + ": the saved match's button says '" + save.label + "'",save.label == words.getName("st_srp_replay_saved"));
         fits(language + ": results Saved",save);
         show(screen,OTHER);
         expect(language + ": Save replay says '" + save.label + "'",save.label == words.getName("st_srp_replay_save"));
         fits(language + ": results Save replay",save);
         expect(language + ": Watch replay is not just left of Save replay",button.x + button.width <= save.x && save.x - (button.x + button.width) <= 20);
         ExternalInterface.calls.length = 0;
         told();
         save.dispatchEvent(new Click(Click.CLICK));
         var asks:Array = saveCalls("victory_screen_request_player_data");
         expect(language + ": Save replay did not ask " + asks,asks.length == 1 && asks[0][1] == 0xFFFFFF01 && asks[0][2] == OTHER
            && save.label == words.getName("st_srp_replay_saving"));
         fits(language + ": results Saving",save);
         expect(language + ": Watch replay is not just left of Saving",button.x + button.width <= save.x);
         saveAnswer("busy",OTHER);
         expect(language + ": Save replay not back after busy",save.label == words.getName("st_srp_replay_save") && told()[0] == words.getName("st_srp_replay_busy"));
         show(screen,MATCH);
         expect(language + ": a Watch my deaths",find(screen,"srpWatchDeaths") == null);
         expect(language + ": the results' number is '" + (number ? number.text : null) + "'",number != null && number.visible && number.text == "#1ce45cbe"
            && number.x + number.width <= button.x);
         idle();
         button.dispatchEvent(new Click(Click.CLICK));
         expect(language + ": Watch replay did not ask",asked()[0] == MATCH && asked()[1] == 0xC0FF0000);
         lobby.answer("gone",MATCH);
         show(screen,MATCH);
         var why:TextField = find(screen,"srpWatchReason") as TextField;
         expect(language + ": a replay not kept shows '" + (why && why.visible ? why.text : null) + "' and Watch or Save replay",!button.visible && !save.visible && why != null && why.visible
            && why.text == words.getName("st_srp_watch_reason_gone") && why.x + why.width <= close.x && number.x + number.width <= why.x);
         Prompt.noteReplay(hex(MATCH),null);
         screen.parent.removeChild(screen);
         trace("REPLAYBUTTONS " + language + ": results: Watch replay, why not, the number");
      }

      private function finish(result:String) : void
      {
         trace("REPLAYBUTTONS RESULT " + result);
      }
   }
}
