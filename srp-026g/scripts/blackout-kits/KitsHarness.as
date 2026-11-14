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
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;

   // Blackout's Tab view and chat (docs/notes/blackout.md) with the hud movie's own classes: its kit
   // panel (HUDProfileSelector, a CLIK ButtonBar of kit tabs), body doll (HUDPlayerInfo's criticalInd)
   // and chat (SrvGameChat) in a stand-in of the hud's root, shown by the hud's own ForcedView in its
   // own KeptLayer (a layer beside the root, as HUD makes them) while the root is hidden, then:
   // - forced, nothing typed, no Tab: the chat is hidden, a new message too;
   // - the chat typed in (focus_chat): the chat and its input shown; sent: hidden again;
   // - Tab held: the chat shown with its history held on (ForcedView's history), and the kits:
   // - kept, the panel stays where it was on the screen;
   // - a kit key (patch 58's select_kit_key: ProfileSelector.pickByKey) picks kit 2, shown chosen
   //   (its tab selected, the glow under it where Ruffle draws it), the pick going to the exe
   //   (the tabs' index change, onProfileSelected's profile_changed);
   // - a real mouse click (check.sh's xdotool on the Ruffle window) on kit 3's tab picks it;
   // - given back (Tab up), the panel is in the root where it was;
   // - the body's state kept inside HUDPlayerInfo with ammo stays where it
   //   was, shown, and takes the body parts' state (partHP) there;
   // - death and respawn with Tab held switch between the kits and body without another Tab event.
   // - the Tab list (SrvPlayerList with srp's PlayerRowMenu, names only as HUD.showScores makes a forced
   //   match's) kept with either view, where it was, the kits over it: its rows' scores hidden, a
   //   right-click (Scaleform's MouseEventEx) on a player's row opening the row menu there with the chat
   //   mute entries (the exe's srp_player_menu answered as patch 86 does), the Tab map's marked battery
   //   (update_objects' "artifact") drawn in the kept list; given back to the root with Tab up;
   // - the use prompt (SrvContextInfo, root.set_context) kept while forced, where it was, its progress
   //   (frame 2) filled there; given back to the root when the match is no longer forced;
   // Prints "KITS ..." lines (trace) and ends with "KITS RESULT pass" or "KITS RESULT fail: ...".
   public class KitsHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      // the hud's root
      private var face:MovieClip = new MovieClip();

      private var info:MovieClip;

      private var layout:Object;

      private var kept:Object;

      private var panel:MovieClip;

      private var selector:Object;

      private var picks:int = 0;

      private var frame:int = 0;

      private var clicked:Point;

      private var target:Point;

      private var doll:MovieClip;

      private var chat:MovieClip;

      private var context:MovieClip;

      private var view:Object;

      private var list:MovieClip;

      private var menu:Object;

      private var Ex:Class;

      private var menuCalls:Array = [];

      private var history:Array = [];

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
         kept = new Kept(face,function(part:DisplayObject):void
         {
            if(part is Sprite && Sprite(part)["layoutData"] != null)
               layout.manage(Sprite(part));
         });
         addChild(DisplayObject(kept));
         panel = new Panel() as MovieClip;
         panel.x = 60;
         panel.y = 300;
         panel.visible = false;         // alive at first
         face.addChild(panel);
         var Info:Class = domain.getDefinition("HUDPlayerInfo") as Class;
         info = new Info() as MovieClip;
         info.x = 700;
         info.y = 380;
         face.addChild(info);
         doll = info.criticalInd;
         var Data:Class = domain.getDefinition("scaleform.clik.layout.LayoutData") as Class;
         info.layoutData = new Data();
         info.layoutData.alignH = "right";
         info.layoutData.alignV = "bottom";
         info.layoutData.offsetH = -20;
         info.layoutData.offsetV = -51;
         var Layout:Class = domain.getDefinition("scaleform.clik.layout.Layout") as Class;
         layout = new Layout();
         layout.rect = new Rectangle(0,0,1280,720);
         face.addChild(DisplayObject(layout));
         layout.manage(info);
         var Chat:Class = domain.getDefinition("SrvGameChat") as Class;
         chat = new Chat() as MovieClip;
         chat.x = 2000;                  // off the window: the kits are clicked
         chat.y = 0;
         face.addChild(chat);
         var Context:Class = domain.getDefinition("SrvContextInfo") as Class;
         context = new Context() as MovieClip;
         context.x = 500;
         context.y = 40;                 // above the kits: their click lands on them
         face.addChild(context);
         // The Tab list as the hud makes it (HUD: SrvPlayerList, PlayerRowMenu in it), off the window: the
         // kits are clicked; names only, as a forced match's (HUD.showScores).
         var List:Class = domain.getDefinition("SrvPlayerList") as Class;
         list = new List() as MovieClip;
         list.x = 2000;
         list.y = 20;
         list.visible = false;
         face.addChild(list);
         var Menu:Class = domain.getDefinition("survarium.controls.PlayerRowMenu") as Class;
         menu = new Menu(list.players);
         list.addChild(DisplayObject(menu));
         menu.call = function(method:String, place:String, ... choice):void
         {
            menuCalls.push([method,place].concat(choice).join(" "));
            if(method == "srp_player_menu")
            {
               menu.offered(uint(place),true,true,0);
            }
         };
         list.players.namesOnly = true;
         Ex = domain.getDefinition("scaleform.gfx.MouseEventEx") as Class;
         var View:Class = domain.getDefinition("survarium.controls.ForcedView") as Class;
         // The hud's lists (HUD.keptWhenForced, keptOnTab, keptWhileTyping).
         view = new View(kept,function():Array
         {
            return [context];
         },function():Array
         {
            return panel.visible ? [chat,list,panel] : [info,chat,list];
         },function():Array
         {
            return [chat];
         },function(hold:Boolean):void
         {
            history.push(hold);
         });
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
            var contextAt:Point = context.localToGlobal(new Point(0,0));
            face.visible = false;
            view.forced = true;
            expectContext(contextAt);
            message("forced, nothing typed, no Tab: a new message");
            expectChat("forced, nothing typed, no Tab, a new message came",false,false);
            view.typing = true;
            call(function():void
            {
               chat.startEnterMessage();
            });
            expectChat("the chat typed in",true,true);
            call(function():void
            {
               chat.stopEnterMessage();
            });
            view.typing = false;
            expectChat("the message sent",false,false);
            message("a message after it");
            expectChat("a message after it, nothing typed, no Tab",false,false);
            var dollAt:Point = doll.localToGlobal(new Point(0,0));
            var listAt:Point = list.localToGlobal(new Point(0,0));
            list.visible = true;        // HUD.show_player_list(true), then the view's Tab
            view.tab = true;
            expectList(listAt);
            expectChat("Tab held: the chat's history",true,false);
            if(history.join() != "true")
            {
               failures.push("Tab held: the history held " + history.join());
            }
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
            if(info.parent != kept || Point.distance(dollAt,dollNow) > 0.5 || !doll.visible || !hp)
            {
               failures.push("the body's state kept: from " + dollAt + " to " + dollNow + ", parts set " + hp);
            }
            // Reflow repeatedly with the same native bottom-right layout. Both
            // children must retain their owner and remain within the viewport.
            for each(var size:Point in [new Point(1024,768),new Point(2560,1080),new Point(1280,720)])
            {
               layout.rect = new Rectangle(0,0,size.x,size.y);
               layout.reflow();
               kept.place();
               var bounds:Rectangle = doll.getBounds(this);
               if(doll.parent != info || info.ammoInd.parent != info || bounds.right > 20 + size.x * 0.8 + 1 || bounds.bottom > size.y * 0.8 + 1)
                  failures.push("body outside viewport after reflow " + size + ": " + bounds);
            }
            trace("KITS body and ammo stay together and on screen across reflows");
            // show_profiles_selector changes visibility, then refreshes the view. Tab is held
            // throughout these changes: no setter of forced/tab/typing is called again.
            if("refresh" in view)
            {
               panel.visible = true;
               view.refresh();
               if(!shown(panel) || shown(doll) || panel.parent != kept || info.parent == kept)
               {
                  failures.push("death while Tab held: the kits must replace the body");
               }
               if(list.parent != kept || !shown(list) || kept.getChildIndex(panel) < kept.getChildIndex(list))
               {
                  failures.push("death while Tab held: the list kept under the kits");
               }
               panel.visible = false;
               view.refresh();
               if(shown(panel) || !shown(doll) || panel.parent != face || info.parent != kept)
               {
                  failures.push("respawn while Tab held: the body must replace the kits");
               }
               if(list.parent != kept || !shown(list))
               {
                  failures.push("respawn while Tab held: the list still kept");
               }
               panel.visible = true;
               view.refresh();
               trace("KITS death and respawn while Tab held: refreshed without another Tab event");
            }
            else
            {
               failures.push("death/respawn with Tab held: ForcedView has no refresh");
               panel.visible = true;
               view.tab = true;        // let the remaining key/click checks run on an old movie
            }
            var after:Point = panel.localToGlobal(new Point(0,0));
            trace("KITS kept: the panel at " + before + " in the root, " + after + " in the kept layer");
            if(panel.parent != kept || Point.distance(before,after) > 0.5)
            {
               failures.push("kept: the panel from " + before + " to " + after + ", in the layer " + (panel.parent == kept));
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
            list.visible = false;   // HUD.show_player_list(false): the menu closed, the list hidden
            menu.close();
            view.tab = false;
         });
         if(list.parent != face || list.x != 2000 || list.y != 20 || shown(list))
         {
            failures.push("given back: the list at " + list.x + "," + list.y + ", in the root " + (list.parent == face) + ", shown " + shown(list));
         }
         if(history.join() != "true,false")
         {
            failures.push("Tab let go: the history " + history.join());
         }
         expectChat("Tab let go",false,false);
         if(info.parent == kept)
         {
            failures.push("given back: the body's state still kept");
         }
         if(panel.parent != face || panel.x != 60 || panel.y != 300)
         {
            failures.push("given back: the panel at " + panel.x + "," + panel.y + ", in the root " + (panel.parent == face));
         }
         // the match no longer forced (force_interface_hidden(false)): the use prompt back in the root
         call(function():void
         {
            view.forced = false;
         });
         trace("KITS unforced: the use prompt in the root " + (context.parent == face) + " at " + context.x + "," + context.y);
         if(context.parent != face || context.x != 500 || context.y != 40)
         {
            failures.push("unforced: the use prompt at " + context.x + "," + context.y + ", in the root " + (context.parent == face));
         }
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      // Tab held: the list kept where it was, shown, its rows by name only; a right-click on a player's
      // row opens the row menu there; the Tab map draws a marked battery.
      private function expectList(at:Point) : void
      {
         var now:Point = list.localToGlobal(new Point(0,0));
         trace("KITS Tab held: the list at " + at + " in the root, " + now + " kept, shown " + shown(list));
         if(list.parent != kept || !shown(list) || Point.distance(at,now) > 0.5)
         {
            failures.push("Tab held: the list kept " + (list.parent == kept) + ", shown " + shown(list) + ", from " + at + " to " + now);
         }
         try
         {
            var Renderer:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("PlayerListRenderer") as Class;
            var row:Object = new Renderer();
            row.owner = list.players.team2list;     // its list, as the list makes its rows (namesOnly)
            row.setData({id:3,team:1,name:"Wolf",kills:2,deaths:1,rank:40,score:120});
            list.players.team2list.addChild(DisplayObject(row));
            row.validateNow();
            var scores:Boolean = shown(row.ui_kills) || shown(row.ui_deaths) || shown(row.ui_rank) || row.ui_score != null && shown(row.ui_score);
            var event:MouseEvent = new Ex(MouseEvent.CLICK) as MouseEvent;
            Object(event).buttonIdx = 1;
            DisplayObject(row).dispatchEvent(event);
            trace("KITS a right-click on a kept row: the menu " + menu.state + ", shown " + shown(DisplayObject(menu)) + ", asked " + menuCalls.join(",") + "; the row's scores shown " + scores);
            if(menu.state != "open" || !shown(DisplayObject(menu)) || menuCalls.join(",") != "srp_player_menu 3" || scores)
            {
               failures.push("the kept list's row: menu " + menu.state + ", shown " + shown(DisplayObject(menu)) + ", asked " + menuCalls.join(",") + ", scores shown " + scores);
            }
         }
         catch(err:Error)
         {
            failures.push("the kept list's row: " + err.getStackTrace());
         }
         try
         {
            list.minimap.setObjectData([{id:40,type:"artifact",pos_x:10,pos_y:10}]);
            var battery:DisplayObject = find(list.minimap,loader.contentLoaderInfo.applicationDomain.getDefinition("ArtifactArrow") as Class);
            trace("KITS the Tab map's marked battery: drawn " + (battery != null) + ", shown " + (battery != null && shown(battery)));
            if(battery == null || !shown(battery))
            {
               failures.push("the Tab map's marked battery: drawn " + (battery != null));
            }
         }
         catch(err2:Error)
         {
            failures.push("the Tab map's objects: " + err2.getStackTrace());
         }
      }

      private static function find(at:DisplayObject, type:Class) : DisplayObject
      {
         if(at is type)
         {
            return at;
         }
         var container:DisplayObjectContainer = at as DisplayObjectContainer;
         for(var i:int = 0; container != null && i < container.numChildren; i++)
         {
            var found:DisplayObject = find(container.getChildAt(i),type);
            if(found != null)
            {
               return found;
            }
         }
         return null;
      }

      // Whether a part is on the screen: itself and every parent up to the stage visible.
      private static function shown(part:DisplayObject) : Boolean
      {
         for(var at:DisplayObject = part; at != null; at = at.parent)
         {
            if(!at.visible)
            {
               return false;
            }
         }
         return part.stage != null;
      }

      // Forced: the use prompt kept where it was in the root, shown, its text and progress set there
      // (HUD.refresh_context: frame 2 for a progress, ArtifactStatusIndicatorLine.set_progress).
      private function expectContext(at:Point) : void
      {
         var set:Boolean = true;
         try
         {
            context.gotoAndStop(2);
            context.contextText.text = "Searching";
            context.progress_indicator.width = context.contextText.textWidth + 10;
            context.progress_indicator.set_progress(40);
         }
         catch(err:Error)
         {
            set = false;
            trace("KITS the use prompt: " + err.message);
         }
         var now:Point = context.localToGlobal(new Point(0,0));
         trace("KITS forced: the use prompt at " + at + " in the root, " + now + " kept, shown " + shown(context) + ", its progress set " + set);
         if(context.parent != kept || !shown(context) || Point.distance(at,now) > 0.5 || !set)
         {
            failures.push("forced: the use prompt kept " + (context.parent == kept) + ", shown " + shown(context) + ", from " + at + " to " + now + ", progress set " + set);
         }
      }

      private function expectChat(what:String, visible:Boolean, input:Boolean) : void
      {
         var on:Boolean = shown(chat);
         var typing:Boolean = on && chat.text_inpput != null && shown(chat.text_inpput);
         trace("KITS " + what + ": the chat " + (on ? "shown" : "hidden") + ", its input " + (typing ? "shown" : "hidden"));
         if(on != visible || typing != input)
         {
            failures.push(what + ": the chat " + on + " (want " + visible + "), its input " + typing + " (want " + input + ")");
         }
      }

      // A message to the chat (Chat.got_message); Ruffle may not style it, the chat holds it anyway.
      private function message(what:String) : void
      {
         try
         {
            chat.addTab({id:1,name:"all",channels:[1],channel_to_send:1,closable:false});
            chat.got_message({type:1,text:what,tab_id:1,sender:"teammate"});
         }
         catch(err:Error)
         {
            trace("KITS a message: " + err.message);
         }
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.tick);
         trace("KITS RESULT " + result);
         fscommand("quit");
      }
   }
}
