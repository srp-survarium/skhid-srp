package survarium.controls
{
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.text.TextFieldType;
   import scaleform.clik.controls.NumericStepper;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   
   public class NumericStepperSrv extends NumericStepper
   {
      
      private var _editable:Boolean = false;
      
      public function NumericStepperSrv()
      {
         super();
      }
      
      override public function set maximum(param1:Number) : void
      {
         _maximum = param1;
      }
      
      [Inspectable(defaultValue="false")]
      public function get editable() : Boolean
      {
         return this._editable;
      }
      
      public function set editable(param1:Boolean) : void
      {
         if(param1 == this._editable)
         {
            return;
         }
         _editable = param1;
         textField.type = param1 ? TextFieldType.INPUT : TextFieldType.DYNAMIC;
         textField.tabEnabled = textField.mouseEnabled = param1;
         trace("editable",this._editable);
         if(param1)
         {
            textField.maxChars = 4;
            textField.restrict = "0-9";
            textField.addEventListener(Event.CHANGE,this.onTextInput,false,0,true);
            textField.addEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut,false,0,true);
         }
         else
         {
            textField.removeEventListener(Event.CHANGE,this.onTextInput);
            textField.removeEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut);
         }
      }
      
      override public function set value(param1:Number) : void
      {
         param1 = this.lockValue(param1);
         if(param1 == _value)
         {
            return;
         }
         var _loc2_:Number = _value;
         _value = param1;
         if(initialized)
         {
            dispatchEventAndSound(new IndexEvent(IndexEvent.INDEX_CHANGE,true,false,value,_loc2_,null));
         }
         invalidate();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         textField.tabEnabled = textField.mouseEnabled = this._editable;
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(textField.selectionBeginIndex != 0 && textField.selectionEndIndex != 0)
         {
            return;
         }
         super.handleInput(param1);
      }
      
      override protected function updateAfterStateChange() : void
      {
         super.updateAfterStateChange();
         textField.tabEnabled = textField.mouseEnabled = this._editable;
         if(this._editable)
         {
            textField.maxChars = 4;
            textField.restrict = "0-9";
            if(!textField.hasEventListener(Event.CHANGE))
            {
               textField.addEventListener(Event.CHANGE,this.onTextInput,false,0,true);
               textField.addEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut,false,0,true);
            }
         }
         else if(textField.hasEventListener(Event.CHANGE))
         {
            textField.removeEventListener(Event.CHANGE,this.onTextInput);
            textField.removeEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut);
         }
      }
      
      protected function onTextFocusOut(param1:Event) : void
      {
         this.updateLabel();
      }
      
      protected function onTextInput(param1:Event) : void
      {
         value = int(textField.text);
      }
      
      override protected function updateLabel() : void
      {
         if(textField.selectionBeginIndex != 0 && textField.selectionEndIndex != 0)
         {
            return;
         }
         var _loc1_:String = _value.toString();
         if(_labelFunction != null)
         {
            _loc1_ = _labelFunction(_value);
         }
         textField.text = _loc1_;
      }
      
      protected function checkInputKeyForSubmit(param1:KeyboardEvent) : void
      {
         if(param1.keyCode == 13)
         {
            param1.stopImmediatePropagation();
            value = int(textField.text);
         }
      }
      
      override protected function lockValue(param1:Number) : Number
      {
         var _loc2_:Number = Math.max(_minimum,Math.min(_maximum,stepSize * Math.round(param1 / stepSize)));
         return _loc2_;
      }
   }
}

