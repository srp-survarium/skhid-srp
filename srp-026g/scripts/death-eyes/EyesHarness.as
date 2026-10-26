package
{
   import flash.display.BlendMode;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.display.StageAlign;
   import flash.display.StageScaleMode;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.getTimer;

   // The eyes closing on the death screen (docs/notes/death-eyes.md) with the hud movie's own class
   // (survarium.controls.DeathEyes) and kit panel (HUDProfileSelector), laid out as HUD lays them:
   // the eyes on the stage, a stand-in of the hud's root with the kits over them.
   // - mode "check" (the default): the countdown as the exe sends it (root.set_respawn_time,
   //   "<prefix> MM:SS" each second, the seconds rounded up, "" at the end) on a made-up clock:
   //   the openness at 0, 25, 50, 75 and 100 % of a 5 s and a 10 s countdown, open again at the
   //   respawn; a time that is not whole seconds; a rollback over the death; a countdown that stops;
   //   the texts; then in real time a 5 s countdown with a real mouse click (check.sh's xdotool) on
   //   kit 3 once the eyes are three quarters shut. Prints "EYES ..." lines, ends with
   //   "EYES RESULT pass" or "EYES RESULT fail: ...".
   // - mode "video": 1 s alive, a 5 s countdown in real time over a backdrop (-Pbackdrop=<png>)
   //   with the kits and the countdown's text, the respawn, 1.5 s alive; check.sh records it. With
   //   -Pblood=under|over|over-normal the hud's blood (BloodEffect) while dead, under the eyes
   //   (HUD's order), over them (multiply, as the game blends it) or over them unblended.
   public class EyesHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var Eyes:Class;

      private var eyes:Sprite;

      // the hud's root
      private var face:Sprite = new Sprite();

      private var panel:MovieClip;

      private var selector:Object;

      private var label:TextField;

      private var blood:MovieClip;

      private var picks:int = 0;

      private var failures:Array = [];

      private var began:int = -1;

      private var seconds:int = 0;

      private var target:Point;

      private var clicked:Point;

      private var mode:String;

      public function EyesHarness()
      {
         stage.scaleMode = StageScaleMode.NO_SCALE;
         stage.align = StageAlign.TOP_LEFT;
         mode = loaderInfo.parameters.mode || "check";
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
            trace("EYES mouse down at " + int(e.stageX) + "," + int(e.stageY));
         });
      }

      private static function text(seconds:int) : String
      {
         return seconds <= 0 ? "" : "Respawn in " + (seconds < 600 ? "0" : "") + int(seconds / 60) + ":" + (seconds % 60 < 10 ? "0" : "") + seconds % 60;
      }

      private function fail(what:String) : void
      {
         failures.push(what);
         trace("EYES FAIL " + what);
      }

      private function near(a:Number, b:Number, by:Number) : Boolean
      {
         return Math.abs(a - b) <= by;
      }

      private function loaded(e:Event) : void
      {
         domain = loader.contentLoaderInfo.applicationDomain;
         Eyes = domain.getDefinition("survarium.controls.DeathEyes") as Class;
         var Panel:Class = domain.getDefinition("HUDProfileSelector") as Class;
         if(mode == "video" && loaderInfo.parameters.backdrop)
         {
            var back:Loader = new Loader();
            back.load(new URLRequest(loaderInfo.parameters.backdrop));
            addChild(back);
         }
         // as HUD: the eyes on the stage below the root, in the stage's pixels
         eyes = new Eyes() as Sprite;
         addChild(eyes);
         Object(eyes).place(stage.stageWidth,stage.stageHeight);
         // the video's blood (-Pblood=under|over|over-normal): the hud's own BloodEffect as add_effect
         // makes it (multiply), placed as placeEffects does, under the eyes (HUD's order) or over them
         var where:String = loaderInfo.parameters.blood;
         if(mode == "video" && where)
         {
            var Blood:Class = domain.getDefinition("BloodEffect") as Class;
            blood = new Blood() as MovieClip;
            blood.gotoAndStop(0);
            blood.blendMode = where == "over-normal" ? BlendMode.NORMAL : BlendMode.MULTIPLY;
            blood.scaleX = stage.stageWidth / 1280;
            blood.scaleY = stage.stageHeight / 720;
            blood.visible = false;
            addChildAt(blood,where == "under" ? getChildIndex(eyes) : getChildIndex(eyes) + 1);
            try
            {
               Object(blood).update(0.8);
            }
            catch(err:Error)
            {
               trace("EYES blood update threw " + err);
            }
         }
         face.scaleX = face.scaleY = stage.stageHeight / 1080;
         addChild(face);
         panel = new Panel() as MovieClip;
         panel.x = int(1920 - 594) / 2;
         panel.y = 1080 - 120;
         face.addChild(panel);
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
         selector.tabPage.addEventListener("clikIndexChange",function(e:Event):void
         {
            picks++;
         },false,1);
         try
         {
            selector.selectProfile(0);
         }
         catch(err:Error)
         {
         }
         label = new TextField();
         label.defaultTextFormat = new TextFormat("_sans",30,0xE8E8E8,true);
         label.autoSize = TextFieldAutoSize.CENTER;
         label.x = 1920 / 2;
         label.y = 90;
         face.addChild(label);
         if(mode == "video")
         {
            panel.visible = false;
            addEventListener(Event.ENTER_FRAME,this.video);
            return;
         }
         panel.visible = true;
         this.checkClock();
         addEventListener(Event.ENTER_FRAME,this.live);
      }

      // A countdown of `total` ms as the exe sends it, until `at` ms: the text at the start, at each
      // whole second left, "" at the end (left out with `before`: the moment just before it).
      private function run(e:Object, total:int, at:int, before:Boolean = false) : void
      {
         var left:int;
         var now:int = 0;
         var shown:int = -1;
         while(now <= at)
         {
            left = Math.max(0,Math.ceil((total - now) / 1000));
            if(left == 0 && before)
            {
               break;
            }
            if(left != shown)
            {
               e.countdown(text(left),now);
               shown = left;
            }
            // the next time the rounded-up seconds change
            if(left == 0)
            {
               break;
            }
            now = total - (left - 1) * 1000;
         }
         e.update(at);
      }

      private function checkClock() : void
      {
         var shares:Array = [0,25,50,75,100];
         var by:Object = {};
         var e:Object;
         var total:int;
         var share:int;
         var open:Number;
         var line:String;
         var at:int;
         for each(total in [5000,10000])
         {
            line = "EYES " + total / 1000 + " s countdown, openness at";
            by[total] = [];
            for each(share in shares)
            {
               e = new Eyes();
               e.place(800,480);
               at = total * share / 100;
               run(e,total,at,true);
               open = e.openness;
               by[total].push(open);
               line += " " + share + "% " + open.toFixed(3);
               if(!near(open,Object(Eyes).opennessAt(share / 100),0.002))
               {
                  fail(total + " ms at " + share + "%: " + open + ", the curve says " + Object(Eyes).opennessAt(share / 100));
               }
               if(share > 0 && share < 100 && !e.visible)
               {
                  fail(total + " ms at " + share + "%: not shown");
               }
            }
            trace(line);
            if(by[total][0] != 1 || by[total][4] != 0)
            {
               fail(total + " ms: not open at the start or not shut at the end");
            }
            for(var i:int = 1; i < shares.length; i++)
            {
               if(by[total][i] >= by[total][i - 1])
               {
                  fail(total + " ms: not closing from " + shares[i - 1] + "% to " + shares[i] + "%");
               }
            }
            // the respawn: "" at the end
            e = new Eyes();
            e.place(800,480);
            run(e,total,total);
            trace("EYES " + total / 1000 + " s countdown, at the respawn: openness " + e.openness + ", shown " + e.visible);
            if(e.openness != 1 || e.visible)
            {
               fail(total + " ms: not open at the respawn");
            }
         }
         for(i = 0; i < shares.length; i++)
         {
            if(!near(by[5000][i],by[10000][i],0.002))
            {
               fail("5 s and 10 s differ at " + shares[i] + "%");
            }
         }
         // a time that is not whole seconds: 8.65 s shows 00:09 first, 00:08 at 0.65 s
         e = new Eyes();
         run(e,8650,4325);
         trace("EYES 8.65 s countdown: at half " + Number(e.openness).toFixed(3) + ", progress " + Number(e.progress).toFixed(3));
         if(!near(e.progress,0.5,0.01))
         {
            fail("8.65 s at 4.325 s: progress " + e.progress);
         }
         e = new Eyes();
         run(e,8650,8650,true);
         if(e.openness != 0)
         {
            fail("8.65 s: not shut at its end (" + e.openness + ")");
         }
         // a rollback over the death: the countdown gone at 6 s of 10 (the exe's "")
         e = new Eyes();
         run(e,10000,6000);
         var before:Number = e.openness;
         e.countdown("",6010);
         trace("EYES rollback at 6 s of 10: openness " + before.toFixed(3) + " -> " + e.openness + ", shown " + e.visible);
         if(!(before < 0.6) || e.openness != 1 || e.visible)
         {
            fail("rollback: openness " + before + " -> " + e.openness);
         }
         // the countdown stops at 00:06 (4 s of 10): the eyes wait for it
         e = new Eyes();
         run(e,10000,4000);
         e.update(9000);
         trace("EYES countdown stopped at 4 s of 10: progress at 9 s " + Number(e.progress).toFixed(3));
         if(!near(e.progress,0.51,0.001))
         {
            fail("stopped countdown: progress " + e.progress + " at 9 s");
         }
         // the texts
         var texts:Object = {"Respawn in 00:05":5,"Відродження через 01:05":65,"Respawn in 10:00":600,"":0,"Respawn":0,"00:00":0,"Respawn in :05":0,"Respawn in 00:":0,"00:05 x":0};
         for(var t:String in texts)
         {
            if(Object(Eyes).secondsOf(t) != texts[t])
            {
               fail("the text '" + t + "' is " + Object(Eyes).secondsOf(t) + " s, not " + texts[t]);
            }
         }
         // never in the way of a click
         if(eyes.mouseEnabled || eyes.mouseChildren || getChildIndex(eyes) > getChildIndex(face))
         {
            fail("the eyes take clicks or are over the root");
         }
      }

      // In real time: a 5 s countdown; at three quarters a real click on kit 3.
      private function live(e:Event) : void
      {
         var now:int = getTimer();
         var left:int;
         if(began < 0)
         {
            began = now;
         }
         left = Math.max(0,Math.ceil((5000 - (now - began)) / 1000));
         if(left != seconds)
         {
            seconds = left;
            Object(eyes).countdown(text(left));
            label.text = text(left);
         }
         if(target == null && Object(eyes).progress >= 0.75)
         {
            var tab:DisplayObject = selector.tabPage.getButtonAt(2) as DisplayObject;
            target = tab.localToGlobal(new Point(tab.width / tab.scaleX / 2,tab.height / tab.scaleY / 2));
            trace("EYES openness " + Number(Object(eyes).openness).toFixed(3) + ", target kit 3 at " + int(target.x) + "," + int(target.y));
            picks = 0;  // the setup's own selection of kit 1 is no pick
         }
         if(target != null && clicked != null && Point.distance(clicked,target) < 3)
         {
            trace("EYES click on kit 3 with the eyes at " + Number(Object(eyes).openness).toFixed(3) + ": selected " + selector.tabPage.selectedIndex + ", picks " + picks);
            if(selector.tabPage.selectedIndex != 2 || picks != 1)
            {
               fail("the click on kit 3 over the eyes picked nothing");
            }
            target = null;
            clicked = null;
            began = int.MAX_VALUE;
         }
         if(began == int.MAX_VALUE)
         {
            finish(failures.length ? "fail: " + failures.join("; ") : "pass");
         }
         else if(now - began > 20000)
         {
            fail("no click on kit 3 came");
            finish("fail: " + failures.join("; "));
         }
      }

      // The video: 1 s alive, 5 s dead, then the respawn and 1.5 s alive.
      private function video(e:Event) : void
      {
         var now:int = getTimer();
         var left:int;
         if(began < 0)
         {
            began = now + 1000;
            trace("EYES video start");
         }
         var t:int = now - began;
         left = t < 0 ? 0 : Math.max(0,Math.ceil((5000 - t) / 1000));
         if(left != seconds)
         {
            seconds = left;
            Object(eyes).countdown(text(left));
            label.text = text(left);
         }
         panel.visible = t >= 0 && t < 5000;
         if(blood != null)
         {
            blood.visible = panel.visible;
         }
         if(t > 7000)
         {
            finish("pass");
         }
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.live);
         removeEventListener(Event.ENTER_FRAME,this.video);
         trace("EYES RESULT " + result);
         fscommand("quit");
      }
   }
}
