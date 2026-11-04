package
{
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.utils.setTimeout;

   // Real rebuilt HUD classes, on a stand-in root (the native game's callbacks are absent).
   public class ModeIntroHarness extends Sprite
   {
      private var loader:Loader = new Loader();
      private var domain:ApplicationDomain;
      private var frames:int;
      private var failures:Array = [];
      private var calls:Array = [];
      private var notices:Array = [];
      private var intro:Object;
      private var view:Object;
      private var face:Sprite = new Sprite();
      private var kept:Object;
      private var chat:Object;

      public function ModeIntroHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void { trace("INTRO RESULT fail: " + e); });
         // HUD's root requires native callbacks. Tested classes below do not suppress errors.
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
         loader.load(new URLRequest(loaderInfo.parameters.movie || "hud.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function expect(ok:Boolean, what:String) : void
      {
         trace("INTRO " + what + ": " + (ok ? "pass" : "FAIL"));
         if(!ok) failures.push(what);
      }

      private function loaded(e:Event) : void
      {
         domain = loader.contentLoaderInfo.applicationDomain;
         var Kept:Class = domain.getDefinition("survarium.controls.KeptLayer") as Class;
         var View:Class = domain.getDefinition("survarium.controls.ForcedView") as Class;
         var Chat:Class = domain.getDefinition("SrvGameChat") as Class;
         addChild(face);
         kept = new Kept(face);
         addChild(DisplayObject(kept));
         chat = new Chat();
         face.addChild(DisplayObject(chat));
         chat.x = 20;
         chat.y = 350;
         view = new View(kept,function():Array { return []; },function():Array { return [chat]; },
            function():Array { return [chat]; },function(hold:Boolean):void {
               if(hold) { chat.messageContainer.showLastMessages(); chat.messageContainer.stopHideout(); }
               else chat.messageContainer.resumeHideout();
            });
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function tick(e:Event) : void
      {
         if(++frames != 5) return;
         removeEventListener(Event.ENTER_FRAME,tick);
         try
         {
            run();
            // A second incoming line used to restart the fade timer even while history was held.
            setupChat();
            view.notice = true;
            chat.addIntroduction("Held System notice");
            chat.messageContainer.addText("another line");
            setTimeout(finish,8500);
         }
         catch(error:Error) { trace("INTRO RESULT fail: " + error.toString() + " " + error.getStackTrace()); }
      }

      private function finish() : void
      {
         expect(chat.messageContainer.numMessages == 2,"incoming line cannot expire held history after seven seconds");
         view.notice = false;
         trace("INTRO RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
      }

      private function send(value:Object, channel:uint) : void
      {
         calls.push(value.text);
         expect(channel == 10,"private control channel");
         expect(intro.receive({type:10,text:value.text,name:"Alice"},0),"local echo consumed");
      }

      private function setupChat() : void
      {
         chat.clear();
         chat.addTab({id:5,name:"All",channels:[5],channel_to_send:5,closable:false});
         chat.addTab({id:6,name:"Team",channels:[6],channel_to_send:6,closable:false});
         chat.send_function = send;
      }

      private function readable(text:String) : Boolean
      {
         if(chat.messageContainer.numMessages != 1) return false;
         for(var i:int=0; i<chat.messageContainer.numChildren; i++)
         {
            var child:Object = chat.messageContainer.getChildAt(i);
            if(child.hasOwnProperty("textContainer") &&
               child.textContainer.textField.text.indexOf("System: " + text) == 0)
               return child.textContainer.alpha == 1 && child.textContainer.textField.textHeight <= 90;
         }
         return false;
      }

      private function run() : void
      {
         var words:Object = domain.getDefinition("survarium.modes.ModeIntroText");
         var Intro:Class = domain.getDefinition("survarium.modes.ModeIntroduction") as Class;
         var Pregame:Class = domain.getDefinition("SrvContextInfoPregame") as Class;
         var pregame:Object = new Pregame();
         addChild(DisplayObject(pregame));
         var data:Object = domain.getDefinition("survarium.data.LocalizationData");
         var all:Array = HarnessText.LANGUAGES;
         expect(!words.ready("blackout"),"wait for native localization push");
         for(var lang:int = 0; lang < all.length; lang++)
         {
            data.setData(HarnessText.DATA[all[lang]]);
            expect(words.ready("blackout"),"localized " + all[lang]);
            expect(words.chat("blackout").indexOf("Tab") >= 0 && words.start("blackout").indexOf("Tab") >= 0,"translated controls " + lang);
            setupChat();
            chat.addIntroduction(words.chat("blackout"));
            expect(readable(words.chat("blackout")),"full System text immediately readable " + lang);
            words.pregame(pregame.contextText,"blackout","00 : 15");
            expect(pregame.contextText.text.indexOf("00 : 15") >= 0 && pregame.contextText.width <= 600 &&
               pregame.contextText.textHeight < 220,"start text and timer fit " + lang);
         }
         words.pregame(pregame.contextText,null,"ordinary countdown");
         expect(pregame.contextText.text == "ordinary countdown" && !pregame.contextText.wordWrap,"normal countdown restored");
         words.pregame(pregame.contextText,"blackout","");
         expect(pregame.contextText.text == "","empty countdown stays empty");
         var Notice:Class = domain.getDefinition("survarium.modes.ModeNotice") as Class;
         var notice:Object = new Notice(pregame.contextText.defaultTextFormat.font);
         addChild(DisplayObject(notice));
         for each(var language:String in all)
         {
            data.setData(HarnessText.DATA[language]);
            notice.reset();
            notice.begin("blackout");
            notice.update(500000,false,false);
            expect(!notice.visible,"notice waits for loaded HUD " + language);
            notice.update(600000,true,false);
            expect(notice.visible && notice.label.text.indexOf("Tab") >= 0,"late join sees notice " + language);
            notice.place(1280,80);
            expect(notice.y > 80 && notice.label.textHeight < 160,"notice below timer and fits " + language);
            words.pregame(pregame.contextText,null,"");
            pregame.visible = false;
            notice.update(619999,true,false);
            expect(notice.visible,"late notice survives native countdown clearing " + language);
            notice.update(620000,true,false);
            expect(!notice.visible,"notice expires after full twenty seconds " + language);
            notice.begin("blackout");
            notice.update(620001,true,false);
            expect(!notice.visible,"repeated mode callbacks do not restart notice " + language);
         }
         notice.reset();
         notice.begin("blackout");
         notice.update(0,true,true);
         notice.update(60000,true,true);
         expect(notice.visible,"pregame notice remains through countdown");
         notice.update(60001,true,false);
         notice.update(80000,true,false);
         expect(notice.visible,"full reading interval after countdown");
         notice.reset();
         expect(!notice.visible && notice.label.text == "","new match resets notice");
         var NoticeView:Class = domain.getDefinition("survarium.controls.ForcedView") as Class;
         face.addChild(DisplayObject(notice));
         var noticeView:Object = new NoticeView(kept,function():Array { return [notice]; },
            function():Array { return []; },function():Array { return [chat]; },function(hold:Boolean):void {});
         face.visible = false;
         noticeView.forced = true;
         notice.begin("blackout");
         notice.update(100000,true,false);
         expect(notice.parent == kept && notice.visible,"notice survives forced-hidden root");
         expect(noticeView.parts().indexOf(chat) < 0,"notice does not reveal chat");
         noticeView.forced = false;
         notice.reset();
         expect(notice.parent == face && !notice.visible,"notice returns and clears on unforce");
         data.setData(HarnessText.DATA["russian"]);
         setupChat();
         face.visible = false;
         view.forced = true;
         intro = new Intro(function(mode:String):void {
            notices.push(mode);
            chat.addIntroduction(words.chat(mode));
         },function(show:Boolean):void { view.notice = show; });
         intro.update(0,true,send);
         expect(calls.length == 0,"ordinary match sends nothing");
         intro.begin("blackout");
         intro.update(0,false,send);
         expect(calls.length == 0,"wait for chat and language");
         intro.update(10,chat.introductionReady(),send);
         intro.update(2009,true,send);
         expect(calls.length == 1,"first ready frame and bounded retry");
         intro.update(2010,true,send);
         expect(calls.length == 2 && calls[0] == calls[1],"retry keeps activation token");
         var reply:Object = {type:10,name:"System",text:calls[0] + ":show"};
         intro.receive({type:10,name:"Mallory",text:reply.text},2020);
         expect(notices.length == 0,"named player cannot trigger notice");
         intro.receive(reply,2020);
         expect(notices.length == 1 && chat.parent == kept && chat.messageContainer.numMessages > 0,"immediate notice above hidden HUD");
         expect(!chat.text_inpput.visible && stage.focus == null,"no typing or focus stolen");
         chat.focusTab(6);
         expect(chat.messageContainer.numMessages > 0,"notice retained in other chat tab");
         intro.receive(reply,3000);
         intro.begin("blackout");
         intro.update(22019,true,send);
         expect(notices.length == 1 && chat.parent == kept,"duplicate reply does not duplicate notice");
         view.typing = true;
         intro.update(22020,true,send);
         expect(chat.parent == kept,"expiry preserves typing view");
         view.tab = true;
         view.typing = false;
         expect(chat.parent == kept,"expiry preserves Tab view");
         view.tab = false;
         expect(chat.parent == face && !face.visible,"expiry restores hidden chat");
         intro.reset();
         intro.begin("blackout");
         intro.update(30000,true,send);
         expect(calls[2] != calls[0],"new activation gets new token");
         intro.receive(reply,30001);
         expect(notices.length == 1,"previous match reply ignored");
         intro.receive({type:10,name:"System",text:calls[2] + ":done"},30002);
         intro.update(40000,true,send);
         expect(calls.length == 3 && notices.length == 1,"allowance exhausted stops requests");
         intro.reset();
         intro.begin("scarcity");
         intro.update(50000,true,send);
         expect(calls[3].indexOf("#mode-intro:scarcity:") == 0,"controller reusable with another mode key");
         intro.reset();
         intro.receive({type:10,name:"System",text:calls[3] + ":show"},50001);
         expect(notices.length == 1 && chat.parent == face,"teardown ignores pending reply");
      }
   }
}
