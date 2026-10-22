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
         var darkColor:uint = 6061383;
         var lightColor:uint = 13582914;
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         this.constraints.addElement("textField",this.textField,Constraints.ALL);
         var styles:StyleSheet = new StyleSheet();
         var style:Object;
         style = new Object();
         style.color = "#b3ea97";
         styles.setStyle("current",style);
         style = new Object();
         style.color = "#D79342";
         styles.setStyle("yellow",style);
         style = new Object();
         style.color = "#F2C98F";
         styles.setStyle("highlight",style);
         style = new Object();
         style.color = "#e9b838";
         styles.setStyle("gold",style);
         style = new Object();
         style.color = "#5b6261";
         styles.setStyle("gray",style);
         style = new Object();
         style.color = "#5b6261";
         style.size = 2;
         style.fontSize = 2;
         styles.setStyle("small",style);
         style = new Object();
         style.color = "#5c7d47";
         styles.setStyle("green",style);
         style = new Object();
         style.color = "#cf4242";
         styles.setStyle("red",style);
         style = new Object();
         style.color = "#BCCBCB";
         styles.setStyle("white",style);
         this.textField.styleSheet = styles;
      }
      
      override protected function invalidateData() : void
      {
         this.textField.wordWrap = false;
         this.textField.multiline = false;
         this.textField.text = _data.name;
         width = this.textField.textWidth;
         height = this.textField.textHeight;
         var element:ConstrainedElement = this.constraints.getElement("textField");
         var newWidth:Number = Math.ceil(this.textField.textWidth + element.left + element.right + 5);
         var newHeight:Number = Math.ceil(this.textField.textHeight + element.top + element.bottom + 5);
         width = newWidth;
         height = newHeight;
         this.constraints.update(width,height);
      }
   }
}

