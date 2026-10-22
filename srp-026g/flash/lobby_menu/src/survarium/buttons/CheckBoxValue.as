package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import survarium.controls.UnfocusedCheckBox;
   import survarium.constants.MoneyType;
   
   public class CheckBoxValue extends UnfocusedCheckBox
   {
      
      // money_type values beyond MoneyType.REGULAR and PREMIUM: the value icon's frame (money_type + 1) for a
      // rename voucher (MessageEnterName) and a repair voucher (MessageRepair).
      public static const RENAME_VOUCHER_ICON:int = 9;
      
      public static const REPAIR_VOUCHER_ICON:int = 10;
      
      public var textFieldValue:TextField;
      
      public var icon:MovieClip;
      
      protected var _value:Number;
      
      protected var _money_type:Number;
      
      public function CheckBoxValue()
      {
         super();
      }
      
      public function set value(newValue:Number) : void
      {
         if(this._value == newValue)
         {
            return;
         }
         _value = newValue;
         invalidateData();
      }
      
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set money_type(value:Number) : void
      {
         _money_type = value;
         this.icon.gotoAndStop(value + 1);
         invalidateData();
      }
      
      override public function get toggle() : Boolean
      {
         return _toggle;
      }
      
      override public function set toggle(value:Boolean) : void
      {
         _toggle = value;
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
            this.textFieldValue.x = this.icon.x + this.icon.width + 5 - (this._money_type != MoneyType.REGULAR ? (this._money_type == MoneyType.PREMIUM ? 18 : 10) : 0);
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

