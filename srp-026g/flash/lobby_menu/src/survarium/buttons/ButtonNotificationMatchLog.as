package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Colors;
   import survarium.data.LocalizationData;
   
   public class ButtonNotificationMatchLog extends ButtonNotificationLog
   {
      
      public var line:MovieClip;
      
      public var rewardBar:SrvButtonBarDynam;
      
      public var textFieldTime:TextField;
      
      public var textFieldResult:TextField;
      
      public var textFieldLabel:TextField;
      
      public function ButtonNotificationMatchLog()
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
         var _loc1_:Array;
         super.updateText();
         if(_data)
         {
            this.rewardBar.mouseEnabled = this.line.mouseEnabled = false;
            this.rewardBar.mouseChildren = false;
            this.rewardBar.spacing = 5;
            textField.text = LocalizationData.getName(_data.match_result > 2 ? "victory" : "defeat");
            textField.textColor = _data.match_result > 2 ? Colors.GREEN : Colors.RED;
            this.textFieldLabel.text = "\"" + _data.map_name + "\"";
            this.textFieldTime.text = _data.time;
            this.textFieldResult.text = _data.result;
            _loc1_ = _data.reward as Array;
            trace("ButtonNotificationMatchLog::",_data.map_name,_loc1_);
            if(_loc1_)
            {
               this.rewardBar.visible = true;
               this.rewardBar.setMaxFit(true);
               this.rewardBar.dataProvider = new DataProvider(_loc1_);
               this.rewardBar.validateNow();
            }
            else
            {
               this.rewardBar.visible = false;
            }
            background.height = 79;
         }
      }
   }
}

