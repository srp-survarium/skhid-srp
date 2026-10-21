package survarium.tutorial
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.buttons.ButtonBlink;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   import survarium.messages.MessageBase;
   
   public class NoteSlides extends MessageBase
   {
      
      public var btnPrevious:Button;
      
      public var btnNext:ButtonBlink;
      
      public var btnClose:Button;
      
      public var notesIndex:uint;
      
      public var notesLabel:String;
      
      public var textFieldLabel:TextField;
      
      public var textField1:TextField;
      
      public var textField2:TextField;
      
      public var textField3:TextField;
      
      public var container:MovieClip;
      
      private var _readed:Boolean = false;
      
      public function NoteSlides()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         trace("NoteSlides",this.btnPrevious,this.btnNext,this.btnClose);
         this.btnPrevious.addEventListener(ButtonEvent.CLICK,this.prevSlide);
         this.btnNext.addEventListener(ButtonEvent.CLICK,this.nextSlide);
         this.btnNext.validateNow();
         this.btnClose.addEventListener(ButtonEvent.CLICK,this.closeTutor);
         this.btnClose.visible = false;
         this.container.gotoAndStop(1);
         if(_data)
         {
            notesLabel = _data.label;
            notesIndex = _data.index;
         }
         this.textFieldLabel.mouseEnabled = false;
         this.updateButtons();
         this.updateSlideData();
      }
      
      private function closeTutor(event:ButtonEvent = null) : void
      {
         Message.onClick(_id);
      }
      
      private function prevSlide(event:ButtonEvent) : void
      {
         trace("prevSlide");
         this.container.gotoAndStop(this.container.currentFrame - 1);
         this.updateButtons();
         this.updateSlideData();
      }
      
      private function updateSlideData() : void
      {
         trace("updateSlideData",this.container.currentFrame);
         var frame:uint = this.container.currentFrame;
         this.textFieldLabel.text = LocalizationData.getName("survival_notes") + ": " + LocalizationData.getName(this.notesLabel) + " " + String(frame.toString() + "/" + this.container.totalFrames.toString());
         textField1 = this.container.getChildByName("text_field_1") as TextField;
         textField2 = this.container.getChildByName("text_field_2") as TextField;
         textField3 = this.container.getChildByName("text_field_3") as TextField;
         if(this.textField1)
         {
            this.textField1.text = LocalizationData.getName(this.notesLabel + "_frame_" + frame + "_text_1").toUpperCase();
         }
         if(this.textField2)
         {
            this.textField2.text = LocalizationData.getName(this.notesLabel + "_frame_" + frame + "_text_2").toUpperCase();
         }
         if(this.textField3)
         {
            this.textField3.text = LocalizationData.getName(this.notesLabel + "_frame_" + frame + "_text_3").toUpperCase();
         }
      }
      
      private function onRewardRemoved(event:Event) : void
      {
         trace("onRewardRemoved");
         event.target.removeEventListener(Event.REMOVED_FROM_STAGE,this.onRewardRemoved);
         event.target.itemSlot.data = null;
      }
      
      private function updateButtons() : void
      {
         this.btnPrevious.enabled = this.container.currentFrame > 1;
         this.btnNext.enabled = true;
         if(this.container.currentFrame == 1)
         {
            this.btnNext.blink = true;
         }
      }
      
      private function nextSlide(event:ButtonEvent) : void
      {
         var rewards:Array;
         var messageData:Object;
         trace("nextSlide");
         if(this.container.currentFrame == this.container.totalFrames)
         {
            if(_data.showRewards)
            {
               rewards = _data.rewards;
               messageData = {
                  label:(LocalizationData.getName("survival_notes_complete") + "\n" + "\"" + LocalizationData.getName("survival_notes") + ", " + LocalizationData.getName(this.notesLabel)).toUpperCase() + "\"",
                  reward_1:(rewards && rewards[0] ? rewards[0].data : null),
                  reward_2:(rewards && rewards[1] ? rewards[1].data : null),
                  reward_3:(rewards && rewards[2] ? rewards[2].data : null),
                  notesIndex:this.notesIndex
               };
               Message.showMessage("tutorial_complete",messageData);
               this.closeTutor();
            }
            else
            {
               if(!this._readed)
               {
                  _readed = true;
                  trace("survival_notes_readed",this.notesIndex,this.container.currentFrame,this.container.totalFrames);
                  ExternalInterface.call("survival_notes_readed",this.notesIndex);
               }
               this.closeTutor();
            }
         }
         this.container.gotoAndStop(this.container.currentFrame + 1);
         this.updateButtons();
         this.updateSlideData();
      }
   }
}

