package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   import survarium.data.LocalizationData;
   
   public class MessageTutorialComplete extends MessageAccept
   {
      
      public var textFieldLabel:TextField;
      
      public var textFieldReward:TextField;
      
      public var plus1:MovieClip;
      
      public var plus2:MovieClip;
      
      public var background:MovieClip;
      
      public var rewardItem1:MovieClip;
      
      public var rewardItem2:MovieClip;
      
      public var rewardItem3:MovieClip;
      
      public function MessageTutorialComplete() : void
      {
         super();
      }
      
      override public function setMessageData(id:int, data:Object) : void
      {
         trace("setMessageData::",data,this.textFieldLabel,this.textFieldReward);
         super.setMessageData(id,data);
         if(!data)
         {
            return;
         }
         trace("MessageTutorialComplete",data.reward_1);
         this.textFieldLabel.text = data.label;
         this.textFieldReward.text = LocalizationData.getName("tutorial_complete_reward_label");
         this.rewardItem1.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         this.rewardItem2.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         this.rewardItem3.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         if(data.reward_1)
         {
            trace("MessageTutorialCompleteReward::",data.reward_1.icon);
         }
         this.rewardItem1.itemSlot.data = data.reward_1;
         this.rewardItem2.itemSlot.data = data.reward_2;
         this.rewardItem3.itemSlot.data = data.reward_3;
         var num:int = 1;
         this.rewardItem2.visible = data.reward_2 != null;
         if(this.rewardItem2.visible)
         {
            num += 1;
         }
         this.rewardItem3.visible = data.reward_3 != null;
         if(this.rewardItem3.visible)
         {
            num += 1;
         }
         this.plus1.visible = this.rewardItem2.visible;
         this.plus2.visible = this.rewardItem3.visible;
         var step:int = 105;
         var pos:int = 24 + (this.background.width - 24 * 2 - step * num + 24) / 2;
         this.rewardItem1.x = pos;
         pos += step;
         if(this.rewardItem2.visible)
         {
            this.rewardItem2.x = pos;
            pos += step;
            this.plus1.x = this.rewardItem1.x + 88;
         }
         if(this.rewardItem3.visible)
         {
            this.rewardItem3.x = pos;
            this.plus2.x = this.rewardItem2.x + 88;
         }
      }
      
      override protected function cleanUp(param0:Event = null) : void
      {
         super.cleanUp(param0);
         this.rewardItem1.itemSlot.data = null;
         this.rewardItem2.itemSlot.data = null;
         this.rewardItem3.itemSlot.data = null;
      }
      
      private function hideMe(event:ButtonEvent) : void
      {
         super.onMessageCancel();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addButton(btnAccept,"btn_end_tutorial_label");
         LocalizationData.addButton(btnCancel,"btn_repeat_tutorial_label");
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         trace("repeat_tutorial");
         ExternalInterface.call("repeat_tutorial");
         super.onMessageCancel();
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         trace("end_tutorial");
         if(_data.notesIndex != undefined)
         {
            trace("survival_notes_readed",_data.notesIndex);
            ExternalInterface.call("survival_notes_readed",_data.notesIndex);
         }
         else if(stage)
         {
            stage.dispatchEvent(new Event(Events.TUTORIAL_COMPLETE));
         }
         ExternalInterface.call("end_tutorial");
         super.onMessageAccept();
      }
   }
}

