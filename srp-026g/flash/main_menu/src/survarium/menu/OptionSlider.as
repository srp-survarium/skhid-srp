package survarium.menu
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.Slider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.SliderEvent;
   
   public class OptionSlider extends MovieClip
   {
      
      public var textField:TextField;
      
      public var textFieldValue:TextField;
      
      public var slider:Slider;
      
      public var checkBox:Button;
      
      protected var _defaultValue:Number = 1;
      
      protected var _editable:Boolean = false;
      
      public function OptionSlider()
      {
         super();
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onSliderChange);
         this.slider.thumb.addEventListener(ButtonEvent.CLICK,this.onSliderThumbClick);
         this.textFieldValue.doubleClickEnabled = true;
         this.textFieldValue.addEventListener(MouseEvent.DOUBLE_CLICK,this.resetScreenAdjust,false,0,true);
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
         addEventListener(Event.RENDER,this.onRender);
      }
      
      private function onRender(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.onRender);
         this.textFieldValue.type = this._editable ? TextFieldType.INPUT : TextFieldType.DYNAMIC;
         trace("editable::",this._editable,this.textFieldValue.type);
         this.textFieldValue.selectable = this.textFieldValue.tabEnabled = this.textFieldValue.mouseEnabled = this._editable;
         if(this._editable)
         {
            this.textFieldValue.maxChars = 4;
            this.textFieldValue.restrict = "0-9 .";
            this.textFieldValue.addEventListener(Event.CHANGE,this.onTextInput,false,0,true);
            this.textFieldValue.addEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut,false,0,true);
         }
         else
         {
            this.textFieldValue.removeEventListener(Event.CHANGE,this.onTextInput);
            this.textFieldValue.removeEventListener(FocusEvent.FOCUS_OUT,this.onTextFocusOut);
         }
      }
      
      protected function onTextFocusOut(param1:Event) : void
      {
         trace("onTextFocusOut",this.slider.position);
         selectedIndex = this.slider.position;
      }
      
      protected function onTextInput(param1:Event) : void
      {
         trace("onTextInput|",this.textFieldValue.text,"|",Number(this.textFieldValue.text));
         var _loc2_:Number = Math.round(Number(this.textFieldValue.text) / 0.01) * 0.01;
         _loc2_ = _loc2_ < this.slider.maximum ? _loc2_ : this.slider.maximum;
         _loc2_ = _loc2_ > this.slider.minimum ? _loc2_ : this.slider.minimum;
         this.slider.position = _loc2_;
         this.slider.invalidateData();
      }
      
      private function onSliderThumbClick(param1:ButtonEvent) : void
      {
         trace("thumbClicked",this.slider.thumb,this.textFieldValue.type,this.textFieldValue.selectable,this.textFieldValue.mouseEnabled);
      }
      
      protected function resetScreenAdjust(param1:Event) : void
      {
         if(this.slider.value != this._defaultValue)
         {
            this.slider.value = this._defaultValue;
         }
      }
      
      public function set defaultValue(param1:Number) : void
      {
         _defaultValue = Math.round(param1 / 0.01) * 0.01;
      }
      
      public function get selectedIndex() : Number
      {
         return this.slider.position;
      }
      
      public function set label(param1:String) : void
      {
         if(this.textField)
         {
            this.textField.text = param1;
         }
      }
      
      public function get value() : Number
      {
         return this.slider.value;
      }
      
      public function set dataProvider(param1:Object) : void
      {
         this.slider.maximum = param1.maximum;
         this.slider.minimum = param1.minimum;
         param1.snapInterval = Math.round(param1.snapInterval / 0.01) * 0.01;
         this.slider.snapInterval = param1.snapInterval;
         this.slider.invalidateData();
      }
      
      public function set selectedIndex(param1:Number) : void
      {
         if(this.slider)
         {
            param1 = Math.round(param1 / 0.01) * 0.01;
            this.slider.position = param1;
            this.slider.invalidateData();
         }
         if(this.textFieldValue)
         {
            this.textFieldValue.text = this.slider.position.toString();
         }
      }
      
      protected function onSliderChange(param1:SliderEvent) : void
      {
         this.textFieldValue.text = this.slider.position.toString();
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE));
      }
   }
}

