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
      
      public function set editable(value:Boolean) : void
      {
         if(value == this._editable)
         {
            return;
         }
         _editable = value;
         addEventListener(Event.RENDER,this.onRender);
      }
      
      private function onRender(e:Event) : void
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
      
      protected function onTextFocusOut(e:Event) : void
      {
         trace("onTextFocusOut",this.slider.position);
         selectedIndex = this.slider.position;
      }
      
      protected function onTextInput(e:Event) : void
      {
         trace("onTextInput|",this.textFieldValue.text,"|",Number(this.textFieldValue.text));
         var value:Number = Math.round(Number(this.textFieldValue.text) / 0.01) * 0.01;
         value = value < this.slider.maximum ? value : this.slider.maximum;
         value = value > this.slider.minimum ? value : this.slider.minimum;
         this.slider.position = value;
         this.slider.invalidateData();
      }
      
      private function onSliderThumbClick(e:ButtonEvent) : void
      {
         trace("thumbClicked",this.slider.thumb,this.textFieldValue.type,this.textFieldValue.selectable,this.textFieldValue.mouseEnabled);
      }
      
      protected function resetScreenAdjust(e:Event) : void
      {
         if(this.slider.value != this._defaultValue)
         {
            this.slider.value = this._defaultValue;
         }
      }
      
      public function set defaultValue(value:Number) : void
      {
         _defaultValue = Math.round(value / 0.01) * 0.01;
      }
      
      public function get selectedIndex() : Number
      {
         return this.slider.position;
      }
      
      public function set label(value:String) : void
      {
         if(this.textField)
         {
            this.textField.text = value;
         }
      }
      
      public function get value() : Number
      {
         return this.slider.value;
      }
      
      public function set dataProvider(value:Object) : void
      {
         this.slider.maximum = value.maximum;
         this.slider.minimum = value.minimum;
         value.snapInterval = Math.round(value.snapInterval / 0.01) * 0.01;
         this.slider.snapInterval = value.snapInterval;
         this.slider.invalidateData();
      }
      
      public function set selectedIndex(value:Number) : void
      {
         if(this.slider)
         {
            value = Math.round(value / 0.01) * 0.01;
            this.slider.position = value;
            this.slider.invalidateData();
         }
         if(this.textFieldValue)
         {
            this.textFieldValue.text = this.slider.position.toString();
         }
      }
      
      protected function onSliderChange(e:SliderEvent) : void
      {
         this.textFieldValue.text = this.slider.position.toString();
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE));
      }
   }
}

