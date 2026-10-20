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
         var _loc1_:StyleSheet = MainScene.textStyleSheet;
         this.textField.styleSheet = _loc1_;
         this.textFieldProp.styleSheet = _loc1_;
      }
      
      override public function set data(param1:Object) : void
      {
         _data = param1;
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
         var _loc1_:uint = 8;
         var _loc2_:uint = Math.max(this.MIN_SIZE,this.textFieldProp.textWidth + _loc1_ + 2);
         this.textField.wordWrap = true;
         this.textField.multiline = true;
         this.textField.width = _loc2_;
         this.textField.text = _data.description_1;
         this.textField.height = this.textField.textHeight + 5;
         trace("HINT::",_loc2_);
         this.background.width = _loc2_ + _loc1_;
         if(this.line.visible)
         {
            this.textFieldProp.y = this.textField.y + this.textField.textHeight + 13;
            this.line.y = this.textFieldProp.y - 4;
            this.line.width = _loc2_ - 10;
            this.background.height = this.textFieldProp.y + this.textFieldProp.textHeight + 11;
         }
         else
         {
            this.background.height = this.textField.y + this.textField.textHeight + 11;
         }
      }
   }
}

