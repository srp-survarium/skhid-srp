package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.data.DataProvider;
   import survarium.data.LocalizationData;
   
   public class ButtonNotificationQuestLog extends ButtonNotificationLog
   {
      
      public var questIcon:MovieClip;
      
      public var line:MovieClip;
      
      public var rewardField:TextField;
      
      public var rewardBar:SrvButtonBarDynam;
      
      public function ButtonNotificationQuestLog()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         var rewards:Array;
         var i:uint;
         super.updateText();
         if(_data)
         {
            this.questIcon.gotoAndStop(_data.icon);
            this.rewardField.text = LocalizationData.getName("reward");
            this.rewardBar.spacing = 5;
            rewards = _data.reward as Array;
            if(rewards)
            {
               i = 0;
               while(i < rewards.length)
               {
                  rewards[i].useMultiplier = _data.useMultiplier != undefined ? _data.useMultiplier : true;
                  i++;
               }
               this.rewardField.y = textField.y + textField.textHeight + 5;
               this.rewardBar.y = this.rewardField.y + 1;
               this.rewardBar.dataProvider = new DataProvider(rewards);
               this.rewardBar.validateNow();
               background.height = this.rewardBar.y + 40;
            }
            else
            {
               this.rewardBar.visible = false;
               this.rewardField.visible = false;
               this.rewardBar.y = this.rewardField.y = 0;
            }
            this.line.height = background.height - 2 * this.line.y;
            this.questIcon.y = (background.height - this.questIcon.height) / 2;
         }
      }
   }
}

