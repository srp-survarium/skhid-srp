package survarium.controls
{
   import flash.display.Sprite;
   import flash.events.Event;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import survarium.buttons.ButtonFriends;
   import survarium.rooms.RoomsWindow;
   import survarium.watch.WatchWindow;
   import survarium.constants.ChatTab;
   
   public class BottomPanel extends UIComponent
   {
      
      public var tabsBar:ButtonBarBottom;
      
      public var btnFriends:ButtonFriends;
      
      public var btn1:Button;
      
      public var btn2:Button;
      
      public var btn3:Button;
      
      public var btnShowLogList:Button;
      
      public var background:Sprite;
      
      public var barInvites:ButtonBarBottom;
      
      public var debugLine:Sprite;
      
      protected var _tabs:Array = [];
      
      protected var _invites:Array = [];

      public function BottomPanel()
      {
         super();
         this.debugLine.visible = false;
      }
      
      public function addIvite(data:Object) : void
      {
         this._invites.push({
            "id":data.id,
            "type":data.type,
            "name":data.name,
            "text":data.text
         });
         this.barInvites.dataProvider = new DataProvider(this._invites);
         this.barInvites.validateNow();
         this.updateInvitesPos();
      }
      
      public function removeInvite(id:uint) : void
      {
         var j:int = 0;
         while(j < this._invites.length)
         {
            if(this._invites[j].id == id)
            {
               this._invites.splice(j,1);
               break;
            }
            j++;
         }
         this.barInvites.dataProvider = new DataProvider(this._invites);
         this.barInvites.validateNow();
         this.updateInvitesPos();
      }
      
      public function hasTab(id:uint) : Boolean
      {
         if(!this._tabs)
         {
            return false;
         }
         var j:int = 0;
         while(j < this._tabs.length)
         {
            if(this._tabs[j].id == id)
            {
               return true;
            }
            j++;
         }
         return false;
      }
      
      public function addTab(data:Object) : void
      {
         var l:int = 0;
         var i:uint = 0;
         var oldSelected:int = -1;
         if(this.hasTab(data.id))
         {
            return;
         }
         if(this.tabsBar.selectedIndex != -1)
         {
            oldSelected = int(this._tabs[this.tabsBar.selectedIndex].id);
         }
         this._tabs.push({
            "id":data.id,
            "channels":data.channels,
            "name":data.name,
            "icon":data.channel_to_send,
            "closable":data.closable,
            "channel_to_send":data.channel_to_send,
            "index":data.id + 1,
            "bool_save_history":data.save_history,
            "tabs_group_id":data.tabs_group_id
         });
         this._tabs.sortOn("id",Array.NUMERIC);
         this.roomChatAfterGeneral();
         this.tabsBar.dataProvider = new DataProvider(this._tabs);
         this.tabsBar.dataProvider.invalidate();
         if(oldSelected != -1)
         {
            l = int(this._tabs.length);
            i = 0;
            while(i < l)
            {
               if(this._tabs[i].id == oldSelected)
               {
                  this.tabsBar.selectedIndex = i;
                  break;
               }
               i++;
            }
         }
      }
      
      public function removeTab(id:uint) : void
      {
         var j:int = 0;
         while(j < this._tabs.length)
         {
            if(this._tabs[j].id == id)
            {
               this._tabs.splice(j,1);
               break;
            }
            j++;
         }
         this.tabsBar.dataProvider = new DataProvider(this._tabs);
      }
      
      override protected function preInitialize() : void
      {
         constraints = new Constraints(this,ConstrainMode.REFLOW);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.tabsBar.setMaxFit(true);
         this.barInvites.setMaxFit(true);
         this.tabsBar.y = 1;
         constraints.addElement("btnFriends",this.btnFriends,Constraints.LEFT);
         constraints.addElement("btnShowLogList",this.btnShowLogList,Constraints.RIGHT);
         this.btnFriends.toggle = true;
         stage.addEventListener(Event.RESIZE,this.onStageResize,false,-1);
         this.btnFriends.addEventListener(Event.RESIZE,this.onStageResize,false,-1);
         this.tabsBar.addEventListener(Event.RESIZE,this.resizeTab);
         this.barInvites.width = 400;
         addChild(this.tabsBar);
         this.barInvites.addEventListener(ButtonEvent.CLICK,this.onInviteItemClick);
         this.makeWindows();
      }
      
      protected function onInviteItemClick(e:Event) : void
      {
         var acceptable:Boolean = !PlayerInfo.isInSquad;
         if(!e.target || !e.target.data)
         {
            return;
         }
         e.target.data.acceptable = acceptable;
         Message.showMessage("invite",e.target.data,true);
      }
      
      protected function updateInvitesPos() : void
      {
         this.barInvites.x = this.background.width - int(this.barInvites.container.width / this.barInvites.container.scaleX) - this.btnShowLogList.width - 5;
         this.resizeTab();
      }
      
      protected function resizeTab(event:Event = null) : void
      {
         var room:Number = NaN;
         this.background.x = 0;
         this.background.width = this.btnShowLogList.x + 194;
         this.debugLine.x = this.background.width * 0.75;
         this.barInvites.setMaxSize(this.background.width / 3.2);
         room = this.barInvites.x - this.tabsBar.x;
         this.tabsBar.setMaxSize(room);
         this.tabsBar.invalidate(InvalidationType.RENDERERS);
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.layout();
         }
         if(WatchWindow.me != null)
         {
            WatchWindow.me.layout();
         }
      }
      
      protected function onStageResize(event:Event = null) : void
      {
         var scale:Number = MainScene.scaleFactor;
         setActualSize(stage.stageWidth / scale,height);
         setSize(stage.stageWidth / scale,height);
         constraints.update(stage.stageWidth / scale,_height);
         invalidate(InvalidationType.SETTINGS);
         this.tabsBar.x = this.btnFriends.x + 154;
         this.resizeTab();
         this.y = MainScene.stageHeight - height + 13;
         if(WatchWindow.me != null)
         {
            WatchWindow.me.layout();
         }
      }
      
      // The rooms window and the Watch window (srp's, in the lobby over this bar; the top menu
      // opens them), made with it.
      protected function makeWindows() : void
      {
         this.makeRoomsWindow();
         try
         {
            if(WatchWindow.me == null)
            {
               WatchWindow.me = new WatchWindow(this);
            }
         }
         catch(err:Error)
         {
         }
      }

      // The rooms window, made once: in the lobby, over this bar (survarium.rooms).
      private function makeRoomsWindow() : void
      {
         try
         {
            if(RoomsWindow.me == null)
            {
               RoomsWindow.me = new RoomsWindow(this);
            }
         }
         catch(err:Error)
         {
         }
      }

      // The tabs sorted by id: a room's chat goes right after the general
      // chat's (the owner's of 2026-10-20: "[General Chat] [Room Chat]").
      private function roomChatAfterGeneral() : void
      {
         var room:int = -1;
         var general:int = -1;
         for(var i:int = 0; i < this._tabs.length; i++)
         {
            if(this._tabs[i].id == ChatTab.ROOM)
            {
               room = i;
            }
            else if(this._tabs[i].id == ChatTab.GENERAL)
            {
               general = i;
            }
         }
         if(room < 0 || general < 0 || room == general + 1)
         {
            return;
         }
         var tab:Object = this._tabs.splice(room,1)[0];
         if(room < general)
         {
            general--;
         }
         this._tabs.splice(general + 1,0,tab);
      }
   }
}

