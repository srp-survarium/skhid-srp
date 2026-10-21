package survarium.controls
{
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.text.TextFieldType;
   import flash.ui.Keyboard;
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
      
      override public function set maximum(value:Number) : void
      {
         _maximum = value;
      }
      
      public function get editable() : Boolean
      {
         return this._editable;
      }
      
      public function set editable(value:Boolean) : void
      {
         if(value == this._editable)
         {
            return;
         }
         _editable = value;
         textField.type = value ? TextFieldType.INPUT : TextFieldType.DYNAMIC;
         textField.tabEnabled = textField.mouseEnabled = value;
         trace("editable",this._editable);
         if(value)
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
      
      override public function set value(v:Number) : void
      {
         v = this.lockValue(v);
         if(v == _value)
         {
            return;
         }
         var previousValue:Number = _value;
         _value = v;
         if(initialized)
         {
            dispatchEventAndSound(new IndexEvent(IndexEvent.INDEX_CHANGE,true,false,value,previousValue,null));
         }
         invalidate();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         textField.tabEnabled = textField.mouseEnabled = this._editable;
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(textField.selectionBeginIndex != 0 && textField.selectionEndIndex != 0)
         {
            return;
         }
         super.handleInput(event);
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
      
      protected function onTextFocusOut(e:Event) : void
      {
         this.updateLabel();
      }
      
      protected function onTextInput(e:Event) : void
      {
         value = int(textField.text);
      }
      
      override protected function updateLabel() : void
      {
         if(textField.selectionBeginIndex != 0 && textField.selectionEndIndex != 0)
         {
            return;
         }
         var label:String = _value.toString();
         if(_labelFunction != null)
         {
            label = _labelFunction(_value);
         }
         textField.text = label;
      }
      
      protected function checkInputKeyForSubmit(event:KeyboardEvent) : void
      {
         if(event.keyCode == Keyboard.ENTER)
         {
            event.stopImmediatePropagation();
            value = int(textField.text);
         }
      }
      
      override protected function lockValue(value:Number) : Number
      {
         var newVal:Number = Math.max(_minimum,Math.min(_maximum,stepSize * Math.round(value / stepSize)));
         return newVal;
      }
   }
}

