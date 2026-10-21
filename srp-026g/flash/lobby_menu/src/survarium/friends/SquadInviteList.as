package survarium.friends
{
   import com.greensock.TweenLite;
   import scaleform.clik.controls.ScrollBar;
   import scaleform.clik.controls.ScrollingListSmooth;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataRenderer;
   import survarium.clan.ClanWindow;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.constants.ClanState;
   
   public class SquadInviteList extends UIComponent implements IDataRenderer
   {
      
      public static const SQUAD_INVITE:uint = 1;
      
      public static const MATCH_INVITE:uint = 4;
      
      public static const CLAN_INVITE:uint = 3;
      
      public var list:ScrollingListSmooth;
      
      public var scrollBar:ScrollBar;
      
      private var _data:Array = [];
      
      private var _scrollBarEnabled:Boolean = false;
      
      public function SquadInviteList()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.list.maskAutoResize = true;
         this.list.addEventListener(ListEvent.ITEM_CLICK,this.onInviteItemClick);
      }
      
      protected function onInviteItemClick(event:ListEvent) : void
      {
         var canAccept:Boolean;
         var invite:Object = event.itemData;
         if(invite)
         {
            if(invite.type == SQUAD_INVITE)
            {
               canAccept = !PlayerInfo.isInSquad && !PlayerInfo.isInTeam;
            }
            else if(invite.type == MATCH_INVITE)
            {
               canAccept = !PlayerInfo.isInSquad;
            }
            else
            {
               canAccept = ClanWindow.getState() == ClanState.RESERVE;
            }
         }
         trace("onInviteItemClick",canAccept,event.itemData,PlayerInfo.isInSquad,PlayerInfo.isInTeam);
         if(!event.itemData)
         {
            return;
         }
         event.itemData.acceptable = canAccept;
         Message.showMessage("invite",event.itemData,true);
      }
      
      public function addInvite(invite:Object) : void
      {
         this._data.unshift(invite);
         this.list.dataProvider = new DataProvider(this._data);
         this.list.playScrollTween();
         this.list.validateNow();
         var isScrollable:Boolean = this.list.getScrollable();
         if(!this._scrollBarEnabled && isScrollable)
         {
            _scrollBarEnabled = true;
            this.scrollBar.visible = true;
            this.scrollBar.alpha = 0;
            TweenLite.to(this.list,0.2,{x:this.list.x + this.scrollBar.width + 4});
            TweenLite.to(this.scrollBar,0.2,{
               x:this.scrollBar.x + this.scrollBar.width + 4,
               alpha:1
            });
         }
         else if(this._scrollBarEnabled && !isScrollable)
         {
            _scrollBarEnabled = false;
            TweenLite.to(this.list,0.2,{x:this.list.x - this.scrollBar.width - 4});
            TweenLite.to(this.scrollBar,0.2,{
               x:this.scrollBar.x - this.scrollBar.width - 4,
               alpha:0,
               onComplete:this.hideScrollBar
            });
         }
         invalidateData();
      }
      
      private function hideScrollBar() : void
      {
         this.scrollBar.visible = false;
      }
      
      public function removeInvite(inviteId:uint) : void
      {
         var count:uint = this._data.length;
         var i:int = 0;
         while(i < count)
         {
            if(this._data[i].id == inviteId)
            {
               this._data.splice(i,1);
               break;
            }
            i++;
         }
         invalidateData();
         this.list.dataProvider = new DataProvider(this._data);
         this.list.validateNow();
      }
      
      public function set data(value:Object) : void
      {
         _data = value as Array;
         this.list.dataProvider = new DataProvider(this._data);
         invalidateData();
      }
      
      public function get data() : Object
      {
         return this._data;
      }
      
      override protected function draw() : void
      {
         super.draw();
      }
   }
}

