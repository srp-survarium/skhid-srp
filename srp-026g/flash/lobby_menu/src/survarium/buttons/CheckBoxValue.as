package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import survarium.controls.UnfocusedCheckBox;
   
   public class CheckBoxValue extends UnfocusedCheckBox
   {
      
      public var textFieldValue:TextField;
      
      public var icon:MovieClip;
      
      protected var _value:Number;
      
      protected var _money_type:Number;
      
      public function CheckBoxValue()
      {
         super();
      }
      
      public function set value(param1:Number) : void
      {
         if(this._value == param1)
         {
            return;
         }
         _value = param1;
         invalidateData();
      }
      
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set money_type(param1:Number) : void
      {
         _money_type = param1;
         this.icon.gotoAndStop(param1 + 1);
         invalidateData();
      }
      
      override public function get toggle() : Boolean
      {
         return _toggle;
      }
      
      override public function set toggle(param1:Boolean) : void
      {
         _toggle = param1;
      }
      
      override protected function updateText() : void
      {
         if(_data)
         {
            _value = _data.value;
            money_type = _data.money_type;
         }
         super.updateText();
         if(this.textFieldValue)
         {
            this.textFieldValue.text = this._value.toString();
         }
         if(this.textFieldValue && this.icon)
         {
            this.textFieldValue.x = this.icon.x + this.icon.width + 5 - (this._money_type != 0 ? (this._money_type == 1 ? 18 : 10) : 0);
         }
      }
      
      public function get money_type() : Number
      {
         return this._money_type;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(constraints)
         {
            constraints.removeAllElements();
         }
         constraintsDisabled = true;
         preventAutosizing = true;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.DATA) && _data)
         {
            enabled = _data.enabled;
         }
         super.draw();
      }
   }
}

