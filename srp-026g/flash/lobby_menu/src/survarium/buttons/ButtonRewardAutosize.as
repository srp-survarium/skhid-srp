package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import survarium.Util;
   import survarium.constants.Reward;
   import survarium.controls.PlayerInfo;
   import survarium.constants.MoneyType;
   
   public class ButtonRewardAutosize extends Button
   {
      
      public var icon:MovieClip;
      
      public function ButtonRewardAutosize()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         textField.autoSize = TextFieldAutoSize.LEFT;
         preventAutosizing = true;
         if(constraints)
         {
            constraints.removeAllElements();
         }
      }
      
      override public function get width() : Number
      {
         return textField != null ? textField.x + textField.textWidth : super.width;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         this.updateContent();
         invalidateData();
      }
      
      protected function updateContent() : void
      {
         var useMultiplier:Boolean;
         if(_data.type == Reward.RATING)
         {
            this.icon.gotoAndStop(5);
            this.icon.visible = true;
         }
         else if(_data.type == Reward.MONEY)
         {
            this.icon.gotoAndStop(_data.param == MoneyType.REGULAR ? 1 : 2);
            this.icon.visible = true;
         }
         else if(_data.type == Reward.EXPERIENCE)
         {
            this.icon.gotoAndStop(4);
            this.icon.visible = true;
         }
         else if(_data.type == Reward.REPUTATION)
         {
            this.icon.gotoAndStop(5 + _data.param);
            this.icon.visible = true;
         }
         else if(_data.type == Reward.ITEM)
         {
            this.icon.visible = false;
         }
         else if(_data.type == Reward.POINT)
         {
            this.icon.gotoAndStop(11);
            this.icon.visible = true;
         }
         else
         {
            this.icon.visible = false;
         }
         if(_data.type == Reward.ITEM)
         {
            if(_data.amount != 1)
            {
               textField.text = String(_data.item_name) + " x" + String(_data.amount);
            }
            else
            {
               textField.text = String(_data.item_name);
            }
         }
         else
         {
            useMultiplier = _data.useMultiplier;
            textField.text = Util.getMoneyString(int(_data.amount * (useMultiplier ? (_data.type == Reward.REPUTATION ? PlayerInfo.questRewardsMultiplierReputation : PlayerInfo.questRewardsMultiplierMoney) : 1)));
         }
         textField.x = this.icon.getRect(this).x * 2 + this.icon.width + 3;
         trace("BRW::updateContent",textField.text,useMultiplier,_data.amount,"|",textField.x,"|",this.icon.getRect(this));
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA) && _data)
         {
            this.updateContent();
         }
      }
   }
}

