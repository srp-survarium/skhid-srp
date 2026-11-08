package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.system.ApplicationDomain;

   // The parts of the lobby the Watch window lives in, made from the lobby movie as the game lays
   // them out (the harness, ReplayButtonsHarness, and the screenshots, WatchShots): a holder in the
   // lobby's design units (1920 x 1080, scaled to the stage as LobbyMenu.onStageResize scales the
   // lobby), the top menu's tabs (SrvTopMenu, where the top bar centres them; the bar itself,
   // SrvTopMenuContainer, takes its texts in a way Ruffle refuses), the History
   // (SrvNotificationListSC, the replay prompt's home), the bottom bar (SrvBottomPanel), which makes
   // the rooms window and the Watch window, and the rooms' pool that names the maps (WatchFake.POOL).
   public class WatchLobby
   {
      public var domain:ApplicationDomain;

      public var holder:MovieClip;

      public var top:*;

      public var panel:*;

      public var history:*;

      public var W:*;

      public function WatchLobby(domain:ApplicationDomain, parent:Sprite, data:Object)
      {
         this.domain = domain;
         var stage:* = parent.stage;
         var k:Number = stage.stageWidth / stage.stageHeight < 1.85 ? stage.stageWidth / 1920 : stage.stageHeight / 1080;
         holder = new MovieClip();
         holder.scaleX = holder.scaleY = k;
         parent.addChild(holder);
         // the lobby's windows the top menu's tabs open (TopMenu.configUI)
         for each(var name:String in ["shop_list","charWindow","inventory_list","ranking","clanWindow","questWindow"])
         {
            holder[name] = new MovieClip();
         }
         var Scene:* = domain.getDefinition("MainScene");
         Scene.stage = holder;
         Scene.scaleFactor = k;
         Scene.layout = make("scaleform.clik.layout.Layout");
         history = make("SrvNotificationListSC");
         history.registerRenderer(domain.getDefinition("survarium.constants.NotificationType").MATCH,domain.getDefinition("NotificationMatchRenderer"));
         history.x = 1920 - 440;
         history.y = 300;
         holder.addChild(history);
         // as the top bar holds it (StatusPanel: its parent's parent is the lobby)
         var bar:Sprite = new Sprite();
         holder.addChild(bar);
         top = make("SrvTopMenu");
         top.questButton = make("SrvButtonSkinned");
         top.x = int((1920 - 990) / 2);
         top.y = 6;
         bar.addChild(top);
         top.tabPage.buttonWidth = 125;
         top.tabPage.width = 990;
         panel = make("SrvBottomPanel");
         holder.addChild(panel);
         panel.y = Scene.stageHeight - panel.height + 13;
         W = domain.getDefinition("survarium.watch.WatchWindow");
         if(W.me == null)
         {
            W.me = new W(panel);
            holder.addChild(W.me);
         }
         W.topMenu = top;
         W.me.layout();
         var Rooms:* = domain.getDefinition("survarium.rooms.RoomsWindow");
         if(Rooms.me == null)
         {
            Rooms.me = new Rooms(panel);
            holder.addChild(Rooms.me);
         }
         Rooms.topMenu = top;
         Rooms.take(WatchFake.rows(WatchFake.POOL,WatchFake.POOL_START));
         words.setData(data);
         top.tabPage.validateNow();
      }

      public function get words() : *
      {
         return domain.getDefinition("survarium.data.LocalizationData");
      }

      public function get win() : *
      {
         return W.me;
      }

      public function make(name:String) : *
      {
         return new (domain.getDefinition(name) as Class)();
      }

      // The rows taken by the Watch window (WatchWindow.take).
      public function take(names:Array) : Boolean
      {
         return W.take(WatchFake.rows(names));
      }

      // A History row of `match` (the results' and the History's Watch ask through it).
      public function historyRow(match:uint, map:String, result:String, won:int) : DisplayObjectContainer
      {
         var types:* = domain.getDefinition("survarium.constants.NotificationType");
         history.addNotification({"notificationType":types.MATCH,"label":"","match_id":match,"map_name":map,
            "time":"19:05","result":result,"match_result":won,"reward":null});
         var rows:Array = [];
         WatchLobby.collect(history,"srpReplayWatch",rows);
         return rows.length > 0 ? rows[rows.length - 1].parent : null;
      }

      // The server's answer to a watch of `match`.
      public function answer(status:String, match:uint, n:int = 0) : void
      {
         var hex:String = match.toString(16);
         while(hex.length < 8)
         {
            hex = "0" + hex;
         }
         domain.getDefinition("survarium.controls.NotificationListScrollContent").replayTake(
            [{"nickname":"R1;" + status + ";" + hex + ";" + n,"place":0,"ranking":0,"ranking_diff":0}]);
      }

      public static function find(box:DisplayObjectContainer, name:String) : DisplayObject
      {
         for(var i:int = 0; i < box.numChildren; i++)
         {
            var child:DisplayObject = box.getChildAt(i);
            if(child.name == name)
            {
               return child;
            }
            if(child is DisplayObjectContainer)
            {
               var deeper:DisplayObject = find(child as DisplayObjectContainer,name);
               if(deeper != null)
               {
                  return deeper;
               }
            }
         }
         return null;
      }

      public static function collect(box:DisplayObjectContainer, name:String, into:Array) : void
      {
         for(var i:int = 0; i < box.numChildren; i++)
         {
            var child:DisplayObject = box.getChildAt(i);
            if(child.name == name)
            {
               into.push(child);
            }
            if(child is DisplayObjectContainer)
            {
               collect(child as DisplayObjectContainer,name,into);
            }
         }
      }

      // The texts of `language` with the stock ones a harness lays out (HarnessText.STOCK).
      public static function texts(language:String) : Object
      {
         var all:Object = {};
         var key:String;
         for(key in HarnessText.DATA[language])
         {
            all[key] = HarnessText.DATA[language][key];
         }
         for(key in HarnessText.STOCK[language])
         {
            all[key] = HarnessText.STOCK[language][key];
         }
         return all;
      }
   }
}
