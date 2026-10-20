package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import survarium.constants.Events;
   
   public class ButtonStyledLabelProfile extends ButtonStyledLabel
   {
      
      public var iconDisabled:MovieClip;
      
      public var textFieldLevel:TextField;
      
      public var btnChangeName:Button;
      
      public function ButtonStyledLabelProfile()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraintsDisabled && constraints)
         {
            constraints.addElement("iconDisabled",this.iconDisabled,Constraints.CENTER_V);
            constraints.addElement("textFieldLevel",this.textFieldLevel,Constraints.LEFT);
            constraints.addElement("btnChangeName",this.btnChangeName,Constraints.RIGHT);
         }
         this.textFieldLevel.styleSheet = MainScene.textStyleSheet;
         this.btnChangeName.addEventListener(ButtonEvent.CLICK,this.onRenameClick);
         tooltipRendererName = "SrvTextHintRendererTip";
         tipEnabled = true;
      }
      
      private function onRenameClick(param1:ButtonEvent) : void
      {
         trace("onRenameClick");
         dispatchEvent(new Event(Events.RENAME,true));
      }
      
      override protected function updateAfterStateChange() : void
      {
         super.updateAfterStateChange();
         if(!initialized)
         {
            return;
         }
         if(constraints != null && !constraintsDisabled && this.iconDisabled != null)
         {
            constraints.updateElement("iconDisabled",this.iconDisabled);
            constraints.updateElement("textFieldLevel",this.textFieldLevel);
            constraints.updateElement("btnChangeName",this.btnChangeName);
         }
         mouseChildren = true;
         textField.mouseEnabled = false;
         this.iconDisabled.mouseEnabled = false;
         this.textFieldLevel.mouseEnabled = false;
      }
      
      override protected function updateText() : void
      {
         var _loc1_:int;
         super.updateText();
         if(_data)
         {
            trace("ButtonStyledLabelProfile",textField,_data.levelLabel,_data.label,_data.tipText,tipEnabled,_tooltipRenderer);
         }
         this.btnChangeName.alpha = state == "over" ? 1 : 0.5;
         this.btnChangeName.visible = enabled && _data && _data.enabled == undefined && _data.editable;
         if(_data && this.textFieldLevel)
         {
            this.textFieldLevel.text = _data.levelLabel;
         }
         if(this.iconDisabled)
         {
            this.iconDisabled.visible = !enabled;
            if(this.iconDisabled.visible)
            {
               _loc1_ = (textField.width * scaleX - textField.textWidth) / 2;
               this.iconDisabled.x = 1 / scaleX * (_loc1_ - this.iconDisabled.width);
            }
         }
      }
   }
}

