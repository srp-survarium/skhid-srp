package survarium.buttons
{
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.LabelCost;
   import survarium.data.LocalizationData;
   
   public class ButtonNotificationInvited extends ButtonNotificationLog
   {
      
      public var labelCost:LabelCost;
      
      public var textFieldReward:TextField;
      
      public function ButtonNotificationInvited()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(ButtonEvent.CLICK,this.onClicked,false,0,true);
      }
      
      private function onClicked(param1:Event) : void
      {
         trace("onClicked::",param1.target);
         if(param1.target == this)
         {
            if(_ignoreClick == true)
            {
               _ignoreClick = false;
            }
            else
            {
               dispatchEvent(new Event("showMessage"));
            }
         }
         else
         {
            _ignoreClick = true;
         }
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         if(_data)
         {
            this.labelCost.mouseEnabled = this.textFieldReward.mouseEnabled = false;
            this.textFieldReward.text = LocalizationData.getName("reward");
            this.labelCost.x = this.textFieldReward.x + this.textFieldReward.textWidth + 9;
            this.labelCost.moneyType = _data.reward_type;
            this.labelCost.cost = _data.reward;
            background.height = 79;
         }
      }
   }
}

