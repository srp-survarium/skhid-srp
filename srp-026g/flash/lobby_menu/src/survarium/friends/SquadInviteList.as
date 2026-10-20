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
      
      protected function onInviteItemClick(param1:ListEvent) : void
      {
         var _loc2_:Boolean;
         var _loc3_:Object = param1.itemData;
         if(_loc3_)
         {
            if(_loc3_.type == SQUAD_INVITE)
            {
               _loc2_ = !PlayerInfo.isInSquad && !PlayerInfo.isInTeam;
            }
            else if(_loc3_.type == MATCH_INVITE)
            {
               _loc2_ = !PlayerInfo.isInSquad;
            }
            else
            {
               _loc2_ = ClanWindow.getState() == 0;
            }
         }
         trace("onInviteItemClick",_loc2_,param1.itemData,PlayerInfo.isInSquad,PlayerInfo.isInTeam);
         if(!param1.itemData)
         {
            return;
         }
         param1.itemData.acceptable = _loc2_;
         Message.showMessage("invite",param1.itemData,true);
      }
      
      public function addInvite(param1:Object) : void
      {
         this._data.unshift(param1);
         this.list.dataProvider = new DataProvider(this._data);
         this.list.playScrollTween();
         this.list.validateNow();
         var _loc2_:Boolean = this.list.getScrollable();
         if(!this._scrollBarEnabled && _loc2_)
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
         else if(this._scrollBarEnabled && !_loc2_)
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
      
      public function removeInvite(param1:uint) : void
      {
         var _loc2_:uint = this._data.length;
         var _loc3_:int = 0;
         while(_loc3_ < _loc2_)
         {
            if(this._data[_loc3_].id == param1)
            {
               this._data.splice(_loc3_,1);
               break;
            }
            _loc3_++;
         }
         invalidateData();
         this.list.dataProvider = new DataProvider(this._data);
         this.list.validateNow();
      }
      
      public function set data(param1:Object) : void
      {
         _data = param1 as Array;
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

