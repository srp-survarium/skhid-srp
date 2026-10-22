package survarium.buttons
{
   import flash.events.Event;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   
   public class ButtonNotificationPremLog extends ButtonNotificationLog
   {
      
      public var rewardBar:SrvButtonBarDynam;
      
      public function ButtonNotificationPremLog()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(ButtonEvent.CLICK,this.onClicked,false,0,true);
      }
      
      private function onClicked(event:Event) : void
      {
         trace("onClicked::",event.target);
         if(event.target == this)
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
         var rewards:Array;
         super.updateText();
         if(_data)
         {
            this.rewardBar.mouseEnabled = false;
            this.rewardBar.mouseChildren = false;
            this.rewardBar.spacing = 5;
            rewards = _data.reward as Array;
            trace("ButtonNotificationPremLog::",rewards);
            if(rewards)
            {
               this.rewardBar.visible = true;
               this.rewardBar.setMaxFit(true);
               this.rewardBar.dataProvider = new DataProvider(rewards);
               this.rewardBar.validateNow();
            }
            else
            {
               this.rewardBar.visible = false;
            }
            background.height = 80;
         }
      }
   }
}

