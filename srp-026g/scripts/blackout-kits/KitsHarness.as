package
{
   import flash.display.DisplayObject;
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

   // Blackout's kits on Tab (docs/notes/blackout.md) with the hud movie's own classes: its kit panel
   // (HUDProfileSelector, a CLIK ButtonBar of kit tabs) in a stand-in of the hud's root, kept by the
   // hud's own KeptLayer (a layer beside the root, as HUD makes it) while the root is hidden, then:
   // - kept, the panel stays where it was on the screen;
   // - a kit key (patch 58's select_kit_key: ProfileSelector.pickByKey) picks kit 2, shown chosen
   //   (its tab selected, the glow under it where Ruffle draws it), the pick going to the exe
   //   (the tabs' index change, onProfileSelected's profile_changed);
   // - a real mouse click (check.sh's xdotool on the Ruffle window) on kit 3's tab picks it;
   // - given back (Tab up), the panel is in the root where it was;
   // - the body's state (HUDPlayerInfo's criticalInd, alive Tab's doll) kept the same way stays where it
   //   was, shown, and takes the body parts' state (partHP) there.
   // Prints "KITS ..." lines (trace) and ends with "KITS RESULT pass" or "KITS RESULT fail: ...".
   public class KitsHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      // the hud's root
      private var face:Sprite = new Sprite();

      private var kept:Object;

      private var panel:MovieClip;

      private var selector:Object;

      private var picks:int = 0;

      private var frame:int = 0;

      private var clicked:Point;

      private var target:Point;

      private var doll:MovieClip;

      private var failures:Array = [];

      public function KitsHarness()
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
            trace("KITS mouse down at " + int(e.stageX) + "," + int(e.stageY));
         });
      }

      private function loaded(e:Event) : void
      {
         var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
         var Panel:Class = domain.getDefinition("HUDProfileSelector") as Class;
         var Kept:Class = domain.getDefinition("survarium.controls.KeptLayer") as Class;
         // the root at an interface scale, the layer over it (HUD: on the stage after its root)
         face.scaleX = face.scaleY = 0.8;
         face.x = 20;
         addChild(face);
         kept = new Kept(face);
         addChild(DisplayObject(kept));
         panel = new Panel() as MovieClip;
         panel.x = 60;
         panel.y = 300;
         face.addChild(panel);
         var Info:Class = domain.getDefinition("HUDPlayerInfo") as Class;
         var info:MovieClip = new Info() as MovieClip;
         info.x = 700;
         info.y = 380;
         face.addChild(info);
         doll = info.criticalInd;
         selector = panel.profileSelector;
         // Ruffle: the selector's own setting of its tabs' renderer (in its constructor, the movie
         // still loading) finds no class by its name; once loaded it does
         selector.tabPage.itemRendererName = "SrvPlayerProfileTabHUD";
         var kits:Array = [];
         for(var i:int = 0; i < 3; i++)
         {
            kits.push({name:"kit " + (i + 1),state:0,level:5,enabled:true});
         }
         selector.setupProfiles(kits);
         // a pick: the tabs' index change that ProfileSelector.onProfileSelected takes to the exe
         // (profile_changed; Ruffle has no ExternalInterface, so it is counted before, by priority)
         selector.tabPage.addEventListener("clikIndexChange",function(e:Event):void
         {
            picks++;
         },false,1);
         call(function():void
         {
            selector.selectProfile(0);
         });
         addEventListener(Event.ENTER_FRAME,this.tick);
      }

      // Ruffle cannot place some of the movie's timeline children (PlaceObject3 with filters): the
      // selector's glow may be missing there and its code throw; the game's selection is not.
      private function call(fn:Function) : void
      {
         try
         {
            fn();
         }
         catch(err:Error)
         {
            if(selector.selectedTabGlow != null)
            {
               failures.push(String(err));
            }
         }
      }

      // The kit shown chosen: only its tab selected, and the glow under it where Ruffle has it.
      private function chosen(index:int) : Boolean
      {
         var tabs:Object = selector.tabPage;
         for(var i:int = 0; i < 3; i++)
         {
            if(tabs.getButtonAt(i).selected != (i == index))
            {
               return false;
            }
         }
         var glow:Object = selector.selectedTabGlow;
         return glow == null || glow.visible && glow.x == tabs.x + index * tabs.buttonWidth - 51;
      }

      private function tick(e:Event) : void
      {
         frame++;
         if(frame == 5)
         {
            picks = 0;  // the setup's own selection of kit 1 is no pick
            var before:Point = panel.localToGlobal(new Point(0,0));
            face.visible = false;
            kept.keep([panel],true);
            var after:Point = panel.localToGlobal(new Point(0,0));
            trace("KITS kept: the panel at " + before + " in the root, " + after + " in the kept layer");
            if(panel.parent != kept || Point.distance(before,after) > 0.5)
            {
               failures.push("kept: the panel from " + before + " to " + after + ", in the layer " + (panel.parent == kept));
            }
            var dollAt:Point = doll.localToGlobal(new Point(0,0));
            kept.keep([doll],true);
            var dollNow:Point = doll.localToGlobal(new Point(0,0));
            var hp:Boolean = true;
            try
            {
               doll.partHP(1,0.2,0.5);
            }
            catch(err:Error)
            {
               hp = false;
            }
            trace("KITS the body's state kept: at " + dollAt + " in the root, " + dollNow + " kept, shown " + doll.visible + ", its parts set " + hp);
            if(doll.parent != kept || Point.distance(dollAt,dollNow) > 0.5 || !doll.visible || !hp)
            {
               failures.push("the body's state kept: from " + dollAt + " to " + dollNow + ", parts set " + hp);
            }
            call(function():void
            {
               selector.pickByKey(1);
            });
            trace("KITS key 2: selected " + selector.tabPage.selectedIndex + ", picks " + picks + ", shown chosen " + chosen(1));
            if(selector.tabPage.selectedIndex != 1 || picks != 1 || !chosen(1))
            {
               failures.push("key 2 with the panel kept: selected " + selector.tabPage.selectedIndex + ", picks " + picks);
            }
            var tab:DisplayObject = selector.tabPage.getButtonAt(2) as DisplayObject;
            target = tab.localToGlobal(new Point(tab.width / tab.scaleX / 2,tab.height / tab.scaleY / 2));
            trace("KITS target kit 3 at " + int(target.x) + "," + int(target.y));
         }
         else if(frame > 5 && frame < 600 && clicked != null && Point.distance(clicked,target) < 3)
         {
            if(selector.tabPage.selectedIndex == 2)
            {
               trace("KITS click on kit 3: selected 2, picks " + picks + ", shown chosen " + chosen(2));
               if(picks != 2 || !chosen(2))
               {
                  failures.push("click on kit 3: picks " + picks + ", shown chosen " + chosen(2));
               }
               back();
            }
         }
         else if(frame == 600)
         {
            failures.push("the click on kit 3 picked nothing (selected " + selector.tabPage.selectedIndex + ")");
            back();
         }
      }

      // Tab up: the hud's layer gives the panel back.
      private function back() : void
      {
         call(function():void
         {
            kept.keep([panel,doll],false);
         });
         if(doll.parent == kept)
         {
            failures.push("given back: the body's state still kept");
         }
         if(panel.parent != face || panel.x != 60 || panel.y != 300)
         {
            failures.push("given back: the panel at " + panel.x + "," + panel.y + ", in the root " + (panel.parent == face));
         }
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.tick);
         trace("KITS RESULT " + result);
         fscommand("quit");
      }
   }
}
