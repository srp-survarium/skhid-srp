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
      
      public function set textColor(param1:uint) : void
      {
         this.textField.textColor = param1;
      }
      
      public function get text() : String
      {
         return this._text;
      }
      
      public function set text(param1:String) : void
      {
         if(this._text == param1)
         {
            return;
         }
         _text = param1;
         invalidateData();
      }
      
      override protected function draw() : void
      {
         var _loc1_:int;
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
            _loc1_ = this.textField.length;
            while(this.textField.textWidth > this._origin_width - 3 && _loc1_)
            {
               this.textField.text = this.textField.text.substr(0,_loc1_) + "..";
               _loc1_--;
            }
         }
         super.draw();
      }
   }
}

