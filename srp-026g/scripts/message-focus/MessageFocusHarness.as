package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.InteractiveObject;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.ui.Keyboard;
   import scaleform.gfx.FocusManager;

   // Closing a message gives the focus back (srp-026g/flash/*/src/survarium/controls/Message.as):
   // the login movie's own LoginMenu (a copy of the movie that starts this driver at the end of its
   // root's constructor) and the main menu's Message, run in Ruffle with a model of GFx's focus
   // manager (scaleform/gfx/FocusManager.as here: GFx implements that class natively, the movies
   // carry an empty stub). A key goes to the focused object, as GFx sends it.
   // - login, Esc: the password field focused, Esc opens the quit dialog, Esc closes it, Enter signs in;
   // - login, the close button: clicked (it takes the focus if it can), the dialog's Cancel clicked,
   //   Enter signs in;
   // - login, Tab after a close: the focus goes round the form's fields;
   // - main menu: a message's close gives the focus and the modal clip back to what had them, two
   //   messages open, the first closed gives them to the second.
   // Prints "FOCUS ..." lines and ends with "FOCUS RESULT pass" or "FOCUS RESULT fail: ...".
   public class MessageFocusHarness
   {
      private static var me:MessageFocusHarness;

      private var stage:Stage;

      private var root:Sprite;

      // The login movie's root calls this at its constructor's end (check.sh adds the call to a copy).
      public static function start(menu:Sprite) : void
      {
         me = new MessageFocusHarness(menu);
      }

      private var mainLoader:Loader = new Loader();

      private var menu:*;

      private var loginDomain:ApplicationDomain;

      private var mainDomain:ApplicationDomain;

      private var steps:Array = [];

      private var wait:int = 0;

      private var failures:Array = [];

      private var signedIn:Boolean;

      private var home:Sprite;

      private var form:Sprite;

      public function MessageFocusHarness(menu:Sprite)
      {
         super();
         this.menu = menu;
         root = menu;
         stage = menu.stage;
         loginDomain = ApplicationDomain.currentDomain;
         mainLoader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,uncaught);
         root.addChild(mainLoader);
         say("login movie: " + menu + ", the focus manager is the model: " + FocusManager.isModel);
         steps = [function():void
         {
            settle(30);
         },escCase,closeButtonCase,tabCase,function():void
         {
            mainLoader.contentLoaderInfo.addEventListener(Event.INIT,mainLoaded);
            mainLoader.load(new URLRequest("main_menu.swf"),new LoaderContext(false,new ApplicationDomain(ApplicationDomain.currentDomain)));
            wait = 1000000;
         }];
         root.addEventListener(Event.ENTER_FRAME,tick);
      }

      private function uncaught(e:UncaughtErrorEvent) : void
      {
         e.preventDefault();
         say("uncaught: " + e.error);
      }

      // The movie is built without trace(): the lines are kept, and the result reports them all.
      private var lines:Array = [];

      private function say(text:String) : void
      {
         lines.push("FOCUS " + text);
      }

      private function check(ok:Boolean, what:String) : void
      {
         say((ok ? "ok " : "FAIL ") + what);
         if(!ok)
         {
            failures.push(what);
         }
      }

      private function finish(result:String) : void
      {
         root.removeEventListener(Event.ENTER_FRAME,tick);
         // uncaught, from the player's own event: Ruffle logs it, its message whole
         throw new Error("\n" + lines.join("\n") + "\nFOCUS RESULT " + result);
      }

      // Each step runs on its own frame, after the frames the step before asked for.
      private function tick(e:Event) : void
      {
         if(wait > 0)
         {
            wait--;
            return;
         }
         if(steps.length == 0)
         {
            finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
            return;
         }
         var step:Function = steps.shift();
         try
         {
            step();
         }
         catch(err:Error)
         {
            check(false,"a step threw " + err + "\n" + err.getStackTrace());
         }
      }

      // A few frames: GFx renders every frame, so the message's RENDER (it takes the focus) runs.
      private function settle(frames:int = 3) : void
      {
         stage.invalidate();
         wait = frames;
      }

      private function message(domain:ApplicationDomain) : *
      {
         return domain.getDefinition("survarium.controls.Message");
      }

      // A key down and up at the focused object, as GFx sends it (the stage without a focus).
      private function key(code:uint) : void
      {
         for each(var type:String in [KeyboardEvent.KEY_DOWN,KeyboardEvent.KEY_UP])
         {
            var target:InteractiveObject = FocusManager.getFocus() || stage;
            if(target.stage == null)
            {
               target = stage;
            }
            try
            {
               target.dispatchEvent(new KeyboardEvent(type,true,false,code == Keyboard.ENTER ? 13 : 0,code));
            }
            catch(err:Error)
            {
               say("  " + type + " " + code + " at " + target + ": " + err.message);
            }
         }
         if(code == Keyboard.TAB)
         {
            // GFx's Tab (and CLIK's FocusHandler, which asks findFocus): the next in the modal clip
            try
            {
               FocusManager.moveFocus("tab");
            }
            catch(err:Error)
            {
               say("  moveFocus: " + err.message);
            }
         }
      }

      // A mouse click on a CLIK button: a focusable one takes the focus (GFx's mouse focus change).
      private function click(button:*) : void
      {
         if(button.tabEnabled)
         {
            FocusManager.setFocus(button);
         }
         var ButtonEvent:Class = loginDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
         button.dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
      }

      private function startSignInWatch() : void
      {
         HarnessExternal.calls = [];
         menu.password_input.text = "secret";
         menu.login_input.text = "player@skhid";
      }

      // sign_in_button_clicked asks the exe to sign in; nothing asked it to quit.
      private function signedInNow() : Boolean
      {
         say("  the exe was asked: " + HarnessExternal.calls.join(", "));
         return HarnessExternal.calls.indexOf("sign_in_button_clicked") != -1 && HarnessExternal.calls.indexOf("exit_button_clicked") == -1;
      }

      private function open(domain:ApplicationDomain) : Boolean
      {
         return message(domain).isAnyMessage() != -1;
      }

      private function named(o:*) : String
      {
         return o == null ? "nothing" : (o.name + " (" + o + ")");
      }

      private function escCase() : void
      {
         startSignInWatch();
         FocusManager.setFocus(menu.password_input);
         say("before: the focus " + named(FocusManager.getFocus()) + ", the modal clip " + named(FocusManager.getModalClip()));
         key(Keyboard.ESCAPE);
         steps.unshift(function():void
         {
            check(open(loginDomain),"login, Esc: the quit dialog is open");
            say("  the dialog has the focus: " + named(FocusManager.getFocus()));
            key(Keyboard.ESCAPE);
            settle();
         },function():void
         {
            check(!open(loginDomain),"login, Esc again: the dialog is closed");
            say("  the focus " + named(FocusManager.getFocus()) + ", the modal clip " + named(FocusManager.getModalClip()));
            check(FocusManager.getFocus() == menu.password_input && FocusManager.getModalClip() == menu,
               "login, Esc: the password field has the focus again, the form the modal clip");
            key(Keyboard.ENTER);
            check(signedInNow(),"login, Esc, Esc, Enter: signs in");
            settle();
         });
         settle();
      }

      private function closeButtonCase() : void
      {
         startSignInWatch();
         FocusManager.setFocus(menu.password_input);
         say("the close button can take the focus: " + menu.exit_btn.tabEnabled);
         click(menu.exit_btn);
         steps.unshift(function():void
         {
            check(open(loginDomain),"login, the close button: the quit dialog is open");
            var dialog:* = find(stage,loginDomain.getDefinition("survarium.messages.MessageApply") as Class);
            check(dialog != null,"login: the dialog is on the stage");
            if(dialog != null)
            {
               click(dialog.btnCancel);
            }
            settle();
         },function():void
         {
            check(!open(loginDomain),"login, Cancel: the dialog is closed");
            say("  the focus " + named(FocusManager.getFocus()) + ", the modal clip " + named(FocusManager.getModalClip()));
            key(Keyboard.ENTER);
            check(signedInNow(),"login, the close button, Cancel, Enter: signs in");
            settle();
         });
         settle();
      }

      private function tabCase() : void
      {
         var seen:Array = [];
         for(var i:int = 0; i < 6; i++)
         {
            key(Keyboard.TAB);
            seen.push(FocusManager.getFocus());
         }
         say("  Tab after a close: " + seen.map(function(o:*, ...rest):String
         {
            return o == null ? "nothing" : o.name;
         }).join(", "));
         check(seen.indexOf(menu.login_input) != -1 && seen.indexOf(menu.password_input) != -1,
            "login, Tab after a close: the focus goes round the form's fields");
         settle();
      }

      private function find(container:DisplayObjectContainer, kind:Class) : *
      {
         for(var i:int = 0; i < container.numChildren; i++)
         {
            var child:DisplayObject = container.getChildAt(i);
            if(child is kind)
            {
               return child;
            }
            var inner:DisplayObjectContainer = child as DisplayObjectContainer;
            if(inner != null)
            {
               var found:* = find(inner,kind);
               if(found != null)
               {
                  return found;
               }
            }
         }
         return null;
      }

      private function mainLoaded(e:Event) : void
      {
         mainDomain = mainLoader.contentLoaderInfo.applicationDomain;
         var M:* = message(mainDomain);
         say("main menu movie: " + mainLoader.content);
         var layer:Sprite = new Sprite();
         stage.addChild(layer);
         M.initStage(layer);
         M.registerBackground(mainDomain.getDefinition("BackBlack") as Class);
         M.registerMessage("harness",mainDomain.getDefinition("survarium.messages.MessageApply") as Class);
         form = new Sprite();
         root.addChild(form);
         home = new Sprite();
         home.tabEnabled = true;
         home.name = "home";
         form.addChild(home);
         FocusManager.setModalClip(form);
         FocusManager.setFocus(home);
         var first:* = M.showMessage("harness",{text:"first"});
         var second:* = M.showMessage("harness",{text:"second"});
         wait = 0;
         steps = [function():void
         {
            settle();
         },function():void
         {
            say("main menu: two messages open, the focus " + named(FocusManager.getFocus()));
            check(FocusManager.getFocus() == first && FocusManager.getModalClip() == first,
               "main menu: the first message has the focus");
            M.close(first.getMessageId());
            check(FocusManager.getFocus() == second && FocusManager.getModalClip() == second,
               "main menu: the first closed, the second has the focus");
            M.close(second.getMessageId());
            say("  both closed: the focus " + named(FocusManager.getFocus()) + ", the modal clip " + named(FocusManager.getModalClip()));
            check(FocusManager.getFocus() == home && FocusManager.getModalClip() == form,
               "main menu: both closed, the focus and the modal clip back where they were");
            settle();
         }];
      }
   }
}
