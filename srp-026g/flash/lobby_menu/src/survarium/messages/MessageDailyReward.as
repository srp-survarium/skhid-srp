package survarium.messages
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.geom.Rectangle;
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

      // How much narrower the window is without the reward panel (closeUpRight).
      private var _cut:Number = 0;

      public function MessageDailyReward()
      {
         super();
      }

      override public function get width() : Number
      {
         return this.MESSAGE_WIDTH - this._cut;
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
         this.closeUpRight();
      }

      // The reward panel's place on the right is empty (the owner, 2026-11-08: "7 day bonuses now has an
      // unnecessary space"): the window ends as far right of the last day as it begins left of the first.
      // The frame's pieces are the timeline's, unnamed: whatever lies wholly right of the days (the close
      // button) moves left by the cut, whatever reaches past them (the frame, the title bar, the text strip,
      // the centred texts) narrows by it. It never cuts into a text's line (the owner, 2026-11-09: "text is
      // now cut").
      private function closeUpRight() : void
      {
         this.rewardsList.validateNow();
         var days:Rectangle = this.rewardsList.getBounds(this);
         var frame:Rectangle = new Rectangle();
         var child:DisplayObject;
         var i:int;
         for(i = 0; i < numChildren; i++)
         {
            child = getChildAt(i);
            if(child.visible && child != this.rewardContainer)
            {
               frame = frame.union(child.getBounds(this));
            }
         }
         // The texts keep their whole line: the window ends no closer to them than to the days.
         var content:Number = days.right;
         for each(var text:TextField in [this.textFieldLabel,this.textFieldDescr])
         {
            var box0:Rectangle = text.getBounds(this);
            content = Math.max(content,box0.left + (box0.width + text.textWidth) / 2 + 4);
         }
         var margin:Number = days.left - frame.left;
         var cut:Number = Math.max(0,Math.floor(frame.right - content - margin));
         for(i = 0; cut > 0 && i < numChildren; i++)
         {
            child = getChildAt(i);
            if(child == this.rewardsList || child == this.rewardContainer)
            {
               continue;
            }
            var box:Rectangle = child.getBounds(this);
            if(box.left >= days.right)
            {
               child.x -= cut;
            }
            else if(box.right > days.right)
            {
               child.width -= cut;
            }
         }
         this._cut = cut;
      }
   }
}
