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
      
      override public function set data(param1:Object) : void
      {
         super.data = param1;
         invalidateData();
      }
      
      override protected function changeFocus() : void
      {
         var _loc1_:String;
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
               _loc1_ = "state" + _focused;
               if(_focusIndicatorLabelHash[_loc1_])
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
         var _loc1_:String = this.getLabel();
         var _loc2_:ColorTransform;
         if(textField.text != _loc1_)
         {
            textField.text = _loc1_;
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
            _loc2_ = new ColorTransform(2.4,2.4,2.4);
            this.icon.transform.colorTransform = _loc2_;
         }
         else if(_state == "over")
         {
            this.icon.alpha = 1;
            _loc2_ = new ColorTransform(2.4,2.4,2.4);
            this.icon.transform.colorTransform = _loc2_;
         }
         else
         {
            _loc2_ = new ColorTransform(2,2,2);
            this.icon.transform.colorTransform = _loc2_;
            this.icon.alpha = 0.7;
         }
      }
   }
}

