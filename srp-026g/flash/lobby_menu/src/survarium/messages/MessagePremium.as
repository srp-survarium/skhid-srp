package survarium.messages
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessagePremium extends MessageAccept
   {
      
      public var list:ButtonBar;
      
      public var dropDownlist:DropdownMenu;
      
      public var background:Sprite;
      
      public var moneyBackContainer:Sprite;
      
      public var textFieldPremium:TextField;
      
      public var textFieldAccount:TextField;
      
      public var textFieldChooseSub:TextField;
      
      public var premiumBonus1:MovieClip;
      
      public var premiumBonus2:MovieClip;
      
      public var premiumBonus3:MovieClip;
      
      public var premiumBonus4:MovieClip;
      
      public var premiumBonus5:MovieClip;
      
      public var textFieldValue:TextField;
      
      public var coin:MovieClip;
      
      public var btnAddPremumMoney:Button;
      
      public var discountContainer:MovieClip;
      
      protected const LIST_OFFSET_TOP:uint = 97;
      
      protected const LIST_OFFSET_BOTTOM:uint = 82;
      
      public function MessagePremium()
      {
         super();
      }
      
      protected function updateTextFormat(param1:TextField, param2:String, param3:int = 55) : void
      {
         var _loc4_:String = param2;
         var _loc5_:int = _loc4_.indexOf(" ");
         var _loc6_:String = _loc4_.substr(0,_loc5_);
         var _loc7_:String = _loc4_.substr(_loc5_ + 1,_loc4_.length);
         var _loc8_:String = "\t" + _loc6_ + "\n" + _loc7_;
         var _loc9_:TextFormat = param1.defaultTextFormat;
         _loc9_.leftMargin = 0;
         _loc9_.tabStops = [param3];
         param1.defaultTextFormat = _loc9_;
         param1.setTextFormat(_loc9_);
         param1.text = _loc8_;
      }
      
      override protected function configUI() : void
      {
         var _loc1_:int;
         var _loc2_:Array;
         var _loc3_:uint;
         var _loc4_:Object;
         var _loc5_:Boolean;
         var _loc6_:String;
         var _loc7_:int;
         var _loc8_:int;
         super.configUI();
         this.coin.gotoAndStop(2);
         this.coin.x += 2;
         this.textFieldValue.x += 2;
         btnAccept.label = LocalizationData.getName("buy");
         this.textFieldValue.mouseEnabled = false;
         if(this.premiumBonus1)
         {
            this.premiumBonus3.textField.width += 10;
            this.updateTextFormat(this.premiumBonus1.textField,LocalizationData.getName("premium_bonus_experience").toUpperCase());
            this.updateTextFormat(this.premiumBonus2.textField,LocalizationData.getName("premium_bonus_money").toUpperCase());
            this.updateTextFormat(this.premiumBonus3.textField,LocalizationData.getName("premium_bonus_reputation").toUpperCase());
            this.updateTextFormat(this.premiumBonus4.textField,LocalizationData.getName("premium_bonus_item_search").toUpperCase());
            this.updateTextFormat(this.premiumBonus5.textField,LocalizationData.getName("premium_bonus_profile").toUpperCase(),25);
         }
         this.textFieldValue.x += 1;
         this.coin.x -= 1;
         tabEnabled = true;
         btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         if(_data)
         {
            _loc1_ = -1;
            _loc2_ = _data.options;
            _loc3_ = 0;
            while(_loc3_ < _loc2_.length)
            {
               _loc4_ = _loc2_[_loc3_];
               _loc5_ = PlayerInfo.premiumMoney >= _loc4_.value;
               _loc4_.valid = _loc5_;
               _loc4_.enabled = true;
               trace("configPM::",_loc4_.label,"|",_loc4_.value2);
               Util.logObject(_loc4_);
               if(_loc4_.text == undefined)
               {
                  _loc4_.text = _loc4_.label;
               }
               if(_loc4_.discount)
               {
                  _loc6_ = Util.styledChar(LocalizationData.getName("discount"),"white") + " " + Util.styledChar(_loc4_.discount,"yellow");
               }
               else
               {
                  _loc6_ = "";
               }
               _loc4_.label = Util.styledChar(_loc4_.text,"yellow") + " " + Util.styledChar(_loc4_.value2,"dark_yellow") + " " + _loc6_;
               if(_loc5_ && _loc1_ == -1)
               {
                  _loc1_ = _loc3_;
               }
               _loc3_++;
            }
            if(this.list)
            {
               _loc7_ = height / scaleY - (this.LIST_OFFSET_TOP + this.LIST_OFFSET_BOTTOM);
               _loc8_ = _loc2_.length * 32;
               this.list.height = _loc8_ + 5;
               this.list.y = (_loc7_ - _loc8_) / 2 + this.LIST_OFFSET_TOP;
               btnAccept.x = int(btnAccept.x + 1);
               this.list.addEventListener(IndexEvent.INDEX_CHANGE,this.onPremiumSelected);
               this.list.dataProvider = new DataProvider(_loc2_);
               this.list.selectedIndex = _loc1_;
            }
            _loc1_ = 3;
            btnAccept.enabled = true;
         }
         if(this.dropDownlist)
         {
            this.dropDownlist.menuWidth = 200;
            this.textFieldChooseSub.text = LocalizationData.getName("choose_premium_subscription");
            this.dropDownlist.dataProvider = new DataProvider(_loc2_);
            this.dropDownlist.addEventListener(ListEvent.INDEX_CHANGE,this.onPremiumSelected);
            this.dropDownlist.autoSize = "left";
            this.dropDownlist.selectedIndex = _loc1_;
            this.dropDownlist.validateNow();
            this.dropDownlist.autoSize = "none";
            this.moneyBackContainer.x = this.dropDownlist.x + this.dropDownlist.width + 3;
            this.coin.x = this.moneyBackContainer.x + 36;
            this.textFieldValue.x = this.moneyBackContainer.x + 55;
            this.onPremiumSelected();
         }
         stage.focus = this.list;
      }
      
      private function onPremiumMoneyAddClick(param1:Event) : void
      {
         MessageMoneyExchange.buyGold();
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
         MessageMoneyExchange.buyGold();
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var _loc2_:uint = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id;
         var _loc3_:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         if(_loc3_)
         {
            super.onMessageAccept();
            trace("ACCEPT_PREMIUM_MESSAGE:",_loc2_);
            if(_data.victory == true)
            {
               ExternalInterface.call("accept_premium_victory_screen",_loc2_,uint(_data.matchId));
            }
            else
            {
               ExternalInterface.call("accept_premium",_loc2_);
            }
            var _loc4_:Function = _data.onYes as Function;
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

