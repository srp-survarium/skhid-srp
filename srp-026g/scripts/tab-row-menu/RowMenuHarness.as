package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;
   import flash.text.TextField;

   // The Tab list's player menu (docs/notes/tab-cursor-friends.md, chat-mute.md) with the hud movie's own
   // classes: its scoreboard (SrvPlayerList, its PlayerListz and row renderers) and srp's PlayerRowMenu
   // over it, the exe's answers made here as patch 86 makes them (root.srp_player_menu,
   // root.srp_friend_request, root.srp_chat_mute):
   // - a right-click (Scaleform's MouseEventEx CLICK, buttonIdx 1) on another player's row asks the exe
   //   (srp_player_menu with the row's place) and opens the menu at the mouse with "Add to friends" and
   //   "Mute chat" (a friend's row: "Mute chat" alone);
   // - "Mute chat" opens its three choices in its place (Hide messages, Show name: <muted>, Unmute chat);
   //   each sends srp_chat_mute with the place and its choice (1, 2, 0) and shows the answer's line
   //   ("Chat muted", "Chat unmuted", "Not sent"); a friend request's answer then changes nothing;
   // - a real mouse click (check.sh's xdotool on the Ruffle window) on that entry sends srp_add_friend
   //   with the place, and the answer's line ("Friend request sent") shows until a press elsewhere;
   // - the own row (isLocal) asks nothing; a row the exe offers nothing for (a friend) opens nothing;
   //   a left click on a row opens nothing; an answer for another place, or one that comes when the menu
   //   is not waiting, changes nothing; "not sent" shows its own line; a press elsewhere and the list
   //   hidden (close, HUD.show_player_list(false)) close the menu;
   // - the match's chat (the hud's SrvGameChat) shows a line the lobby masked for this player as
   //   "Name: <muted>", the brackets as text (match and team channels).
   // Prints "MENU ..." lines (trace) and ends with "MENU RESULT pass" or "MENU RESULT fail: ...".
   public class RowMenuHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var list:MovieClip;

      private var menu:Object;

      private var Ex:Class;

      // the exe's calls the menu made: "method place"
      private var calls:Array = [];

      // the places the exe offers no friend request for (friends), and the requests' answer
      private var friends:Array = [2];

      private var sent:Boolean = true;

      // the exe answers inside its call (as patch 86 does); off: the answer comes later, by hand
      private var answering:Boolean = true;

      private var frame:int = 0;

      private var clicked:Point;

      private var target:Point;

      private var closedAt:int = -1;

      // 0 the cases, 1 waiting for check.sh's first click (where the window puts the stage), 2 the
      // menu open for the real click
      private var phase:int = 0;

      private var enemy:DisplayObject;

      private var failures:Array = [];

      public function RowMenuHarness()
      {
         var movie:String = loaderInfo.parameters.movie || "hud.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the hud's own root fails outside the game (no exe): only its classes are used
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
         stage.addEventListener(MouseEvent.MOUSE_DOWN,function(e:MouseEvent):void
         {
            clicked = new Point(e.stageX,e.stageY);
            trace("MENU mouse down at " + int(e.stageX) + "," + int(e.stageY));
         },true);
      }

      private function loaded(e:Event) : void
      {
         domain = loader.contentLoaderInfo.applicationDomain;
         (domain.getDefinition("survarium.data.LocalizationData") as Class).setData({
            st_srp_hud_add_friend:"Add to friends",
            st_srp_hud_friend_sent:"Friend request sent",
            st_srp_hud_friend_not_sent:"Friend request not sent",
            st_srp_hud_mute_chat:"Mute chat",
            st_srp_hud_mute_hide:"Hide messages",
            st_srp_hud_mute_mask:"Show name: <muted>",
            st_srp_hud_unmute:"Unmute chat",
            st_srp_hud_muted:"Chat muted",
            st_srp_hud_unmuted:"Chat unmuted",
            st_srp_hud_mute_not_sent:"Not sent"
         });
         Ex = domain.getDefinition("scaleform.gfx.MouseEventEx") as Class;
         var List:Class = domain.getDefinition("SrvPlayerList") as Class;
         list = new List() as MovieClip;
         list.x = 40;
         list.y = 20;
         addChild(list);
         var Menu:Class = domain.getDefinition("survarium.controls.PlayerRowMenu") as Class;
         menu = new Menu(list.players);
         list.addChild(DisplayObject(menu));
         // patch 86's answers, as the exe gives them inside the call
         menu.call = function(method:String, place:String, ... choice):void
         {
            calls.push([method,place].concat(choice).join(" "));
            if(!answering)
            {
               return;
            }
            if(method == "srp_player_menu")
            {
               menu.offered(uint(place),friends.indexOf(uint(place)) < 0,true);
            }
            else if(method == "srp_add_friend")
            {
               menu.requested(uint(place),sent);
            }
            else if(method == "srp_chat_mute")
            {
               menu.muted(uint(place),sent);
            }
         };
         addEventListener(Event.ENTER_FRAME,this.tick);
      }

      // A row of the list's team 2 (the enemies) as the list shows it: the movie's own renderer.
      private function row(data:Object) : DisplayObject
      {
         var Renderer:Class = domain.getDefinition("PlayerListRenderer") as Class;
         var renderer:Object = new Renderer();
         renderer.setData(data);
         DisplayObjectContainer(list.players.team2list).addChild(DisplayObject(renderer));
         renderer.y = 30 * DisplayObjectContainer(list.players.team2list).numChildren;
         return DisplayObject(renderer);
      }

      // Scaleform's click on `at`: its MouseEventEx of the given button (the menu takes a row's on the way
      // down).
      private function click(at:DisplayObject, button:uint) : void
      {
         var event:MouseEvent = new Ex(MouseEvent.CLICK) as MouseEvent;
         Object(event).buttonIdx = button;
         at.dispatchEvent(event);
      }

      private function menuText() : String
      {
         var texts:Array = [];
         collect(DisplayObjectContainer(menu),texts);
         return texts.join("|");
      }

      private static function collect(at:DisplayObjectContainer, texts:Array) : void
      {
         for(var i:int = 0; i < at.numChildren; i++)
         {
            var child:DisplayObject = at.getChildAt(i);
            if(child is TextField)
            {
               texts.push(TextField(child).text);
            }
            else if(child is DisplayObjectContainer)
            {
               collect(DisplayObjectContainer(child),texts);
            }
         }
      }

      private function expect(what:String, state:String, text:String, made:Array) : void
      {
         var got:String = menu.state + " [" + menuText() + "] " + calls.join(",");
         var want:String = state + " [" + text + "] " + made.join(",");
         trace("MENU " + what + ": " + got);
         if(got != want)
         {
            failures.push(what + ": " + got + ", not " + want);
         }
         calls = [];
      }

      private function tick(e:Event) : void
      {
         frame++;
         if(frame == 5)
         {
            var own:DisplayObject = row({id:0,team:0,name:"me",isLocal:true,kills:0,deaths:0,rank:0});
            enemy = row({id:3,team:1,name:"Wolf",kills:2,deaths:1,rank:40});
            var friend:DisplayObject = row({id:2,team:1,name:"Bear",kills:0,deaths:0,rank:0});
            click(own,1);
            expect("the own row",'closed',"",[]);
            click(enemy,0);
            expect("a left click on a row",'closed',"",[]);
            click(friend,1);
            expect("a friend's row: Mute chat alone",'open',"Mute chat",["srp_player_menu 2"]);
            menu.close();
            click(enemy,1);
            expect("another player's row",'open',"Add to friends|Mute chat",["srp_player_menu 3"]);
            menu.offered(3,true,true);
            menu.requested(3,true);
            menu.muted(3,true);
            expect("answers that come unasked",'open',"Add to friends|Mute chat",[]);
            stage.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true));
            expect("a press elsewhere",'closed',"",[]);
            answering = false;
            click(enemy,1);
            menu.offered(5,true,true);
            expect("an answer for another place",'asking',"",["srp_player_menu 3"]);
            menu.offered(3,false,false);
            expect("a place that offers nothing",'closed',"",[]);
            click(enemy,1);
            menu.offered(3,true,true);
            expect("then its own answer",'open',"Add to friends|Mute chat",["srp_player_menu 3"]);
            click(entryAt(1),0);
            expect("Mute chat: its three choices",'open',"Hide messages|Show name: <muted>|Unmute chat",[]);
            click(entryAt(0),0);
            menu.requested(3,true);
            expect("Hide messages, a friend request's answer",'sending',"Hide messages|Show name: <muted>|Unmute chat",
               ["srp_chat_mute 3 1"]);
            menu.muted(3,true);
            expect("then the mute's answer",'told',"Chat muted",[]);
            answering = true;
            menu.close();
            click(enemy,1);
            click(entryAt(1),0);
            click(entryAt(1),0);
            expect("Show name: <muted>",'told',"Chat muted",["srp_player_menu 3","srp_chat_mute 3 2"]);
            menu.close();
            click(enemy,1);
            click(entryAt(1),0);
            click(entryAt(2),0);
            expect("Unmute chat",'told',"Chat unmuted",["srp_player_menu 3","srp_chat_mute 3 0"]);
            menu.close();
            sent = false;
            click(friend,1);
            click(entryAt(0),0);
            click(entryAt(0),0);
            expect("a friend's mute not sent",'told',"Not sent",["srp_player_menu 2","srp_chat_mute 2 1"]);
            sent = true;
            menu.close();
            checkMutedChat();
            menu.close();
            click(enemy,1);
            menu.close();
            expect("the list hidden",'closed',"",["srp_player_menu 3"]);
            sent = false;
            click(enemy,1);
            calls = [];
            var entry:DisplayObject = DisplayObjectContainer(menu).getChildAt(0);
            click(entry,0);
            expect("a request not sent",'told',"Friend request not sent",["srp_add_friend 3"]);
            menu.close();
            sent = true;
            clicked = null;
            phase = 1;
            trace("MENU calibrate");
         }
         else if(phase == 1 && clicked != null)
         {
            phase = 2;
            click(enemy,1);
            expect("the menu for the click",'open',"Add to friends|Mute chat",["srp_player_menu 3"]);
            var entry2:DisplayObject = DisplayObjectContainer(menu).getChildAt(0);
            target = entry2.localToGlobal(new Point(entry2.width / 2,12));
            clicked = null;
            trace("MENU target the entry at " + int(target.x) + "," + int(target.y));
         }
         else if(phase == 2 && closedAt < 0 && clicked != null && Point.distance(clicked,target) < 3)
         {
            if(menu.state == "told")
            {
               expect("a real click on Add to friends",'told',"Friend request sent",["srp_add_friend 3"]);
               closedAt = frame;
            }
         }
         else if(closedAt > 0 && frame == closedAt + 150)
         {
            expect("2.5 s after: the answer's line stays",'told',"Friend request sent",[]);
            var line:DisplayObject = DisplayObjectContainer(menu).getChildAt(0);
            line.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true));
            expect("a press on the line",'told',"Friend request sent",[]);
            stage.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true));
            expect("a press elsewhere",'closed',"",[]);
            finish(failures.length ? "fail: " + failures.join("; ") : "pass");
         }
         else if(frame == 900)
         {
            failures.push("the real click on the entry did nothing (" + menu.state + ")");
            finish("fail: " + failures.join("; "));
         }
      }

      private function entryAt(i:int) : DisplayObject
      {
         return DisplayObjectContainer(menu).getChildAt(i);
      }

      // The lobby sends a muted player's line to this player with the body "<muted>" (chat-mute.md): the
      // hud's chat shows it as its text, brackets and all, on the match and the team channels.
      private function checkMutedChat() : void
      {
         try
         {
            var chat:Object = new (domain.getDefinition("SrvGameChat") as Class)();
            chat.setLocalPlayer("me");
            chat.test_message({name:"Wolf",text:"<muted>",type:5,color:"White"});
            chat.test_message({name:"Bear",text:"<muted>",type:6,color:"Green"});
            var count:int = chat.messageContainer.numChildren;
            var match:String = chat.messageContainer.getChildAt(count - 2).textContainer.textField.text;
            var team:String = chat.messageContainer.getChildAt(count - 1).textContainer.textField.text;
            trace("MENU the match's chat: " + match.replace(/\s+$/,"") + " | " + team.replace(/\s+$/,""));
            if(match.indexOf("Wolf: <muted>") < 0 || team.indexOf("Bear: <muted>") < 0)
            {
               failures.push("the chat does not show Name: <muted> (" + match + " | " + team + ")");
            }
         }
         catch(err:Error)
         {
            failures.push("the match's chat: " + err.getStackTrace());
         }
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.tick);
         trace("MENU RESULT " + result);
         fscommand("quit");
      }
   }
}
