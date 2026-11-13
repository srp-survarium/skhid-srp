package
{
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.TextEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;
   import flash.text.TextField;

   // A player's name, clan tag or line in the chats, whatever it holds, is shown as typed and read as
   // no markup (player-to-player.md §8): the lobby's chat (SrvLobbyChat: the name a link its click
   // reads back whole) and the match's (SrvGameChat), the rebuilt movies' own classes.
   public class ChatNamesHarness extends Sprite
   {
      private var lobby:Loader = new Loader();
      private var hud:Loader = new Loader();
      private var loaded:int = 0;
      private var frames:int = 0;
      private var failures:Array = [];

      // Names a player could take: markup that would end the link's attribute and add its own, a tag
      // left open (as the cut to 63 bytes can leave one), a colon, an emoji, an entity, a Cyrillic name.
      private static const NAMES:Array = [
         "Bob",
         "Вася_99",
         "a:b",
         "X😀",
         "constructor",
         "O'Neil",
         "Bob \"the\" Builder",
         "Eve' href='event:name:Bob'><font color='#FF0000'>[admin] Bob",
         "<font size='40'><b>Mallory",
         "x&amp;y &lt;z&gt; \"q\" 100%"
      ];

      public function ChatNamesHarness()
      {
         for each(var movie:Loader in [lobby,hud])
         {
            movie.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            movie.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void { loaded++; });
            movie.contentLoaderInfo.addEventListener("ioError",function(e:Event):void { finish("fail: " + e); });
         }
         lobby.load(new URLRequest(loaderInfo.parameters.lobby || "lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
         hud.load(new URLRequest(loaderInfo.parameters.hud || "hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function expect(ok:Boolean, what:String) : void
      {
         trace("NAMES " + what + ": " + (ok ? "ok" : "FAIL"));
         if(!ok) failures.push(what);
      }

      private function tick(e:Event) : void
      {
         if(loaded < 2 || ++frames < 5) return;
         removeEventListener(Event.ENTER_FRAME,tick);
         try { checkLobby(); } catch(error:Error) { failures.push("lobby: " + error.getStackTrace()); }
         try { checkHud(); } catch(error:Error) { failures.push("hud: " + error.getStackTrace()); }
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      private function finish(result:String) : void
      {
         trace("NAMES RESULT " + result);
         fscommand("quit");
      }

      // The last line of a field's text (its lines end in "\n" or "\r").
      private static function lastLine(field:TextField) : String
      {
         var lines:Array = field.text.replace(/[\r\n]+$/,"").split(/[\r\n]/);
         return lines[lines.length - 1];
      }

      private function checkLobby() : void
      {
         var domain:ApplicationDomain = lobby.contentLoaderInfo.applicationDomain;
         var place:Sprite = new Sprite();
         addChild(place);
         // What the lobby's root gives its windows (the root needs the game's callbacks).
         var scene:Object = domain.getDefinition("MainScene");
         scene.stage = place;
         scene.scaleFactor = 1;
         var chat:Object = new (domain.getDefinition("SrvLobbyChat") as Class)();
         place.addChild(DisplayObject(chat));
         // The bottom panel's tab bar, which focusTab reads (LobbyMenu gives it the panel's).
         var bar:Object = new (domain.getDefinition("survarium.controls.ButtonBarBottom") as Class)();
         place.addChild(DisplayObject(bar));
         chat.tapPageRenderer = bar;
         chat.setLocalPlayer("me");
         chat.addTab({id:1,name:"All",channels:[5],channel_to_send:5,closable:false});
         chat.validateNow();
         chat.focusTab(1);
         var tab:Object = place.getChildByName("tab_All");
         var field:TextField = tab.text_chat.textField;
         // LobbyChat calls its tab's protected addMessage, which Scaleform finds and Ruffle does not:
         // the same, a public one on the tab (SrvChatTab is dynamic).
         var quiet:Boolean = false;
         tab.addMessage = function(text:String):void { if(!quiet) { tab.text_chat.appendHtml(text + "\n"); tab.text_chat.validateNow(); } };
         var clicked:String;
         for each(var name:String in NAMES)
         {
            chat.got_message({type:5,name:name,abbr:"<i>",text:"<b>hi</b> & bye"});
            var line:String = lastLine(field);
            expect(line.substr(line.indexOf(" [")) == " [<i>] " + name + ":  <b>hi</b> & bye","lobby: the line of " + name + " as typed (" + line + ")");
            // The link: the characters of "<name>: ", and no other.
            var at:int = field.text.lastIndexOf(name + ": ");
            var url:String = at < 0 ? "" : String(field.getTextFormat(at,at + 1).url);
            expect(/^event:name:[0-9]+$/.test(url) && String(field.getTextFormat(at + name.length + 1,at + name.length + 2).url) == url,
               "lobby: " + name + "'s link over its name (" + url + ")");
            expect(String(field.getTextFormat(at - 2,at - 1).url) != url && String(field.getTextFormat(at + name.length + 3,at + name.length + 4).url) != url,
               "lobby: " + name + "'s link only over its name");
            // A left click on it starts a line to that player (LobbyChat.clickLinkHandler).
            field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,url.substr("event:".length)));
            clicked = tab.text_inpput.text;
            expect(clicked == name + ", ","lobby: a click on " + name + " names it (" + clicked + ")");
         }
         // A link to no sender the chat holds names nobody.
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,"name:99999"));
         expect(tab.text_inpput.text == clicked,"lobby: a link to no sender does nothing (" + tab.text_inpput.text + ")");
         // A line after them all is as a plain name's: nothing a name opened reaches it.
         chat.got_message({type:5,name:"Bob",text:"after"});
         var bob:int = field.text.indexOf("Bob: ");
         var after:int = field.text.lastIndexOf("Bob: ");
         var first:Object = field.getTextFormat(bob,bob + 1);
         var last:Object = field.getTextFormat(after,after + 1);
         expect(first.size == last.size && first.color == last.color && first.bold == last.bold,"lobby: the line after them as the first line");
         // A line with no sender (a room's notice, the clan's lines) holds names too.
         chat.got_message({type:5,text:"<font size='40'>Eve joined"});
         expect(lastLine(field).indexOf("<font size='40'>Eve joined") > 0,"lobby: a line without a sender as given (" + lastLine(field) + ")");
         // The chat keeps the 2048 senders who spoke last, each its number while kept. Lines of
         // many senders, not shown (the tab keeps 180 lines; all of them would only be slow here).
         var many:Function = function(from:int, count:int):void {
            quiet = true;
            for(var n:int = from; n < from + count; n++) chat.got_message({type:5,name:"p" + n,text:"x"});
            quiet = false;
         };
         // Bob, then 1500 others, then Bob, then 1000 newcomers: Bob spoke among the last 2048,
         // so his new line is his, under his first number (0).
         many(0,1500);
         chat.got_message({type:5,name:"Bob",text:"still here"});
         many(1500,1000);
         at = field.text.lastIndexOf("Bob: ");
         url = String(field.getTextFormat(at,at + 1).url);
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,url.substr("event:".length)));
         expect(url == "event:name:0" && tab.text_inpput.text == "Bob, ","lobby: a sender who spoke again is kept (" + url + ", " + tab.text_inpput.text + ")");
         // The newcomers before him are let go first: p0 names nobody, the last one is named.
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,"name:" + NAMES.length));
         expect(tab.text_inpput.text == "Bob, ","lobby: the sender who spoke longest ago is let go (" + tab.text_inpput.text + ")");
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,"name:" + (NAMES.length + 2499)));
         expect(tab.text_inpput.text == "p2499, ","lobby: the last sender kept (" + tab.text_inpput.text + ")");
         // 2048 newcomers after Bob: his number names nobody; speaking again, he gets a new one.
         many(2500,2048);
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,"name:0"));
         expect(tab.text_inpput.text == "p2499, ","lobby: a sender let go names nobody (" + tab.text_inpput.text + ")");
         chat.got_message({type:5,name:"Bob",text:"back"});
         at = field.text.lastIndexOf("Bob: ");
         url = String(field.getTextFormat(at,at + 1).url);
         field.dispatchEvent(new TextEvent(TextEvent.LINK,false,false,url.substr("event:".length)));
         expect(url != "event:name:0" && tab.text_inpput.text == "Bob, ","lobby: a sender back is named again (" + url + ", " + tab.text_inpput.text + ")");
      }

      private function checkHud() : void
      {
         var domain:ApplicationDomain = hud.contentLoaderInfo.applicationDomain;
         var chat:Object = new (domain.getDefinition("SrvGameChat") as Class)();
         chat.x = 2000;
         addChild(DisplayObject(chat));
         chat.setLocalPlayer("me");
         chat.addTab({id:3,name:"All",channels:[5],channel_to_send:5,closable:false});
         for each(var name:String in NAMES)
         {
            chat.got_message({type:5,name:name,text:"<b>hi</b> & bye",color:"White"});
            var field:TextField = chat.messageContainer.getChildAt(chat.messageContainer.numChildren - 1).textContainer.textField;
            expect(lastLine(field) == name + ": <b>hi</b> & bye","hud: the line of " + name + " as typed (" + lastLine(field) + ")");
         }
         chat.got_message({type:5,text:"<font size='40'>Eve left"});
         field = chat.messageContainer.getChildAt(chat.messageContainer.numChildren - 1).textContainer.textField;
         expect(lastLine(field) == "<font size='40'>Eve left","hud: a line without a sender as given (" + lastLine(field) + ")");
         chat.addIntroduction("Don't <b>stop</b> & go");
         field = chat.messageContainer.getChildAt(chat.messageContainer.numChildren - 1).textContainer.textField;
         expect(lastLine(field) == "System: Don't <b>stop</b> & go","hud: a System line as given (" + lastLine(field) + ")");
      }
   }
}
