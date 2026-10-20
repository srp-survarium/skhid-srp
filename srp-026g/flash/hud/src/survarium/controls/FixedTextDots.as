package survarium.controls
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.utils.Constraints;
   
   public class FixedTextDots extends UIComponent
   {
      
      public var textField:TextField;
      
      protected var _text:String = "";
      
      protected var _origin_width:Number = 0;
      
      public function FixedTextDots()
      {
         super();
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         constraints.addElement("textField",this.textField,Constraints.LEFT | Constraints.RIGHT);
         constraints.update(width,height);
         _origin_width = width;
         if(this.textField.text)
         {
            _text = this.textField.text;
         }
         this.textField.autoSize = TextFieldAutoSize.NONE;
         this.textField.defaultTextFormat = this.textField.getTextFormat();
      }
      
      override protected function configUI() : void
      {
      }
      
      public function set textColor(value:uint) : void
      {
         this.textField.textColor = value;
      }
      
      public function get text() : String
      {
         return this._text;
      }
      
      public function set text(value:String) : void
      {
         if(this._text == value)
         {
            return;
         }
         _text = value;
         invalidateData();
      }
      
      override protected function draw() : void
      {
         var charIndex:int;
         if(isInvalid(InvalidationType.SIZE))
         {
            _origin_width = width;
            setSize(this._origin_width,height);
            setActualSize(this._origin_width,height);
            constraints.update(this._origin_width,height);
         }
         if(isInvalid(InvalidationType.DATA) || isInvalid(InvalidationType.SIZE))
         {
            this.textField.text = this._text;
            charIndex = this.textField.length;
            while(this.textField.textWidth > this._origin_width - 3 && charIndex)
            {
               this.textField.text = this.textField.text.substr(0,charIndex) + "..";
               charIndex--;
            }
         }
         super.draw();
      }
   }
}

