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
            this.rewardBar.mouseEnabled = false;
            this.rewardBar.mouseChildren = false;
            this.rewardBar.spacing = 5;
            _loc1_ = _data.reward as Array;
            trace("ButtonNotificationPremLog::",_loc1_);
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
            background.height = 80;
         }
      }
   }
}

