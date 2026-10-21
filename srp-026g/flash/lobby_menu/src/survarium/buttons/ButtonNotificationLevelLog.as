package survarium.buttons
{
   import flash.text.TextField;
   import scaleform.clik.data.DataProvider;
   import survarium.data.LocalizationData;
   
   public class ButtonNotificationLevelLog extends ButtonNotificationLog
   {
      
      public var rewardField:TextField;
      
      public var rewardBar:SrvButtonBarDynam;
      
      public function ButtonNotificationLevelLog()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         var rewards:Array;
         super.updateText();
         if(_data)
         {
            this.rewardField.text = LocalizationData.getName("reward");
            this.rewardBar.spacing = 5;
            rewards = _data.reward as Array;
            if(rewards)
            {
               this.rewardField.y = textField.y + textField.textHeight + 5;
               this.rewardBar.y = this.rewardField.y + 1;
               this.rewardBar.dataProvider = new DataProvider(rewards);
               this.rewardBar.validateNow();
            }
            else
            {
               this.rewardBar.visible = false;
               this.rewardField.visible = false;
               this.rewardBar.y = this.rewardField.y = 0;
            }
            background.height = 70;
         }
      }
   }
}

