package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import scaleform.clik.constants.InvalidationType;
   
   public class TabToggleButton extends ButtonClick
   {
      
      public var container:Sprite;
      
      public var icon:MovieClip;
      
      protected var _iconY:int = 0;
      
      public function TabToggleButton()
      {
         super();
         preventAutosizing = true;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         constraintsDisabled = true;
         preventAutosizing = true;
         constraints.removeAllElements();
         if(this.icon)
         {
            _iconY = this.icon.y;
         }
      }
      
      override public function set data(value:Object) : void
      {
         super.data = value;
         invalidateData();
      }
      
      override protected function changeFocus() : void
      {
         var frameLabel:String;
         if(!enabled)
         {
            return;
         }
         if(_focusIndicator == null)
         {
            setState(_mouseOver ? "over" : "out");
            if(_pressedByKeyboard && !_focused)
            {
               _pressedByKeyboard = false;
            }
         }
         else
         {
            if(_focusIndicator.totalframes == 1)
            {
               _focusIndicator.visible = _focused > 0;
            }
            else
            {
               frameLabel = "state" + _focused;
               if(_focusIndicatorLabelHash[frameLabel])
               {
                  _newFocusIndicatorFrame = "state" + _focused;
               }
               else
               {
                  _newFocusIndicatorFrame = _focused || _displayFocus ? "show" : "hide";
               }
               invalidateState();
            }
            if(_pressedByKeyboard && !_focused)
            {
               setState("kb_release");
               _pressedByKeyboard = false;
            }
         }
      }
      
      override protected function draw() : void
      {
         if(_data && isInvalid(InvalidationType.DATA) && _data.enabled != undefined)
         {
            enabled = _data.enabled;
         }
         super.draw();
      }
      
      protected function getLabel() : String
      {
         return _data.index == undefined ? String(_data.name).toUpperCase() : String(_data.name);
      }
      
      override protected function updateText() : void
      {
         if(!_data)
         {
            return;
         }
         trace("TTB::updateText",textField,this.icon);
         var newLabel:String = this.getLabel();
         var tint:ColorTransform;
         if(textField.text != newLabel)
         {
            textField.text = newLabel;
         }
         this.icon.gotoAndStop(_data.icon);
         this.icon.y = _state == "down" ? this._iconY + 1 : this._iconY;
         if(!enabled)
         {
            this.icon.transform.colorTransform = null;
            this.icon.alpha = 0.7;
         }
         else if(_state == "down" || selected)
         {
            this.icon.alpha = 1;
            tint = new ColorTransform(2.4,2.4,2.4);
            this.icon.transform.colorTransform = tint;
         }
         else if(_state == "over")
         {
            this.icon.alpha = 1;
            tint = new ColorTransform(2.4,2.4,2.4);
            this.icon.transform.colorTransform = tint;
         }
         else
         {
            tint = new ColorTransform(2,2,2);
            this.icon.transform.colorTransform = tint;
            this.icon.alpha = 0.7;
         }
      }
   }
}

