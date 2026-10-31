package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.utils.getQualifiedClassName;
   import flash.utils.setTimeout;

   // The lobby scene's black (exe patch 81, docs/notes/lobby-scenes.md) with the lobby movie's own
   // root (LobbyMenu.srp_scene_fade(per mille, loading), the exe stepping it a frame each), a panel
   // of ours on it as the lobby's panels are:
   // - half (500, 0): the black the root's first child, under the panel, half drawn; no click on it;
   // - whole with the loading spinner (1000, 1): black where no panel is, the spinner on it, the
   //   panel drawn over it; a second call: one black, still the first child;
   // - gone (0, 0): off the display list;
   // - the lobby window (srp_lobby_window, its button's toggle): four cards, the state the exe
   //   tells (srp_lobby_state: the pick 3 marked), opened and closed, then left open (check.sh's
   //   window.png); the player panel with the lobby button in the premium button's place (none
   //   left there), drawn at the top left.
   // Prints "FADE ..." lines (trace) and ends with "FADE RESULT pass" or "FADE RESULT fail: ...".
   public class FadeHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var lobby:Object;

      private var panel:Sprite = new Sprite();

      // the black, as the first fade put it
      private var shade:Sprite;

      private var failures:Array = [];

      public function FadeHarness()
      {
         var movie:String = loaderInfo.parameters.movie || "lobby_menu.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): its fade does not need it
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
      }

      private function loaded(e:Event) : void
      {
         try
         {
            lobby = loader.content;
            // The English texts, as the exe gives them (set_localization_data).
            loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            addChild(loader);
            // a panel of the lobby's: a white square at 100,100
            panel.graphics.beginFill(0xFFFFFF,1);
            panel.graphics.drawRect(100,100,100,100);
            panel.graphics.endFill();
            lobby.addChild(panel);
            lobby.srp_scene_fade(500,0);
         }
         catch(err:Error)
         {
            finish("fail: the black: " + err);
            return;
         }
         setTimeout(this.half,100);
      }

      private function half() : void
      {
         var first:Sprite = this.fade();
         shade = first;
         if(first == null || first == panel || first.mouseEnabled || Math.abs(first.alpha - 0.5) > 0.01)
            failures.push("half: the first child is not the half black (" + first + ")");
         var outside:uint = at(300,300);
         trace("FADE half: " + outside.toString(16) + " where no panel is");
         if(outside != 0x80000000 && (outside >>> 24) < 0x70)
            failures.push("half: not half black (" + outside.toString(16) + ")");
         lobby.srp_scene_fade(1000,1);
         lobby.srp_scene_fade(1000,1);
         setTimeout(this.black,100);
      }

      private function fade() : Sprite
      {
         return lobby.getChildAt(0) as Sprite;
      }

      // The colour drawn at x,y of the lobby's root (the root alone, as the game draws it over the scene).
      private function at(x:int, y:int) : uint
      {
         var shot:BitmapData = new BitmapData(400,400,true,0);
         shot.draw(DisplayObject(lobby));
         return shot.getPixel32(x,y);
      }

      private function black() : void
      {
         if(lobby.getChildAt(0) != shade || lobby.getChildAt(1) == shade)
            failures.push("whole: not one black, the first child");
         if(lobby.getChildIndex(panel) <= 0)
            failures.push("whole: the panel under the black");
         var spinner:Object = shade.numChildren > 0 ? shade.getChildAt(0) : null;
         if(spinner == null || !spinner.visible)
            failures.push("whole: no loading spinner");
         var outside:uint = at(20,20);
         var inside:uint = at(150,150);
         trace("FADE whole: " + outside.toString(16) + " where no panel is, " + inside.toString(16) + " on the panel");
         if(outside != 0xFF000000)
            failures.push("whole: not black where no panel is (" + outside.toString(16) + ")");
         if(inside != 0xFFFFFFFF)
            failures.push("whole: the panel not drawn over it (" + inside.toString(16) + ")");
         lobby.srp_scene_fade(0,0);
         if(shade.parent != null)
            failures.push("gone: the black still on the root");
         lobby.srp_lobby_state(3,2);
         lobby.srp_lobby_window();
         setTimeout(this.window,200);
      }

      private function window() : void
      {
         var w:Object = null;
         for(var i:int = 0; i < lobby.numChildren; i++)
         {
            if(lobby.getChildAt(i).name == "srpLobbyWindow")
               w = lobby.getChildAt(i);
         }
         if(w == null)
         {
            failures.push("the lobby window not opened");
         }
         else
         {
            trace("FADE window: " + int(w.width) + " x " + int(w.height) + " at " + int(w.x) + "," + int(w.y));
            // two by two of vote-sized cards (280 x 190)
            if(w.width < 2 * 280 || w.height < 2 * 190)
               failures.push("the lobby window too small: " + w.width + " x " + w.height);
            lobby.srp_lobby_window();
            if(w.parent != null)
               failures.push("the lobby window not closed by its toggle");
            lobby.srp_lobby_window();
            var b:Object = button(w);
            if(b != null)
            {
               // its states once it is on the stage (CLIK sets its mouse handlers then)
               setTimeout(this.states,200,b);
               return;
            }
         }
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      // The lobby button's states: it has no timeline (one frame: no state frame places a coin,
      // an amount or a hint again), every state CLIK can ask for has one of the money boxes' four
      // back pieces, and through a press, a selection and disabled it holds only that piece and
      // its label; no tooltip and no data (no hint). (Ruffle has no ExternalInterface, which the
      // game's roll-over and release play their sound through, so those two are not driven here.)
      private function states(b:Object) : void
      {
         var Button:Object = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.lobbypick.LobbyButton");
         var pieces:Array = ["btnBackUp", "btnBackHover", "btnBackDown", "btnBackDisabled"];
         if(b.totalFrames != 1)
            failures.push("the lobby button has " + b.totalFrames + " frames");
         for each(var state:String in Button.states)
         {
            if(pieces.indexOf(Button.backOf(state)) < 0)
               failures.push("the lobby button's state " + state + " shows " + Button.backOf(state));
         }
         for each(var s:String in ["over", "release", "selecting", "down", "disabled"])
         {
            if(Button.backOf(s) != {over:"btnBackHover", release:"btnBackHover", selecting:"btnBackHover",
                  down:"btnBackDown", disabled:"btnBackDisabled"}[s])
               failures.push("the lobby button's state " + s + ": " + Button.backOf(s));
         }
         var steps:Array = [["up", null, "btnBackUp"], ["down", MouseEvent.MOUSE_DOWN, "btnBackDown"],
            ["selected", "select", null], ["disabled", "disable", "btnBackDisabled"], ["enabled", "enable", null]];
         for each(var step:Array in steps)
         {
            if(step[1] == "select")
               b.selected = true;
            else if(step[1] == "disable")
               b.enabled = false;
            else if(step[1] == "enable")
               b.enabled = true;
            else if(step[1] != null)
               b.dispatchEvent(new MouseEvent(step[1], true, false, 1, 1));
            var names:Array = [];
            for(var i:int = 0; i < b.numChildren; i++)
               names.push(getQualifiedClassName(b.getChildAt(i)) + ":" + b.getChildAt(i).name);
            trace("FADE lobby button " + step[0] + ": " + b.shown + " [" + names.join(", ") + "]");
            if(b.numChildren != 2 || getQualifiedClassName(b.getChildAt(0)) != b.shown || pieces.indexOf(b.shown) < 0
               || b.getChildAt(1).name != "lobbyLabel" || (step[2] != null && b.shown != step[2]))
               failures.push("the lobby button " + step[0] + ": " + names.join(", "));
         }
         if(b.tooltipRendererName || b.data != null)
            failures.push("the lobby button has a tooltip or data (" + b.tooltipRendererName + ", " + b.data + ")");
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      // The lobby button: the player panel's (its symbol's class, made here: the lobby's status panel
      // needs the exe), in the premium button's place (none left there). Its label, in every
      // language: the dictionary's text in upper case, an embedded Calibri bold (the top menu's),
      // every glyph drawn (each at least 5 px wide), inside the box with 4 px margins and centred
      // in it both ways to the pixel.
      private function button(w:Object) : Object
      {
         var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
         var Panel:Class = domain.getDefinition("top_fla.topPanelLeftContainer_56") as Class;
         var dictionary:Object = domain.getDefinition("survarium.data.LocalizationData");
         var panel:Sprite = null;
         // a panel a language (the dictionary's text is read as the panel is made)
         for each(var language:String in HarnessText.LANGUAGES.concat(["english"]))
         {
            dictionary.setData(HarnessText.DATA[language]);
            panel = new Panel() as Sprite;
            var b:Object = panel.getChildByName("btn_lobby");
            var label:Object = b != null ? b.getChildByName("lobbyLabel") : null;
            if(panel.getChildByName("btn_premium") != null)
               failures.push("the premium button still in the player panel");
            if(b == null || b != Object(panel).btn_lobby || label == null)
            {
               failures.push("no lobby button with its label in the player panel");
               return null;
            }
            var text:String = HarnessText.DATA[language]["st_srp_lobby_title"];
            var f:Object = label.getTextFormat();
            var dx:Number = label.x + label.width / 2 - b.boxWidth / 2;
            var dy:Number = label.y + label.height / 2 - b.boxHeight / 2;
            trace("FADE lobby button " + language + ": '" + label.text + "' " + int(label.textWidth) + " x " + int(label.textHeight)
               + " in " + int(b.boxWidth) + " x " + int(b.boxHeight) + ", off centre " + dx.toFixed(1) + "," + dy.toFixed(1));
            if(label.text != text.toUpperCase() || !label.embedFonts || f.font != "Calibri" || !f.bold
               || label.textWidth < 5 * text.length || label.width > b.boxWidth - 8
               || Math.abs(dx) > 1 || Math.abs(dy) > 1)
               failures.push("the lobby button's label in " + language + ": '" + label.text + "'");
         }
         // the English one drawn at the top left, as the lobby draws it (window.png)
         panel.x = 10;
         panel.y = 4;
         addChild(panel);
         return panel.getChildByName("btn_lobby");
      }

      private function finish(result:String) : void
      {
         trace("FADE RESULT " + result);
      }
   }
}
