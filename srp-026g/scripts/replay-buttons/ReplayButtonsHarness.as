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
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import flash.ui.Keyboard;
   import flash.utils.getQualifiedClassName;
   import harness.extern.ExternalInterface;

   // srp's Watch (research/watch-redesign.html) in the lobby movie, in every language (HarnessText),
   // the lobby's parts as WatchLobby lays them out at the stage's size (check.sh: 1280x720, 1920x1080):
   // - the top menu: Watch between Rooms and Ranking, every right tab's label whole, none past the
   //   bar's width nor over another; its badge counts the live matches that can be watched now
   //   (none for 0; a plus at the list's most; a yellow person with a friend in one, as on that
   //   match's card), its tip says both; its tab opens
   //   Watch, Rooms' tab closes it (and Watch closes Rooms); no Watch tab on the bottom bar;
   // - the window: tabs Live (its count) and Replays; chips Everyone, Friends on Live, Mine (the
   //   default), Friends, Everyone on Replays, each the list it names (Live's Friends: the matches
   //   with a friend); the search box: a match's code with # watches it (one typed without # is
   //   searched as a name first, watched when nobody has that name), a name lists that player's matches
   //   (find_players' answer: the name itself, else the first found for three letters or more or
   //   when one is found; then the P1 rows; another's rows left; a name not found says so; the
   //   asker's own is Mine); the player pop-up's Matches (WatchWindow.openPlayer) searches the
   //   name; every label and hint whole, the chips clear of the box;
   // - the searches by name (LobbyMenu.findPlayers): each answer to its own ask, Watch's or a
   //   friends' list's; one sent at a time and a second at most (the next waits for the answer
   //   and its turn, an asker's newer one replacing its unsent one), the same name again while its
   //   search goes keeps it; at a reconnect (playerSearchesLost) or after 15 s unanswered, the
   //   asker is told null (Watch: no answer, not before, held behind another's search too) and a
   //   later answer is nobody's; after a drop for 15 s or a stray answer the next waits until 3 s
   //   pass without one;
   // - a press the content got itself (Scaleform's routing) on a card's control scrolled away under the
   //   toolbar is the toolbar's (a chip under it), not the hidden card's; on a greyed
   //   Watch nothing; on a card's Save icon (a plain Sprite) a save;
   // - the cards: one Watch, or why not (not kept, old version; live: counting down, full, closed,
   //   over; an answer's word), the texts apart and clear of it; the code selectable (no copy icon);
   //   updated in place keyed by match: a refresh under a press keeps the card and its Watch, and
   //   the click still asks; a match gone from the list loses its card;
   // - Live's Watch asks at once from the place of the friend in the match (or anyone's), no
   //   roster; a friend's Spectate watches the friend's match from the friend's place, waits while
   //   it gathers or counts down (its 60 s anew from each list that says so), says so when its
   //   60 s end or the match can't be watched;
   // - one ask-and-poll loop (ReplayAsks) on the window's second: a watch polls 2200000000, a save
   //   2210000000, both from the same tick; after a watch asked from the window, its view again at
   //   the return after its load (a show_match_making(false) before the load brings nothing back),
   //   not after a refusal, nor for an ask under another's; a watch given up (no answer) or whose
   //   prompt is closed (Close or Enter) withdrawn once (`3D 00 FFFFFF05 <match>`), Watch asking
   //   again after, not one refused; `in_room` said
   //   in its own words; a `none` after its own `preparing` ends it, prompt gone, nothing withdrawn;
   //   a live match's `watching` to the ask itself closes the asking prompt, nothing withdrawn;
   //   a match loading ends a watch asked, unsaid: its prompt gone, nothing polled, withdrawn or
   //   said after, in the match or back in the lobby;
   //   Mine not asked at once back in the lobby (BACK_ASK_MS: the history row is not written yet);
   // - a lobby window opening (MainScene.activeWindow) or the rooms window closes Watch; back from a
   //   watch in a room, one playing without the player gives the Watch view back, its vote shows it;
   //   the room's word coming with the results up keeps the view till they close; its call back taken
   //   before the lobby is back (a state asked while the watch unloads) keeps the room, its notice
   //   from the return;
   // - Save replay (ReplayAsks): a save icon right of the code on the cards that may be watched
   //   (none on live cards or ones that cannot be watched); a press asks `3D 00 FFFFFF01 <match>`
   //   and polls 2210000000; `R2;preparing` keeps it saving, `R2;ready` hands the link to the exe once
   //   (video_tutorial_link_clicked, no argument) and shows it saved, and it may be saved again (the
   //   updater's download may fail); a refusal (`off` naming no match too), or no answer, says why
   //   and it may be saved again; a match loading ends it unsaid (no poll, link or box in the match);
   //   one save at a time: every other Save not pressable meanwhile;
   //   the saving and saved words fit the card; a ranking row named as a header is not taken;
   // - the results screen: a red Watch replay, then Save replay (Save replay, Saving..., Saved),
   //   left of the close button, their labels whole, the three as a row centred where the close
   //   button alone is; the match's number left of them, selected whole by a press and not changed
   //   by typing; why not in their place once known; no Watch my deaths; the movie's own Share and
   //   Save buttons hidden; a save's refusal said at once over them (they hold messages back);
   //   Save replay greyed while another match's save is under way;
   // - a History row: Watch, or why not, and the match's number under it, inside the row.
   // Ruffle has no exe: its calls land in the stand-in (scripts/exe-stand-in, ruffle_movie.py --exe),
   // and the server's answers are told to the movie as the exe would. The movie has no test hooks but
   // the clock of the searches by name (LobbyMenu.searchClock: one sent a second at most).
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

      // LobbyMenu, and the clock of its searches by name (LobbyMenu.searchClock)
      private var Menu:*;

      private var clock:int = 0;


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
            Menu = domain.getDefinition("LobbyMenu");
            Menu.searchClock = function():int
            {
               return clock;
            };
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

      // fill_players_search's players, a second later (a search held meanwhile sent first), to the
      // search by name they answer (LobbyMenu.playersFound): true when one waited for them.
      private function found(players:Array) : Boolean
      {
         later(1100);
         return Menu.playersFound(players);
      }

      // The searches' clock `ms` on, and their tick (a held search sent in its turn, one that waited
      // too long for its answer dropped).
      private function later(ms:int) : void
      {
         clock += ms;
         Menu.playerSearchTick();
      }

      // The searches by name sent to the exe: their names.
      private function searchesSent() : Array
      {
         return saveCalls("find_players").map(function(c:Array, i:int, all:Array):String
         {
            return String(c[1]);
         });
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

      // The watches withdrawn (`3D 00 FFFFFF05 <match>`, the lobby server's REPLAY_WITHDRAW): their
      // matches.
      private function withdrawn() : Array
      {
         return saveCalls("victory_screen_request_player_data").filter(function(c:Array, i:int, all:Array):Boolean
         {
            return c[1] == 0xFFFFFF05;
         }).map(function(c:Array, i:int, all:Array):uint
         {
            return uint(c[2]);
         });
      }

      // The player's Close on the watch prompt on top (its only button, with the movie's own box,
      // MessageApply, registered as "apply"): what it said (null: no prompt).
      private function closePrompt() : String
      {
         var box:* = promptBox();
         if(box == null)
            return null;
         var text:String = String(box.textField.text);
         box.btnCancel.dispatchEvent(new Click(Click.CLICK));
         return text;
      }

      // The player's Enter on the watch prompt on top (MessageAcceptBase.handleInput: the box's
      // accept, onMessageAccept): what it said (null: no prompt).
      private function enterPrompt() : String
      {
         var box:* = promptBox();
         if(box == null)
            return null;
         var text:String = String(box.textField.text);
         var Input:Class = domain.getDefinition("scaleform.clik.events.InputEvent") as Class;
         var Details:Class = domain.getDefinition("scaleform.clik.ui.InputDetails") as Class;
         try
         {
            box.handleInput(new Input("input",new Details("key",Keyboard.ENTER,"keyUp")));
         }
         catch(err:Error)
         {
            failures.push("Enter on the watch prompt: " + err);
         }
         return text;
      }

      // The watch prompt on top, null none.
      private function promptBox() : *
      {
         var box:* = null;
         for(var i:int = 0; i < lobby.holder.numChildren; i++)
         {
            var child:* = lobby.holder.getChildAt(i);
            if(String(child.name).indexOf("message_") == 0)
               box = child;
         }
         return box == null || !("btnCancel" in box) || box.btnCancel == null ? null : box;
      }

      private function promptShown() : Boolean
      {
         return promptBox() != null;
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
            // the "ok" box's text, or the watch prompt's ("apply", MessageApply: its textField)
            said.push(String("textField" in box ? box.textField.text : box.text));
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
         // the badge: a red dot and the live matches that can be watched now (2 of WatchFake.LIVE's
         // 6: the others start, are over, full or closed), a yellow person with a friend in one; a
         // plus when the list is at its most; under the tab what they mean, while the arrow is on it
         // (the movie's own root, which fails outside the game, makes a top menu of its own)
         W.topMenu = top;
         // the eye, each frame as bright as the tab makes its icon now (hover, chosen, disabled)
         var art:* = watch.icon;
         art.alpha = 0.37;
         win.dispatchEvent(new Event(Event.ENTER_FRAME));
         var eye:DisplayObject = find(top,"srpWatchEye");
         expect(language + ": the Watch tab's eye does not follow its art: " + (eye ? eye.alpha : null),eye != null && eye.visible
            && Math.abs(eye.alpha - art.alpha) < 0.01 && !art.visible);
         art.alpha = 1;
         win.dispatchEvent(new Event(Event.ENTER_FRAME));
         lobby.take(WatchFake.LIVE);
         var badge:DisplayObjectContainer = find(top,"srpWatchBadge") as DisplayObjectContainer;
         var badgeText:TextField = badge != null ? badge.getChildAt(0) as TextField : null;
         expect(language + ": the badge does not say 2: " + (badgeText ? badgeText.text : null),badge != null && badge.visible && badgeText.text == "2");
         var friendMark:DisplayObject = badge != null ? badge.getChildByName("srpWatchBadgeFriend") : null;
         expect(language + ": the badge with a friend's match has no person on it, in its plate",friendMark != null && friendMark.visible
            && friendMark.x >= badgeText.x + badgeText.textWidth && friendMark.x + friendMark.width <= badge.width);
         watch.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
         var tip:DisplayObjectContainer = find(top,"srpWatchTip") as DisplayObjectContainer;
         var tipText:TextField = tip != null ? tip.getChildAt(0) as TextField : null;
         expect(language + ": the badge's tip does not say live matches, then the friend: " + (tipText ? tipText.text : null),tip != null && tip.visible
            && tipText.text.indexOf(words.getName("st_srp_watch_badge_live")) == 0
            && tipText.text.indexOf(words.getName("st_srp_watch_badge_friend")) > 0 && tipText.numLines == 2 && tipText.textWidth > 20);
         var tipBox:Rectangle = tip != null ? tip.getBounds(top) : new Rectangle();
         expect(language + ": the badge's tip is not under the Watch tab, in the bar's width: " + tipBox,tipBox.y >= watch.getBounds(top).bottom
            && tipBox.x >= 0 && tipBox.right <= top.tabPage.x + top.tabPage.width);
         lobby.take(WatchFake.LIVE.slice(0,8));
         expect(language + ": the badge without a friend has a person, or its tip is not the plain one: " + (tipText ? tipText.text : null),
            badgeText != null && badgeText.text == "2" && friendMark != null && !friendMark.visible && tipText != null && tip.visible
            && tipText.text == words.getName("st_srp_watch_badge_live") && tipText.numLines == 1);
         watch.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
         expect(language + ": the badge's tip stays after the arrow left",tip != null && !tip.visible);
         var many:Array = ["L0;20;30"];
         for(var m:int = 0; m < 20; m++)
         {
            many.push("L1;000001" + (m + 16).toString(16) + ";61;5;2;8;8;2;1;422;0;" + (m < 12 ? 1 : 0));
         }
         lobby.take(many);
         expect(language + ": the badge of a list at its most does not say 12+: " + (badgeText ? badgeText.text : null),badgeText != null && badgeText.text == "12+"
            && badge.width >= badgeText.textWidth);
         lobby.take(["L0;0"]);
         expect(language + ": a badge for no live match",badge != null && !badge.visible);
         watch.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
         expect(language + ": a tip without a badge",tip != null && !tip.visible);
         watch.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
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
         // a ranking page whose first player is named as a header is the ranking's
         for each(var named:String in ["L0;5","M0;5;0","P1;5;x","R1;watching;" + hex(MATCH) + ";0","R2;ready;" + hex(MATCH) + ";1"])
         {
            expect(language + ": a ranking row named " + named + " taken",!lobby.take([named,"Bob"],5));
         }
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
         // an answer is filed by the list its head names: Everyone's refresh, in flight when Friends
         // was chosen, is not Friends'; a head naming no list, or another, is nobody's
         for each(var other:String in ["M0;0;0","M0;0","M0;0;2","M0;0;1;0"])
         {
            lobby.take([other]);
            expect(language + ": Friends took " + other,cards().length == 1);
         }
         lobby.take(["M0;0;1"]);
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
            // a match loaded with the window up (the load closes the prompt, not the window): the
            // window asks the lobby for no list until the lobby is back (its refresh is 30 s)
            W.away();
            ExternalInterface.calls.length = 0;
            for(var s:int = 0; s < 70; s++)
               second();
            var inMatch:int = 0;
            for each(var made:Array in ExternalInterface.calls)
            {
               if(made[0] == "request_players_range" && [W.LIVE_START,W.MATCHES_START,W.MATCHES_START + 1,W.RECENT_START,W.RECENT_START + 1].indexOf(made[1]) >= 0)
                  inMatch++;
            }
            expect(language + ": the window asked the lobby for " + inMatch + " lists in a match",inMatch == 0);
            W.back();
         }
         trace("REPLAYBUTTONS " + language + ": Replays: Mine, Friends, Everyone, the reasons, Watch, back after a replay");
      }

      private function typeIn(text:String) : void
      {
         var box:TextField = find(win,"srpWatchSearch") as TextField;
         box.text = text;
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN,true,false,0,Keyboard.ENTER));
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_UP,true,false,0,Keyboard.ENTER));
      }

      private function search(language:String) : void
      {
         // a match's code with # watches it at once
         idle();
         typeIn("#1CE45CBE");
         expect(language + ": the code '#1CE45CBE' asked " + asked(),asked()[0] == MATCH && asked()[1] == 0xC0FF0000);
         idle();
         // Enter's press alone asks nothing (its release would close the prompt), nor does a release
         // begun elsewhere (the one that closed a box)
         var calls:int = ExternalInterface.calls.length;
         var box:TextField = find(win,"srpWatchSearch") as TextField;
         box.text = "#1ce45cbe";
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_UP,true,false,0,Keyboard.ENTER));
         expect(language + ": a release alone sent the code",ExternalInterface.calls.length == calls);
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN,true,false,0,Keyboard.ENTER));
         expect(language + ": Enter's press sent the code",ExternalInterface.calls.length == calls);
         box.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_UP,true,false,0,Keyboard.ENTER));
         expect(language + ": Enter did not send the code",asked()[0] == MATCH && ExternalInterface.calls.length > calls);
         idle();
         // a code typed without # may be a player's name: the name searched first, nothing asked
         // of the match server meanwhile; the player named so (a loose find is not the name), else
         // the code watched and the search ended
         later(3000);
         ExternalInterface.calls.length = 0;
         typeIn("1ce45cbe");
         expect(language + ": a bare code asked a watch before its name search: " + asked() + ", " + searchesSent(),asked()[0] == 0
            && searchesSent().join(",") == "1ce45cbe" && shownChip() == "player");
         expect(language + ": the bare code's name search took no answer",found([{"id":9,"name":"1ce45cbf"}]));
         expect(language + ": a bare code nobody is named did not watch it: " + asked() + ", " + shownChip(),asked()[0] == MATCH
            && asked()[1] == 0xC0FF0000 && shownChip() != "player");
         idle();
         later(3000);
         ExternalInterface.calls.length = 0;
         typeIn("deadbeef");
         expect(language + ": the name search of 'deadbeef' took no answer",found([{"id":9,"name":"DeadBeef"}]));
         expect(language + ": a bare code that is a name watched: " + asked(),asked()[0] == 0 && shownChip() == "player"
            && (find(win,"srpWatchSearch") as TextField).text == "DeadBeef");
         // with # it is a code alone
         later(3000);
         ExternalInterface.calls.length = 0;
         typeIn("#deadbeef");
         lobby.answer("gone",0xDEADBEEF);
         later(1100);
         expect(language + ": a code with # gone searched a name: " + searchesSent(),asked()[0] == 0xDEADBEEF && searchesSent().length == 0);
         Prompt.noteReplay("deadbeef",null);
         told();
         // a name: the player found, then its matches
         var Info:* = domain.getDefinition("survarium.controls.PlayerInfo");
         Info.nickname = "PocketTest1";
         typeIn("ada");
         expect(language + ": a name did not search",shownTab() == "replays" && shownChip() == "player" && textOf(win,"srpWatchEmpty") == "…");
         expect(language + ": the search took find_players' answer",found([{"id":6,"name":"Adam_K"},{"id":5,"name":"Ada"}]));
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
         expect(language + ": a players' answer nobody asked for taken",!found([]));
         // (a stray answer holds the next search 3 s: another may follow it)
         later(3000);
         // a friends' search asked while Watch's waits, and the other way round: each answer goes
         // to its own ask, in the order asked
         var friends:Object = {};
         var friendsGot:Array = null;
         var friendsTold:int = 0;
         var friendsAsk:Function = function(players:Array):void
         {
            friendsGot = players;
            friendsTold++;
         };
         typeIn("ada");
         Menu.findPlayers(friends,"bob",friendsAsk);
         found([{"id":5,"name":"Ada"}]);
         found([{"id":7,"name":"Bob"}]);
         expect(language + ": Watch's search, then a friends' search: not each its own answer",(find(win,"srpWatchSearch") as TextField).text == "Ada"
            && friendsGot != null && friendsGot.length == 1 && friendsGot[0].name == "Bob");
         friendsGot = null;
         Menu.findPlayers(friends,"bob",friendsAsk);
         typeIn("adm");
         found([{"id":7,"name":"Bob"}]);
         found([{"id":6,"name":"Adam_K"}]);
         expect(language + ": a friends' search, then Watch's: not each its own answer",(find(win,"srpWatchSearch") as TextField).text == "Adam_K"
            && friendsGot != null && friendsGot.length == 1 && friendsGot[0].name == "Bob");
         // two searches within a second: the second waits its turn (the lobby server finds nobody
         // for one sooner), and the first's answer is not taken for it
         idle();
         later(2000);
         ExternalInterface.calls.length = 0;
         typeIn("Bob_X");
         typeIn("ada");
         later(500);
         expect(language + ": two searches sent within a second: " + searchesSent(),searchesSent().join(",") == "Bob_X");
         found([{"id":7,"name":"Bob_X"}]);
         expect(language + ": the second search not sent in its turn: " + searchesSent(),searchesSent().join(",") == "Bob_X,ada");
         expect(language + ": the first search's answer taken for the second",(find(win,"srpWatchSearch") as TextField).text == "ada"
            && textOf(win,"srpWatchEmpty") == "…");
         found([{"id":5,"name":"Ada"}]);
         expect(language + ": the second search's answer not taken",(find(win,"srpWatchSearch") as TextField).text == "Ada");
         // the same name again while its search finds the player (Enter twice, Enter and the
         // magnifier), or asks for its matches: that search kept, nothing sent again
         later(2000);
         ExternalInterface.calls.length = 0;
         typeIn("Adm");
         typeIn("adm");
         later(2000);
         expect(language + ": a repeated name searched again: " + searchesSent(),searchesSent().join(",") == "Adm");
         found([{"id":6,"name":"Adam_K"}]);
         typeIn("Adam_K");
         later(2000);
         expect(language + ": a repeated name kept its search: " + searchesSent(),searchesSent().join(",") == "Adm"
            && (find(win,"srpWatchSearch") as TextField).text == "Adam_K");
         // an asker's newer search takes the place of its unsent one, which is never answered
         later(2000);
         ExternalInterface.calls.length = 0;
         var firstGot:Array = null;
         friendsGot = null;
         var unsentTold:Boolean = false;
         Menu.findPlayers(friends,"one",function(players:Array):void
         {
            firstGot = players;
         });
         Menu.findPlayers(friends,"two",function(players:Array):void
         {
            unsentTold = true;
         });
         Menu.findPlayers(friends,"three",friendsAsk);
         found([{"id":1,"name":"One"}]);
         found([{"id":3,"name":"Three"}]);
         expect(language + ": an asker's unsent search not replaced: " + searchesSent(),searchesSent().join(",") == "one,three"
            && firstGot != null && firstGot[0].name == "One" && friendsGot != null && friendsGot[0].name == "Three" && !unsentTold);
         // one search sent at a time (an answer says nothing of its ask): the next waits for the
         // answer even past its turn
         later(2000);
         ExternalInterface.calls.length = 0;
         friendsGot = null;
         Menu.findPlayers(friends,"bob",friendsAsk);
         typeIn("adm");
         later(5000);
         expect(language + ": a search sent while another waits for its answer: " + searchesSent(),searchesSent().join(",") == "bob");
         expect(language + ": the waiting search's answer not its own",Menu.playersFound([{"id":7,"name":"Bob"}])
            && friendsGot != null && friendsGot[0].name == "Bob" && searchesSent().join(",") == "bob,adm");
         found([{"id":6,"name":"Adam_K"}]);
         expect(language + ": the search sent after the wait not answered its own",(find(win,"srpWatchSearch") as TextField).text == "Adam_K");
         // the lobby's connection made anew (close_sync): the search sent has no answer coming;
         // its asker told so (null: a friends' list's spinner stops; Watch says no answer), the
         // next sent at once, and a later answer is nobody's
         later(2000);
         friendsTold = 0;
         Menu.findPlayers(friends,"bob",friendsAsk);
         typeIn("ada");
         later(1100);
         Menu.playerSearchesLost();
         expect(language + ": a search lost at a reconnect not told so",friendsTold == 1 && friendsGot == null);
         later(1100);
         Menu.playerSearchesLost();
         expect(language + ": Watch's search lost at a reconnect not told so",textOf(win,"srpWatchEmpty") == words.getName("st_srp_matches_no_answer"));
         expect(language + ": an answer after a reconnect taken",!found([{"id":5,"name":"Ada"}]));
         // Watch's search waits for its own answer as long as LobbyMenu keeps it (held behind
         // another's, then sent): no answer only once LobbyMenu drops it unanswered (15 s from
         // its send), and an answer before that is taken
         later(3000);
         Menu.findPlayers(friends,"bob",friendsAsk);
         typeIn("ada");
         later(14000);
         second();
         expect(language + ": Watch's search held behind another's gave up: " + textOf(win,"srpWatchEmpty"),textOf(win,"srpWatchEmpty") == "…");
         Menu.playersFound([{"id":7,"name":"Bob"}]);
         later(14000);
         second();
         expect(language + ": Watch's search gave up before LobbyMenu dropped it: " + textOf(win,"srpWatchEmpty"),textOf(win,"srpWatchEmpty") == "…");
         later(1000);
         expect(language + ": Watch's search dropped unanswered not told so",textOf(win,"srpWatchEmpty") == words.getName("st_srp_matches_no_answer"));
         // one unanswered for 15 s: dropped, its asker told so; its late answer is nobody's, and
         // the next search is sent only once 3 s passed without one (a late answer taken for the
         // next would throw the next's own away)
         later(3000);
         friendsTold = 0;
         friendsGot = [];
         Menu.findPlayers(friends,"bob",friendsAsk);
         later(15000);
         expect(language + ": a search unanswered for 15 s not told so",friendsTold == 1 && friendsGot == null);
         ExternalInterface.calls.length = 0;
         Menu.findPlayers(friends,"bob_2",friendsAsk);
         later(1000);
         expect(language + ": a search sent just after one was dropped: " + searchesSent(),searchesSent().length == 0);
         expect(language + ": the dropped search's late answer taken",!Menu.playersFound([{"id":7,"name":"Bob"}]) && friendsGot == null);
         later(2000);
         expect(language + ": a search sent within 3 s of a late answer: " + searchesSent(),searchesSent().length == 0);
         later(1100);
         expect(language + ": the search after the late answer not sent: " + searchesSent(),searchesSent().join(",") == "bob_2");
         expect(language + ": the search after the late answer not answered its own",found([{"id":8,"name":"Bob_2"}])
            && friendsGot != null && friendsGot[0].name == "Bob_2");
         // the asker's own match of a private room or against bots: a field more, which a card says
         var Rows:* = domain.getDefinition("survarium.watch.WatchRows");
         var privateRow:Object = Rows.played("P2;00000031;1;61;5;1;9;2;180;1;1");
         var botsRow:Object = Rows.played("P2;00000032;1;61;5;1;9;2;180;1;2");
         var publicRow:Object = Rows.played("P2;00000033;1;61;5;1;9;2;180;1");
         expect(language + ": an own row's kind is not read",privateRow != null && privateRow.shown == 1 && botsRow != null && botsRow.shown == 2
            && publicRow != null && publicRow.shown == 0);
         expect(language + ": a row of a kind nobody knows, or with fields past it, is taken",Rows.played("P2;00000034;1;61;5;1;9;2;180;1;3") == null
            && Rows.played("P2;00000034;1;61;5;1;9;2;180;1;1;0") == null);
         expect(language + ": no word for a private room's match",String(words.getName("st_srp_watch_private")).length > 2
            && words.getName("st_srp_watch_private") != "st_srp_watch_private");
         // a name typed loosely: the first found (srp's best match), none the name itself
         typeIn("adm");
         found([{"id":6,"name":"Adam_K"},{"id":5,"name":"Ada"}]);
         expect(language + ": a loose name did not take the first found: " + (find(win,"srpWatchSearch") as TextField).text,
            (find(win,"srpWatchSearch") as TextField).text == "Adam_K" && shownChip() == "player");
         // two letters and several found: nobody's
         typeIn("ad");
         found([{"id":6,"name":"Adam_K"},{"id":5,"name":"Ada"}]);
         expect(language + ": two letters took one of several players",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_no_player").split("%1$s").join("ad"));
         typeIn("ad");
         found([{"id":6,"name":"Adam_K"}]);
         expect(language + ": two letters did not take the one player found",(find(win,"srpWatchSearch") as TextField).text == "Adam_K");
         // a name nobody has
         typeIn("Nobody_Here");
         found([]);
         expect(language + ": a name not found says '" + textOf(win,"srpWatchEmpty") + "'",textOf(win,"srpWatchEmpty") == words.getName("st_srp_watch_no_player").split("%1$s").join("Nobody_Here"));
         // the asker's own name: Mine
         typeIn("pockettest1");
         expect(language + ": the own name is not Mine",shownChip() == "mine");
         // the player pop-up's Matches
         win.show(false);
         W.openPlayer("Ada");
         expect(language + ": the pop-up's Matches did not search Ada",win.visible && shownTab() == "replays" && shownChip() == "player"
            && (find(win,"srpWatchSearch") as TextField).text == "Ada");
         found([{"id":5,"name":"Ada"}]);
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
         // the friend's match is card 1 of RECENT, and alone in Everyone's next answer: the same
         // object, moved up
         lobby.take(["M0;1;0"].concat(WatchFake.FRIENDS.slice(1)));
         var after:Array = cards();
         expect(language + ": the kept match is not the same card " + after.length,after.length == 1 && after[0] === before[1] && after[0].y == 0 && find(after[0],"srpWatchWatch") === watch);
         expect(language + ": a match no longer listed kept its card",before[0].parent == null && before[2].parent == null);
         trace("REPLAYBUTTONS " + language + ": cards in place, keyed by match, through a press and a new order");
         scrolledAway(language,watch);
      }

      // A press the content got itself (Scaleform's routing): on a card's Watch where the list shows
      // it, the Watch's; on a chip with that Watch scrolled away under it (outside the list's mask),
      // the chip's.
      private function scrolledAway(language:String, watch:DisplayObject) : void
      {
         var chip:DisplayObject = find(win,"srpWatchChip_friends");
         var content:DisplayObjectContainer = chip == null ? null : chip.parent;
         var inner:DisplayObjectContainer = watch.parent == null ? null : watch.parent.parent;
         expect(language + ": no chip and list for the scrolled press",content != null && inner != null);
         if(content == null || inner == null)
            return;
         var centre:Function = function(of:DisplayObject):Point
         {
            var r:Rectangle = of.getBounds(stage);
            return new Point(r.x + r.width / 2,r.y + r.height / 2);
         };
         var pressContent:Function = function(at:Point):void
         {
            var local:Point = content.globalToLocal(at);
            content.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,local.x,local.y));
         };
         idle();
         ExternalInterface.calls.length = 0;
         pressContent(centre(watch));
         expect(language + ": a press on the shown Watch given to the content did not ask: " + asked(),asked()[0] == OTHER);
         idle();
         // a greyed game button takes no press given to the content
         Object(watch).enabled = false;
         ExternalInterface.calls.length = 0;
         pressContent(centre(watch));
         Object(watch).enabled = true;
         expect(language + ": a press on a greyed Watch given to the content asked: " + asked(),asked()[0] == 0);
         idle();
         // the list scrolled so the Watch lies under the chip
         var x0:Number = inner.x;
         var y0:Number = inner.y;
         var to:Point = inner.parent.globalToLocal(centre(chip));
         var from:Point = inner.parent.globalToLocal(centre(watch));
         inner.x += to.x - from.x;
         inner.y += to.y - from.y;
         var under:Boolean = watch.hitTestPoint(centre(chip).x,centre(chip).y,false);
         ExternalInterface.calls.length = 0;
         pressContent(centre(chip));
         inner.x = x0;
         inner.y = y0;
         expect(language + ": the Watch is not under the chip",under);
         expect(language + ": the chip under a scrolled-away Watch did not take the press: " + shownChip() + ", asked " + asked(),
            shownChip() == "friends" && asked()[0] == 0);
         trace("REPLAYBUTTONS " + language + ": a press on a card scrolled away under the toolbar is the toolbar's");
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
         // given up: withdrawn at the lobby server (a session not sent yet is over there), once
         expect(language + ": a watch given up not withdrawn once: " + withdrawn(),withdrawn().join(",") == String(OTHER));
         W.away();
         W.back();
         expect(language + ": a watch never answered left a view to come back to",!win.visible);
         // the prompt (the movie's own box, shown here for these asks only): its Close while it
         // prepares withdraws the watch at once, once, and it is polled no more
         told();
         Message.registerMessage("apply",domain.getDefinition("survarium.messages.MessageApply"));
         W.open("live");
         lobby.take(WatchFake.LIVE);
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         second();
         lobby.answer("preparing",OTHER,1);
         expect(language + ": withdrawn before Close: " + withdrawn(),withdrawn().length == 0);
         var shown:String = closePrompt();
         expect(language + ": no preparing prompt to close: " + shown,shown != null && shown.length > 0);
         told();
         expect(language + ": Close did not withdraw the watch once: " + withdrawn(),withdrawn().join(",") == String(OTHER));
         second();
         expect(language + ": polled after Close: " + polls(2200000000),polls(2200000000) == 1);
         // Enter on the prompt (MessageApply.onMessageAccept, which calls it with its data) does
         // the same: withdrawn once, polled no more, and Watch asks again
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         second();
         lobby.answer("preparing",OTHER,1);
         shown = enterPrompt();
         expect(language + ": no preparing prompt to accept: " + shown,shown != null && shown.length > 0);
         told();
         expect(language + ": Enter did not withdraw the watch once: " + withdrawn(),withdrawn().join(",") == String(OTHER));
         second();
         expect(language + ": polled after Enter: " + polls(2200000000),polls(2200000000) == 1);
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         expect(language + ": no Watch asked after Enter: " + asked(),asked()[0] == OTHER);
         closePrompt();
         told();
         // a refusal ends the watch at the server: nothing to withdraw; the room's own word says why
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         lobby.answer("in_room",OTHER);
         shown = closePrompt();
         expect(language + ": in_room said " + shown,shown == words.getName("st_srp_replay_in_room")
            && shown != words.getName("st_srp_replay_busy"));
         told();
         expect(language + ": a refused watch withdrawn: " + withdrawn(),withdrawn().length == 0);
         // its session over at the server with no word for it (the room called the player back
         // before the game was sent): `none` once the ask said `preparing` ends it, its prompt
         // gone, nothing withdrawn, the next Watch asked; a `none` before is no answer of its own
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         lobby.answer("none",0);
         second();
         expect(language + ": a none before the ask's own answer ended it: " + polls(2200000000),polls(2200000000) == 1);
         lobby.answer("preparing",OTHER,1);
         lobby.answer("none",0);
         shown = closePrompt();
         expect(language + ": the prompt stayed after its session ended: " + shown,shown == null);
         second();
         expect(language + ": polled after its session ended: " + polls(2200000000),polls(2200000000) == 1);
         expect(language + ": an ended session withdrawn: " + withdrawn(),withdrawn().length == 0);
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         expect(language + ": no Watch asked after a none: " + asked(),asked()[0] == OTHER);
         // a live match answers `watching` to the ask itself, before its game loads: the asking
         // prompt goes with the ask, nothing withdrawn
         expect(language + ": no asking prompt",promptShown());
         lobby.answer("watching",OTHER);
         shown = closePrompt();
         expect(language + ": the prompt stayed after a live watching: " + shown,shown == null);
         expect(language + ": a live watching withdrawn: " + withdrawn(),withdrawn().length == 0);
         idle();
         told();
         // a replay that said `preparing`, then loaded and watched for 4 min (the lobby's seconds
         // ticking in the match or not): the load ended the ask, its prompt with it; nothing polled,
         // withdrawn or said then, nor back in the lobby (no "still preparing")
         ExternalInterface.calls.length = 0;
         click(cards()[1],"srpWatchWatch");
         second();
         lobby.answer("preparing",OTHER,1);
         expect(language + ": no preparing prompt before the load",promptShown());
         W.away();
         expect(language + ": the preparing prompt outlived the load",!promptShown());
         for(p = 0; p < 240; p++)
            second();
         W.back();
         second();
         var past:Array = told();
         expect(language + ": a watch past its load polled " + polls(2200000000) + ", withdrawn " + withdrawn() + ", said " + past,
            polls(2200000000) == 1 && withdrawn().length == 0 && past.length == 0);
         win.show(false);
         Message.registerMessage("apply",null);
         win.show(false);
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
         // the watch that played, in the game's own order: `watching`, a show_match_making(false)
         // from the lobby scene going away (before the load: it brings nothing back), the load
         // (set_loading_level, whose Modals.escape closes the window on top), and the return
         lobby.answer("watching",OTHER);
         win.show(false);
         W.back();
         expect(language + ": Watch came back before the watch's load",!win.visible);
         W.away();
         expect(language + ": the load showed Watch",!win.visible);
         W.back();
         expect(language + ": not back on Live after the watch",win.visible && shownTab() == "live");
         // once: the next return to the lobby brings nothing
         win.show(false);
         W.back();
         expect(language + ": Watch came back a second time",!win.visible);
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
         // its 60 s are counted anew from each list that still has it starting
         for(p = 0; p < 90; p++)
         {
            second();
            if(p % 5 == 4)
               lobby.take(["L0;1","L1;00000006;64;5;0;4;4;0;1;0;0;1","F1;00000006;3;0;Petro"]);
         }
         said = told();
         expect(language + ": Spectate gave up on a match still starting: " + said,said.length == 0 && asked()[0] == 0);
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
         if(language == HarnessText.LANGUAGES[0])
         {
            watchReturn(Rooms);
            W.open("live");
         }
         trace("REPLAYBUTTONS " + language + ": a lobby window or the rooms window closes Watch");
      }

      // Back from a watch in a room (RoomsWindow.holdWatchReturn): a room playing a match the
      // player is not in gives the Watch view back (one with a place for it calls it back); its
      // vote shows the room; its word with the results up keeps the view till they close.
      private function watchReturn(Rooms:*) : void
      {
         var state:int = domain.getDefinition("survarium.rooms.RoomsWire").STATE;
         // the room's state: `code` srp's word in it (18: it called us back from the watch)
         var room:Function = function(phase:int, code:int = 0):void
         {
            var names:Array = [["S1","R",7,15,15,10,30,phase,754,0,5,5,7,0,63,2,code].join(";"),"C;0;61;2;1",
               ["P",0,15,1,0,phase == 2 ? 1 : 0,0,0,"Ada"].join(";"),["P",1,15,0,1,0,0,0,"PocketTest1"].join(";"),"L;0"];
            var rows:Array = [];
            for(var r:int = 0; r < names.length; r++)
               rows.push({"place":state + r,"nickname":names[r]});
            Rooms.take(rows);
         };
         var resumed:int = 0;
         var resume:Function = function():void
         {
            resumed++;
         };
         room(0);
         Rooms.me.show(false);
         expect("watch return: not held in a room",Rooms.me.holdWatchReturn(resume));
         room(2);
         expect("watch return: a room playing without us did not give the Watch view back (" + resumed + ", rooms shown " + Rooms.me.shown + ")",
            resumed == 1 && !Rooms.me.shown);
         Rooms.me.holdWatchReturn(resume);
         room(3);
         expect("watch return: a voting room did not show itself (" + resumed + ", rooms shown " + Rooms.me.shown + ")",resumed == 1 && Rooms.me.shown);
         Rooms.me.show(false);
         // the room's word after the results came up (the lobby back, the return held, the
         // results, then the state): the Watch view waits for their close (its tab would close
         // them), then the room is asked again
         W.open("live");
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.LIVE);
         click(cards()[1],"srpWatchWatch");
         lobby.answer("watching",OTHER);
         win.show(false);
         W.away();
         W.back();
         expect("watch return: Watch back before the room's word",!win.visible);
         var screen:* = lobby.make("SrvVictoryScreenReskin");
         lobby.holder.addChild(screen);
         var had:* = Menu.id.victory_screen;
         Menu.id.victory_screen = screen;
         room(2);
         expect("watch return: Watch opened over the results",!win.visible && Menu.id.resultsUp);
         lobby.holder.removeChild(screen);
         Menu.id.victory_screen = had;
         W.resultsClosed();
         expect("watch return: Watch back before the room's word after the results",!win.visible);
         room(2);
         expect("watch return: Watch not back on Live once the results closed",win.visible && shownTab() == "live");
         win.show(false);
         // the room's call back (said once) taken while the watch unloads (a state asked before the
         // lobby is back): its notice from the return on, the room kept, Watch's view not over it
         // once the next state (no call back in it) comes
         W.open("live");
         press(win,"srpWatchChip_everyone");
         lobby.take(WatchFake.LIVE);
         click(cards()[1],"srpWatchWatch");
         lobby.answer("watching",OTHER);
         win.show(false);
         W.away();
         room(2,18);
         var note:DisplayObject = find(Rooms.me,"srpRoomsCalledBack");
         expect("watch return: the call back's notice before the lobby is back",note == null || !note.visible);
         W.back();
         note = find(Rooms.me,"srpRoomsCalledBack");
         expect("watch return: no room and notice back from a watch called back on the way (rooms shown " + Rooms.me.shown + ", Watch shown " + win.visible + ")",
            Rooms.me.shown && !win.visible && note != null && note.visible);
         room(2);
         expect("watch return: Watch over the room that called us back",Rooms.me.shown && !win.visible);
         Rooms.me.show(false);
         Rooms.take([{"place":state - 1,"nickname":"S1;N;0;0;0"}]);
         expect("watch return: still in a room",!Rooms.me.holdWatchReturn(resume));
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
            // one save at a time: the other cards' Save shows itself not pressable meanwhile
            expect(language + ": another card's Save pressable while one is under way",!Asks.canSave && !Object(find(cards()[1],"srpWatchSave")).buttonMode);
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
            expect(language + ": the cards' Save not pressable again after the save",Asks.canSave && Object(find(cards()[0],"srpWatchSave")).buttonMode
               && Object(find(cards()[1],"srpWatchSave")).buttonMode);
            var before:int = polls(2210000000);
            second();
            expect(language + ": polled after the save",polls(2210000000) == before);
            // saved only means handed to the updater, whose download may fail: it may be saved again
            var askedSoFar:int = saveCalls("victory_screen_request_player_data").length;
            click(cards()[0],"srpWatchSave");
            expect(language + ": a saved replay not asked again",saveCalls("victory_screen_request_player_data").length == askedSoFar + 1
               && saveState(cards()[0]) == savingWord);
            saveAnswer("ready",MATCH,4404019);
            expect(language + ": the second save's link not handed to the exe",saveCalls("video_tutorial_link_clicked").length == 2);
         }
         expect(language + ": the saved card says '" + saveState(cards()[0]) + "'",saveState(cards()[0]) == savedWord);
         cardFits(language + ": the saved card",cards()[0],cards()[0].width);
         // refused: why, and it may be saved again; no answer: the same
         click(cards()[1],"srpWatchSave");
         expect(language + ": the other card does not say saving",saveState(cards()[1]) == savingWord);
         expect(language + ": the saved card's Save pressable while another's is under way",!Object(find(cards()[0],"srpWatchSave")).buttonMode
            && saveState(cards()[0]) == savedWord);
         cardFits(language + ": the saving card",cards()[1],cards()[1].width);
         saveAnswer("gone",OTHER);
         var said:Array = told();
         expect(language + ": a refusal told " + said,said.length == 1 && said[0] == words.getName("st_srp_replay_gone") && saveState(cards()[1]) == null);
         // `gone` says the replay is not kept: the card says so, no Save or Watch to press again
         expect(language + ": a save's gone left the card's Save or Watch",find(cards()[1],"srpWatchSave") == null && find(cards()[1],"srpWatchWatch") == null
            && textOf(cards()[1],"srpWatchReason") == words.getName("st_srp_watch_reason_gone"));
         // a list saying it is kept again
         lobby.take(WatchFake.played("PocketTest1"));
         click(cards()[1],"srpWatchSave");
         for(var p:int = 0; p <= 30; p++)
            second();
         said = told();
         expect(language + ": no answer told " + said,said.length == 1 && said[0] == words.getName("st_srp_replay_no_answer") && Asks.stateOf(OTHER) == "save");
         // replays off at the server: its `off` names no match, and says so
         click(cards()[1],"srpWatchSave");
         saveAnswer("off",0);
         said = told();
         expect(language + ": off told " + said,said.length == 1 && said[0] == words.getName("st_srp_replay_off") && Asks.stateOf(OTHER) == "save");
         // a match loading ends a save under way, unsaid: no poll, no link to the exe, no box in
         // the match; its Save pressable again back in the lobby
         click(cards()[1],"srpWatchSave");
         W.away();
         expect(language + ": a save still under way in a match",Asks.canSave && Asks.stateOf(OTHER) == "save");
         ExternalInterface.calls.length = 0;
         for(p = 0; p <= 30; p++)
            second();
         saveAnswer("ready",OTHER,1000);
         saveAnswer("failed",OTHER);
         said = told();
         expect(language + ": a save in a match polled " + polls(2210000000) + ", handed " + saveCalls("video_tutorial_link_clicked").length + ", told " + said,
            polls(2210000000) == 0 && saveCalls("video_tutorial_link_clicked").length == 0 && said.length == 0);
         W.back();
         click(cards()[1],"srpWatchSave");
         expect(language + ": Save not asked again after a match",saveCalls("victory_screen_request_player_data").length == 1 && Asks.stateOf(OTHER) == "saving");
         saveAnswer("failed",OTHER);
         told();
         // a press Scaleform gave the content itself (RoomsLook.onContentDown) on the Save icon (a
         // plain Sprite, no `enabled`) saves too
         ExternalInterface.calls.length = 0;
         var icon:DisplayObject = find(cards()[1],"srpWatchSave");
         var content:DisplayObjectContainer = find(win,"srpWatchChip_friends").parent;
         var r:Rectangle = icon.getBounds(stage);
         var at:Point = content.globalToLocal(new Point(r.x + r.width / 2,r.y + r.height / 2));
         try
         {
            content.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,at.x,at.y));
         }
         catch(err:Error)
         {
            failures.push(language + ": a press on Save given to the content: " + err.message);
         }
         expect(language + ": a press on Save given to the content did not save",saveCalls("victory_screen_request_player_data").length == 1
            && Asks.stateOf(OTHER) == "saving");
         saveAnswer("failed",OTHER);
         told();
         trace("REPLAYBUTTONS " + language + ": Save: the icon, asked, polled, the link once, saved, refused, off, no answer, ended by a match");
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
         var close:* = screen.btn_close;
         // the close button alone is where the movie has it: the row's middle
         var middle:Number = close.x + close.width / 2;
         expect(language + ": the movie's Share or Save button shows",!screen.btnShare.visible && !screen.btnSave.visible);
         // nor their icons, one shape of the screen's: nothing shows where the two are
         var area:Rectangle = screen.btnShare.getBounds(screen).union(screen.btnSave.getBounds(screen));
         var shown:int = 0;
         for(var c:int = 0; c < screen.numChildren; c++)
         {
            var art:DisplayObject = screen.getChildAt(c);
            var box:Rectangle = art.getBounds(screen);
            // the screen's panels are larger than the two and lie under them
            shown += art.visible && box.width > 0 && box.width < 2 * area.width && box.intersects(area) ? 1 : 0;
         }
         expect(language + ": art of the movie's Share or Save button shows",shown == 0);
         fresh();
         show(screen,MATCH);
         number = find(screen,"srpMatchNumber") as TextField;
         expect(language + ": the results' row is not centred: " + button.x + ".." + (close.x + close.width) + " about " + middle,
            Math.abs((button.x + close.x + close.width) / 2 - middle) <= 1);
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
         expect(language + ": the results' row is not centred while saving",Math.abs((button.x + close.x + close.width) / 2 - middle) <= 1);
         // why it failed shows at once, over the results that hold the lobby's messages back
         // (LobbyMenu.show_victory_screen: Message.hideMessages)
         Message.hideMessages();
         saveAnswer("busy",OTHER);
         var busy:Array = told();
         Message.showMessages();
         expect(language + ": Save replay not back after busy, or busy not said over the results: " + busy,save.label == words.getName("st_srp_replay_save")
            && busy.length == 1 && busy[0] == words.getName("st_srp_replay_busy"));
         // another match's save under way: Save replay greyed till it ends
         expect(language + ": Save replay greyed with no save under way",save.enabled);
         Asks.save(MATCH);
         expect(language + ": Save replay pressable while another match's save is under way",!save.enabled && save.label == words.getName("st_srp_replay_save"));
         saveAnswer("failed",MATCH);
         told();
         expect(language + ": Save replay not back after another match's save",save.enabled);
         // a save's refusal that says the replay is not kept: Watch replay and Save go at once (each
         // press would count against the server's lookups)
         save.dispatchEvent(new Click(Click.CLICK));
         saveAnswer("gone",OTHER);
         told();
         expect(language + ": Watch or Save replay still offered after a save's gone",!button.visible && !save.visible);
         Prompt.noteReplay(hex(OTHER),null);
         show(screen,MATCH);
         expect(language + ": a Watch my deaths",find(screen,"srpWatchDeaths") == null);
         expect(language + ": the results' number is '" + (number ? number.text : null) + "'",number != null && number.visible && number.text == "#1ce45cbe"
            && number.x + number.width <= button.x);
         // copied as a Watch card's code: an input field, a press selects it all, typing changes nothing
         number.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
         expect(language + ": the results' number is not selected whole by a press: " + number.selectionBeginIndex + ".." + number.selectionEndIndex,
            number.type == TextFieldType.INPUT && number.selectable && number.selectionBeginIndex == 0 && number.selectionEndIndex == number.length);
         number.text = "#1ce45cbe7";
         number.dispatchEvent(new Event(Event.CHANGE));
         expect(language + ": typing changed the results' number to '" + number.text + "'",number.text == "#1ce45cbe");
         idle();
         button.dispatchEvent(new Click(Click.CLICK));
         expect(language + ": Watch replay did not ask",asked()[0] == MATCH && asked()[1] == 0xC0FF0000);
         lobby.answer("gone",MATCH);
         expect(language + ": Watch or Save replay still offered after a watch's gone",!button.visible && !save.visible);
         show(screen,MATCH);
         var why:TextField = find(screen,"srpWatchReason") as TextField;
         expect(language + ": a replay not kept shows '" + (why && why.visible ? why.text : null) + "' and Watch or Save replay",!button.visible && !save.visible && why != null && why.visible
            && why.text == words.getName("st_srp_watch_reason_gone") && why.x + why.width <= close.x && number.x + number.width <= why.x);
         expect(language + ": the close button alone is not back in the middle: " + close.x,Math.abs(close.x + close.width / 2 - middle) <= 1);
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
