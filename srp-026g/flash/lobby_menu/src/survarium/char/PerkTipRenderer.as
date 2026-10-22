package survarium.char
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.TipRenderer;
   
   public class PerkTipRenderer extends TipRenderer
   {
      
      public var textField:TextField;
      
      public var textFieldProp:TextField;
      
      public var line:Sprite;
      
      public var background:MovieClip;
      
      protected const MIN_SIZE:uint = 100;
      
      public function PerkTipRenderer()
      {
         super();
         var styleSheet:StyleSheet = MainScene.textStyleSheet;
         this.textField.styleSheet = styleSheet;
         this.textFieldProp.styleSheet = styleSheet;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         this.invalidateData();
      }
      
      override protected function invalidateData() : void
      {
         trace("HINT::invalidateData",_data.resize);
         if(_data.description_2_string)
         {
            this.line.visible = true;
            if(_data.resize)
            {
               this.textFieldProp.wordWrap = false;
               this.textFieldProp.multiline = false;
               this.textFieldProp.autoSize = TextFieldAutoSize.LEFT;
            }
            else
            {
               this.textFieldProp.width = 250;
               this.textFieldProp.multiline = true;
               this.textFieldProp.wordWrap = true;
               this.textFieldProp.autoSize = TextFieldAutoSize.LEFT;
            }
            this.textFieldProp.text = _data.description_2_string;
         }
         else
         {
            this.line.visible = false;
            this.textFieldProp.visible = false;
            this.textFieldProp.y = this.textField.y;
            this.textFieldProp.height = 1;
            this.textFieldProp.text = "";
         }
         var padding:uint = 8;
         var tipWidth:uint = Math.max(this.MIN_SIZE,this.textFieldProp.textWidth + padding + 2);
         this.textField.wordWrap = true;
         this.textField.multiline = true;
         this.textField.width = tipWidth;
         this.textField.text = _data.description_1;
         this.textField.height = this.textField.textHeight + 5;
         trace("HINT::",tipWidth);
         this.background.width = tipWidth + padding;
         if(this.line.visible)
         {
            this.textFieldProp.y = this.textField.y + this.textField.textHeight + 13;
            this.line.y = this.textFieldProp.y - 4;
            this.line.width = tipWidth - 10;
            this.background.height = this.textFieldProp.y + this.textFieldProp.textHeight + 11;
         }
         else
         {
            this.background.height = this.textField.y + this.textField.textHeight + 11;
         }
      }
   }
}

