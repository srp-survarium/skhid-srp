package survarium.messages
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.constants.Reward;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessagePostPremium extends MessageAccept
   {
      
      public static var discount:Number = 1;
      
      public var list:ButtonBar;
      
      public var dropDownlist:DropdownMenu;
      
      public var textFieldLabel:TextField;
      
      public var textFieldDescr:TextField;
      
      public var moneyBackContainer:MovieClip;
      
      public var reward1:MovieClip;
      
      public var reward2:MovieClip;
      
      public var reward3:MovieClip;
      
      public var textFieldValue:TextField;
      
      public var coin:MovieClip;
      
      public var discountContainer:MovieClip;
      
      protected const LIST_OFFSET_TOP:uint = 97;
      
      protected const LIST_OFFSET_BOTTOM:uint = 82;
      
      public function MessagePostPremium()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.coin.gotoAndStop(2);
         this.coin.x += 2;
         this.textFieldValue.x += 2;
         var _loc1_:String = "";
         var _loc2_:int;
         var _loc3_:Array;
         var _loc4_:uint;
         var _loc5_:Object;
         var _loc6_:Boolean;
         var _loc7_:String;
         var _loc8_:int;
         var _loc9_:int;
         btnAccept.label = LocalizationData.getName("buy_post_premium").toUpperCase();
         this.textFieldLabel.text = LocalizationData.getName("premium_post_reward_label").toUpperCase() + _loc1_;
         this.textFieldDescr.text = LocalizationData.getName("premium_post_reward_descr").toUpperCase() + _loc1_;
         this.textFieldValue.mouseEnabled = false;
         this.textFieldValue.x += 1;
         this.coin.x -= 1;
         tabEnabled = true;
         btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         if(_data)
         {
            _loc2_ = -1;
            _loc3_ = _data.options;
            _loc4_ = 0;
            while(_loc4_ < _loc3_.length)
            {
               _loc5_ = _loc3_[_loc4_];
               _loc6_ = PlayerInfo.premiumMoney >= _loc5_.value;
               _loc5_.valid = _loc6_;
               _loc5_.enabled = true;
               trace("configPM::",_loc5_.label,"|",_loc5_.value2);
               if(_loc5_.text == undefined)
               {
                  _loc5_.text = _loc5_.label;
               }
               if(_loc5_.discount)
               {
                  _loc7_ = Util.styledChar(LocalizationData.getName("discount"),"white") + " " + Util.styledChar(_loc5_.discount,"yellow");
               }
               else
               {
                  _loc7_ = "";
               }
               _loc5_.label = Util.styledChar(_loc5_.text,"yellow") + " " + Util.styledChar(_loc5_.value2,"dark_yellow") + " " + _loc7_;
               if(_loc6_ && _loc2_ == -1)
               {
                  _loc2_ = _loc4_;
               }
               _loc4_++;
            }
            if(this.list)
            {
               _loc8_ = height / scaleY - (this.LIST_OFFSET_TOP + this.LIST_OFFSET_BOTTOM);
               _loc9_ = _loc3_.length * 32;
               this.list.height = _loc9_ + 5;
               this.list.y = (_loc8_ - _loc9_) / 2 + this.LIST_OFFSET_TOP;
               btnAccept.x = int(btnAccept.x + 1);
               this.list.addEventListener(IndexEvent.INDEX_CHANGE,this.onPremiumSelected);
               this.list.dataProvider = new DataProvider(_loc3_);
               this.list.selectedIndex = _loc2_;
            }
            _loc2_ = 3;
            btnAccept.enabled = true;
         }
         if(this.dropDownlist)
         {
            this.dropDownlist.menuWidth = 200;
            this.dropDownlist.dataProvider = new DataProvider(_loc3_);
            this.dropDownlist.addEventListener(ListEvent.INDEX_CHANGE,this.onPremiumSelected);
            this.dropDownlist.autoSize = "left";
            this.dropDownlist.selectedIndex = _loc2_;
            this.dropDownlist.validateNow();
            this.dropDownlist.autoSize = "none";
            this.moneyBackContainer.x = this.dropDownlist.x + this.dropDownlist.width + 3;
            this.coin.x = this.moneyBackContainer.x + 36;
            this.textFieldValue.x = this.moneyBackContainer.x + 55;
            this.onPremiumSelected();
         }
         stage.focus = this.list;
         this.reward1.textFieldRewardName.text = LocalizationData.getName("money").toUpperCase();
         this.reward2.textFieldRewardName.text = LocalizationData.getName("experience").toUpperCase();
         this.reward3.textFieldRewardName.text = LocalizationData.getName("reputation").toUpperCase();
         this.reward1.reward.data = {
            type:Reward.MONEY,
            amount:_data.money
         };
         this.reward2.reward.data = {
            type:Reward.EXPERIENCE,
            amount:_data.experience
         };
         this.reward3.reward.data = {
            type:Reward.REPUTATION,
            amount:_data.reputation,
            param:_data.factionId
         };
         this.reward1.reward.x = (this.reward1.width - this.reward1.reward.width) / 2;
         this.reward2.reward.x = (this.reward2.width - this.reward2.reward.width) / 2;
         this.reward3.reward.x = (this.reward3.width - this.reward3.reward.width) / 2;
      }
      
      private function onPremiumSelected(param1:ListEvent = null) : void
      {
         if(this.dropDownlist.selectedIndex == -1)
         {
            return;
         }
         var _loc2_:Object = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex];
         this.textFieldValue.text = Util.getMoneyString(_loc2_.value);
         if(_loc2_.discount)
         {
            if(this.discountContainer.alpha == 0)
            {
               TweenLite.to(this.discountContainer,0.3,{alpha:1});
            }
            else
            {
               TweenLite.to(this.discountContainer,0.3,{alpha:0});
               TweenLite.to(this.discountContainer,0.3,{
                  alpha:1,
                  delay:0.3
               });
            }
            this.discountContainer.textField.text = LocalizationData.getName("discount") + " " + _loc2_.discount;
         }
         else if(this.discountContainer.alpha == 1)
         {
            TweenLite.to(this.discountContainer,0.3,{alpha:0});
         }
      }
      
      protected function onNoMoneyEnought() : void
      {
         var _loc1_:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",_loc1_,true,0,true);
      }
      
      protected function goToStore() : void
      {
         ExternalInterface.call("go_to_store");
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var _loc2_:uint = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id;
         var _loc3_:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         var _loc4_:Function;
         if(_loc3_)
         {
            super.onMessageAccept();
            trace("ACCEPT_PREMIUM_MESSAGE_discount:",_loc2_,_data.matchId,_data.victory,"d::",discount);
            if(_data.victory == true)
            {
               ExternalInterface.call("accept_premium_victory_screen",_loc2_,uint(_data.matchId));
            }
            else
            {
               ExternalInterface.call("accept_premium_with_bonuses",_loc2_,discount,_data.money,_data.experience,_data.reputation);
            }
            _loc4_ = _data.onYes as Function;
            _data.onNo = null;
            _data.onYes = null;
            if(_loc4_ is Function)
            {
               _loc4_(_data);
            }
         }
         else
         {
            this.onNoMoneyEnought();
         }
      }
   }
}

