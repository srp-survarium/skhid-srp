package survarium.tutorial
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import survarium.constants.Events;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   
   public class TextSlide extends UIComponent
   {
      
      public var textField:TextField;
      
      public var textField2:TextField;
      
      public var closeBtn:Button;
      
      public var background:MovieClip;
      
      public function TextSlide()
      {
         super();
      }
      
      override public function get height() : Number
      {
         return this.background.height;
      }
      
      public function setText(mainText:String, targetText:String) : void
      {
         this.textField.autoSize = TextFieldAutoSize.LEFT;
         this.textField2.autoSize = TextFieldAutoSize.LEFT;
         this.textField.styleSheet = MainScene.textStyleSheet;
         this.textField.text = mainText;
         this.textField2.text = targetText;
         this.textField2.y = this.textField.y + this.textField.textHeight + 19;
         var offset:int = this.closeBtn.visible ? 19 : -10;
         if(targetText)
         {
            this.closeBtn.y = this.textField2.y + this.textField2.textHeight + offset;
         }
         else
         {
            this.closeBtn.y = this.textField.y + this.textField.textHeight + offset;
         }
         trace("TS::setText",offset,this.closeBtn.y,mainText,targetText);
         this.background.height = this.closeBtn.y + this.closeBtn.height + 13;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.closeBtn.addEventListener(ButtonEvent.CLICK,this.onExitClick);
         this.closeBtn.addEventListener(ComponentEvent.STATE_CHANGE,this.onExitState,false,0,true);
         this.closeBtn.label = LocalizationData.getName("button_skip_tutorial_label");
      }
      
      private function onExitState(e:ComponentEvent) : void
      {
         this.closeBtn.alpha = this.closeBtn.state == "over" ? 1 : 0.5;
      }
      
      private function onExitClick(e:ButtonEvent) : void
      {
         var _obj:Object = new Object();
         _obj.text = LocalizationData.getName("tutorial_exit_warning");
         _obj.onYes = this.onSkipTutor;
         Message.showMessage("apply",_obj,true);
      }
      
      private function onSkipTutor(e:* = null) : void
      {
         ExternalInterface.call("skip_tutorial");
         trace("skip_tutorial",stage);
         if(stage)
         {
            stage.dispatchEvent(new Event(Events.TUTORIAL_COMPLETE));
         }
         dispatchEvent(new Event(Event.CLOSE));
      }
   }
}

