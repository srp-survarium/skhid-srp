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
         var suffix:String = "";
         var selectedOption:int;
         var options:Array;
         var i:uint;
         var option:Object;
         var affordable:Boolean;
         var discountText:String;
         var listSpace:int;
         var listHeight:int;
         btnAccept.label = LocalizationData.getName("buy_post_premium").toUpperCase();
         this.textFieldLabel.text = LocalizationData.getName("premium_post_reward_label").toUpperCase() + suffix;
         this.textFieldDescr.text = LocalizationData.getName("premium_post_reward_descr").toUpperCase() + suffix;
         this.textFieldValue.mouseEnabled = false;
         this.textFieldValue.x += 1;
         this.coin.x -= 1;
         tabEnabled = true;
         btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         if(_data)
         {
            selectedOption = -1;
            options = _data.options;
            i = 0;
            while(i < options.length)
            {
               option = options[i];
               affordable = PlayerInfo.premiumMoney >= option.value;
               option.valid = affordable;
               option.enabled = true;
               trace("configPM::",option.label,"|",option.value2);
               if(option.text == undefined)
               {
                  option.text = option.label;
               }
               if(option.discount)
               {
                  discountText = Util.styledChar(LocalizationData.getName("discount"),"white") + " " + Util.styledChar(option.discount,"yellow");
               }
               else
               {
                  discountText = "";
               }
               option.label = Util.styledChar(option.text,"yellow") + " " + Util.styledChar(option.value2,"dark_yellow") + " " + discountText;
               if(affordable && selectedOption == -1)
               {
                  selectedOption = i;
               }
               i++;
            }
            if(this.list)
            {
               listSpace = height / scaleY - (this.LIST_OFFSET_TOP + this.LIST_OFFSET_BOTTOM);
               listHeight = options.length * 32;
               this.list.height = listHeight + 5;
               this.list.y = (listSpace - listHeight) / 2 + this.LIST_OFFSET_TOP;
               btnAccept.x = int(btnAccept.x + 1);
               this.list.addEventListener(IndexEvent.INDEX_CHANGE,this.onPremiumSelected);
               this.list.dataProvider = new DataProvider(options);
               this.list.selectedIndex = selectedOption;
            }
            selectedOption = 3;
            btnAccept.enabled = true;
         }
         if(this.dropDownlist)
         {
            this.dropDownlist.menuWidth = 200;
            this.dropDownlist.dataProvider = new DataProvider(options);
            this.dropDownlist.addEventListener(ListEvent.INDEX_CHANGE,this.onPremiumSelected);
            this.dropDownlist.autoSize = "left";
            this.dropDownlist.selectedIndex = selectedOption;
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
      
      private function onPremiumSelected(event:ListEvent = null) : void
      {
         if(this.dropDownlist.selectedIndex == -1)
         {
            return;
         }
         var option:Object = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex];
         this.textFieldValue.text = Util.getMoneyString(option.value);
         if(option.discount)
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
            this.discountContainer.textField.text = LocalizationData.getName("discount") + " " + option.discount;
         }
         else if(this.discountContainer.alpha == 1)
         {
            TweenLite.to(this.discountContainer,0.3,{alpha:0});
         }
      }
      
      protected function onNoMoneyEnought() : void
      {
         var messageData:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true,0,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var premiumId:uint = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id;
         var affordable:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         var onYes:Function;
         if(affordable)
         {
            super.onMessageAccept();
            trace("ACCEPT_PREMIUM_MESSAGE_discount:",premiumId,_data.matchId,_data.victory,"d::",discount);
            if(_data.victory == true)
            {
               ExternalInterface.call("accept_premium_victory_screen",premiumId,uint(_data.matchId));
            }
            else
            {
               ExternalInterface.call("accept_premium_with_bonuses",premiumId,discount,_data.money,_data.experience,_data.reputation);
            }
            onYes = _data.onYes as Function;
            _data.onNo = null;
            _data.onYes = null;
            if(onYes is Function)
            {
               onYes(_data);
            }
         }
         else
         {
            this.onNoMoneyEnought();
         }
      }
   }
}

