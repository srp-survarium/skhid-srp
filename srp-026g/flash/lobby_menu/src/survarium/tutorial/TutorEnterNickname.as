package survarium.tutorial
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.Message;
   import survarium.controls.TextInputValidate;
   import survarium.data.LocalizationData;
   
   public class TutorEnterNickname extends UIComponent
   {
      
      public var btnClose:Button;
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      public var textField1:TextField;
      
      public var textField2:TextField;
      
      public var textField3:TextField;
      
      public var plus1:MovieClip;
      
      public var plus2:MovieClip;
      
      public var rewardItem1:MovieClip;
      
      public var rewardItem2:MovieClip;
      
      public var rewardItem3:MovieClip;
      
      public var textInput:TextInputValidate;
      
      public var data:Object;
      
      public function TutorEnterNickname()
      {
         super();
      }
      
      public function handleNicknameValidation(param1:Boolean, param2:String, param3:String) : void
      {
         if(param2 == this.data.nickname)
         {
            param1 = true;
         }
         this.textInput.handleTextValidation(param1,param2,param3);
      }
      
      override protected function configUI() : void
      {
         var try_enter_name:Function;
         try_enter_name = function(param1:KeyboardEvent):void
         {
            param1.preventDefault();
            if(param1.keyCode == 13 && btnAccept.enabled)
            {
               nextTutorial(null);
            }
         };
         super.configUI();
         this.btnAccept.enabled = true;
         this.textInput.addEventListener(Event.CHANGE,this.onTextInputStateChanged);
         this.btnClose.addEventListener(ButtonEvent.CLICK,this.onExitClick);
         this.btnCancel.addEventListener(ButtonEvent.CLICK,this.onExitClick);
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.nextTutorial);
         this.textInput.setText(this.data.nickname);
         this.textInput.setRequest("request_nickname_validation");
         this.btnAccept.label = LocalizationData.getName("slide1_button_accept_label");
         this.btnCancel.label = LocalizationData.getName("button_skip_tutorial_label");
         this.textField1.text = LocalizationData.getName("slide1_text");
         this.textField2.text = LocalizationData.getName("slide1_text_enter_name");
         this.textField3.text = LocalizationData.getName("slide1_tutorial_reward");
         this.rewardItem1.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         this.rewardItem2.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         this.rewardItem3.itemSlot.tooltipRendererName = "SrvTextHintRendererTip";
         if(this.data.reward_1)
         {
            trace("TutorEnterNickname::",this.data.reward_1.icon);
         }
         this.rewardItem1.itemSlot.data = this.data.reward_1;
         this.rewardItem2.itemSlot.data = this.data.reward_2;
         this.rewardItem3.itemSlot.data = this.data.reward_3;
         this.rewardItem2.visible = this.data.reward_2 != null;
         this.rewardItem3.visible = this.data.reward_3 != null;
         this.plus1.visible = this.rewardItem2.visible;
         this.plus2.visible = this.rewardItem3.visible;
         addEventListener(Event.RENDER,this.onRedner);
         addEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         this.textInput.textInput.addEventListener(KeyboardEvent.KEY_DOWN,try_enter_name);
      }
      
      private function onRemoved(param1:Event) : void
      {
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         this.rewardItem1.itemSlot.data = null;
         this.rewardItem2.itemSlot.data = null;
         this.rewardItem3.itemSlot.data = null;
      }
      
      private function onRedner(param1:Event) : void
      {
         if(stage)
         {
            stage.focus = this.textInput.textInput;
            this.textInput.textInput.textField.setSelection(0,99);
         }
         removeEventListener(Event.RENDER,this.onRedner);
      }
      
      private function nextTutorial(param1:ButtonEvent) : void
      {
         var _loc2_:String = this.textInput.textInput.text;
         var _loc3_:uint = Math.pow(2,32) - 1;
         trace("change_player_nickname",_loc2_,_loc3_,this.data.nickname);
         if(_loc2_ != this.data.nickname)
         {
            ExternalInterface.call("change_player_nickname",_loc2_,_loc3_);
         }
         ExternalInterface.call("tutorial_nickname_ok_clicked");
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      private function lockWindow() : void
      {
         this.textInput.textInput.enabled = false;
         this.btnAccept.enabled = false;
         this.btnCancel.enabled = false;
      }
      
      private function onExitClick(param1:ButtonEvent) : void
      {
         var _loc2_:Object = new Object();
         _loc2_.text = LocalizationData.getName("tutorial_exit_warning");
         _loc2_.onYes = this.skipTutorial;
         Message.showMessage("apply",_loc2_,true);
      }
      
      private function skipTutorial(param1:* = null) : void
      {
         trace("skip_tutorial");
         ExternalInterface.call("skip_tutorial");
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      private function onTextInputStateChanged(param1:Event) : void
      {
         this.btnAccept.enabled = this.textInput.enabled;
      }
   }
}

