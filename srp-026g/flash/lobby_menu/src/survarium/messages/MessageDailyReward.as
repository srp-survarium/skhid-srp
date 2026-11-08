package survarium.messages
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import survarium.Util;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;

   // The daily bonus window: the run's days and their progress. The run's 7th-day reward is gone (the
   // owner, 2026-11-08; LobbyMenu.show_daily_reward_window), so its panel (rewardContainer) is hidden and
   // the window has no reward card or hint.
   public class MessageDailyReward extends MessageInfoBase
   {

      public var btnClose:Button;

      public var rewardContainer:MovieClip;

      public var textFieldLabel:TextField;

      public var textFieldDescr:TextField;

      public var rewardsList:ButtonBar;

      protected const MESSAGE_WIDTH:uint = 604;

      public function MessageDailyReward()
      {
         super();
      }

      override public function get width() : Number
      {
         return this.MESSAGE_WIDTH;
      }

      override public function set width(value:Number) : void
      {
         super.width = value;
      }

      override protected function configUI() : void
      {
         super.configUI();
         this.btnClose.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         this.rewardsList.y -= 1;
         this.rewardsList.focusable = false;
         this.rewardsList.spacing = 0;
         this.rewardContainer.visible = false;
         this.rewardsList.dataProvider = new DataProvider(_data.data as Array);
         this.rewardsList.selectedIndex = _data.current_day - 1;
         this.textFieldLabel.styleSheet = MainScene.textStyleSheet;
         this.textFieldDescr.styleSheet = MainScene.textStyleSheet;
         this.textFieldLabel.text = LocalizationData.getName("daily_bonus_label") + ", " + Util.styledChar(PlayerInfo.nickname,"beige_bright") + "!";
         this.textFieldDescr.text = LocalizationData.getName("daily_bonus_description");
      }
   }
}
