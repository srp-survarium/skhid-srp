package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import survarium.buttons.ButtonFriends;
   import survarium.data.LocalizationData;
   import survarium.live.LiveWindow;
   import survarium.rooms.RoomsWindow;
   import survarium.constants.ChatTab;
   
   public class BottomPanel extends UIComponent
   {
      
      public static var matchesHolder:BottomPanel;

      public var tabsBar:ButtonBarBottom;
      
      public var btnFriends:ButtonFriends;
      
      public var btn1:Button;
      
      public var btn2:Button;
      
      public var btn3:Button;
      
      public var btnShowLogList:Button;
      
      public var background:Sprite;
      
      public var barInvites:ButtonBarBottom;
      
      public var debugLine:Sprite;
      
      public var matchesTab:Button;
      
      protected var _tabs:Array = [];
      
      protected var _invites:Array = [];
      
      // the Matches tab's window (survarium.live), made at its first opening
      protected var _live:LiveWindow;

      public function BottomPanel()
      {
         super();
         this.debugLine.visible = false;
         BottomPanel.matchesHolder = this;
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
         this.createMatchesTab();
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
         if(this.matchesTab)
         {
            room -= 150;
         }
         this.tabsBar.setMaxSize(room);
         this.tabsBar.invalidate(InvalidationType.RENDERERS);
         this.placeMatchesTab();
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.layout();
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
         this.placeMatchesWindow();
      }
      
      protected function onMatchesLocal(value:String) : void
      {
         if(this.matchesTab)
         {
            this.matchesTab.data = {
               "id":-1,
               "name":LocalizationData.getName("st_srp_matches_title")
            };
         }
      }

      protected function createMatchesTab() : void
      {
         var icon:Shape = null;
         var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
         this.matchesTab = new tabClass() as Button;
         this.matchesTab.name = "matchesTab";
         this.matchesTab.data = {
            "id":-1,
            "name":LocalizationData.getName("st_srp_matches_title")
         };
         this.matchesTab.y = 1;
         addChild(this.matchesTab);
         icon = new Shape();
         icon.graphics.lineStyle(1.6,10266277);
         icon.graphics.moveTo(1,12);
         icon.graphics.lineTo(12,1);
         icon.graphics.moveTo(12,12);
         icon.graphics.lineTo(1,1);
         icon.graphics.moveTo(2,8);
         icon.graphics.lineTo(5,11);
         icon.graphics.moveTo(8,11);
         icon.graphics.lineTo(11,8);
         icon.x = 11;
         icon.y = 8;
         this.matchesTab.addChild(icon);
         this.matchesTab.addEventListener(ButtonEvent.CLICK,this.onMatchesTabClick);
         this.matchesTab.addEventListener(Event.ENTER_FRAME,this.onMatchesTabFrame);
         LocalizationData.addCallback(this.onMatchesLocal,"st_srp_matches_title");
         this.placeMatchesTab();
         // the rooms window (in the lobby, over this bar) is made with it
         this.makeRoomsWindow();
      }

      // the chat tab's own icon off (the tab is a chat tab's look)
      protected function onMatchesTabFrame(e:Event) : void
      {
         var chatIcon:DisplayObject = null;
         var back:DisplayObjectContainer = this.matchesTab["background"] as DisplayObjectContainer;
         if(back)
         {
            chatIcon = back.getChildByName("icon");
            if(chatIcon && chatIcon.visible)
            {
               chatIcon.visible = false;
            }
         }
      }

      protected function placeMatchesTab() : void
      {
         var used:Number = NaN;
         if(!this.matchesTab || !this.tabsBar || !this.tabsBar.container)
         {
            return;
         }
         used = this.tabsBar.container.width / this.tabsBar.container.scaleX;
         this.matchesTab.x = this.tabsBar.x + int(used);
         this.placeMatchesWindow();
         // The rooms window follows the bar.
         if(RoomsWindow.me != null)
         {
            RoomsWindow.me.layout();
         }
      }

      protected function onMatchesTabClick(e:Event) : void
      {
         this.showMatches(!(this._live && this._live.shown));
      }

      // The running matches (LiveWindow), over the bar at the tab.
      public function showMatches(value:Boolean) : void
      {
         if(value && !this._live)
         {
            this._live = new LiveWindow(this.matchesTab.textField.getTextFormat().font);
            this._live.addEventListener(Event.RESIZE,this.onLiveResize,false,0,true);
            parent.addChild(this._live);
         }
         if(this._live)
         {
            this._live.visible = value;
            if(value)
            {
               parent.setChildIndex(this._live,parent.numChildren - 1);
               this._live.show();
            }
            else
            {
               this._live.hide();
            }
         }
         this.matchesTab.selected = value;
         this.placeMatchesWindow();
      }

      protected function onLiveResize(e:Event) : void
      {
         this.placeMatchesWindow();
      }

      protected function placeMatchesWindow() : void
      {
         if(!this._live)
         {
            return;
         }
         this._live.x = int(this.x + this.matchesTab.x);
         this._live.y = int(this.y - this._live.height - 4);
      }

      // The rooms window, made once, with the Matches tab: in the lobby, over
      // this bar (survarium.rooms).
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

