package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.display.StageAlign;
   import flash.display.StageScaleMode;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.text.TextFieldType;

   // A private room's window as the lobby draws it, for a screenshot (shots.sh): the lobby
   // movie's own RoomsWindow over its bottom bar, on a stage of the game's size (the movie's
   // MainScene.stage is this harness), told one room state through RoomsWindow.take.
   // Parameters: movie, members (1 to 20), owner (1: the asker owns the room), teams (a digit a
   // member, its team), phase (0 waiting, 2 in match), bots (the room's bots a team), create
   // ("open" or "private": New room's form instead, from no room, a PIN typed for "private").
   // Traces "SHOT ready" once drawn.
   public class RoomsShots extends Sprite
   {
      private static const STATE:int = 2060000001;

      private static const NAMES:Array = ["PocketTest1","Bob","Сергій_Коваленко","Dave","xX_Very_Long_Nickname_Here_Xx",
         "Erin","Frank","Grace","Heidi","Ivan","Judy","Kim","Leo","Mallory","Niaj","Olivia","Peggy","Rupert",
         "Sybil","Trent"];

      private var loader:Loader = new Loader();

      public function RoomsShots()
      {
         // The stage as the game's: its whole size, from the top left.
         stage.align = StageAlign.TOP_LEFT;
         stage.scaleMode = StageScaleMode.NO_SCALE;
         var movie:String = loaderInfo.parameters.movie || "lobby_menu.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
      }

      private function loaded(e:Event) : void
      {
         var p:Object = loaderInfo.parameters;
         var count:int = int(p.members || 1);
         var owner:Boolean = p.owner != "0";
         var phase:int = int(p.phase || 0);
         var teams:String = p.teams || "";
         try
         {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            var Scene:* = domain.getDefinition("MainScene");
            Scene.stage = this;
            var Win:* = domain.getDefinition("survarium.rooms.RoomsWindow");
            var panel:* = new (domain.getDefinition("SrvBottomPanel") as Class)();
            panel.y = stage.stageHeight - panel.height + 13;
            addChild(panel);
            var win:* = new Win(panel);
            Win.me = win;
            addChild(win);
            var head:String = ["S1","R",4242,7,0,10,30,phase,0,owner ? 1 : 0,24,count,99,1,0,1,0,int(p.bots || 0)].join(";");
            var names:Array = [head,"C;0;61;1;0","C;1;63;0;0","C;2;117;0;0"];
            for(var i:int = 0; i < count; i++)
            {
               // P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>
               var me:Boolean = owner ? i == 0 : i == 1;
               names.push(["P",i,4 + i % 9,i == 0 ? 1 : 0,me ? 1 : 0,phase == 2 ? 1 : 0,i == 3 ? 1 : 0,i == 6 ? 1 : 0,NAMES[i]].join(";"));
               if(teams.length <= i)
               {
                  teams += String(i % 2);
               }
            }
            names.push("A;" + teams);
            names.push("L;0");
            var rows:Array = [];
            for(var r:int = 0; r < names.length; r++)
            {
               rows.push({
                  "place":STATE + r,
                  "nickname":names[r]
               });
            }
            if(p.create == "open" || p.create == "private")
            {
               rows = [{
                  "place":STATE,
                  "nickname":"S1;N;0;0;0"
               }];
               // A pool of a map the bots play, the same map in Research and a map without their
               // data, and a form of `bots` a team: with bots the last two greyed out.
               var pool:Array = ["S1;M;3;0","0;61;1;1;2;0;1","1;71;1;1;5;0;0","2;63;2;1;2;0;0"];
               var poolRows:Array = [];
               for(var q:int = 0; q < pool.length; q++)
               {
                  poolRows.push({
                     "place":STATE + 9 + q,
                     "nickname":pool[q]
                  });
               }
               Win.take(poolRows);
               Win.take([{
                  "place":STATE + 8,
                  "nickname":"S1;F;1;8;1;0;30;;" + int(p.bots || 0)
               },{
                  "place":STATE + 9,
                  "nickname":"E;0;7"
               }]);
            }
            Win.take(rows);
            win.layout();
            if(p.create == "open" || p.create == "private")
            {
               var press:Class = domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
               // In no room the window shows when it is opened (the top menu's tab).
               win.show(true);
               find(win,function(at:DisplayObject):Boolean
               {
                  try
                  {
                     return Object(at)["label"] == "New room";
                  }
                  catch(err:Error)
                  {
                  }
                  return false;
               }).dispatchEvent(new press(press.CLICK));
               if(p.create == "private")
               {
                  var pin:TextField = TextField(find(win,function(at:DisplayObject):Boolean
                  {
                     return at is TextField && TextField(at).type == TextFieldType.INPUT;
                  }));
                  pin.text = "1234";
                  pin.dispatchEvent(new Event(Event.CHANGE));
               }
            }
         }
         catch(err:Error)
         {
            trace("SHOT fail: " + err);
            return;
         }
         var frames:int = 0;
         addEventListener(Event.ENTER_FRAME,function wait(f:Event):void
         {
            if(++frames == 20)
            {
               removeEventListener(Event.ENTER_FRAME,wait);
               trace("SHOT ready " + stage.stageWidth + "x" + stage.stageHeight + " window scale " + win.scaleX);
            }
         });
      }

      // The first shown object under `at` that `which` takes.
      private static function find(at:DisplayObject, which:Function) : DisplayObject
      {
         if(!at.visible)
         {
            return null;
         }
         if(which(at))
         {
            return at;
         }
         var box:DisplayObjectContainer = at as DisplayObjectContainer;
         for(var i:int = 0; box != null && i < box.numChildren; i++)
         {
            var found:DisplayObject = find(box.getChildAt(i),which);
            if(found != null)
            {
               return found;
            }
         }
         return null;
      }
   }
}
