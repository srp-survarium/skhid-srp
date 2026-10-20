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
         var _loc1_:Array;
         var _loc2_:uint;
         super.updateText();
         if(_data)
         {
            this.questIcon.gotoAndStop(_data.icon);
            this.rewardField.text = LocalizationData.getName("reward");
            this.rewardBar.spacing = 5;
            _loc1_ = _data.reward as Array;
            if(_loc1_)
            {
               _loc2_ = 0;
               while(_loc2_ < _loc1_.length)
               {
                  _loc1_[_loc2_].useMultiplier = _data.useMultiplier != undefined ? _data.useMultiplier : true;
                  _loc2_++;
               }
               this.rewardField.y = textField.y + textField.textHeight + 5;
               this.rewardBar.y = this.rewardField.y + 1;
               this.rewardBar.dataProvider = new DataProvider(_loc1_);
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

