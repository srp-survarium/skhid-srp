package survarium.controls
{
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.utils.Constraints;
   import survarium.Util;
   import survarium.constants.Colors;
   
   public class LabelDiffIndicator extends DiffIndicator
   {
      
      public var textFieldValue:TextField;
      
      public var textFieldLabel:TextField;
      
      public var textFieldDiff:TextField;
      
      protected var _prefix:String = "";
      
      protected var _postfix:String = "";
      
      protected var _label:String = "";
      
      public function LabelDiffIndicator()
      {
         super();
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         invalidateData();
      }
      
      public function get prefix() : String
      {
         return this._prefix;
      }
      
      public function set prefix(value:String) : void
      {
         _prefix = value;
         invalidateData();
      }
      
      public function get postfix() : String
      {
         return this._postfix;
      }
      
      public function set postfix(value:String) : void
      {
         _postfix = value;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraints)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
         this.textFieldLabel.x = -2;
         if(constraints)
         {
            constraints.addElement("textFieldValue",this.textFieldValue,Constraints.RIGHT);
            constraints.addElement("textFieldLabel",this.textFieldLabel,Constraints.LEFT);
         }
         this.textFieldLabel.defaultTextFormat = this.textFieldLabel.getTextFormat();
         this.textFieldValue.defaultTextFormat = this.textFieldValue.getTextFormat();
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.SIZE))
         {
            if(constraints)
            {
               constraints.update(_width,_height);
            }
         }
      }
      
      override protected function updatePosition() : void
      {
         var diffText:String;
         var betterColor:uint;
         var worseColor:uint;
         super.updatePosition();
         if(this._label)
         {
            this.textFieldLabel.text = this._label;
         }
         if(_valueDiff == 0)
         {
            diffText = "";
         }
         else
         {
            diffText = (_valueDiff > 0 ? "+" : "") + Util.getNumberString(_valueDiff).toString();
         }
         this.textFieldValue.defaultTextFormat = this.textFieldValue.getTextFormat();
         this.textFieldDiff.defaultTextFormat = this.textFieldDiff.getTextFormat();
         this.textFieldValue.textColor = 15911311;
         this.textFieldDiff.text = "";
         if(_valueDiff == 0)
         {
            this.textFieldDiff.textColor = 13356477;
            if(this._prefix == "-" && value != 0)
            {
               _prefix = "";
            }
            this.textFieldValue.text = this._prefix + Util.getNumberString(value) + this._postfix;
         }
         else
         {
            betterColor = Colors.GREEN_DIFF;
            worseColor = Colors.RED_DIFF;
            if(_direction == 0)
            {
               this.textFieldValue.textColor = _valueDiff < 0 ? worseColor : betterColor;
            }
            else
            {
               this.textFieldValue.textColor = _valueDiff < 0 ? betterColor : worseColor;
            }
            if(this._prefix == "-" && _otherValue != 0)
            {
               _prefix = "";
            }
            this.textFieldValue.text = this._prefix + Util.getNumberString(_value) + this._postfix;
         }
         if(this.textFieldDiff.text.length)
         {
            this.textFieldValue.x = this.textFieldDiff.x + this.textFieldDiff.width - this.textFieldValue.width - this.textFieldDiff.textWidth - 4;
         }
         else
         {
            this.textFieldValue.x = this.textFieldDiff.x + 4;
         }
      }
   }
}

