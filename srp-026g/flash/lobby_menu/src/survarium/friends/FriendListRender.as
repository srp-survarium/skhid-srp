package survarium.friends
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.clan.ClanWindow;
   import survarium.controls.FixedTextDots;
   import survarium.controls.ListItemBase;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   
   public class FriendListRender extends ListItemBase
   {
      
      public var status_tx:TextField;
      
      public var titul_tx:TextField;
      
      public var name_tx:FixedTextDots;
      
      public var menuButton:MovieClip;
      
      public var btn_ignore:MovieClip;
      
      public var btn_acccept:Button;
      
      public var btn_cancel:Button;
      
      public var background:MovieClip;
      
      public var status_indicator:MovieClip;
      
      public var separator:MovieClip;
      
      public function FriendListRender()
      {
         super();
      }
      
      public function removeFriend() : void
      {
         ExternalInterface.call("remove_friend",data.id);
      }
      
      override public function set selected(param1:Boolean) : void
      {
      }
      
      public function set highlighted(param1:Boolean) : void
      {
         super.selected = param1;
      }
      
      public function handleCrossClick(param1:Event = null) : void
      {
         if(data.ignored)
         {
            this.removeIgnored();
         }
         else
         {
            this.removeFriend();
         }
      }
      
      public function addToFriends() : void
      {
         ExternalInterface.call("add_friend",data.id);
      }
      
      public function removeIgnored() : void
      {
         ExternalInterface.call("remove_ignored",data.id);
      }
      
      public function addToIgnore() : void
      {
         ExternalInterface.call("add_ignore",data.id);
      }
      
      public function askToJoin() : void
      {
         ExternalInterface.call("ask_join_game",data.id);
      }
      
      public function askToJoinSquad() : void
      {
         ExternalInterface.call("invite_to_squad",data.name);
      }
      
      public function sendMessage() : void
      {
         var _loc1_:String = data.name;
         ExternalInterface.call("start_friend_message",_loc1_);
         dispatchEvent(new Event(Event.CONNECT,true));
      }
      
      override public function get height() : Number
      {
         if(!_data || _data.request == undefined)
         {
            return 27;
         }
         return 54;
      }
      
      override protected function setState(param1:String) : void
      {
         if(_data && _data.isSeparator)
         {
            return;
         }
         super.setState(param1);
      }
      
      override protected function updateText() : void
      {
         if(!data)
         {
            visible = false;
            return;
         }
         visible = true;
         var _loc1_:Boolean = _data.isSeparator;
         this.separator.visible = _loc1_;
         this.status_indicator.visible = this.status_tx.visible = this.name_tx.visible = !this.separator.visible;
         if(_loc1_)
         {
            this.separator.textField.text = _data.status_text;
            this.menuButton.visible = false;
         }
         else
         {
            this.menuButton.visible = true;
         }
         this.name_tx.text = data.abbr ? "[" + data.abbr + "] " + data.name : data.name;
         this.name_tx.validateNow();
         this.menuButton.x = this.name_tx.textField.textWidth + this.name_tx.x + 7;
         var _loc2_:Boolean = data.request != undefined;
         var _loc3_:Boolean;
         if(this.background)
         {
            this.background.height = _loc2_ ? 54 : 27;
         }
         this.btn_acccept.y = this.btn_cancel.y = _loc2_ ? 27 : 0;
         if(_loc2_)
         {
            this.status_indicator.gotoAndStop("invite");
         }
         else if(_data.ignored)
         {
            this.status_indicator.gotoAndStop("offline");
         }
         else if(_data.status == 0)
         {
            this.status_indicator.gotoAndStop("online");
         }
         else if(_data.status == 1)
         {
            this.status_indicator.gotoAndStop("ingame");
         }
         else
         {
            this.status_indicator.gotoAndStop("offline");
         }
         this.status_tx.text = "";
         this.status_tx.visible = false;
         if(_loc2_ && enabled)
         {
            this.btn_acccept.visible = true;
            this.btn_cancel.visible = true;
            _loc3_ = false;
            if(_data.sended != true)
            {
               _loc3_ = true;
            }
            if(this.btn_acccept.enabled != _loc3_)
            {
               this.btn_acccept.enabled = _loc3_;
               this.btn_acccept.validateNow();
            }
            if(this.btn_cancel.enabled != _loc3_)
            {
               this.btn_cancel.enabled = _loc3_;
               this.btn_cancel.validateNow();
            }
         }
         else
         {
            this.btn_acccept.visible = false;
            this.btn_cancel.visible = false;
            this.btn_acccept.enabled = false;
            this.btn_cancel.enabled = false;
         }
         this.name_tx.buttonMode = enabled;
         selectable = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         addEventListener(MouseEvent.CLICK,this.onClick);
         if(this.btn_ignore)
         {
            this.btn_ignore.addEventListener(ButtonEvent.CLICK,this.handleCrossClick);
         }
         this.name_tx.mouseEnabled = false;
         this.name_tx.mouseChildren = false;
         this.name_tx.textField.mouseEnabled = false;
         this.btn_acccept.addEventListener(ButtonEvent.CLICK,this.handleButtonClick);
         this.btn_cancel.addEventListener(ButtonEvent.CLICK,this.handleButtonClick);
         this.menuButton.addEventListener(ButtonEvent.CLICK,this.showContextMenu);
         LocalizationData.addButton(this.btn_acccept,"add");
         LocalizationData.addButton(this.btn_cancel,"cancel");
         selectable = false;
      }
      
      protected function showContextMenu(param1:Event = null) : void
      {
         var _loc2_:Array;
         var _loc3_:String;
         var _loc4_:Boolean;
         var _loc5_:Boolean;
         var _loc6_:Boolean;
         var _loc7_:Boolean;
         var _loc8_:Boolean;
         var _loc9_:Boolean;
         var _loc10_:Array;
         if(!data.ignored)
         {
            if(_data.status == -1)
            {
               return;
            }
            _loc2_ = [];
            _loc3_ = data.name;
            _loc4_ = FriendsMenu.isIgnored(_loc3_);
            _loc5_ = _data.status == 0 || _data.status == 1 || _data.status == 2 || _data.status == 3;
            _loc6_ = _data.status == 0 || _data.status == 1 || _data.status == 4;
            _loc7_ = _data.status == 0 || _data.status == 1 || _data.status == 3;
            _loc8_ = _data.status == undefined;
            if(_loc8_)
            {
               _loc9_ = FriendsMenu.isFriend(_loc3_);
               if(!_loc9_)
               {
                  _loc2_.push({
                     name:LocalizationData.getName("add_to_friends"),
                     click_function:this.addToFriends
                  });
               }
               if(!_loc4_)
               {
                  _loc2_.push({
                     name:LocalizationData.getName("add_to_ignore"),
                     click_function:this.addToIgnore
                  });
               }
            }
            if(_loc6_)
            {
               _loc2_.push({
                  name:LocalizationData.getName("send_message"),
                  click_function:this.sendMessage
               });
            }
            if(_loc5_)
            {
               _loc2_.push({
                  name:LocalizationData.getName("remove_from_friends"),
                  click_function:this.removeFriend
               });
               if(!_loc4_)
               {
                  _loc2_.push({
                     name:LocalizationData.getName("add_to_ignore"),
                     click_function:this.addToIgnore
                  });
               }
               _loc10_ = [];
               _loc10_.push({
                  name:LocalizationData.getName("bot_or_cheater_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:1
               });
               _loc10_.push({
                  name:LocalizationData.getName("swearing_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:2
               });
               _loc10_.push({
                  name:LocalizationData.getName("non_sport_behavior_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:3
               });
               _loc10_.push({
                  name:LocalizationData.getName("account_trading_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:1,
                  subIndex:4
               });
               _loc2_.push({
                  name:LocalizationData.getName("claim_player"),
                  options:_loc10_
               });
               trace("showContextMenu::",_data.status,_loc7_);
            }
            if(PlayerInfo.commander && !PlayerInfo.isSquadFull && (_loc5_ && _loc7_ || _loc8_) && !SquadList.isInSquad(_loc3_))
            {
               _loc2_.push({
                  name:LocalizationData.getName("invite_to_squad"),
                  click_function:this.askToJoinSquad
               });
            }
            if((PlayerInfo.isClanManager || PlayerInfo.isClanIviter) && !ClanWindow.isInClan(_loc3_))
            {
               _loc2_.push({
                  name:LocalizationData.getName("invite_to_clan"),
                  click_function:this.askToJoinClan
               });
            }
            highlighted = true;
            SrvContextMenu.show(_loc2_);
            SrvContextMenu.getInstance().addEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
         }
         else
         {
            highlighted = true;
            SrvContextMenu.show([{
               name:LocalizationData.getName("remove_from_ignore"),
               click_function:this.removeIgnored
            }]);
            SrvContextMenu.getInstance().addEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
         }
      }
      
      private function askToJoinClan() : void
      {
         trace("askToJoinClan",data.name);
         ExternalInterface.call("invite_player_to_clan",data.name);
      }
      
      protected function flagPlayer(param1:Object) : void
      {
         trace("comment_about_user",data.name,param1.index,param1.subIndex);
         ExternalInterface.call("comment_about_user",data.name,param1.index,param1.subIndex);
      }
      
      protected function handleButtonClick(param1:ButtonEvent) : void
      {
         var _loc2_:uint = param1.target == this.btn_acccept ? 0 : 1;
         this.btn_acccept.enabled = false;
         this.btn_cancel.enabled = false;
         _data.sended = true;
         dispatchEvent(new Event(Event.CONNECT,true));
         var _loc3_:uint = (_data.type);
         if(data.id != 666)
         {
            ExternalInterface.call("important_message_read",data.id,_loc2_,_loc3_);
         }
      }
      
      protected function deselect(param1:Event = null) : void
      {
         highlighted = false;
         SrvContextMenu.getInstance().removeEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
      }
      
      protected function onClick(param1:MouseEventEx) : void
      {
         if(!data)
         {
            return;
         }
         if(param1.buttonIdx == 1 && _data.name)
         {
            this.showContextMenu();
         }
      }
   }
}

