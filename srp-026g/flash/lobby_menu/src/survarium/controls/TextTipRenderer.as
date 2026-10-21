package survarium.controls
{
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.controls.TipRenderer;
   import scaleform.clik.utils.ConstrainedElement;
   import scaleform.clik.utils.Constraints;
   
   public class TextTipRenderer extends TipRenderer
   {
      
      public var textField:TextField;
      
      public var constraints:Constraints;
      
      public function TextTipRenderer()
      {
         super();
         var _loc1_:uint = 6061383;
         var _loc2_:uint = 13582914;
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         this.constraints.addElement("textField",this.textField,Constraints.ALL);
         var _loc3_:StyleSheet = new StyleSheet();
         var _loc4_:Object;
         _loc4_ = new Object();
         _loc4_.color = "#b3ea97";
         _loc3_.setStyle("current",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#D79342";
         _loc3_.setStyle("yellow",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#F2C98F";
         _loc3_.setStyle("highlight",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#e9b838";
         _loc3_.setStyle("gold",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#5b6261";
         _loc3_.setStyle("gray",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#5b6261";
         _loc4_.size = 2;
         _loc4_.fontSize = 2;
         _loc3_.setStyle("small",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#5c7d47";
         _loc3_.setStyle("green",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#cf4242";
         _loc3_.setStyle("red",_loc4_);
         _loc4_ = new Object();
         _loc4_.color = "#BCCBCB";
         _loc3_.setStyle("white",_loc4_);
         this.textField.styleSheet = _loc3_;
      }
      
      override protected function invalidateData() : void
      {
         this.textField.wordWrap = false;
         this.textField.multiline = false;
         this.textField.text = _data.name;
         width = this.textField.textWidth;
         height = this.textField.textHeight;
         var _loc1_:ConstrainedElement = this.constraints.getElement("textField");
         var _loc2_:Number = Math.ceil(this.textField.textWidth + _loc1_.left + _loc1_.right + 5);
         var _loc3_:Number = Math.ceil(this.textField.textHeight + _loc1_.top + _loc1_.bottom + 5);
         width = _loc2_;
         height = _loc3_;
         this.constraints.update(width,height);
      }
   }
}

