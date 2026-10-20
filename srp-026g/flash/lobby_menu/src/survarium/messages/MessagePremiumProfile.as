package survarium.messages
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessagePremiumProfile extends UIComponent
   {
      
      public var dropDownlist:DropdownMenu;
      
      public var moneyBackContainer:Sprite;
      
      public var textField:TextField;
      
      public var textFieldValue:TextField;
      
      public var coin:MovieClip;
      
      public var btnAccept:Button;
      
      public var data:Object;
      
      public function MessagePremiumProfile()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.coin.gotoAndStop(2);
         this.textFieldValue.mouseEnabled = false;
         tabEnabled = true;
         this.btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         LocalizationData.addTextField(this.textField,"premium_profile_unlock_description");
         LocalizationData.addButton(this.btnAccept,"buy");
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept);
      }
      
      public function setData(param1:Object) : void
      {
         var _loc2_:int;
         var _loc3_:Array;
         var _loc4_:uint;
         var _loc5_:Object;
         var _loc6_:Boolean;
         var _loc7_:String;
         data = param1;
         if(this.data)
         {
            _loc2_ = -1;
            _loc3_ = this.data.options;
            _loc4_ = 0;
            while(_loc4_ < _loc3_.length)
            {
               _loc5_ = _loc3_[_loc4_];
               _loc6_ = PlayerInfo.premiumMoney >= _loc5_.value;
               _loc5_.valid = _loc6_;
               _loc5_.enabled = true;
               trace("configPM::",_loc5_.label,"|",_loc5_.value2,_loc5_.discount_value);
               Util.logObject(_loc5_);
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
            _loc2_ = int(_loc3_.length / 2);
            this.btnAccept.enabled = true;
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
            this.textFieldValue.x = this.moneyBackContainer.x + 58;
            this.onPremiumSelected();
         }
      }
      
      private function onPremiumSelected(param1:ListEvent = null) : void
      {
         if(this.dropDownlist.selectedIndex == -1)
         {
            return;
         }
         var _loc2_:Object = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex];
         this.textFieldValue.text = Util.getMoneyString(_loc2_.value);
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
      
      protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         if(!this.btnAccept.enabled)
         {
            return;
         }
         var _loc2_:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         var _loc3_:uint;
         if(_loc2_)
         {
            this.btnAccept.enabled = false;
            _loc3_ = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id;
            trace("accept_premium:",_loc3_);
            ExternalInterface.call("accept_premium",_loc3_);
         }
         else
         {
            this.onNoMoneyEnought();
         }
      }
   }
}

